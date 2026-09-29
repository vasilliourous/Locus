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
    time::Instant,
};
use crate::core::runstate::{RUN_STATE, RealEnv, RunStateStore};
use crate::singleton;

pub(crate) static CLASH_LOGGER: Lazy<Arc<LogRing>> = Lazy::new(|| Arc::new(LogRing::new()));

const CORE_READINESS_ACTIVE_BIT: u64 = 1;
const CORE_READINESS_GENERATION_STEP: u64 = 1 << 1;

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
        self.run_state.core_stopped();
    }

    /// A start attempt is under way and the Core is not serving yet.
    ///
    /// Must be paired with [`Self::core_start_settled`] on every path out, including the ones
    /// where the start never happened.
    pub fn core_starting(&self) {
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
    /// A Core that is still starting reports [`Readiness::NotReady`], which the
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

        // The second half of "is the tunnel working": a real packet through the
        // proxy. Only attempted when the local probe already proved the Core is
        // serving — a stopped Core has nothing to carry traffic with, and asking
        // it to run a delay test would be a request guaranteed to fail. A Core
        // that is up but cannot move a packet reports NoEgress, which the UI
        // renders as "connected but no service" rather than "connected" — the
        // fix for the machine with no wifi that still claimed a tunnel.
        let egress = probe::probe_egress(outcome == probe::ProbeOutcome::Serving).await;

        probe::decide(outcome, egress, latch_active)
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
