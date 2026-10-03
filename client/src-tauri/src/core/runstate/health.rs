//! Run State separates observed Service health from session intent.
//! Core lifecycle remains in `CoreManager`.

use crate::core::manager::RunningMode;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ServiceHealth {
    #[default]
    Unknown,
    Ready,
    NotInstalled,
    VersionMismatch,
    Unavailable(String),
}

impl ServiceHealth {
    const fn kind(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Ready => "ready",
            Self::NotInstalled => "notInstalled",
            Self::VersionMismatch => "versionMismatch",
            Self::Unavailable(_) => "unavailable",
        }
    }

    fn reason(&self) -> Option<String> {
        match self {
            Self::Unavailable(reason) => Some(reason.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PendingAction {
    Install,
    Uninstall,
    Reinstall,
    ForceReinstall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunState {
    pub health: ServiceHealth,
    pub pending: Option<PendingAction>,
    pub mode: RunningMode,
    pub is_admin: bool,
    /// A privileged operation is currently in flight, so `health` may be stale.
    pub op_in_flight: bool,
}

impl RunState {
    #[must_use]
    pub const fn service_ready(&self) -> bool {
        matches!(self.health, ServiceHealth::Ready)
    }

    #[must_use]
    pub const fn service_usable(&self) -> bool {
        self.service_ready() && !self.op_in_flight
    }

    #[must_use]
    pub const fn tun_capable(&self) -> bool {
        self.is_admin || self.service_usable()
    }

    /// In-flight and merely-absent states require no user decision.
    ///
    /// The Service is the only supported way to run the Core, so a Service that
    /// is present but unusable is now always the student's to resolve. There is
    /// no longer an accepted-Sidecar escape that makes such a Service ignorable.
    ///
    /// `NotInstalled` is deliberately NOT listed here, and that is not the same
    /// as saying a missing Service needs no attention. It is answered elsewhere:
    /// startup raises an install request for an absent Service on macOS (see
    /// `prepare_startup`), and `pending` below makes that request a decision.
    /// Keeping it out of this arm preserves the runtime safety valve in
    /// `tun_should_be_disabled` — a confirmed-absent Service with nothing
    /// outstanding has to turn TUN off on its own, or the Core can never start.
    /// Including it here would silently disable that valve on every platform,
    /// which is why the affordance is raised through `pending` instead.
    #[must_use]
    pub const fn service_needs_attention(&self) -> bool {
        if self.op_in_flight {
            return false;
        }
        if self.pending.is_some() {
            return true;
        }
        matches!(
            self.health,
            ServiceHealth::VersionMismatch | ServiceHealth::Unavailable(_)
        )
    }

    /// Unknown, in-flight, and attention-required states are inconclusive.
    #[must_use]
    pub const fn tun_should_be_disabled(&self, tun_enabled: bool) -> bool {
        if !tun_enabled || self.tun_capable() {
            return false;
        }
        !matches!(self.health, ServiceHealth::Unknown) && !self.op_in_flight && !self.service_needs_attention()
    }

    /// Whether *startup* should turn the stored TUN preference off.
    ///
    /// Scoped to the platforms where an absent Service means it was REMOVED.
    ///
    /// The reasoning behind this branch is about a Service that went away: a
    /// preference left on across a removal would make every future start wait
    /// for a Service that is not coming, which is why startup clears it.
    ///
    /// That premise does not hold on macOS, where an absent Service is the
    /// *ordinary first-run state* — a `.app` dragged out of the `.dmg` has one,
    /// because nothing in that path registers it. Clearing the preference there
    /// is actively harmful: it is half of the deadlock. The install request is
    /// driven by the Service's absence (see `prepare_startup`), so the state
    /// stays "needs an answer" and the correct outcome is to leave the
    /// preference alone and let the student install the Service.
    ///
    /// Keeping the Windows behaviour rather than flipping it globally is the
    /// point: on Windows an absent Service really does mean removal, because
    /// NSIS registers it during setup.
    #[must_use]
    pub const fn startup_tun_should_be_disabled(&self, tun_enabled: bool) -> bool {
        if !matches!(self.health, ServiceHealth::NotInstalled) {
            return false;
        }
        // macOS: an absent Service is first-run, not removal. Do not clear.
        if cfg!(target_os = "macos") {
            return false;
        }
        self.tun_should_be_disabled(tun_enabled)
    }

    #[must_use]
    pub fn to_view(&self) -> RunStateView {
        RunStateView {
            mode: self.mode,
            service: self.health.kind(),
            service_unavailable_reason: self.health.reason(),
            pending_action: self.pending,
            is_admin: self.is_admin,
            op_in_flight: self.op_in_flight,
            service_usable: self.service_usable(),
            tun_capable: self.tun_capable(),
            service_needs_attention: self.service_needs_attention(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStateView {
    pub mode: RunningMode,
    pub service: &'static str,
    pub service_unavailable_reason: Option<String>,
    pub pending_action: Option<PendingAction>,
    pub is_admin: bool,
    pub op_in_flight: bool,
    pub service_usable: bool,
    pub tun_capable: bool,
    pub service_needs_attention: bool,
}

/// Service state protected by one lock.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct StoredService {
    pub health: ServiceHealth,
    pub pending: Option<PendingAction>,
}

impl StoredService {
    pub fn observe(&mut self, health: ServiceHealth) {
        self.health = health;
        self.pending = None;
    }

    pub const fn request(&mut self, action: PendingAction) {
        self.pending = Some(action);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, reason = "tests assert by panicking")]
mod tests {
    use super::*;

    fn state(health: ServiceHealth, is_admin: bool, op_in_flight: bool) -> RunState {
        RunState {
            health,
            pending: None,
            mode: RunningMode::NotRunning,
            is_admin,
            op_in_flight,
        }
    }

    #[test]
    fn a_service_that_is_merely_absent_needs_no_decision() {
        // `NotInstalled` alone is not a decision. The macOS affordance is raised
        // through a pending install request instead, so that this arm stays out
        // of `tun_should_be_disabled`'s way (see its doc comment).
        assert!(!state(ServiceHealth::NotInstalled, false, false).service_needs_attention());
        assert!(!state(ServiceHealth::Ready, false, false).service_needs_attention());
        assert!(!state(ServiceHealth::Unknown, false, false).service_needs_attention());
    }

    /// The macOS first-run path raises the install request, and THAT is what
    /// makes the state a decision the student must answer.
    ///
    /// This is the link the bug was missing. Excluding `NotInstalled` from
    /// `service_needs_attention` is only safe because startup requests the
    /// install for an absent Service on macOS, which sets `pending` — so the
    /// dialog opens by that route. If the request stops being raised, the
    /// student is back to a dead end with nothing to press, which is what this
    /// asserts against.
    #[test]
    fn an_absent_service_with_an_install_request_is_a_decision() {
        let mut requested = state(ServiceHealth::NotInstalled, false, false);
        requested.pending = Some(PendingAction::Install);

        assert!(
            requested.service_needs_attention(),
            "a raised install request must open the install affordance, even though \
             NotInstalled alone does not"
        );
    }

    #[test]
    fn a_present_but_unusable_service_needs_a_decision() {
        assert!(state(ServiceHealth::VersionMismatch, false, false).service_needs_attention());
        assert!(state(ServiceHealth::Unavailable("boom".into()), false, false).service_needs_attention());
    }

    #[test]
    fn a_requested_action_needs_an_answer_until_it_is_being_carried_out() {
        for action in [
            PendingAction::Install,
            PendingAction::Uninstall,
            PendingAction::Reinstall,
            PendingAction::ForceReinstall,
        ] {
            let mut run_state = state(ServiceHealth::NotInstalled, false, false);
            run_state.pending = Some(action);
            assert!(run_state.service_needs_attention(), "{action:?} is still a question");

            run_state.op_in_flight = true;
            assert!(
                !run_state.service_needs_attention(),
                "{action:?} is under way, which is progress rather than a question"
            );
        }
    }

    #[test]
    fn an_operation_under_way_is_never_a_decision_to_put_to_the_user() {
        for health in [
            ServiceHealth::Unknown,
            ServiceHealth::Ready,
            ServiceHealth::NotInstalled,
            ServiceHealth::VersionMismatch,
            ServiceHealth::Unavailable("boom".into()),
        ] {
            let mut run_state = state(health.clone(), false, true);
            run_state.pending = Some(PendingAction::Uninstall);

            assert!(!run_state.service_needs_attention(), "{health:?}");
        }
    }

    #[test]
    fn the_view_carries_the_derived_answers_and_the_unavailable_reason() {
        let mut run_state = state(ServiceHealth::Unavailable("socket refused".into()), true, false);
        run_state.pending = Some(PendingAction::Reinstall);

        let view = run_state.to_view();

        assert_eq!(view.service, "unavailable");
        assert_eq!(view.service_unavailable_reason.as_deref(), Some("socket refused"));
        assert_eq!(view.pending_action, Some(PendingAction::Reinstall));
        assert!(view.tun_capable, "elevation alone makes TUN possible");
        assert!(!view.service_usable);
        assert!(view.service_needs_attention);
    }

    #[test]
    fn a_healthy_view_reports_no_unavailable_reason() {
        let view = state(ServiceHealth::Ready, false, false).to_view();

        assert_eq!(view.service, "ready");
        assert_eq!(view.service_unavailable_reason, None);
        assert!(view.service_usable);
        assert!(!view.service_needs_attention);
    }

    #[test]
    fn tun_is_left_alone_when_the_user_has_not_asked_for_it() {
        assert!(!state(ServiceHealth::NotInstalled, false, false).tun_should_be_disabled(false));
    }

    #[test]
    fn tun_is_left_alone_while_it_still_works() {
        assert!(!state(ServiceHealth::Ready, false, false).tun_should_be_disabled(true));
        assert!(!state(ServiceHealth::NotInstalled, true, false).tun_should_be_disabled(true));
    }

    #[test]
    fn tun_is_disabled_once_we_know_it_cannot_work() {
        assert!(state(ServiceHealth::NotInstalled, false, false).tun_should_be_disabled(true));
    }

    #[test]
    fn startup_tun_is_disabled_only_for_a_confirmed_absent_service() {
        // Platform-scoped, because the premise differs: on Windows an absent
        // Service means it was REMOVED (NSIS registers it at setup), so a
        // preference left on would wait forever. On macOS it is first-run, and
        // clearing the preference is half of the connect deadlock.
        let settled = state(ServiceHealth::NotInstalled, false, false);
        let expected = !cfg!(target_os = "macos");
        assert_eq!(
            settled.startup_tun_should_be_disabled(true),
            expected,
            "an absent Service clears TUN only where absence means removal"
        );

        for health in [
            ServiceHealth::Unknown,
            ServiceHealth::Ready,
            ServiceHealth::VersionMismatch,
            ServiceHealth::Unavailable("socket refused".into()),
        ] {
            assert!(
                !state(health.clone(), false, false).startup_tun_should_be_disabled(true),
                "{health:?} is not proof that the Service was removed"
            );
        }
    }

    /// The macOS no-clear rule needs its own assertion, not a `cfg!` in a shared
    /// test: a `cfg!`-driven expectation still passes on Windows when the macOS
    /// branch is deleted, so it would not protect the half of the fix that runs
    /// on the platform the bug was found on.
    #[test]
    #[cfg(target_os = "macos")]
    fn startup_does_not_clear_tun_for_an_absent_service_on_macos() {
        assert!(
            !state(ServiceHealth::NotInstalled, false, false).startup_tun_should_be_disabled(true),
            "clearing the preference on first run recreates the deadlock: with TUN off, \
             nothing requests the Service, and the student has no way to connect"
        );
    }

    #[test]
    fn startup_leaves_a_present_but_broken_service_to_the_runtime_reconciler() {
        for health in [
            ServiceHealth::VersionMismatch,
            ServiceHealth::Unavailable("socket refused".into()),
        ] {
            let settled = state(health.clone(), false, false);

            assert!(!settled.startup_tun_should_be_disabled(true), "{health:?}");
        }
    }

    #[test]
    fn startup_tun_is_kept_when_disabled_elevated_or_still_undecided() {
        assert!(!state(ServiceHealth::NotInstalled, false, false).startup_tun_should_be_disabled(false));
        assert!(!state(ServiceHealth::NotInstalled, true, false).startup_tun_should_be_disabled(true));
        assert!(!state(ServiceHealth::NotInstalled, false, true).startup_tun_should_be_disabled(true));

        let mut requested = state(ServiceHealth::NotInstalled, false, false);
        requested.pending = Some(PendingAction::Install);
        assert!(
            !requested.startup_tun_should_be_disabled(true),
            "an install the user has still to answer is not a settled absence"
        );
    }

    #[test]
    fn tun_is_never_disabled_while_an_operation_is_in_flight() {
        assert!(!state(ServiceHealth::Ready, false, true).tun_should_be_disabled(true));
        assert!(!state(ServiceHealth::NotInstalled, false, true).tun_should_be_disabled(true));
    }

    #[test]
    fn tun_is_never_disabled_on_an_unprobed_service() {
        assert!(!state(ServiceHealth::Unknown, false, false).tun_should_be_disabled(true));
    }

    #[test]
    fn tun_is_never_disabled_while_a_decision_is_pending() {
        for health in [
            ServiceHealth::VersionMismatch,
            ServiceHealth::Unavailable("boom".into()),
        ] {
            assert!(
                !state(health.clone(), false, false).tun_should_be_disabled(true),
                "{health:?} is the user's to resolve",
            );
        }

        let mut requested = state(ServiceHealth::NotInstalled, false, false);
        requested.pending = Some(PendingAction::Install);
        assert!(!requested.tun_should_be_disabled(true));
    }

    #[test]
    fn tun_is_disabled_once_a_removed_service_settles_without_a_decision() {
        // The runtime reconciler's job: a Service confirmed absent, with nothing
        // pending and no operation in flight, is the one case where TUN has to
        // go off on its own — otherwise the Core can never start at all.
        let settled = state(ServiceHealth::NotInstalled, false, false);

        assert!(settled.tun_should_be_disabled(true));
    }
}
