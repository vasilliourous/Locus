mod config;
mod lifecycle;
pub mod probe;
mod state;

use anyhow::Result;
use arc_swap::ArcSwapOption;
use clash_verge_logging::{LogRing, Type, logging};
use once_cell::sync::Lazy;
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use crate::core::runstate::{RUN_STATE, RealEnv, RunStateStore};
use crate::singleton;

pub(crate) static CLASH_LOGGER: Lazy<Arc<LogRing>> = Lazy::new(|| Arc::new(LogRing::new()));

const CORE_READINESS_ACTIVE_BIT: u64 = 1;
const CORE_READINESS_GENERATION_STEP: u64 = 1 << 1;

/// How long a completed through-tunnel egress result stays fresh.
///
/// Readiness is polled far faster than a real round trip needs to be re-proven:
/// the status command re-reads every 750 ms while a connect settles, and the
/// connect loop every 250 ms. Re-running a *real* request through the tunnel on
/// every one of those reads is both wasteful and, worse, makes a working tunnel
/// look flaky — each poll is an independent chance to catch a transient miss.
///
/// A window short enough that a tunnel that genuinely goes down is noticed within
/// a fraction of a second, and long enough to collapse the poll storm into one
/// round trip. The window applies to `Ok` and `Failing` alike: caching only
/// success would let the UI oscillate, and caching only failure would hide a
/// recovered tunnel.
const EGRESS_CACHE_TTL: Duration = Duration::from_millis(1500);

const fn next_active_core_readiness_state(state: u64) -> u64 {
    (state.wrapping_add(CORE_READINESS_GENERATION_STEP) & !CORE_READINESS_ACTIVE_BIT) | CORE_READINESS_ACTIVE_BIT
}

const fn inactive_core_readiness_state(state: u64) -> u64 {
    state.wrapping_add(CORE_READINESS_GENERATION_STEP) & !CORE_READINESS_ACTIVE_BIT
}

const fn active_core_readiness_generation(state: u64) -> Option<u64> {
    if state & CORE_READINESS_ACTIVE_BIT != 0 {
        Some(state)
    } else {
        None
    }
}

#[cfg(test)]
fn claim_core_readiness_generation(state: &AtomicU64, captured_generation: u64) -> bool {
    state
        .compare_exchange(
            captured_generation,
            inactive_core_readiness_state(captured_generation),
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
}

#[derive(Clone, Copy, Debug, serde::Serialize, PartialEq, Eq)]
pub enum RunningMode {
    Service,
    NotRunning,
}

impl fmt::Display for RunningMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service => write!(f, "Service"),
            Self::NotRunning => write!(f, "NotRunning"),
        }
    }
}

#[derive(Debug)]
pub struct CoreManager {
    /// The Run State this manager reports transitions to.
    ///
    /// A reference rather than the global directly, so a test can supervise a Core without
    /// racing every other test for one process-wide Running Mode.
    run_state: &'static RunStateStore<RealEnv>,
    last_update: ArcSwapOption<Instant>,
    config_update_in_progress: AtomicBool,
    core_readiness_state: AtomicU64,
    /// The last completed through-tunnel egress check, and when it ran.
    ///
    /// Readiness is polled from several places at once — the status command on a
    /// 750 ms timer while a connect settles, and the connect loop itself every
    /// 250 ms — and the egress check is the expensive half of the probe. Without
    /// this, every poller starts its own round trip, so the Core is asked to run
    /// the same delay test many times over and the slow case multiplies instead
    /// of being bounded by [`probe::EGRESS_DEADLINE`].
    ///
    /// A recent result is reused (see [`Self::observe_egress`]) so the common
    /// case — a poll a few hundred milliseconds after the last one — costs no
    /// round trip at all.
    last_egress: std::sync::Mutex<Option<(Instant, probe::EgressOutcome)>>,
    /// Serialises the through-tunnel check so only one runs at a time.
    ///
    /// Taken only around the egress probe inside [`Self::observe_readiness`],
    /// never held across the lifecycle lock, so the fixed lock order
    /// `config_update_in_progress → lifecycle_lock` is unaffected.
    egress_probe_lock: tokio::sync::Mutex<()>,
    // 串行化 start/stop/restart。
    // 锁序固定为 config_update_in_progress → lifecycle_lock。
    pub(crate) lifecycle_lock: tokio::sync::Mutex<()>,
}

/// Process-level state owned by `CoreManager`.
///
/// Running Mode deliberately does *not* live here — it belongs to `core::runstate`, which
/// keeps it consistent with Service Health and derives PAC availability from it.
impl Default for CoreManager {
    fn default() -> Self {
        Self {
            run_state: &RUN_STATE,
            last_update: ArcSwapOption::new(None),
            config_update_in_progress: AtomicBool::new(false),
            core_readiness_state: AtomicU64::new(0),
            last_egress: std::sync::Mutex::new(None),
            egress_probe_lock: tokio::sync::Mutex::new(()),
            lifecycle_lock: tokio::sync::Mutex::new(()),
        }
    }
}

impl CoreManager {
    fn new() -> Self {
        Self::default()
    }

    /// A manager with its own Run State, for tests that must not disturb the process-wide one.
    #[cfg(test)]
    fn isolated() -> Self {
        Self {
            run_state: Box::leak(Box::new(RunStateStore::new(RealEnv))),
            ..Self::default()
        }
    }

    pub fn get_running_mode(&self) -> Arc<RunningMode> {
        self.run_state.mode_arc()
    }

    pub fn get_last_update(&self) -> Option<Arc<Instant>> {
        self.last_update.load_full()
    }

    /// The Core is now running in `mode`.
    ///
    /// Run State derives PAC availability and the outward mode mirror from this; callers must
    /// not set those alongside.
    pub fn core_started(&self, mode: RunningMode) {
        let previous = *self.get_running_mode();
        if previous != mode {
            logging!(info, Type::Core, "Core running mode changed: {previous} -> {mode}");
        }
        // A fresh start must not answer egress from before it: the whole point of
        // the check is that it observed *this* Core moving a packet, and a cached
        // result from a previous Core would let a just-starting tunnel read as
        // connected on the strength of a round trip it never made.
        self.clear_egress();
        self.run_state.core_started(mode);
    }

    /// The Core is no longer running, for any reason.
    ///
    /// Core readiness is invalidated here rather than inside Run State because readiness
    /// belongs to the process this manager supervises.
    pub fn core_stopped(&self) {
        let previous = *self.get_running_mode();
        if !matches!(previous, RunningMode::NotRunning) {
            logging!(info, Type::Core, "Core running mode changed: {previous} -> NotRunning");
        }
        self.invalidate_core_readiness();
        self.clear_egress();
        self.run_state.core_stopped();
    }

    /// A start attempt is under way and the Core is not serving yet.
    ///
    /// Must be paired with [`Self::core_start_settled`] on every path out, including the ones
    /// where the start never happened.
    pub fn core_starting(&self) {
        // The transition begins here, so a result from the previous Core must not
        // survive into this one: a *new* start could otherwise read as egress-proven
        // on the strength of a round trip made before it existed.
        self.clear_egress();
        self.run_state.core_starting();
    }

    /// The start attempt is over: PAC goes back to following the Running Mode.
    pub fn core_start_settled(&self) {
        self.run_state.core_start_settled();
    }

    fn mark_core_ready(&self) -> u64 {
        let mut current = self.core_readiness_state.load(Ordering::Acquire);
        loop {
            let next = next_active_core_readiness_state(current);
            match self
                .core_readiness_state
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return next,
                Err(changed) => current = changed,
            }
        }
    }

    fn current_core_readiness_generation(&self) -> Option<u64> {
        active_core_readiness_generation(self.core_readiness_state.load(Ordering::Acquire))
    }

    /// Whether a start was recorded and not undone.
    ///
    /// This is the **latch**, and it is private on purpose. It reports only "we
    /// asked the Core to start and have not declared it stopped" — it observes
    /// nothing, so a Core that died, or one that started into a machine with no
    /// usable network, still reads `true`. That is precisely the value that used
    /// to be published as "ready", and publishing it was the bug: the client
    /// reported "connected" with no wifi, and through a window where both traffic
    /// counters sat at zero.
    ///
    /// It is retained because it is still the one fact that separates *stopped*
    /// from *failing to start*, which is what lets the UI say "connecting" rather
    /// than flashing "disconnected" at a Core that is still coming up. That
    /// distinction is now consumed only by [`Self::observe_readiness`], where it
    /// is combined with an actual probe instead of standing in for one.
    #[must_use]
    fn core_readiness_latched(&self) -> bool {
        !matches!(*self.get_running_mode(), RunningMode::NotRunning) && self.current_core_readiness_generation().is_some()
    }

    /// Whether the Core is actually answering, as observed rather than assumed.
    ///
    /// This is the honest answer to "is the tunnel working right now", and the
    /// one the UI renders from. It probes the Core's own control API and, when
    /// the Core has stopped answering, **drops the readiness latch** so the stale
    /// `true` cannot outlive the evidence against it.
    ///
    /// Why it must revoke rather than merely report: the latch is read from
    /// several places that have no probe of their own. If a failed probe only
    /// changed this function's return value, every other reader would keep
    /// seeing the latched `true` and the app would go on claiming a tunnel that
    /// the probe had just disproved.
    ///
    /// A Core that is still starting reports [`Readiness::Connecting`], which the
    /// UI renders as "connecting" — not "disconnected", because work is in
    /// flight, and not "connected", because nothing has been proven.
    pub async fn observe_readiness(&self) -> probe::Readiness {
        let running = !matches!(*self.get_running_mode(), RunningMode::NotRunning);
        // The latch, read once so the probe and the revoke decision cannot
        // disagree about what was true when the Core was asked.
        let latch_active = self.core_readiness_latched();
        let outcome = probe::probe_core_api(running).await;

        if probe::revokes_latch(outcome, latch_active) {
            logging!(
                warn,
                Type::Core,
                "Core is not answering its control API; revoking readiness"
            );
            self.invalidate_core_readiness();
        }

        // The through-tunnel packet that decides everything. It probes each
        // outbound the tier defines rather than the select group, because the
        // group's delay test only ever answers for its *currently selected*
        // member — which may not be the one carrying the student's traffic. See
        // `probe::egress_attempt_once` for the defect that caused.
        //
        // The local probe above is now only a gate on whether asking is worth it:
        // a Core that is not answering cannot run a delay test, and asking would
        // be a request guaranteed to fail.
        let egress = self
            .observe_egress(outcome == probe::ProbeOutcome::Serving)
            .await;

        probe::decide(outcome, egress, latch_active)
    }

    /// The through-tunnel egress answer, coalesced and briefly cached.
    ///
    /// The expensive half of readiness, so it is run once at a time and its
    /// result reused for [`EGRESS_CACHE_TTL`]. This is what stops the poll storm
    /// from turning one round trip into many: without it, the 750 ms status timer
    /// and the 250 ms connect loop each start their own request through the
    /// tunnel, so a slow tunnel looks flaky rather than merely slow, and a
    /// transient miss on any one of them strands the UI on "connecting" even
    /// though the tunnel is carrying traffic.
    ///
    /// Fresh results are returned without a round trip. Stale ones are re-proven
    /// under a lock, so concurrent callers serialise onto a single check rather
    /// than racing; a caller that arrives while a check is in flight waits for it
    /// and then reuses its result.
    async fn observe_egress(&self, core_serving: bool) -> probe::EgressOutcome {
        // A Core that is not serving has no egress to check, and asking is both
        // pointless and a request guaranteed to fail. Short-circuit before the
        // lock so a stopped Core never blocks on it.
        if !core_serving {
            self.store_egress(probe::EgressOutcome::NotAttempted);
            return probe::EgressOutcome::NotAttempted;
        }

        if let Some(outcome) = self.fresh_egress() {
            return outcome;
        }

        let _guard = self.egress_probe_lock.lock().await;

        // Re-check under the lock: another caller may have completed the check
        // while this one waited, in which case there is nothing left to do.
        if let Some(outcome) = self.fresh_egress() {
            return outcome;
        }

        let outcome = probe::probe_egress(true).await;
        self.store_egress(outcome);
        outcome
    }

    /// The cached egress result, if one was recorded within the TTL.
    fn fresh_egress(&self) -> Option<probe::EgressOutcome> {
        let cached = self.last_egress.lock().ok()?;
        let (at, outcome) = (*cached)?;
        let fresh = at.elapsed() < EGRESS_CACHE_TTL;
        // Drop the guard before returning so the lock is held only for the read.
        drop(cached);
        fresh.then_some(outcome)
    }

    /// Forgets the cached egress result.
    ///
    /// Called on every start and stop so a result is only ever reused across polls
    /// of the *same* running Core, never across a transition.
    fn clear_egress(&self) {
        if let Ok(mut cached) = self.last_egress.lock() {
            *cached = None;
        }
    }

    /// Records an egress result and the instant it was observed.
    ///
    /// A poisoned lock is ignored rather than propagated: the value it guards is
    /// a cache, and refusing to update it would leave a stale `Failing` in place
    /// for a tunnel that has since recovered — the worse failure.
    fn store_egress(&self, outcome: probe::EgressOutcome) {
        if let Ok(mut cached) = self.last_egress.lock() {
            *cached = Some((Instant::now(), outcome));
        }
    }

    pub(crate) fn invalidate_core_readiness(&self) {
        let _ = self
            .core_readiness_state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                active_core_readiness_generation(current).map(inactive_core_readiness_state)
            });
    }

    pub fn set_last_update(&self, time: Instant) {
        self.last_update.store(Some(Arc::new(time)));
    }

    /// Claims the single config-update slot, or reports that one is already held.
    pub(crate) fn try_start_config_update(&self) -> bool {
        !self.config_update_in_progress.swap(true, Ordering::AcqRel)
    }

    pub(crate) fn finish_config_update(&self) {
        self.config_update_in_progress.store(false, Ordering::Release);
    }

    #[tracing::instrument(skip_all, level = "info", fields(retries = 0))]
    pub async fn init(&self) -> Result<bool> {
        const MAX_PORT_FALLBACK_RETRIES: usize = 3;

        if let Some(reason) = crate::config::Config::startup_core_block_reason() {
            anyhow::bail!("core startup blocked after mixed proxy port fallback failure: {reason}");
        }

        let mut retries = 0;
        loop {
            match self.start_core().await {
                Ok(()) => {
                    crate::config::Config::notify_startup_mixed_port_fallback();
                    return Ok(!matches!(*self.get_running_mode(), RunningMode::NotRunning));
                }
                Err(start_error) if retries < MAX_PORT_FALLBACK_RETRIES => {
                    if !matches!(*self.get_running_mode(), RunningMode::NotRunning) {
                        crate::config::Config::notify_startup_mixed_port_fallback();
                        return Err(start_error);
                    }
                    match crate::config::Config::resolve_startup_mixed_port().await {
                        Ok(true) => {
                            retries += 1;
                            tracing::Span::current().record("retries", retries);
                            logging!(
                                warn,
                                Type::Core,
                                "Retrying core startup after mixed proxy port fallback ({}/{}): {start_error:#}",
                                retries,
                                MAX_PORT_FALLBACK_RETRIES
                            );
                        }
                        Ok(false) => {
                            crate::config::Config::notify_startup_mixed_port_fallback();
                            return Err(start_error);
                        }
                        Err(fallback_error) => {
                            crate::config::Config::block_startup_core(&fallback_error);
                            return Err(start_error.context(format!(
                                "the mixed proxy port fallback did not rescue core startup: {fallback_error:#}"
                            )));
                        }
                    }
                }
                Err(error) => {
                    crate::config::Config::notify_startup_mixed_port_fallback();
                    return Err(error);
                }
            }
        }
    }
}

singleton!(CoreManager, CORE_MANAGER);
