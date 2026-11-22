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
//!   * a **suspension** stops the tunnel and records a REVERSIBLE state — the
//!     code is kept, so unsuspending restores the session. It does **not** stop
//!     the loop: the loop is the only thing that can discover the operator
//!     lifting it, so it keeps asking on `SUSPENDED_INTERVAL` until the hub says
//!     200. Stopping it here is the defect that made a kick-out permanent
//!     (`docs/reference/SUSPENSION.md` §3);
//!   * any **other refusal** (unknown code, expiry) clears the entitlement,
//!     because a device the hub has refused must not keep a tunnel up on the
//!     strength of a stale local record;
//!   * an **update signal** is recorded, not acted on. Installing is the
//!     student's decision, and doing it silently mid-session would kill their
//!     connection without asking.

use crate::locus::heartbeat::{BeatOutcome, Credential, HeartbeatLoop};
use crate::locus::{apply, store};
use clash_verge_logging::{Type, logging};
use std::sync::Mutex;
use tauri::Emitter as _;

/// The event that tells a live window the ACCOUNT state changed.
///
/// The frontend used to learn about a refusal only from its own 5-minute poll
/// (`SUBSCRIPTION_POLL_MS`). That is a defensible cadence for a warning badge and
/// an indefensible one for "your access has been withdrawn": a student kicked out
/// at 10:00 kept working until 10:05. This event closes that, and it is the
/// backend's only channel to the gate besides `locus_status`.
///
/// Carries a short reason label (`"suspended"`, `"refused"`, `"reinstated"`,
/// `"activated"`, `"signed-out"`, `"resumed"`) for logging and for the UI to log;
/// the authoritative state is always re-read from `locus_status`, so a missed
/// event delays the screen rather than making it lie.
pub const ENTITLEMENT_CHANGED_EVENT: &str = "locus://entitlement-changed";

/// Tells any live window that the account state changed.
///
/// One function rather than a bare `emit` at each site, because there are now six
/// of them and the event name is a string contract with the frontend — a typo in
/// one site is a screen that silently does not update, which is the failure this
/// event exists to prevent. Callers pass only the label.
pub fn notify_entitlement_changed(reason: &str) {
    let _ = crate::core::handle::Handle::app_handle().emit(ENTITLEMENT_CHANGED_EVENT, reason);
}

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

/// Ensures a loop is beating, starting one from the stored record if not.
///
/// # Why a device needs to be able to resume itself
///
/// The loop owns the only channel that can change the account state, so "is it
/// running?" is the same question as "can this device learn that an operator
/// changed their mind?". Two situations answer it "no" until something re-arms it:
///
///   * the app was quit while the device was suspended, and the launch that
///     follows finds a suspended record — [`start`] is called at startup, but a
///     loop that was refused on its first beat must not leave the device
///     unwatched for the rest of the run;
///   * the loop was stopped by a path that had a reason at the time (a refusal
///     about the credential) which has since been repaired by a new activation.
///
/// Idempotent, so every caller can be optimistic: a running loop is left alone, a
/// record with no code is a no-op, and a device that is genuinely signed out stays
/// silent. This is the "self-heal" half of the invariant — the *write* half is
/// [`store::clear_entitlement`], which guarantees a signed-out device is never
/// reported as suspended, so this can never resurrect a watch for a device with no
/// code to watch with.
pub async fn resume() {
    if is_running() {
        return;
    }

    let verge = crate::config::Config::verge().await;
    match store::read(&verge.latest_arc()) {
        Some(activation) => {
            logging!(
                info,
                Type::Config,
                "[locus] no heartbeat was running; resuming from the stored activation"
            );
            start(activation);
        }
        None => logging!(
            debug,
            Type::Config,
            "[locus] no heartbeat to resume: this device holds no activation"
        ),
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
async fn record_update_offer(
    response: &crate::locus::heartbeat::HeartbeatResponse,
) -> Option<crate::locus::update::NoOfferReason> {
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
        logging!(
            warn,
            Type::System,
            "[locus] could not record the offered update: {error:#}"
        );
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

/// Applies what a SUCCESSFUL beat says about the account's standing.
///
/// Two jobs, joined because they are two readings of the same event and because
/// `handle_success` is at its complexity ceiling:
///
/// * [`record_hub_enforcement`] — which enforcement logic answered, so a
///   deployment behind the suspension fix becomes a fact in the product rather
///   than an inference from a support call;
/// * leaving the suspended state, below.
///
/// # Why an unsuspension is detected HERE
///
/// A successful beat is the hub saying this code is in good standing again, which
/// is the only event that means an operator undid a kick-out. Clearing the flag is
/// what lets the gate hand the student back their normal app without re-entering
/// the code, and it costs nothing when there was no suspension — the flag is
/// already absent and this returns immediately.
///
/// Split out of [`handle_success`] because it is one self-contained decision with
/// its own failure modes, and inlining it pushed that function past the complexity
/// ceiling — a fair signal that it did not belong there. That happened twice, which
/// is why the enforcement record now lives here rather than alongside it.
async fn leave_suspension_if_reinstated(response: &crate::locus::heartbeat::HeartbeatResponse) {
    record_hub_enforcement(response).await;

    // `is_suspended` reads the flag AND the stored activation, so the flag alone
    // is not enough to conclude anything here. Checking the flag's raw value would
    // treat a device that holds no code as "suspended, now reinstated" on its
    // first ever beat, and clear a state that does not exist.
    let verge = crate::config::Config::verge().await;
    if !verge.latest_arc().locus_suspended.unwrap_or(false) || store::read(&verge.latest_arc()).is_none() {
        return;
    }

    logging!(
        info,
        Type::Config,
        "[locus] the hub has reinstated this code; leaving the suspended state"
    );

    if let Err(error) = store::clear_suspension().await {
        logging!(warn, Type::Config, "[locus] could not clear the suspension: {error:#}");
    }

    // The loop is no longer stopped by a suspension (see `handle_refusal`), so
    // there is usually nothing to restart here — it kept asking, which is how this
    // function was reached at all. `resume` still has to run for the paths where
    // the loop is genuinely absent: a device that was suspended when the app quit
    // and whose launch has not started one yet. It is idempotent, and it cannot
    // resurrect a loop for a device with no code.
    resume().await;

    // Tell the window, so the kick-out page is replaced now rather than at the
    // next poll. A reinstatement the student has to wait for is indistinguishable
    // from one that did not work.
    notify_entitlement_changed("reinstated");
}

/// Records which enforcement logic the hub is running, and says so when it is behind.
///
/// # Why this is a function of its own
///
/// Two reasons, and the second is the interesting one.
///
/// It is a self-contained decision with its own log line and its own storage
/// failure mode, so it reads better here than in the middle of `handle_success`.
/// And inlining it pushed that function past this crate's cognitive-complexity
/// ceiling — which is exactly the signal `leave_suspension_if_reinstated` was
/// extracted on, and it is right again: the complexity is real, and the place it
/// belongs is in the function that owns the question.
///
/// # What it does with the answer
///
/// Records it, and LOGS AT WARN when the hub is behind. The question it answers —
/// "can this hub enforce a suspension?" — is a property of the deployment, and an
/// operator can redeploy under a running app, so it is written on every healthy
/// beat rather than once at activation. A value recorded once would go stale in the
/// one direction that matters.
///
/// It is never a gate. Refusing service to a student because the operator's box is
/// behind would be a worse defect than the one it reports, and the student cannot
/// fix it. The Account page shows it; a student reading it can quote the number, and
/// that is the whole of the intended effect.
async fn record_hub_enforcement(response: &crate::locus::heartbeat::HeartbeatResponse) {
    if let Some(version) = response.enforcement_version
        && version < crate::locus::contract::ENFORCEMENT_VERSION
    {
        logging!(
            warn,
            Type::Config,
            "[locus] this hub runs enforcement version {version}, but this client needs {}. \
             A suspension set in the console will NOT reach this device until the hub's \
             hooks are redeployed (see docs/reference/SUSPENSION.md \u{a7}4).",
            crate::locus::contract::ENFORCEMENT_VERSION
        );
    }

    if let Err(error) = store::record_hub_enforcement(response.enforcement_version).await {
        logging!(
            warn,
            Type::Config,
            "[locus] could not record the hub's enforcement version: {error:#}"
        );
    }
}

/// A successful beat: record it, then act on anything the hub changed.
async fn handle_success(response: &crate::locus::heartbeat::HeartbeatResponse) {
    leave_suspension_if_reinstated(response).await;

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

    // Adopt a hub address the operator offered.
    //
    // This is the escape hatch for a network that refuses the compiled hostname:
    // the heartbeat is the only channel that reaches an already-installed device
    // without a release, so the offer arrives here and is stored for the NEXT
    // launch to dial.
    //
    // Written only when the hub actually sent one, and through the same validator
    // the connect path uses — so a malformed or plaintext value is ignored rather
    // than adopted, and a hub that stops sending it does NOT clear a working
    // override (the operator's remedy must not evaporate because one beat omitted
    // the key). Reaching the new address mid-session is deliberately not attempted:
    // swapping the hub under a running tunnel would re-dial the connection the
    // student is using, and the value takes effect on the next launch.
    if let Some(alias) = &response.hub_alias
        && let Err(error) = store::record_hub_override(alias).await
    {
        logging!(
            warn,
            Type::Config,
            "[locus] could not store the hub alias the hub offered: {error:#}"
        );
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

/// Whether a refusal message means "an operator suspended this code".
///
/// # Why this keys on the message, and why that is acceptable HERE
///
/// The hub answers a suspension with a 403 whose body carries the sentence, and
/// the client has no other channel for the distinction — the status code is
/// shared with every other refusal. That is the same fragile coupling
/// `activation_contract.rs` already pins for the activation path, and the wording
/// is frozen for exactly this reason.
///
/// The consequence of getting it WRONG is deliberately mild in one direction: a
/// message we fail to recognise falls through to the destructive path, which is
/// the pre-existing behaviour — the device is refused and its code cleared. The
/// failure mode is "we did not get the nicer, reversible behaviour", never "a
/// refusal was ignored". The opposite mistake (treating a non-suspension as one)
/// is guarded by the substring being specific to the two sentences the hub emits.
#[must_use]
fn is_suspension(reason: &str) -> bool {
    reason.to_ascii_lowercase().contains("suspended")
}

/// The hub has definitively refused this device: suspended, expired, or unknown.
///
/// Stops the tunnel and records the refusal. Leaving the tunnel up would keep
/// working on the strength of a stale local record, which is precisely what
/// suspension exists to prevent.
///
/// # A suspension is reversible, and nothing else is
///
/// Both branches stop the tunnel; the difference is what happens to the
/// credential and to the heartbeat loop:
///
///   * **Suspended** — the code is kept, along with the tier and expiry, and a
///     flag is set. The loop KEEPS BEATING on `SUSPENDED_INTERVAL`, because the
///     loop is the only channel that can learn an operator lifted the
///     suspension. An unsuspend then restores the session with no re-entry and
///     no restart. Deleting the code here would make "undo" mean "find your card
///     again", on every machine.
///   * **Anything else** (unknown code, expiry, a credential the hub will not
///     accept) — the code is cleared and the loop is stopped, because there is
///     nothing left to authenticate with. A code the hub does not recognise is
///     not going to start being recognised.
async fn handle_refusal(reason: &str) {
    logging!(warn, Type::Config, "[locus] the hub refused this device: {reason}");

    // Stop the TUNNEL, never the loop — and the distinction is the whole
    // difference between a suspension that can be lifted and one that cannot.
    //
    // This used to call `stop()`, which is correct for a refusal about the
    // CREDENTIAL (the code is about to be deleted, so there is nothing left to
    // authenticate with) and wrong for a suspension, which keeps the code
    // precisely so the session can be restored. Stopping the loop there meant the
    // one channel that could learn about an unsuspension had been switched off by
    // the suspension itself.
    //
    // So the loop's fate is decided below, per arm: a suspension keeps it beating
    // on `SUSPENDED_INTERVAL`; any other refusal stops it, because its credential
    // is gone and a loop with no credential can only hammer the hub.
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
    if is_suspension(reason) {
        // `record_suspension` writes the reason too, so it is not called twice.
        if let Err(error) = store::record_suspension(reason).await {
            logging!(warn, Type::Config, "[locus] could not record the suspension: {error:#}");
        }
        // The credential is deliberately left in place — see `record_suspension`.
        logging!(
            info,
            Type::Config,
            "[locus] suspended: the code is kept so unsuspending restores the session, \
             and the heartbeat keeps asking whether it has been lifted"
        );
        // Re-arm the loop if it is not running. `handle_refusal` is reached from a
        // beat, so one IS running in the ordinary case and this is a no-op; it
        // matters for the paths that reach here with no loop — a manual status
        // check after a restart, an activation refused as suspended by the hub.
        // Without it, `runtime::resume` is the only thing that ever starts the
        // watch, and a device suspended between launch and its first beat would
        // sit unwatched until the next launch.
        resume().await;
        // The window must show this NOW. Without the event the kick-out page waits
        // for the gate's finite fallback poll, which on a background app is the
        // whole working day.
        notify_entitlement_changed("suspended");
        return;
    }

    // Not reversible: the credential is about to be deleted, so there is nothing
    // left for a loop to authenticate with.
    stop();

    if let Err(error) = store::record_refusal(reason).await {
        logging!(
            warn,
            Type::Config,
            "[locus] could not record the refusal reason: {error:#}"
        );
    }

    if let Err(error) = store::clear_entitlement().await {
        logging!(warn, Type::Config, "[locus] could not clear the entitlement: {error:#}");
    }

    // Same reason as the suspension arm: the gate polls, and a withdrawal the
    // student has to notice by waiting is a withdrawal they can act on meanwhile.
    notify_entitlement_changed("refused");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Source with `//` line comments removed.
    ///
    /// The absence assertions in this module must not be satisfied — or defeated —
    /// by a doc comment that *names* the removed call in order to explain why it is
    /// gone. That is not hypothetical: the first version of
    /// `a_suspension_keeps_the_heartbeat_beating` failed against its own
    /// explanation. Failing loudly is the good direction to be wrong in, but a guard
    /// that cannot survive documentation is still no guard. Stripping comments first
    /// means a check fails only when real code carries the call — the same helper
    /// `activation_contract.rs` uses for the same reason.
    fn strip_line_comments(source: &str) -> String {
        source
            .lines()
            .map(|line| line.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Stopping when nothing is running must be harmless — it is called from
    /// shutdown paths where being wrong would panic on quit.
    #[test]
    fn stopping_when_not_running_is_harmless() {
        stop();
        stop();
    }

    /// A refusal is a definitive statement about the device, so it must be the
    /// Both frozen suspension sentences must be recognised as a suspension.
    ///
    /// These are the exact strings the hub emits (`heartbeat.pb.js` and
    /// `activation.pb.js`), and the client keys on the substring "suspended" to
    /// decide between the REVERSIBLE path (keep the code) and the destructive one
    /// (clear it). Getting this wrong the safe way — failing to recognise one —
    /// degrades to the old destructive behaviour; getting it wrong the other way
    /// would keep a dead code forever, which is why the substring is narrow.
    #[test]
    fn both_frozen_suspension_sentences_are_recognised() {
        for reason in [
            "Account suspended — contact your middleman",
            "Code suspended",
            // The kick-out path appends the operator's own words.
            "Account suspended — chargeback",
        ] {
            assert!(
                is_suspension(reason),
                "{reason:?} is the hub's suspension wording and must take the \
                 reversible path, or unsuspending cannot restore the session"
            );
        }
    }

    /// Every other refusal must stay destructive.
    ///
    /// An unknown or expired code is not coming back, so keeping its credential on
    /// disk would leave a bearer token that can never authenticate — the exact
    /// state `clear_entitlement` exists to remove.
    #[test]
    fn other_refusals_are_not_treated_as_suspensions() {
        for reason in [
            "Code expired",
            "Code not found",
            "the hub refused this device (401)",
            "Account is on hold",
        ] {
            assert!(
                !is_suspension(reason),
                "{reason:?} is not a suspension; treating it as reversible would \
                 keep a credential the hub will never accept"
            );
        }
    }

    /// A suspension must not reach `clear_entitlement`, and a non-suspension must.
    ///
    /// Asserted on the source because the branch needs a running app and a real
    /// hub to execute — the same trade `signout.rs` makes for its call order. This
    /// is the property the whole feature depends on: the credential surviving is
    /// what makes an unsuspend a one-click repair instead of "find your card".
    #[test]
    fn only_a_suspension_keeps_the_credential() {
        let source = include_str!("runtime.rs");
        let start = source
            .find("async fn handle_refusal")
            .expect("handle_refusal must exist");
        let body = &source[start..];
        let end = body.find("#[cfg(test)]").unwrap_or(body.len());
        let body = &body[..end];

        let suspension_guard = body
            .find("if is_suspension(reason) {")
            .expect("the reversible path must be guarded by `is_suspension`");
        let clearing = body
            .find("store::clear_entitlement()")
            .expect("the destructive path must still exist");

        assert!(
            suspension_guard < clearing,
            "the suspension branch must be decided BEFORE the entitlement is \
             cleared. If the clear moves above the guard, an operator's kick-out \
             destroys the code and unsuspending can never restore the session — \
             the student is sent back to a card they may not have."
        );

        // The suspension arm returns rather than falling through, so the clear
        // below cannot run on a suspension path.
        let arm = &body[suspension_guard..clearing];
        assert!(
            arm.contains("return;"),
            "the suspension arm does not return, so it falls through into \
             `clear_entitlement` and the reversible behaviour is lost"
        );
        assert!(
            arm.contains("record_suspension"),
            "the suspension arm no longer records the suspension, so the client \
             could not tell a kick-out from any other refusal"
        );
    }

    /// A suspension must not stop the loop, because the loop is how it is lifted.
    ///
    /// # The defect this is written against
    ///
    /// `handle_refusal` began with an unconditional `runtime::stop()`, and the loop
    /// itself ended on any refusal (`Refused => break`). Together those meant a
    /// suspension switched off the ONE channel that could discover the operator
    /// undoing it. An operator unsuspends a code, the hub answers 200, and nothing
    /// was listening — so the student sat on a screen telling them to wait, and
    /// waiting did nothing until they restarted the app.
    ///
    /// Asserted on the source because the branch needs a live hub and a running app
    /// to execute — the same trade `only_a_suspension_keeps_the_credential` makes.
    /// The property is structural: the suspension arm must not stop the heartbeat.
    #[test]
    fn a_suspension_keeps_the_heartbeat_beating() {
        let source = include_str!("runtime.rs");
        let start = source
            .find("async fn handle_refusal")
            .expect("handle_refusal must exist");
        let body = &source[start..];
        let body = &body[..body.find("#[cfg(test)]").unwrap_or(body.len())];

        // The suspension arm runs from its guard to its `return`. Slicing on
        // `record_refusal` instead would include the code ABOVE the guard — where
        // the destructive `stop()` now lives — and the guard would fail against a
        // correct implementation. It did exactly that; the defect is worth keeping
        // in the comment because the next person to move this code will reach for
        // the same slice.
        let arm_start = body
            .find("if is_suspension(reason) {")
            .expect("the reversible path must still be guarded by `is_suspension`");
        let arm = &body[arm_start..];
        let arm = &arm[..arm.find("return;").unwrap_or(arm.len())];

        // Comment-stripped, so this tests the CODE and not the prose above it — the
        // prose quotes `stop()` precisely to explain why it is not there, and a
        // guard that fails on its own documentation is a guard nobody keeps.
        let code_only = strip_line_comments(arm);

        assert!(
            !code_only.contains("stop();"),
            "the suspension arm must not stop the heartbeat. A stopped loop cannot notice \
             that an operator lifted the suspension, which is the difference between a \
             kick-out that is reversible and one that needs a restart."
        );
        assert!(
            code_only.contains("resume()"),
            "the suspension arm must ensure a loop is running: a device suspended between \
             launch and its first beat would otherwise sit unwatched until the next launch."
        );

        // The other half, one file away: the loop must have no terminal arm left.
        let loop_source = include_str!("heartbeat.rs");
        assert!(
            !loop_source.contains("BeatOutcome::Refused { .. } => {\n                        on_outcome(outcome);"),
            "the heartbeat loop must not end on a refusal: that arm is what made a \
             suspension unrecoverable without a restart"
        );
    }

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
        assert_eq!(offered_version_to_record(&response, "linux", "3.2.1"), Some("3.3.0"));
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
            hub_alias: None,
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
            // Absent, the same as a hub below enforcement version 2. The tests
            // above are about the update offer and must not accidentally assert
            // anything about deployment freshness.
            enforcement_version: None,
        }
    }
}
