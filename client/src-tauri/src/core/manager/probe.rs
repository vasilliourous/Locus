//! Asking the Core whether it can actually carry a packet.
//!
//! # Why this exists
//!
//! Readiness used to be a **latch**: `mark_core_ready()` was called the instant
//! the start call returned, and `is_core_ready()` answered by consulting that
//! latch. Nothing ever re-observed the Core, so "ready" meant "we once asked it
//! to start", not "it is working".
//!
//! That produced two lies a student could see:
//!
//!   * the button said **connected** while the machine had no usable network at
//!     all — the Core process existed, so the latch was set, so the app claimed a
//!     tunnel that could not carry anything;
//!   * the button said **connected** through a window where both traffic
//!     counters sat at zero, because a Core that bound nothing still satisfied
//!     the latch.
//!
//! The fix is to make readiness an **observation**. The Core exposes its own
//! control API (mihomo's external controller); if that API answers, the Core is
//! up and serving. If it does not answer while a running mode says it should be
//! up, the honest answer is "not ready" — and that is exactly the signal the UI
//! needs to render *connecting* instead of *connected*.
//!
//! # The second half: a serving Core is not the same as a working tunnel
//!
//! Making readiness an observation fixed "connected" for a Core that had *not
//! started*, but it still lied in the case that actually matters on a school
//! network: **the Core is up and answering locally, and the machine has no
//! usable internet**. mihomo binds its external controller and answers `/version`
//! whether or not a single packet can leave the machine — so a Core that can
//! route nothing still reads `Serving`, and the app still said "Connected".
//!
//! So readiness has two questions, not one:
//!
//!   1. is the Core up?            → [`probe_core_api`] (a local `/version`)
//!   2. can it carry a packet?     → [`probe_egress`] (a real request THROUGH the
//!      tunnel, via mihomo's own delay test for the tier's proxy group)
//!
//! Both must hold for [`Readiness::Ready`]. A Core that is up but whose egress
//! check fails is [`Readiness::NoEgress`]: the UI shows "connecting / no
//! service" rather than claiming a tunnel that cannot carry traffic.
//!
//! # The shape, and why the decision is separate from the request
//!
//! [`probe_core_api`] and [`probe_egress`] perform I/O. [`Readiness`] is the pure
//! rule that turns their outcomes into a state. Keeping them apart is what lets
//! the rule that matters — *a failed probe revokes readiness* — be pinned by a
//! unit test that never opens a socket. A test that needs a live Core to check a
//! rule about a dead Core is a test that cannot fail on the machine that runs it.

use std::time::Duration;

/// How long to wait for the Core's control API before calling it unresponsive.
///
/// Short on purpose. This runs on the status path, which the UI polls at 750 ms
/// while a connect is settling, and a probe that can block for seconds would
/// make the button feel broken in the one moment it is being watched. A Core that
/// cannot answer a local HTTP request in this window is not ready to carry a
/// student's traffic, so the timeout is a *decision*, not just a guard.
pub const PROBE_TIMEOUT: Duration = Duration::from_millis(400);

/// How long a **single** through-tunnel egress attempt may take, as reported to
/// mihomo's own delay test.
///
/// This is the per-attempt budget handed to `delay_group`, not a cap on the
/// whole check — see [`EGRESS_DEADLINE`]. It is longer than the local probe
/// because it is a *real* request that leaves the machine, crosses the tunnel
/// and returns, and the first request through a freshly-dialled tunnel pays a
/// cold start that a warm link never does.
///
/// Deliberately generous relative to the old 3 s: on a slow school link the
/// first packet can take several seconds to get through, and calling that "no
/// egress" is the single most likely way to strand a *working* tunnel on
/// "connecting" forever. A tunnel this slow is still a tunnel the student
/// bought.
pub const EGRESS_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the **whole** egress check may take, across all attempts.
///
/// Bounds the status path — the UI polls at 750 ms while a connect settles, and
/// an unbounded check would make the button feel frozen in the one moment it is
/// being watched. It is larger than a single attempt so the Core's own answer
/// (a timeout, a refusal) wins in the normal case, and large enough to cover
/// [`EGRESS_ATTEMPTS`] attempts plus the small gap between them.
pub const EGRESS_DEADLINE: Duration = Duration::from_secs(12);

/// How many times one egress check may try before it reports `Failing`.
///
/// More than one because a single failed round trip is not evidence the tunnel
/// is broken: a CDN blip, a DNS answer that had to be re-queried, or a cold
/// proxy connection all fail once and succeed immediately after. The retired
/// behaviour judged the tunnel on one attempt, so a transient miss flipped a
/// working tunnel back to "connecting" — the report this fixes. All attempts
/// still share [`EGRESS_DEADLINE`], so a genuinely dead tunnel is reported fast
/// rather than retried forever.
pub const EGRESS_ATTEMPTS: u32 = 2;

/// The pause between egress attempts.
///
/// A transient failure — a proxy socket still tearing down, a DNS answer still in
/// flight — will fail again instantly if the retry races it, so the second
/// attempt waits long enough for the first attempt's socket to be gone. Short
/// enough not to matter against [`EGRESS_DEADLINE`], long enough to actually be
/// a second attempt rather than a repeat of the first.
pub const EGRESS_RETRY_GAP: Duration = Duration::from_millis(500);

/// The URL the egress check fetches **through the tunnel**.
///
/// A 204-only endpoint on a highly-available CDN: tiny, cacheable, and it
/// returns a body-less success so a captive portal answering in the tunnel's
/// place is caught (a portal returns a redirect/HTML, not a 204 from this host).
/// mihomo's own default "connectivity check" family; chosen over the Locus hub
/// so the check tests *general* internet egress, not one host — a tunnel that
/// reaches only the hub has not proven it can carry the student's traffic.
pub const EGRESS_TEST_URL: &str = "http://cp.cloudflare.com/generate_204";

/// What the Core's control API said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// The Core answered with a usable response. It is up and serving.
    Serving,
    /// A running mode claims the Core is up, but it did not answer.
    ///
    /// This covers a connection refused, a timeout, a transport error, and a
    /// response that did not come from the Core (a proxy or portal answering in
    /// its place). They are one outcome on purpose: from the student's point of
    /// view "the tunnel is not working" is the same fact, and the UI does not
    /// have a different action for each.
    Unresponsive,
    /// No Core is supposed to be running, so there was nothing to ask.
    NotRunning,
}

/// What the through-tunnel egress check observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EgressOutcome {
    /// A real request completed **through** the tunnel. Traffic can move.
    Ok,
    /// The check could not complete: no uplink, a dead proxy, a captive portal,
    /// a timeout. From the student's point of view this is one fact — the tunnel
    /// is not carrying traffic — so the UI does not need a different action for
    /// each, and they are one outcome on purpose.
    Failing,
    /// The check was not attempted, because the local Core is not even up (or
    /// nothing is meant to be running). Distinct from `Failing`: there is no
    /// point reporting "no egress" for a Core that has not started.
    NotAttempted,
}

/// The readiness decision, given the last probes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
    /// The Core is up, answering, **and** a real request completed through the
    /// tunnel. Only this state may be shown as "connected".
    Ready,
    /// The Core should be up but is not answering: still dialling, or broken.
    /// Either way the UI must not claim the tunnel is up.
    NotReady,
    /// The Core is up and answering locally, but no traffic is getting through:
    /// the machine has no usable uplink, the chosen server is unreachable, or a
    /// captive portal is intercepting. This is the "connected with no wifi" case
    /// and it must NOT render as connected.
    NoEgress,
    /// Nothing is meant to be running.
    Stopped,
}

impl Readiness {
    /// Whether the Core can carry a packet right now.
    #[must_use]
    pub const fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Whether the Core is up (even if traffic is not yet flowing). The UI uses
    /// this to keep showing *connecting* rather than flashing *disconnected*
    /// while a Core that has started fails to get traffic through.
    #[must_use]
    pub const fn is_core_up(self) -> bool {
        matches!(self, Self::Ready | Self::NoEgress)
    }
}

/// Turns the two probes into the readiness the rest of the app should report.
///
/// Everything here is a *precondition for claiming a tunnel*, and [`Readiness::Ready`]
/// requires **all** of them:
///
///   * `outcome`       — the Core answered its control API (it is up);
///   * `egress`        — a real request completed through the tunnel;
///   * `latch_active`  — a Core was started and not declared stopped.
///
/// `latch_active` is the old signal that used to be sufficient by itself. It is
/// retained because it still distinguishes *stopped* from *failing to start*,
/// but it can no longer produce [`Readiness::Ready`] alone. That is the whole
/// point of this function — the latch is necessary and no longer sufficient.
///
/// The two questions stay separate because their *failures* differ and the UI
/// must render them differently: a Core that is not answering is *connecting*,
/// while a Core that is up but cannot move a packet is *connected but no
/// service* — the school-wifi case, and the one this function exists to stop
/// rendering as "Connected".
#[must_use]
pub const fn decide(outcome: ProbeOutcome, egress: EgressOutcome, latch_active: bool) -> Readiness {
    match outcome {
        // The API answered AND a real request went through the tunnel. This is
        // the only path to Ready: a Core that cannot prove it is serving, or
        // that is serving but cannot carry a packet, can never be reported as
        // connected.
        ProbeOutcome::Serving if matches!(egress, EgressOutcome::Ok) => Readiness::Ready,
        // The Core is up but nothing is getting through. The tunnel exists and
        // the machine cannot use it — no wifi, a dead server, a captive portal.
        // This is the "connected with no internet" lie; it must not be Ready.
        ProbeOutcome::Serving => Readiness::NoEgress,
        // It should be up (the latch says a start was attempted and not
        // undone) but it is not answering. Report NotReady rather than Stopped
        // so the UI keeps saying "connecting" instead of flashing
        // "disconnected" at a Core that is still coming up.
        ProbeOutcome::Unresponsive if latch_active => Readiness::NotReady,
        // Either nothing was asked (no Core) or nothing has started. Stopped.
        ProbeOutcome::Unresponsive | ProbeOutcome::NotRunning => Readiness::Stopped,
    }
}

/// Whether a probe outcome should revoke an already-active readiness latch.
///
/// This is the fix for "it said connected with no wifi": once we have evidence
/// the Core is not answering, the latch must be dropped, or the next
/// `is_core_ready()` reads the stale `true` and the lie survives the probe that
/// disproved it.
#[must_use]
pub const fn revokes_latch(outcome: ProbeOutcome, latch_active: bool) -> bool {
    latch_active && matches!(outcome, ProbeOutcome::Unresponsive)
}

/// Asks the Core whether it is serving.
///
/// Uses the Core's own control API (`/version` through the mihomo plugin), which
/// is the cheapest question that still proves the Core is up *and* able to answer
/// on the channel the app talks to it over. The plugin already owns the
/// transport choice — HTTP to the external controller, or the sidecar's Unix
/// socket — so this does not re-derive an address or a secret and cannot drift
/// from how the rest of the app reaches the Core.
///
/// # Why the timeout is applied here rather than on the client
///
/// The plugin builds its request client without a timeout, so a Core that has
/// accepted the socket but stopped answering would hang this call indefinitely.
/// Since the status command is what renders the button, a hang would freeze the
/// UI at the exact moment it is being watched. The timeout is applied around the
/// call instead, and a timeout is [`ProbeOutcome::Unresponsive`] — the same
/// answer a refusal gets, because both mean "this tunnel is not carrying
/// traffic".
///
/// # Why a caller with no core running never reaches the network
///
/// `running` is passed in from the run state rather than read here, so the
/// "nothing is meant to be up" case costs nothing and a stopped Core does not
/// generate a request that is guaranteed to fail.
pub async fn probe_core_api(running: bool) -> ProbeOutcome {
    if !running {
        return ProbeOutcome::NotRunning;
    }

    // Read through the plugin's context, which is the same channel every other
    // Core call uses. A version call is the lightest endpoint that still proves
    // the API is answering rather than merely listening.
    let probe = crate::core::handle::Handle::mihomo().get_version();

    match tokio::time::timeout(PROBE_TIMEOUT, probe).await {
        Ok(Ok(_version)) => ProbeOutcome::Serving,
        // Answered, but not with success: a 401 from a rotated secret, a 404
        // from something else listening on the port, a refused connection. All
        // "not this Core, serving this student".
        Ok(Err(_error)) => ProbeOutcome::Unresponsive,
        Err(_elapsed) => ProbeOutcome::Unresponsive,
    }
}

/// Asks the Core to carry a real packet **through the tunnel**.
///
/// This is the half the local `/version` probe cannot answer. mihomo answers its
/// own control API whether or not it can reach anything, so a Core on a machine
/// with no uplink still reads `Serving`. The only honest proof that the tunnel
/// works is a request that goes out through the proxy and comes back.
///
/// # Why mihomo's own delay test, rather than a request from this process
///
/// mihomo's `delay_group` makes the *Core* dial the test URL through the
/// selected proxy and reports the round-trip. That exercises the real path
/// (outbound, proxy, DNS resolution, the return route) rather than re-deriving
/// the proxy address, port and secret here and running a second HTTP client that
/// could disagree with the Core about how traffic leaves. It also needs no
/// privileges of its own.
///
/// # Why it is not attempted when the Core is down
///
/// Asking a stopped Core to run a delay test is a request guaranteed to fail, so
/// `core_serving` is passed in from the local probe and a down Core short-circuits
/// to [`EgressOutcome::NotAttempted`]. That also keeps this function's cost off
/// the path where it cannot change the answer.
///
/// # Interpreting the result
///
/// `delay_group` returns a map of proxy-name -> delay for the group's members.
/// mihomo only inserts an entry for a member whose test *succeeded*, so a
/// non-empty map already means at least one member completed a round trip. The
/// values are still classified — see [`delay_is_a_measurement`] — rather than
/// merely checked for `> 0`, so this probe and the delay UI agree about what a
/// real measurement is. An error (no uplink, dead server, DNS failure), a
/// timeout, or a map with no usable value is [`EgressOutcome::Failing`].
///
/// # Why it tries more than once
///
/// One failed round trip is not proof the tunnel is broken. A cold proxy
/// connection, a CDN blip or a DNS answer that had to be re-queried all fail
/// once and succeed on the next attempt, and judging the tunnel on a single
/// attempt is what let a working connection drop back to "connecting" — the
/// report this fixes. `EGRESS_ATTEMPTS` bounds the retries and `EGRESS_DEADLINE`
/// bounds the whole check, so a genuinely dead tunnel is reported promptly.
pub async fn probe_egress(core_serving: bool) -> EgressOutcome {
    if !core_serving {
        return EgressOutcome::NotAttempted;
    }

    // One deadline for the whole check, not per attempt: a Core that answers
    // each attempt slowly must not be given `EGRESS_ATTEMPTS * budget` on a path
    // the UI polls every 750 ms.
    let deadline = tokio::time::Instant::now() + EGRESS_DEADLINE;

    for _ in 0..EGRESS_ATTEMPTS {
        if tokio::time::Instant::now() >= deadline {
            break;
        }

        if egress_attempt_once().await {
            return EgressOutcome::Ok;
        }

        // A short gap so a transient failure (a socket still tearing down, a DNS
        // cache miss in flight) is given a moment to clear before the retry —
        // otherwise the second attempt races the first and fails the same way.
        tokio::time::sleep(EGRESS_RETRY_GAP).await;
    }

    EgressOutcome::Failing
}

/// One through-tunnel attempt. `true` when a real round trip completed.
///
/// Split out so the retry loop above reads as policy (how many tries, how long)
/// while the single attempt stays the one place that talks to the Core.
async fn egress_attempt_once() -> bool {
    let timeout_secs = u32::try_from(EGRESS_ATTEMPT_TIMEOUT.as_secs()).unwrap_or(5);
    let probe = crate::core::handle::Handle::mihomo().delay_group(
        crate::locus::tier::GROUP_NAME,
        EGRESS_TEST_URL,
        timeout_secs,
    );

    // The plugin's delay call has its own timeout (passed above), but wrap it
    // too: a Core that accepts the request and then never answers must not hang
    // the status path. The wrapper is slightly longer than the inner timeout so
    // the Core's own answer wins in the normal case.
    match tokio::time::timeout(EGRESS_ATTEMPT_TIMEOUT + PROBE_TIMEOUT, probe).await {
        Ok(Ok(delays)) => delays
            .values()
            .any(|&delay| delay_is_a_measurement(delay, timeout_secs)),
        Ok(Err(_)) | Err(_) => false,
    }
}

/// Whether a mihomo delay value is a real round-trip measurement.
///
/// This mirrors the frontend's `classifyDelay` (`client/src/utils/delay.ts`),
/// which is the app's single definition of what a delay value means. Keeping the
/// two in step is the point: mihomo reports non-measurements inside the same
/// numeric field it reports measurements in — `0` for a failed test and the
/// timeout value itself for a timed-out one — so a probe that merely checked
/// `> 0` would accept a sentinel and announce a tunnel that had just timed out.
///
/// The sentinels, and what they mean:
///
///   * `0`                    -> the test failed; not a measurement.
///   * `>= timeout_secs*1000` -> the test hit its budget; not a measurement.
///   * `> 100_000` (1e5)      -> implausible as milliseconds; an error sentinel.
///   * anything else `> 0`    -> a measured round trip.
///
/// The budget used here is the probe's own [`EGRESS_ATTEMPT_TIMEOUT`], not the
/// frontend's `DEFAULT_DELAY_TIMEOUT`: the probe is judging *its* request, so its
/// own deadline is the one that decides when a value is a timeout.
#[must_use]
const fn delay_is_a_measurement(delay: u32, timeout_secs: u32) -> bool {
    const IMPLAUSIBLE_DELAY: u32 = 100_000;
    let timeout_ms = timeout_secs.saturating_mul(1000);
    delay > 0 && delay < timeout_ms && delay <= IMPLAUSIBLE_DELAY
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The load-bearing rule: without the API answering, there is no Ready —
    /// regardless of how confidently the latch claims a Core was started.
    #[test]
    fn an_unresponsive_core_is_never_ready() {
        assert_eq!(
            decide(ProbeOutcome::Unresponsive, EgressOutcome::NotAttempted, true),
            Readiness::NotReady
        );
        assert!(!decide(ProbeOutcome::Unresponsive, EgressOutcome::NotAttempted, true).is_ready());
    }

    /// The exact state the old latch produced: a start was recorded, nothing was
    /// ever verified, and the app called it connected. This must now be
    /// NotReady, because the two inputs are identical and the only new fact is
    /// that we asked.
    #[test]
    fn a_latch_alone_can_no_longer_report_connected() {
        for outcome in [ProbeOutcome::Unresponsive, ProbeOutcome::NotRunning] {
            assert!(
                !decide(outcome, EgressOutcome::NotAttempted, true).is_ready(),
                "{outcome:?} with an active latch must not be Ready"
            );
        }
    }

    /// Ready needs BOTH a serving Core and proven egress. If either half ever
    /// stops being required, a false "connected" comes straight back.
    #[test]
    fn only_a_serving_core_with_egress_is_ready() {
        assert!(decide(ProbeOutcome::Serving, EgressOutcome::Ok, true).is_ready());
        assert!(decide(ProbeOutcome::Serving, EgressOutcome::Ok, false).is_ready());
        // Serving but no egress is NOT ready — the whole bug this guards.
        assert!(!decide(ProbeOutcome::Serving, EgressOutcome::Failing, true).is_ready());
    }

    /// THE headline rule: a Core that is up and answering locally but cannot
    /// carry a packet must never render as connected.
    ///
    /// This is the school-wifi case: mihomo binds its control port and answers
    /// `/version` with no uplink at all, so the local probe alone reported
    /// Connected while every packet died. It must now be `NoEgress`, never
    /// `Ready`.
    #[test]
    fn a_serving_core_without_egress_is_not_connected() {
        let readiness = decide(ProbeOutcome::Serving, EgressOutcome::Failing, true);
        assert_eq!(readiness, Readiness::NoEgress);
        assert!(!readiness.is_ready(), "no egress must never be Ready");
        // But it is still "core up", so the UI shows connecting / no service
        // rather than flashing disconnected.
        assert!(readiness.is_core_up());
    }

    /// `NotAttempted` for a serving Core is treated as no egress: if we did not
    /// prove traffic moves, we must not claim it does.
    #[test]
    fn unproven_egress_is_not_ready() {
        assert_eq!(
            decide(ProbeOutcome::Serving, EgressOutcome::NotAttempted, true),
            Readiness::NoEgress
        );
    }

    /// A failing start must read as *connecting*, not as *disconnected*: the
    /// latch says work is in flight, and flashing "not connected" at a Core that
    /// is still coming up is the other half of the original complaint.
    #[test]
    fn a_failing_start_reads_as_connecting_not_stopped() {
        assert_eq!(
            decide(ProbeOutcome::Unresponsive, EgressOutcome::NotAttempted, true),
            Readiness::NotReady
        );
    }

    /// With nothing started there is nothing to be unresponsive, so it is
    /// Stopped — a fresh install must not sit on "connecting" forever.
    #[test]
    fn nothing_started_is_stopped() {
        assert_eq!(
            decide(ProbeOutcome::Unresponsive, EgressOutcome::NotAttempted, false),
            Readiness::Stopped
        );
        assert_eq!(
            decide(ProbeOutcome::NotRunning, EgressOutcome::NotAttempted, true),
            Readiness::Stopped
        );
    }

    /// An egress failure does not, by itself, mean the Core is gone: a running
    /// Core with no uplink must not be reported as Stopped (which would let a
    /// restart-on-stopped path fire, or the latch be dropped for the wrong
    /// reason). Only an unresponsive Core revokes the latch.
    #[test]
    fn egress_failure_alone_does_not_revoke_the_latch() {
        assert!(!revokes_latch(ProbeOutcome::Serving, true));
    }

    /// The revocation rule that makes the probe stick: a failed probe must drop
    /// the latch, or `is_core_ready()` keeps answering from stale memory.
    #[test]
    fn a_failed_probe_revokes_an_active_latch() {
        assert!(revokes_latch(ProbeOutcome::Unresponsive, true));
        assert!(!revokes_latch(ProbeOutcome::Serving, true));
        assert!(!revokes_latch(ProbeOutcome::NotRunning, true));
        // Nothing to revoke.
        assert!(!revokes_latch(ProbeOutcome::Unresponsive, false));
    }

    /// The egress target is a 204-only endpoint on a CDN.
    ///
    /// Pinned by value so a change is deliberate: the URL decides whether the
    /// probe tests *general* internet egress (the point — a tunnel that reaches
    /// only the hub has not proven it can carry the student's traffic) and
    /// whether a captive portal answering in the tunnel's place is caught, which
    /// depends on the target returning a body-less 204 rather than a redirect or
    /// an HTML page. `tests/egress_probe_engine.rs` restates this URL because
    /// `core` is private to the crate; this test is what makes a change here
    /// visible.
    #[test]
    fn the_egress_target_is_the_expected_cdn_204() {
        assert_eq!(EGRESS_TEST_URL, "http://cp.cloudflare.com/generate_204");
    }

    /// A real measurement is a positive delay that fits inside the attempt's own
    /// budget. This is the case the probe is *for*: a round trip happened.
    #[test]
    fn a_plausible_delay_is_a_measurement() {
        assert!(delay_is_a_measurement(1, 5));
        assert!(delay_is_a_measurement(54, 5));
        assert!(delay_is_a_measurement(4999, 5));
    }

    /// The zero sentinel: mihomo reports a *failed* test as `0` in the same field
    /// it reports measurements in. Counting it as egress would announce a tunnel
    /// whose test had just failed — the exact false "connected" the readiness
    /// probe exists to prevent.
    #[test]
    fn a_zero_delay_is_not_a_measurement() {
        assert!(!delay_is_a_measurement(0, 5));
    }

    /// The timeout sentinel: mihomo reports a *timed-out* test as the timeout
    /// value itself (ms). A mere `> 0` check would count this as success, so the
    /// probe would call a timed-out tunnel connected. This is the specific
    /// disagreement with `classifyDelay` this function removes.
    #[test]
    fn a_timeout_sentinel_is_not_a_measurement() {
        // 5 s budget -> a delay of 5000 ms is the timeout sentinel, not a result.
        assert!(!delay_is_a_measurement(5000, 5));
        // Anything at or beyond the budget is the same non-answer.
        assert!(!delay_is_a_measurement(6000, 5));
    }

    /// An implausible value is an error sentinel, not a round trip. It must not be
    /// allowed to outrank a real measurement, and it must not read as success.
    #[test]
    fn an_implausible_delay_is_not_a_measurement() {
        assert!(!delay_is_a_measurement(100_001, 60));
        assert!(!delay_is_a_measurement(u32::MAX, 5));
    }
}
