//! Supervising the heartbeat loop for the running app.
//!
//! [`crate::locus::heartbeat`] implements the loop but owns no app state: it
//! knows how to beat, back off and stop, and nothing about Tauri. Something has
//! to start it after activation, restart it if the code changes, and stop it on
//! exit. That is this.
//!
//! # Why one supervisor rather than "start it where it is needed"
//!
//! The loop must never run twice. Two loops means two beats per interval, double
//! the hub load, and a device that looks like it is flapping. It must also not
//! outlive the app: a heartbeat that keeps beating for a quit instance would keep
//! a suspended code looking alive.
//!
//! Both of those are properties of a single owner, not of a caller remembering.
//!
//! # What it does with the hub's answer
//!
//! The loop is deliberately policy-free, and this is where the policy lives:
//!
//!   * a **config refresh** is applied through the same path activation uses, so
//!     there is one route from "the hub said something changed" to "the core is
//!     running it";
//!   * a **suspension or expiry** clears the entitlement and takes the tunnel
//!     down, because a device the hub has refused must not keep a tunnel up on
//!     the strength of a stale local record;
//!   * an **update signal** is recorded, not acted on. Installing is the
//!     student's decision, and doing it silently mid-session would kill their
//!     connection without asking.

use crate::locus::heartbeat::{BeatOutcome, Credential, HeartbeatLoop};
use crate::locus::{apply, store};
use clash_verge_logging::{Type, logging};
use std::sync::Mutex;

/// Holds the running loop, if any.
static RUNNING: Mutex<Option<HeartbeatLoop>> = Mutex::new(None);

/// Starts beating for this activation, replacing any loop already running.
///
/// Replacing rather than refusing is deliberate: the caller's intent is always
/// "beat for *this* activation", and the most likely reason a loop is already
/// running is a re-activation that changed the code or tier. Silently keeping the
/// old one would leave the device reporting a tier it no longer has.
pub fn start(activation: store::Activation) {
    // A device always authenticates with its activation code now. This used to
    // fall back to a session token minted at device recognition, for a device
    // that had no code because recognition had restored it; recognition and the
    // token are both gone, and the code is kept across restarts by
    // `crate::locus::credential` instead.
    if activation.code.trim().is_empty() {
        // No code: this run cannot authenticate at all. Refusing to start is the
        // honest answer — a loop beating with an empty credential would hammer
        // the hub with 400s and report nothing useful.
        logging!(
            warn,
            Type::Config,
            "[locus] refusing to start a heartbeat with no activation code"
        );
        return;
    }

    start_with(Credential::code(activation.code.clone()), activation);
}

/// Starts (or replaces) the heartbeat loop with an already-resolved credential.
///
/// Synchronous, and called from an async context only via a plain call:
/// starting the loop is `spawn` + a mutex write, neither of which awaits. It was
/// split from [`start`] when `start` still had to read a stored session token
/// (a device restored by recognition); with tokens gone, `start` has nothing to
/// await either, and both are synchronous. Keeping them separate still holds —
/// `start` validates the credential, `start_with` does the work — so a caller
/// that has already resolved a credential cannot accidentally re-derive one.
pub fn start_with(credential: Credential, activation: store::Activation) {
    // Stop any previous loop first, so its task cannot deliver one more outcome
    // against the old activation. Doing this before the credential is resolved
    // in `start` is fine — both orderings end with exactly one loop running.
    stop();

    let fingerprint = activation.fingerprint.clone();

    logging!(
        info,
        Type::Config,
        "[locus] starting heartbeat (tier {})",
        activation.tier
    );

    let loop_handle = HeartbeatLoop::start(credential, fingerprint, move |outcome| {
        // The callback is synchronous and the work it triggers is async, so it
        // hands off to the runtime rather than blocking the loop's task. A
        // heartbeat that stalled on config application would look like a
        // transport failure and back off for no reason.
        crate::process::AsyncHandler::spawn(move || async move {
            handle_outcome(outcome).await;
        });
    });

    match RUNNING.lock() {
        Ok(mut guard) => *guard = Some(loop_handle),
        Err(poisoned) => {
            // A poisoned lock means a previous holder panicked. The loop itself
            // is still valid, so recover the guard rather than leaving the app
            // unable to heartbeat for the rest of its life.
            logging!(warn, Type::Config, "[locus] heartbeat mutex was poisoned; recovering");
            *poisoned.into_inner() = Some(loop_handle);
        }
    }
}

/// Stops the loop if one is running.
///
/// Safe to call when nothing is running, and safe to call twice — this is called
/// from shutdown paths where "is it running?" is not a question worth getting
/// wrong.
pub fn stop() {
    let taken = match RUNNING.lock() {
        Ok(mut guard) => guard.take(),
        Err(poisoned) => poisoned.into_inner().take(),
    };

    if let Some(loop_handle) = taken {
        // Blocking on the task from a synchronous context would deadlock the
        // runtime it is scheduled on, so it is dropped into the runtime instead.
        crate::process::AsyncHandler::spawn(|| async move {
            loop_handle.stop().await;
            logging!(info, Type::Config, "[locus] heartbeat stopped");
        });
    }
}

/// Whether a loop is currently held.
#[must_use]
pub fn is_running() -> bool {
    match RUNNING.lock() {
        Ok(guard) => guard.is_some(),
        Err(poisoned) => poisoned.into_inner().is_some(),
    }
}

/// Asks the running loop to beat now, and reports whether one was running.
///
/// The account's state is only ever confirmed by the hub, and that confirmation
/// arrives on the heartbeat's own schedule — a 5 minute floor, and up to two
/// hours after failures. This is what makes "check my status now" mean *check*,
/// rather than re-read a local copy that cannot have changed since the last beat.
///
/// Returns `false` when nothing is beating (no activation, or the loop is
/// stopped), which the caller reports honestly rather than pretending a check
/// happened.
///
/// Returns as soon as the request is *registered*, not when the beat completes.
/// Callers with nothing to report about the outcome want this one; a UI control
/// that must bracket a pending state wants [`beat_now_and_wait`].
#[must_use]
pub fn beat_now() -> bool {
    with_running(HeartbeatLoop::beat_now).is_some()
}

/// Asks the running loop to beat, and waits for that beat to complete.
///
/// # Why the UI needs this one
///
/// `beat_now` only wakes the loop, so a control that treated its return as
/// "finished" cleared its pending state before the request had left the machine —
/// the "Check status now" button flicked to "Checking…" and back in
/// single-digit milliseconds, unrelated to the hub round trip the student was
/// actually waiting for.
///
/// # What resolves, and what does not
///
/// Resolves `true` once a requested beat has completed **and its outcome has been
/// applied** (`runtime::handle_outcome` runs first), so a `locus_status` issued
/// afterwards sees the result. Resolves `false` immediately when nothing is
/// beating — no activation, or the loop is stopped — because in that case no beat
/// will complete and waiting would hang the control forever.
///
/// Deliberately does not return the outcome. The hub's answer is interpreted in
/// exactly one place, which also stops the tunnel on a refusal; a verdict
/// returned here would be a second interpretation that could disagree with it.
/// The caller re-reads status instead, the same single source the rest of the UI
/// uses.
///
/// The wait is bounded by the beat's own request timeout, so a dead network
/// delays this by that timeout rather than indefinitely.
pub async fn beat_now_and_wait() -> bool {
    // The control handle is cloned out of the lock, and the guard dropped, before
    // anything is awaited. Holding the mutex across the network round trip would
    // block every other reader of `RUNNING` — including the one that stops the
    // loop on exit.
    let Some(control) = with_running(HeartbeatLoop::control) else {
        return false;
    };

    control.beat_now_and_wait().await;
    true
}

/// Runs `f` against the running heartbeat handle, if there is one.
///
/// One place for the lock/poison dance, because duplicating it is how the
/// poisoned-arm behaviour drifted out of step with the healthy one.
fn with_running<T>(f: impl FnOnce(&HeartbeatLoop) -> T) -> Option<T> {
    match RUNNING.lock() {
        Ok(guard) => guard.as_ref().map(f),
        // A panic while holding this lock poisoned it. The handle is still
        // usable — it is an `Arc<Notify>` pair and some atomics — so recover it
        // rather than silently dropping the ability to check status at all.
        Err(poisoned) => poisoned.into_inner().as_ref().map(f),
    }
}

/// Applies the policy for one beat's outcome.
async fn handle_outcome(outcome: BeatOutcome) {
    match outcome {
        BeatOutcome::Ok(response) => handle_success(&response).await,
        BeatOutcome::Refused { reason } => handle_refusal(&reason).await,
        BeatOutcome::Unreachable { reason } => {
            // NOT necessarily an entitlement problem. The tunnel keeps working
            // through the grace period, and `locus_status` reports the remaining
            // window so the UI can warn before access ends. Taking the tunnel
            // down the moment a beat fails would punish a student for the
            // school's network.
            //
            // But the grace period is a LIMIT, not a promise that access lasts
            // forever offline: once it is spent, the device is running on an
            // entitlement nobody has confirmed for a week, and it must stop.
            // That check was never made, so a device that could not reach the
            // hub stayed connected indefinitely on a stale local record — the
            // second half of why expiry never bit. Enforced here, on the path
            // where a beat *failed*, because a successful beat resets the clock.
            logging!(
                debug,
                Type::Config,
                "[locus] heartbeat could not reach the hub: {reason}"
            );
            enforce_grace_period().await;
        }
    }
}

// `renew_credential` and `stored_identity` — REMOVED.
//
// Both existed to answer a 401 by re-running device recognition and minting a
// fresh session token. Recognition is gone and the client only ever
// authenticates with its activation code, so a hub that will not accept that
// code is a refusal and the device is torn down — see `handle_refusal`.

/// Stops the tunnel when the grace period since the last good beat is spent.
///
/// The grace period exists so a student is not cut off by a school network that
/// blocks the hub for a while. It is bounded so that a device which simply never
/// reaches the hub again does not run forever on an entitlement the hub has
/// never confirmed — and so a lapsed or refunded code eventually stops even if
/// the refusal never arrives.
///
/// A device that has never beaten (`last_ok == 0`) is given the full window, not
/// zero: it is a fresh install that has not been online yet, not a lapsed one.
///
/// Crucially, a spent grace period does NOT discard the activation code. It is a
/// local statement about this session — "we cannot confirm the entitlement" —
/// not a statement that the code is bad. See [`store::record_lapsed_grace`].
async fn enforce_grace_period() {
    let verge = crate::config::Config::verge().await;
    let last_ok = verge.latest_arc().last_heartbeat_ok.unwrap_or(0);
    let now = chrono::Utc::now().timestamp();

    let remaining = crate::locus::heartbeat::Backoff::remaining_grace(last_ok, now);
    if !remaining.is_zero() {
        logging!(
            debug,
            Type::Config,
            "[locus] {remaining:?} of the heartbeat grace period left"
        );
        return;
    }

    logging!(
        warn,
        Type::Config,
        "[locus] heartbeat grace period spent with no successful check-in; stopping the tunnel"
    );

    stop();
    if let Err(error) = crate::core::CoreManager::global().stop_core().await {
        logging!(
            warn,
            Type::Config,
            "[locus] could not stop the core after the grace period expired: {error:#}"
        );
    }
    if let Err(error) = store::record_lapsed_grace().await {
        logging!(
            warn,
            Type::Config,
            "[locus] could not clear the session state after the grace period expired: {error:#}"
        );
    }
}

/// Decodes the hub's update signal and persists the offered version.
///
/// Decoding rather than reading `update_available` directly is deliberate: the
/// decoder applies the platform, URL, checksum and signature requirements, and
/// the local no-downgrade gate. A signal that fails any of them is not an offer,
/// and prompting for something that cannot be installed would send the student
/// into a download that ends in a verification failure.
///
/// The rejection reason is logged, because "an update was advertised and silently
/// ignored" is the exact failure this whole path exists to avoid.
async fn record_update_offer(response: &crate::locus::heartbeat::HeartbeatResponse) -> Option<crate::locus::update::NoOfferReason> {
    let Some(platform) = crate::locus::update::Platform::current() else {
        // Not a build target we ship updates for.
        let reason = crate::locus::update::NoOfferReason::UnsupportedPlatform;
        logging!(info, Type::System, "[locus] no update offered: {}", reason.describe());
        return Some(reason);
    };

    let Some(version) = offered_version_to_record(response, platform.key(), env!("CARGO_PKG_VERSION")) else {
        // `offered_version_to_record` logs the specific reason at info level —
        // it must not be silent, see `NoOfferReason`.
        return None;
    };

    // Honour the student's "check for updates automatically" preference. Read it
    // here, at the one place an offer becomes a prompt, so the setting has
    // exactly one effect: `false` means the hub's offer is not recorded and no
    // dialog appears; the manual check on the Account page still works, because
    // that is an explicit request rather than an automatic one.
    //
    // `unwrap_or(true)` is deliberate: an absent value is a fresh or migrated
    // install, and the product decision is that automatic checking is ON by
    // default so clients pick up fixes without waiting to be told. Defaulting the
    // other way would silently opt every existing device out of updates.
    let auto = crate::config::Config::verge()
        .await
        .latest_arc()
        .auto_check_update
        .unwrap_or(true);
    if !auto {
        // Logged at WARN, not debug, and returned so the Account page can say so.
        //
        // This is the case a student cannot diagnose themselves: they are told
        // nothing, the setting is on a page they may never open, and the field is
        // inherited from upstream — so it can be `false` from an era when the row
        // was mis-labelled. A debug-level line meant this left no trace at all.
        let reason = crate::locus::update::NoOfferReason::AutomaticChecksOff {
            version: version.to_owned(),
        };
        logging!(warn, Type::System, "[locus] no update offered: {}", reason.describe());
        return Some(reason);
    }

    logging!(info, Type::System, "[locus] the hub is offering update {version}");
    if let Err(error) = store::record_update_offer(version).await {
        logging!(warn, Type::System, "[locus] could not record the offered update: {error:#}");
    }
    None
}

/// Whether a heartbeat's update signal is worth prompting a student about.
///
/// Pure, so the rule can be tested without a heartbeat or an app handle. It
/// answers with the version to record, or `None` for every case that must stay
/// silent — and the silent cases matter as much as the loud one: a prompt for
/// something that cannot be installed sends a student into a download that ends
/// in a verification failure.
///
/// The rejections, each deliberate:
///
///  - **nothing advertised** — the common case, and not a failure;
///  - **not newer than what is running** — a stale or rolled-back
///    `update_config` row must not be able to advertise a downgrade, and there
///    is no server-driven downgrade to recover from;
///  - **no artifact for this platform** — the signal named a version but nothing
///    to download for this build target, which is an operator error and would
///    present to the student as a broken update.
///
/// The stricter checks (checksum, signature) are deliberately NOT repeated here.
/// They are the plugin's, applied at install time against `/api/update` — the
/// same source this offer came from — so anything accepted here is checked there,
/// and duplicating them would mean two implementations that can disagree.
fn offered_version_to_record<'a>(
    response: &'a crate::locus::heartbeat::HeartbeatResponse,
    platform: &str,
    current_version: &str,
) -> Option<&'a str> {
    let version = response.update_available.as_deref().filter(|v| !v.is_empty())?;

    if let Some(reason) = crate::locus::update::rejection_reason(version, current_version) {
        // INFO, not debug. A rejection that leaves no trace at the default log
        // level is how "the update wasn't offered" became undiagnosable; see
        // `NoOfferReason`.
        logging!(info, Type::System, "[locus] no update offered: {reason}");
        return None;
    }

    if response.download_url(platform).is_none() {
        logging!(
            warn,
            Type::System,
            "[locus] no update offered: {version} carries no artifact for {platform} — \
             the release is incomplete for this platform"
        );
        return None;
    }

    Some(version)
}

/// A successful beat: record it, then act on anything the hub changed.
async fn handle_success(response: &crate::locus::heartbeat::HeartbeatResponse) {
    let now = chrono::Utc::now().timestamp();
    if let Err(error) = store::record_heartbeat_success(now).await {
        logging!(
            warn,
            Type::Config,
            "[locus] could not record a successful heartbeat: {error:#}"
        );
    }

    if let Some(config) = &response.server_config {
        apply_refreshed_config(config, response.udp_relay).await;
    }

    // Persist the expiry on EVERY successful beat, deliberately outside the
    // `server_config` branch above.
    //
    // Expiry changes for reasons that have nothing to do with the connection
    // details — a renewal extends the date while the server and password stay
    // identical — so gating this on a config change would leave the Account
    // screen showing a stale date for a student who had just paid. This is the
    // value the client could previously only learn once, at activation.
    //
    // Stored as an `Option` and written only when the hub actually said
    // something: a hub that predates the field sends nothing, and overwriting a
    // known date with `None` because of an older server would lose information
    // the client legitimately has.
    if response.expires_at.is_some()
        && let Err(error) = store::record_expiry(response.expires_at.clone()).await
    {
        logging!(
            warn,
            Type::Config,
            "[locus] could not store the subscription expiry: {error:#}"
        );
    }

    // Cache the free-tier allowance the hub advertised, so a status read can
    // classify usage without a live beat — the Connection screen polls far more
    // often than the client beats.
    //
    // Written on EVERY beat, and to `None` when the hub sends nothing, because
    // here the absence is the answer rather than a gap: a code that moved to a
    // paying tier must stop being metered, and a hub that stops sending the
    // allowance is the operator switching the quota off. Leaving a stale
    // allowance in place would throttle a student the hub had released.
    //
    // (This is the opposite rule from `expires_at` above, and the difference is
    // deliberate: an absent expiry means "this hub cannot tell you", where an
    // absent allowance means "no allowance applies".)
    if let Some(raw) = response.free_allowance_mb
        && raw > crate::locus::usage::MAX_SANE_ALLOWANCE_MB
    {
        // The hub sent a figure the arithmetic cannot trust. The client clamps
        // it (see `usage::sane_allowance_mb`), so the student is safe either
        // way — but this is the ONE place the bad value is still visible, and
        // an operator seeing it here can fix the row before it reaches anyone
        // else. Silently clamping would hide a server-side fault that the next
        // client version might not clamp.
        logging!(
            warn,
            Type::Config,
            "[locus] the hub advertised an implausible free-tier allowance ({} MB); \
             clamping to {} MB. The student is unaffected, but the hub's tier row is wrong.",
            raw,
            crate::locus::usage::MAX_SANE_ALLOWANCE_MB
        );
    }
    if let Err(error) = store::store_allowance(response.free_allowance_mb).await {
        logging!(
            warn,
            Type::Config,
            "[locus] could not store the free-tier allowance: {error:#}"
        );
    }
    // …and the speed to drop to once it is spent. Same rule: the hub's latest
    // word replaces the previous one, including when that word is "nothing".
    if let Err(error) = store::store_throttle_mbps(response.free_throttle_mbps).await {
        logging!(
            warn,
            Type::Config,
            "[locus] could not store the free-tier throttle speed: {error:#}"
        );
    }

    // An update signal is RECORDED, not installed. Installing is the student's
    // decision: doing it silently mid-session would drop their connection
    // without warning, and on a school network that is the worst moment.
    //
    // This used to log the version and do nothing else — so "recorded, not
    // installed" described an intention rather than the code, and the popup
    // could never appear because nothing had recorded anything. The signal is now
    // actually decoded (which also applies the no-downgrade gate a second time,
    // client-side) and the offered version is persisted for the prompt.
    //
    // A reason for *not* offering is persisted too, so the Account page can
    // answer "why is this device not updating?" — the question a student asks
    // when nothing appears, and could not answer at all before.
    let no_offer = record_update_offer(response).await;
    if let Err(error) = store::record_update_check_reason(no_offer.as_ref()).await {
        logging!(
            warn,
            Type::Config,
            "[locus] could not record the update-check reason: {error:#}"
        );
    }
}

/// Applies a config the hub supplied on a heartbeat, if it actually changed.
///
/// The `tier_changed` check is what prevents a RESTART STORM: the hub sends
/// `server_config` on every beat, and applying it unconditionally would reload
/// or restart the core every five minutes, dropping every connection the student
/// has, forever, for no reason.
async fn apply_refreshed_config(config: &crate::locus::contract::TierConfig, udp_relay: bool) {
    if !store::tier_changed(config, udp_relay).await {
        return;
    }

    logging!(
        info,
        Type::Config,
        "[locus] hub supplied new connection details; applying"
    );

    if let Err(error) = store::store_tier_config(config, udp_relay).await {
        logging!(
            warn,
            Type::Config,
            "[locus] could not store the refreshed config: {error:#}"
        );
        // Continue to the apply attempt: the in-memory config is still correct
        // for this session even if it could not be persisted, and refusing to
        // apply would leave the student on a server the hub has moved away from.
    }

    match apply::apply_tier(config, udp_relay).await {
        Ok(apply::ApplyOutcome::Rejected { reason }) => {
            logging!(warn, Type::Config, "[locus] refreshed config was refused: {reason}")
        }
        Err(error) => logging!(
            warn,
            Type::Config,
            "[locus] could not apply the refreshed config: {error:#}"
        ),
        Ok(_) => logging!(info, Type::Config, "[locus] refreshed config applied"),
    }
}

/// The hub has definitively refused this device: suspended, expired, or unknown.
///
/// Tears the entitlement down and stops the tunnel. Leaving it up would keep
/// working on the strength of a stale local record, which is precisely what
/// suspension exists to prevent.
async fn handle_refusal(reason: &str) {
    logging!(warn, Type::Config, "[locus] the hub refused this device: {reason}");

    stop();

    if let Err(error) = crate::core::CoreManager::global().stop_core().await {
        logging!(
            warn,
            Type::Config,
            "[locus] could not stop the core after refusal: {error:#}"
        );
    }

    // Record WHY before clearing WHAT. The clear removes the entitlement, and
    // with it every trace of the reason — this refusal arrives from a background
    // beat, so by the time the student looks at the app the only remaining
    // evidence is whatever was written here. Order matters: a crash between the
    // two leaves an explained refusal, never an unexplained one.
    if let Err(error) = store::record_refusal(reason).await {
        logging!(warn, Type::Config, "[locus] could not record the refusal reason: {error:#}");
    }

    if let Err(error) = store::clear_entitlement().await {
        logging!(warn, Type::Config, "[locus] could not clear the entitlement: {error:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stopping when nothing is running must be harmless — it is called from
    /// shutdown paths where being wrong would panic on quit.
    #[test]
    fn stopping_when_not_running_is_harmless() {
        stop();
        stop();
    }

    /// A refusal is a definitive statement about the device, so it must be the
    /// branch that tears down. This pins the classification rather than the
    /// effect, because the effect needs a running app.
    #[test]
    fn a_refusal_is_distinct_from_an_outage() {
        let refused = BeatOutcome::Refused {
            reason: "suspended".to_owned(),
        };
        let unreachable = BeatOutcome::Unreachable {
            reason: "timeout".to_owned(),
        };

        assert!(
            matches!(refused, BeatOutcome::Refused { .. }),
            "a suspension must not be treated as a transport failure"
        );
        assert!(
            matches!(unreachable, BeatOutcome::Unreachable { .. }),
            "a timeout must not be treated as a refusal"
        );
    }

    /// A heartbeat carrying a real offer for this platform must produce a version
    /// to prompt about.
    ///
    /// This is the regression: the signal used to be received and logged, and
    /// nothing else, so no student was ever told an update existed.
    #[test]
    fn an_actionable_offer_is_recorded() {
        let response = heartbeat(Some("3.3.0"), Some("https://hub/updates/locus-linux"), "linux");
        assert_eq!(
            offered_version_to_record(&response, "linux", "3.2.1"),
            Some("3.3.0")
        );
    }

    /// A version that is not newer must never be prompted about.
    ///
    /// A stale or rolled-back `update_config` row would otherwise advertise a
    /// downgrade, and there is no server-driven downgrade to recover from.
    #[test]
    fn an_offer_that_is_not_newer_is_ignored() {
        for advertised in ["3.2.1", "3.2.0", "1.0.0"] {
            let response = heartbeat(Some(advertised), Some("https://hub/updates/locus-linux"), "linux");
            assert_eq!(
                offered_version_to_record(&response, "linux", "3.2.1"),
                None,
                "{advertised} must not be offered over 3.2.1"
            );
        }
    }

    /// A version with no artifact for THIS platform is an operator error, and
    /// prompting would send the student into a download that cannot complete.
    #[test]
    fn an_offer_with_no_artifact_for_this_platform_is_ignored() {
        let windows_only = heartbeat(
            Some("3.3.0"),
            Some("https://hub/updates/locus-windows-amd64.exe"),
            "windows", // a URL exists only for windows
        );
        assert_eq!(
            offered_version_to_record(&windows_only, "linux", "3.2.1"),
            None,
            "a windows-only artifact must not be offered to a linux client"
        );
        assert_eq!(
            offered_version_to_record(&windows_only, "windows", "3.2.1"),
            Some("3.3.0"),
            "the platform the artifact IS for must still be offered"
        );
    }

    /// The ordinary case: nothing advertised. Silence, not a failure.
    #[test]
    fn nothing_advertised_produces_no_offer() {
        for empty in [None, Some("")] {
            let response = heartbeat(empty, Some("https://hub/updates/locus-linux"), "linux");
            assert_eq!(offered_version_to_record(&response, "linux", "3.2.1"), None);
        }
    }

    /// Builds a heartbeat response advertising `version`, with a per-platform URL
    /// only for `url_platform` (when given).
    ///
    /// An explicit platform rather than the host's, so every branch is exercised
    /// wherever the tests run rather than only on one target.
    fn heartbeat(
        version: Option<&str>,
        url: Option<&str>,
        url_platform: &str,
    ) -> crate::locus::heartbeat::HeartbeatResponse {
        let for_key = |key: &str| url.filter(|_| key == url_platform).map(str::to_owned);
        crate::locus::heartbeat::HeartbeatResponse {
            status: "ok".to_owned(),
            tier: None,
            server_config: None,
            udp_relay: false,
            expires_at: None,
            free_allowance_mb: None,
            free_throttle_mbps: None,
            update_available: version.map(str::to_owned),
            update_linux: for_key("linux"),
            update_windows: for_key("windows"),
            update_macos_intel: for_key("macos_intel"),
            update_macos_arm: for_key("macos_arm"),
            update_sha256_linux: None,
            update_sha256_windows: None,
            update_sha256_macos_intel: None,
            update_sha256_macos_arm: None,
            update_url: None,
            update_sha256: None,
        }
    }
}
