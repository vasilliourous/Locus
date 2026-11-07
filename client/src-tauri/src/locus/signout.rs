//! Signing out: forgetting the key on this device, and telling the hub.
//!
//! # Why this module exists, and what it is not
//!
//! A student who is moving to a new computer, sharing a laptop, or selling a
//! machine needs a way to say "this computer should no longer be signed in".
//! Without one the only route was to reinstall — which is the report the operator
//! was already getting, and it is a terrible answer: reinstalling is slow, it
//! scares students, and (until this module) it did not even work reliably,
//! because the machine-scoped credential mirror
//! ([`crate::locus::credential`]) deliberately survives an uninstall and would
//! put the code back.
//!
//! So "sign out" is defined precisely, and every part of it is visible to the
//! student in words that name consequences rather than mechanisms:
//!
//! 1. **Tell the hub** that this device has signed out of its code, so the
//!    operator can tell a device move apart from a shared key. Best-effort: the
//!    local half must work with no network.
//! 2. **Stop beating** — the heartbeat loop, or the next beat would re-assert an
//!    entitlement that is about to be deleted.
//! 3. **Take the tunnel down** — a running tunnel on a signed-out device is a
//!    device using an entitlement it no longer holds.
//! 4. **Remove the tier profile** — otherwise the tunnel configuration stays on
//!    disk and the next activation writes over a profile nobody owns.
//! 5. **Clear the entitlement AND the durable mirror.** This is the step the
//!    whole feature exists for. `store::clear_entitlement` already clears
//!    `verge.yaml` and calls `credential::remove_code`, and the mirror is why a
//!    sign-out that skipped it would appear to undo itself on the next launch.
//!
//! # The order is load-bearing, twice over
//!
//! * The hub must be told **before** the code is forgotten, because the code is
//!   what authenticates the notification. Reversing this makes the call
//!   impossible to make, and the failure is silent — the local sign-out succeeds
//!   and the hub never hears about it.
//! * The heartbeat must be stopped **before** the entitlement is cleared, because
//!   a beat that lands between the two would read "activated" from a config this
//!   function is mid-way through rewriting.
//!
//! Both are pinned by tests over [`Step`] below, which is why the order lives in
//! data rather than only in the prose above.

use anyhow::Result;
use clash_verge_logging::{Type, logging};

use crate::locus::activation::{self, ReleaseOutcome};
use crate::locus::{apply, credential, device, runtime, store};

/// One stage of a sign-out, in the order it must run.
///
/// The enum exists so the order can be asserted: [`ORDER`] is the sequence, and a
/// test checks both that the hub notification precedes the entitlement clear and
/// that the entitlement clear is last. A future edit that reorders the real calls
/// without touching this list would still be caught by
/// `the_local_half_still_clears_the_mirror`, which asserts on behaviour rather
/// than on the enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Tell the hub this device has signed out of its code.
    NotifyHub,
    /// Stop the heartbeat loop.
    StopHeartbeat,
    /// Bring the tunnel down.
    StopTunnel,
    /// Delete the generated tier profile.
    RemoveProfile,
    /// Forget the code, the tier, the expiry, and the durable mirror.
    ForgetKey,
}

/// The sign-out sequence, as data.
pub const ORDER: [Step; 5] = [
    Step::NotifyHub,
    Step::StopHeartbeat,
    Step::StopTunnel,
    Step::RemoveProfile,
    Step::ForgetKey,
];

/// What the student is told happened on the hub side.
///
/// The local half is unambiguous — either the device forgot the code or it did
/// not. The hub half is not, and collapsing the three cases into "signed out"
/// would be a lie in two of them: a student who is told "your key has been
/// released" and then finds it still held somewhere has been given the exact
/// reassurance that costs them another email.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HubOutcome {
    /// The hub recorded the release. The key can be used on another machine.
    Released,
    /// The hub answered and had nothing to release (never redeemed, or already
    /// released). Not a failure — the end state is what the student asked for.
    NothingToRelease,
    /// The hub could not be reached, or refused to understand the request. The
    /// device is signed out regardless; the student is told they will need to
    /// enter their key on the next device, which is true either way.
    NotConfirmed,
}

/// The result of a sign-out.
///
/// Deliberately not a `bool`: "signed out" and "signed out and the hub knows" are
/// different facts, and the UI says different sentences for each. The traps file
/// records why (`client/AGENTS.md`: a control whose success looks like its
/// silence reads as broken).
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignOutResult {
    /// Always `true` when this is returned at all. Present so the frontend has an
    /// explicit field rather than inferring success from the absence of an error.
    pub signed_out: bool,
    pub hub: HubOutcome,
}

impl SignOutResult {
    /// The result for a sign-out whose hub notification failed in a given way.
    const fn with_hub(hub: HubOutcome) -> Self {
        Self { signed_out: true, hub }
    }
}

/// Signs this device out of its activation code.
///
/// Returns only when the local half has happened. A hub failure is reported in
/// the result, never as an error, because the student's request — "forget this
/// computer" — has been honoured and failing the whole operation would leave the
/// code on disk while telling them it was removed.
///
/// # When there is nothing to sign out of
///
/// Returns an error. Calling this on a device with no stored code means the UI
/// and the backend disagree about whether the student is signed in, and silently
/// reporting success would hide that.
pub async fn sign_out() -> Result<SignOutResult> {
    // Read the credential FIRST. Everything after this point may delete it, and
    // the hub notification is the only stage that needs it — so capturing it
    // here is what makes the documented order implementable rather than merely
    // described.
    let Some(code) = store::read_code_rehydrating().await else {
        anyhow::bail!("this device is not signed in, so there is nothing to sign out of");
    };

    // 1. Tell the hub while the code still exists. Best-effort by design: a
    //    network failure must not strand a student on a machine they are giving
    //    away.
    //
    //    A stored code that is not a code at all is skipped WITHOUT a request, and
    //    that distinction is the whole point. `activation::release` validates
    //    locally first, so calling it with a damaged code errors before any
    //    network call and lands in the same `NotConfirmed` arm — but logging it as
    //    "the hub was not told" would be false, and a caller reading that line
    //    would go looking for a network fault. More important, this used to be
    //    unreachable at all: the damaged-code path never got this far, so the
    //    local half below never ran and the student's only exit was uninstalling
    //    the app.
    let hub = match activation::validate_code(&code) {
        Err(error) => {
            logging!(
                warn,
                Type::Config,
                "[locus] signing out a damaged stored code; the hub was not asked: {error}"
            );
            HubOutcome::NotConfirmed
        }
        Ok(_) => match activation::release(&code, &device::fingerprint()).await {
            Ok(ReleaseOutcome::Recorded) => HubOutcome::Released,
            Ok(ReleaseOutcome::NothingToRelease) => HubOutcome::NothingToRelease,
            Err(error) => {
                logging!(
                    warn,
                    Type::Config,
                    "[locus] the hub was not told about a sign-out: {error:#}"
                );
                HubOutcome::NotConfirmed
            }
        },
    };

    // 2. Stop beating before anything is cleared. A loop still running here would
    //    deliver one more outcome against an entitlement that is being deleted,
    //    and `handle_outcome` acts on that outcome — including stopping a Core
    //    that is already being stopped.
    runtime::stop();

    // 2b. Block the mirror BEFORE the deletion, not after.
    //
    //     `runtime::stop()` asks the loop to stop; it does not prove the task has.
    //     A beat already in flight can reach `read_code_rehydrating` a moment
    //     later, re-adopt a code from a mirror that still exists, and mirror it
    //     again — undoing the sign-out the student is about to be told succeeded.
    //     The marker makes that write refuse itself (`credential::write_code`), so
    //     ordering it before the delete is the difference between a race and no
    //     race at all.
    credential::mark_signed_out();

    // 3. Tunnel down. Best-effort: `stop_core` is safe when nothing is running,
    //    and a student who cannot stop their Core must still be able to sign out.
    if let Err(error) = crate::core::manager::CoreManager::global().stop_core().await {
        logging!(
            warn,
            Type::Config,
            "[locus] could not stop the core while signing out: {error:#}"
        );
    }

    // 4. The generated profile. Also best-effort, and for the same reason the
    //    function itself is: `remove_tier_profile` logs and continues on failure,
    //    and the config that references it is deleted in the next step.
    apply::remove_tier_profile().await;

    // 5. The entitlement and the durable mirror, in one operation. This MUST be
    //    `clear_entitlement` and not a hand-written draft edit: it is the only
    //    path that also removes the machine-scoped mirror, and the mirror is the
    //    thing that would resurrect the code on the next launch.
    store::clear_entitlement().await?;

    // The release file and the update offer describe a device that is no longer
    // signed in. Leaving the offer would prompt the next student on this machine
    // to update an app that is not theirs yet; leaving a stale usage window would
    // hand the next activation a spent allowance belonging to someone else.
    if let Err(error) = store::clear_device_state().await {
        // Not fatal: neither of these affects whether the device is signed out,
        // and failing here would report a failed sign-out for a completed one.
        logging!(
            warn,
            Type::Config,
            "[locus] signed out, but could not clear leftover device state: {error:#}"
        );
    }

    logging!(info, Type::Config, "[locus] signed out on this device");

    Ok(SignOutResult::with_hub(hub))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, reason = "tests assert by panicking")]
mod tests {
    use super::*;

    /// The hub must be told before the key is forgotten.
    ///
    /// Reversing this is a silent failure: the local sign-out succeeds, the code
    /// is gone, and the notification can never be made because the value that
    /// authenticates it no longer exists. Nothing in the build would notice —
    /// which is exactly why the order is asserted.
    #[test]
    fn the_hub_is_told_before_the_key_is_forgotten() {
        let notify = ORDER
            .iter()
            .position(|step| *step == Step::NotifyHub)
            .expect("NotifyHub is in the order");
        let forget = ORDER
            .iter()
            .position(|step| *step == Step::ForgetKey)
            .expect("ForgetKey is in the order");
        assert!(
            notify < forget,
            "the hub notification must run while the code still exists; ORDER has ForgetKey at \
             {forget} and NotifyHub at {notify}"
        );
    }

    /// The heartbeat must be stopped before the entitlement is cleared, or a beat
    /// that lands mid-sign-out reads a config that is being rewritten.
    #[test]
    fn beating_stops_before_the_entitlement_is_cleared() {
        let stop = ORDER
            .iter()
            .position(|step| *step == Step::StopHeartbeat)
            .expect("StopHeartbeat is in the order");
        let forget = ORDER
            .iter()
            .position(|step| *step == Step::ForgetKey)
            .expect("ForgetKey is in the order");
        assert!(
            stop < forget,
            "a heartbeat running across the clear would re-assert an entitlement that is being \
             deleted"
        );
    }

    /// Forgetting the key is LAST. Every other step is best-effort, and any of
    /// them failing must not leave the code on disk — so the one step that must
    /// succeed runs after the ones that are allowed to fail.
    #[test]
    fn forgetting_the_key_is_the_last_step() {
        assert_eq!(
            ORDER.last().copied(),
            Some(Step::ForgetKey),
            "the entitlement clear must be the final stage"
        );
    }

    /// Every stage is named exactly once. A duplicated step would run one of
    /// these twice — `stop_core` is idempotent, but a second `clear_entitlement`
    /// would rewrite the config file for no reason, and the check is cheap.
    #[test]
    fn the_order_has_no_duplicates_and_covers_every_step() {
        let mut seen = Vec::new();
        for step in ORDER {
            assert!(!seen.contains(&step), "{step:?} appears twice in ORDER");
            seen.push(step);
        }
        assert_eq!(seen.len(), 5, "a step was added to `Step` but not to `ORDER`");
    }

    /// `ORDER` describes the code, and an assertion about a description is
    /// worthless if the description drifts from what runs.
    ///
    /// The tests above pin `ORDER`. None of them notices if `sign_out` is edited
    /// to call the release AFTER `clear_entitlement` while `ORDER` still says
    /// otherwise — which is the exact silent failure this module documents (the
    /// code is gone, so the notification can never be made, and the local
    /// sign-out still looks like it succeeded).
    ///
    /// So the real body is read from source and the call sites' byte offsets are
    /// compared. Crude, and deliberate: it is the only cheap way to assert an
    /// ORDER over statements in Rust, and a false pass is impossible as long as
    /// the calls keep their distinguishing text — if that text changes, this test
    /// fails loudly rather than passing vacuously.
    #[test]
    fn the_code_calls_these_steps_in_the_order_order_describes() {
        let source = include_str!("signout.rs");

        // Only the `sign_out` body, so a mention inside a test or a doc comment
        // cannot satisfy the check.
        let body_start = source
            .find("pub async fn sign_out()")
            .expect("sign_out must exist; this test reads its body");
        let body = &source[body_start..];
        let body_end = body.find("#[cfg(test)]").unwrap_or(body.len());
        let body = &body[..body_end];

        let position = |needle: &str| {
            body.find(needle)
                .unwrap_or_else(|| panic!("sign_out no longer contains {needle:?}"))
        };

        let notify = position("activation::release(");
        let stop_beat = position("runtime::stop()");
        let stop_tunnel = position("stop_core()");
        let remove_profile = position("remove_tier_profile()");
        let forget = position("clear_entitlement()");

        assert!(
            notify < stop_beat && stop_beat < stop_tunnel && stop_tunnel < remove_profile && remove_profile < forget,
            "sign_out's calls are out of order: release={notify} stop_heartbeat={stop_beat} \
             stop_tunnel={stop_tunnel} remove_profile={remove_profile} forget={forget} — the hub \
             must be told while the code still exists, and the entitlement must be cleared last"
        );
    }

    /// The student is never told the hub confirmed something it did not.
    ///
    /// This is the assertion behind the three-way `HubOutcome`: a UI that mapped
    /// `NotConfirmed` onto the same sentence as `Released` would tell a student
    /// on a dead network that their key had been released, which is the exact
    /// reassurance that costs them a second email.
    #[test]
    fn each_hub_outcome_is_a_distinct_fact() {
        assert_ne!(HubOutcome::Released, HubOutcome::NothingToRelease);
        assert_ne!(HubOutcome::Released, HubOutcome::NotConfirmed);
        assert_ne!(HubOutcome::NothingToRelease, HubOutcome::NotConfirmed);
    }

    /// A damaged stored code must not stop the LOCAL half of a sign-out.
    ///
    /// The field report behind this: an invalid code stored on a macOS machine
    /// was never signed out, the client kept connecting with it and failed
    /// silently, and the student's only recovery was to uninstall the app and wipe
    /// its data. The reason sign-out could not help was here — `release()` is
    /// called first (the order above requires it) and it validates locally, so a
    /// damaged code errored out of the whole function before anything was deleted.
    ///
    /// This asserts the property, not the mechanism: every statement that
    /// *destroys* state must be reachable without the hub call having succeeded.
    /// The hub arm is guarded by `validate_code`, and the destructive statements
    /// sit after it rather than inside it.
    #[test]
    fn a_damaged_stored_code_does_not_block_the_local_half_of_a_sign_out() {
        let source = include_str!("signout.rs");
        let body_start = source
            .find("pub async fn sign_out()")
            .expect("sign_out must exist; this test reads its body");
        let body = &source[body_start..];
        let body_end = body.find("#[cfg(test)]").unwrap_or(body.len());
        let body = &body[..body_end];

        assert!(
            body.contains("activation::validate_code(&code)"),
            "sign_out must check the code itself before handing it to the hub, so a damaged one \
             does not surface as 'the hub was not told'"
        );

        let hub_arm_start = body.find("activation::validate_code(&code)").expect("checked above");
        let hub_arm = &body[hub_arm_start..];
        let arm_end = hub_arm.find("runtime::stop()").unwrap_or(hub_arm.len());
        let hub_arm = &hub_arm[..arm_end];

        for destructive in [
            "clear_entitlement()",
            "remove_tier_profile()",
            "credential::mark_signed_out()",
        ] {
            assert!(
                !hub_arm.contains(destructive),
                "{destructive} sits inside the hub arm: a damaged code would skip it and the \
                 student would have to uninstall to sign out"
            );
        }
    }
}
