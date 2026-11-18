//! A periodic update check for an app that is never relaunched.
//!
//! # Why this has to exist at all
//!
//! Locus is a background VPN. The student opens it, connects, and then leaves it
//! minimised for days — so "check for an update on launch" means "check for an
//! update approximately never", and a released fix reaches nobody. The offer
//! itself already travels on the heartbeat, which is the right transport (it is
//! authenticated, it knows the tier, and it is already running); what was missing
//! was any way for a *recorded* offer to reach the screen without a relaunch, and
//! any re-ask at all while the app stayed open.
//!
//! So this is deliberately thin and adds no new protocol:
//!
//!   * It does not talk to the hub. `/api/update` is read by
//!     [`crate::locus::update::install::PendingInstall::check`], which is the same
//!     path the manual "check now" uses — one implementation of the host
//!     allowlist, the checksum requirement and the no-downgrade gate.
//!   * It does not decide anything. It asks that path what is installable and
//!     records the answer, which is exactly what a heartbeat does with an offer.
//!     The prompt still reads the recorded offer, so there is one source of truth
//!     for "what is being offered".
//!
//! # Why hourly
//!
//! The cadence is a product decision, not a technical limit: long enough that a
//! fleet of clients is not talking to the hub for no reason, short enough that a
//! fix published in the morning is on a student's screen before the end of the
//! school day. It is one unauthenticated GET per client per hour.
//!
//! # What it must never do
//!
//! Install anything. An update replaces the running binary and drops the
//! connection; doing that unasked on a background VPN is the worst possible
//! moment to interrupt someone. This records an offer and the prompt asks.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use clash_verge_logging::{Type, logging};
use tauri::Emitter as _;

/// How often a running app re-asks the hub what is installable.
///
/// See the module docs for why this value. Exposed so the loop and its tests
/// cannot drift apart.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// How long to wait after start-up before the first periodic check.
///
/// Not zero, and not `CHECK_INTERVAL`. A check on launch is the behaviour that
/// already exists and already failed to help; repeating it here would double the
/// work at the busiest moment (window creation, core start, first paint) for no
/// benefit — the offer from this launch's heartbeat, if any, is what the prompt
/// shows. This offset simply puts the steady-state re-ask somewhere other than
/// launch, so an app that is only ever open for minutes still gets a second look
/// in a later session rather than none.
pub const FIRST_CHECK_DELAY: Duration = Duration::from_secs(5 * 60);

/// The event that tells a live window an offer is available.
///
/// The prompt used to read `offeredVersion` exactly once, on mount, so a beat
/// that recorded an offer thirty seconds after launch reached nothing: the
/// student had to relaunch to be asked. This is what closes that gap.
pub const UPDATE_OFFERED_EVENT: &str = "locus://update-offered";

/// The running checker, if any.
///
/// A single slot, like the heartbeat's: exactly one poller may exist, because two
/// would double the fleet's request rate and could emit the same offer twice.
static RUNNING: Mutex<Option<Arc<PollControl>>> = Mutex::new(None);

/// What the loop waits on, so it can be stopped without leaking a task.
struct PollControl {
    /// Set to `true` by [`stop`]. Checked each time the loop wakes.
    stopping: Arc<std::sync::atomic::AtomicBool>,
    /// Wakes the loop out of its sleep so a stop is immediate rather than up to
    /// an hour late. A loop that outlived the app would keep asking the hub on
    /// behalf of a process that no longer exists.
    wake: Arc<tokio::sync::Notify>,
}

/// Starts the periodic check, replacing any previous one.
///
/// Idempotent in the sense that matters — calling it twice yields exactly one
/// loop — and safe to call on a device with no activation, which simply learns
/// nothing (there is nothing to install for a device that cannot reach the hub,
/// and the check is the same unauthenticated route either way).
pub fn start(app: tauri::AppHandle) {
    stop();

    let control = Arc::new(PollControl {
        stopping: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        wake: Arc::new(tokio::sync::Notify::new()),
    });

    match RUNNING.lock() {
        Ok(mut guard) => *guard = Some(Arc::clone(&control)),
        Err(poisoned) => *poisoned.into_inner() = Some(Arc::clone(&control)),
    }

    crate::process::AsyncHandler::spawn(move || async move {
        logging!(
            info,
            Type::System,
            "[locus] periodic update check started (every {} minutes)",
            CHECK_INTERVAL.as_secs() / 60
        );

        // The first wait is shorter, and is also where a stop is noticed before
        // any network call happens.
        if !wait_or_stop(&control, FIRST_CHECK_DELAY).await {
            return;
        }

        loop {
            check_once(&app).await;

            if !wait_or_stop(&control, CHECK_INTERVAL).await {
                logging!(info, Type::System, "[locus] periodic update check stopped");
                return;
            }
        }
    });
}

/// Stops the loop if one is running. Safe when none is, and safe twice.
pub fn stop() {
    let taken = match RUNNING.lock() {
        Ok(mut guard) => guard.take(),
        Err(poisoned) => poisoned.into_inner().take(),
    };

    if let Some(control) = taken {
        control.stopping.store(true, std::sync::atomic::Ordering::SeqCst);
        control.wake.notify_waiters();
    }
}

/// Waits for `duration`, resolving `false` when a stop was requested first.
///
/// `tokio::select!` rather than a polling sleep so a stop is immediate: the app
/// can be quitting, and holding the process open for the remainder of an hour is
/// not acceptable.
async fn wait_or_stop(control: &PollControl, duration: Duration) -> bool {
    if control.stopping.load(std::sync::atomic::Ordering::SeqCst) {
        return false;
    }
    tokio::select! {
        () = tokio::time::sleep(duration) => true,
        () = control.wake.notified() => false,
    }
}

/// One check: ask the shared install path, record what it says, announce it.
///
/// Every failure is a log line and nothing else. This runs in the background with
/// nobody looking, so there is no error to show — and a check that failed says
/// nothing about the student's account, unlike a refused heartbeat.
async fn check_once(app: &tauri::AppHandle) {
    // Honour the student's preference. Read here, at the entry to the automatic
    // check, so the setting has exactly one effect and the manual "check now" on
    // the Account page still works — that is an explicit request.
    let auto = crate::config::Config::verge()
        .await
        .latest_arc()
        .auto_check_update
        .unwrap_or(true);
    if !auto {
        logging!(
            debug,
            Type::System,
            "[locus] periodic update check skipped: automatic checks are off"
        );
        return;
    }

    let hub = crate::locus::contract::hub_url().await;
    let current = env!("CARGO_PKG_VERSION");

    match crate::locus::update::install::PendingInstall::check(app, &hub, current).await {
        Ok(Some(pending)) => {
            let version = pending.version().to_owned();
            // Only announce on a CHANGE. Re-announcing the same offer every hour
            // would re-open a prompt the student has already declined, which is
            // the nagging behaviour the dismissal exists to prevent.
            let already = crate::config::Config::verge()
                .await
                .latest_arc()
                .locus_update_offered
                .clone();
            if already.as_deref() == Some(version.as_str()) {
                return;
            }

            logging!(
                info,
                Type::System,
                "[locus] the hub is offering update {version} (periodic check)"
            );
            if let Err(error) = crate::locus::store::record_update_offer(&version).await {
                logging!(
                    warn,
                    Type::System,
                    "[locus] could not record the offered update: {error:#}"
                );
                return;
            }
            // Announce AFTER recording: the prompt re-reads the stored offer so
            // that a window which was not listening still shows the right thing
            // on its next read. Emitting first would let a listener query a value
            // that had not been written yet.
            if let Err(error) = app.emit(UPDATE_OFFERED_EVENT, &version) {
                logging!(
                    warn,
                    Type::System,
                    "[locus] could not announce the offered update: {error}"
                );
            }
        }
        // Nothing to install. If an offer is still recorded it is spent, and the
        // prompt must stop asking about it — the same clear the install path
        // performs, applied to the path that discovers it first.
        Ok(None) => {
            let stale = crate::config::Config::verge()
                .await
                .latest_arc()
                .locus_update_offered
                .clone();
            if stale.is_some() {
                logging!(
                    info,
                    Type::System,
                    "[locus] the recorded update offer is no longer available; clearing it"
                );
                let _ = crate::locus::store::clear_update_offer().await;
            }
        }
        Err(error) => {
            // Includes "the updater plugin is missing from this build". Logged
            // rather than surfaced: a periodic check failing is not an account
            // problem, and the Account page already reports the manual path's
            // failures with far more context.
            logging!(
                debug,
                Type::System,
                "[locus] periodic update check found nothing installable: {error:#}"
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, reason = "tests assert by panicking")]
mod tests {
    use super::*;

    /// An hour is the decision, and a silent change to it is a fleet-wide change
    /// in request rate. Pinned so it has to be made on purpose.
    #[test]
    fn the_check_interval_is_hourly() {
        assert_eq!(CHECK_INTERVAL, Duration::from_secs(3600), "one hour");
    }

    /// The first check must not be at the beat the heartbeat already covers.
    #[test]
    fn the_first_check_is_delayed_but_sooner_than_the_steady_state() {
        assert!(
            FIRST_CHECK_DELAY > Duration::ZERO,
            "a check at t=0 duplicates the launch heartbeat for no benefit"
        );
        assert!(
            FIRST_CHECK_DELAY < CHECK_INTERVAL,
            "the first check must arrive before the steady-state interval, or an \
             app that is only ever open briefly would never check twice"
        );
    }

    /// The event name is a wire name between the backend and its own windows.
    ///
    /// Renaming it silently would leave the Rust side emitting an event nothing
    /// listens for — a prompt that never appears, with both halves compiling.
    /// `update-prompt.tsx` matches this string; `check-consistency.sh` asserts the
    /// pair, and this pins the constant itself.
    #[test]
    fn the_offer_event_is_the_name_the_frontend_listens_for() {
        assert_eq!(UPDATE_OFFERED_EVENT, "locus://update-offered");
    }

    /// A stop before the first wait must resolve immediately rather than sleeping
    /// for the first delay.
    #[tokio::test]
    async fn a_stop_before_the_first_wait_does_not_sleep() {
        let control = PollControl {
            stopping: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            wake: Arc::new(tokio::sync::Notify::new()),
        };
        // Would be 5 minutes if the flag were not checked.
        assert!(!wait_or_stop(&control, FIRST_CHECK_DELAY).await);
    }

    /// A wake during the wait must return promptly, not after the full interval.
    #[tokio::test]
    async fn a_stop_during_the_wait_is_prompt() {
        let control = Arc::new(PollControl {
            stopping: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            wake: Arc::new(tokio::sync::Notify::new()),
        });
        let waiter = Arc::clone(&control);
        let handle = tokio::spawn(async move { wait_or_stop(&waiter, CHECK_INTERVAL).await });

        // Let the waiter reach its select, then stop it.
        tokio::time::sleep(Duration::from_millis(50)).await;
        control.stopping.store(true, std::sync::atomic::Ordering::SeqCst);
        control.wake.notify_waiters();

        let stopped = tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("a stopped loop must not wait out the hour")
            .expect("the task must not panic");
        assert!(!stopped);
    }
}
