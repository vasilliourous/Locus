//! The free tier's monthly allowance — counting, rollover and the throttle decision.
//!
//! # Why this is client-side
//!
//! The hub has **no per-user accounting**, and cannot have one cheaply: each
//! tier is one shadowsocks instance holding a **single shared password**, so
//! every free user is indistinguishable on the wire. There is nowhere on the
//! server to count bytes per student. So the client counts its own traffic.
//!
//! The honest consequences, stated here because they are a design decision
//! rather than an oversight (`docs/business/04-tiers.md` §4.4.3):
//!
//! * **It is soft.** A modified client could ignore the cap.
//! * **It is good enough.** The threat model is a student who wants free fast
//!   internet, not a determined adversary — and the alternative (per-user
//!   SS2022 credentials plus a server-side usage table) is real work for little
//!   return at this scale.
//!
//! # What lives here, and what does not
//!
//! This module is **pure**: given a counter state, a timestamp and an
//! allowance, it answers "how much is used", "has the month rolled over" and
//! "should this connection be throttled". It does not read the clock, touch the
//! disk, or talk to the core — the caller passes `now_ms` in, which is what
//! makes it testable with a fake clock.
//!
//! The one thing it deliberately does not do is decide *what throttling means
//! mechanically* (which tc class, which local cap). That is the caller's job,
//! because it depends on the core's capabilities rather than on arithmetic.

use serde::{Deserialize, Serialize};

/// Milliseconds in the nominal month the allowance resets on.
///
/// Deliberately a fixed 30-day window rather than a calendar month: the client
/// has no trustworthy timezone, the hub stores no billing date per student, and
/// a fixed window is the one rule that cannot disagree with itself across a
/// daylight-saving boundary or a device whose clock is set to the wrong zone.
/// The student sees "resets every 30 days", which is also what the operator can
/// explain.
pub const WINDOW_MS: u64 = 30 * 24 * 60 * 60 * 1000;

/// The fraction of the allowance at which the UI starts warning.
///
/// 80%, per the decided behaviour. Kept here rather than in the UI so the
/// warning and the throttle can never disagree about where the line is.
pub const WARN_FRACTION: f64 = 0.8;

/// The persisted shape of a free user's usage, stored alongside the rest of the
/// product state in `verge.yaml` (see [`crate::locus::store`]).
///
/// Every field is `default`ed so a config written before the free tier existed
/// deserialises cleanly into a fresh, unused window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Usage {
    /// Bytes counted in the current window.
    #[serde(default)]
    pub used_bytes: u64,
    /// The window's start, as a Unix millisecond timestamp.
    ///
    /// `0` means "no window has started yet" — the state of a config that
    /// predates the free tier, or one whose window was reset. [`Self::window`]
    /// treats it as "start one now" rather than as "the epoch", so the first
    /// free byte a student ever moves begins their first month rather than
    /// retroactively expiring it.
    #[serde(default)]
    pub window_start_ms: u64,
    /// Whether the UI has already raised the 80% warning for this window.
    ///
    /// Persisted so the warning fires **once per window**, not once per launch.
    /// A flag rather than a timestamp: the student does not need to be reminded
    /// daily, and a reminder that repeats is one they learn to dismiss.
    #[serde(default)]
    pub warned: bool,
}

/// What the caller should show and do, derived from [`Usage`] and an allowance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowanceState {
    /// No allowance applies — a paying tier, or a hub that predates the field.
    /// Never throttle on this.
    Unlimited,
    /// Under the allowance.
    Within { used_bytes: u64, allowance_bytes: u64 },
    /// Past the warning line but still under the allowance.
    Warning { used_bytes: u64, allowance_bytes: u64 },
    /// The allowance is spent; the connection should carry the throttle.
    Throttled { used_bytes: u64, allowance_bytes: u64 },
}

impl AllowanceState {
    /// Whether the connection should be carrying the free-tier throttle.
    #[must_use]
    pub const fn is_throttled(&self) -> bool {
        matches!(self, Self::Throttled { .. })
    }

    /// Whether the UI should raise the one-time warning.
    #[must_use]
    pub const fn is_warning(&self) -> bool {
        matches!(self, Self::Warning { .. })
    }
}

impl Usage {
    /// Advanced `now_ms`, rolling the window over if 30 days have passed since
    /// its start.
    ///
    /// **Rollover discards the count rather than subtracting a window.** A
    /// student who reappears after three months gets a fresh allowance, not
    /// three months of accumulated debt — the latter would punish exactly the
    /// casual user the free tier exists to keep installed, and there is no way
    /// to spend unused allowance anyway.
    #[must_use]
    pub fn window(&self, now_ms: u64) -> Self {
        if self.window_start_ms == 0 {
            // First ever reading: start the window here, keep the count at zero.
            // Setting the count to zero matters — a config carrying a stale
            // count but no window start is the migrated-config case, and
            // honouring that count would spend an allowance the student never
            // had a window for.
            return Self {
                used_bytes: 0,
                window_start_ms: now_ms,
                warned: false,
            };
        }
        // A start in the FUTURE must be corrected before the subtraction below,
        // or `now - start` underflows: in debug that panics, in release it wraps
        // to an enormous value that reads as "the window just rolled" on every
        // call — the quota silently ceases to exist. Resetting to `now` is the
        // safe repair; the count is discarded because it belongs to a window
        // whose bounds we cannot trust.
        if self.window_start_is_skewed(now_ms) {
            return Self {
                used_bytes: 0,
                window_start_ms: now_ms,
                warned: false,
            };
        }
        if now_ms.saturating_sub(self.window_start_ms) >= WINDOW_MS {
            return Self {
                used_bytes: 0,
                // Start the new window at now, not at `start + WINDOW_MS`:
                // a student who was away for two windows gets a full month from
                // today, which is what "resets every 30 days" means to them.
                window_start_ms: now_ms,
                warned: false,
            };
        }
        self.clone()
    }

    /// Records `delta` bytes moved, advancing the window first.
    ///
    /// `delta` saturates on overflow rather than wrapping. A wrap would be a
    /// free-allowance reset, and wrapping at `u64` is close enough to
    /// unreachable that a bug there is not worth a panic at runtime.
    #[must_use]
    pub fn record(&self, now_ms: u64, delta: u64) -> Self {
        let mut next = self.window(now_ms);
        next.used_bytes = next.used_bytes.saturating_add(delta);
        next
    }

    /// Marks the warning as raised, so it does not repeat this window.
    #[must_use]
    pub fn mark_warned(&self) -> Self {
        Self {
            warned: true,
            ..self.clone()
        }
    }

    /// Classifies this usage against an allowance.
    ///
    /// `allowance` is `None` for a paying tier or a hub that does not send one,
    /// and **that never throttles** — see [`AllowanceState::Unlimited`]. The
    /// check is `>=` at the allowance and `>=` at the warning line: landing
    /// exactly on 5 GiB has spent it.
    #[must_use]
    pub fn classify(&self, allowance: Option<u64>, now_ms: u64) -> AllowanceState {
        let Some(allowance) = allowance.filter(|a| *a > 0) else {
            return AllowanceState::Unlimited;
        };
        let used = self.window(now_ms).used_bytes;
        if used >= allowance {
            return AllowanceState::Throttled {
                used_bytes: used,
                allowance_bytes: allowance,
            };
        }
        // Integer maths on a float comparison: `used * 10 >= allowance * 8` is
        // exactly `used / allowance >= 0.8` without a float, so the boundary
        // cannot drift with rounding. Saturating, because a free tier's
        // allowance times ten still fits and this must never panic.
        if used.saturating_mul(10) >= allowance.saturating_mul(8) {
            return AllowanceState::Warning {
                used_bytes: used,
                allowance_bytes: allowance,
            };
        }
        AllowanceState::Within {
            used_bytes: used,
            allowance_bytes: allowance,
        }
    }

    /// The percentage of the allowance used, clamped to `0..=100`.
    ///
    /// For display only. Clamped because a throttled user's bar should sit at
    /// "full", not overflow the widget the way an unclamped count would.
    #[must_use]
    pub fn percent_used(&self, allowance: u64, now_ms: u64) -> u8 {
        if allowance == 0 {
            return 0;
        }
        let used = self.window(now_ms).used_bytes;
        let pct = used.saturating_mul(100) / allowance;
        u8::try_from(pct.min(100)).unwrap_or(100)
    }
}

/// The largest allowance the client will believe, in mebibytes.
///
/// **A pre-emptive guard, not a limit on the product.** The allowance arrives on
/// the wire from the hub, and a number is the one thing a corrupt row or a
/// hand-edited `update_config` can produce absurdly without anything erroring.
/// The failure mode this prevents is subtle and student-facing: at `u64::MAX`
/// mebibytes the byte conversion *saturates*, so `used * 10` overflows and `>=`
/// misreads every window as spent — the student is throttled instantly and
/// permanently, on a hub that believes it sent them an enormous allowance.
///
/// 1 PiB is far above any real plan (at the free tier's 1 Mbps it is ~280,000
/// years of continuous transfer) and far below the point where the arithmetic is
/// unsafe. A value above it is clamped rather than refused, so an absurd number
/// degrades to "effectively unlimited" instead of to "throttled".
pub const MAX_SANE_ALLOWANCE_MB: u64 = 1024 * 1024 * 1024; // 1 PiB

/// One mebibyte, named once so the conversion cannot drift between call sites.
pub const MIB: u64 = 1024 * 1024;

/// How far in the future a window start may sit before it is treated as skew.
///
/// The window compares `now - start`, and a **future** start makes that
/// subtraction underflow — in debug it panics, in release it wraps to an
/// enormous value that reads as "the window just rolled" on every single call.
/// The student would then get a fresh allowance on every poll: the quota
/// silently ceases to exist.
///
/// A start a little ahead of now is ordinary (an NTP step, a timestamp written a
/// second early), so this allows five minutes and treats anything further ahead
/// as a clock problem to be corrected.
pub const MAX_WINDOW_SKEW_MS: u64 = 5 * 60 * 1000;

impl Usage {
    /// Whether the stored window start is implausibly far in the future.
    ///
    /// When true the caller should reset the window rather than trust the
    /// subtraction. Exposed so the runtime can log a sentence about *why* the
    /// counter restarted, instead of a student's allowance silently reappearing.
    #[must_use]
    pub const fn window_start_is_skewed(&self, now_ms: u64) -> bool {
        self.window_start_ms > now_ms.saturating_add(MAX_WINDOW_SKEW_MS)
    }
}

/// Clamps an allowance from the hub to a figure the arithmetic can trust.
///
/// See [`MAX_SANE_ALLOWANCE_MB`] for why an absurd figure is clamped rather than
/// honoured or rejected: clamping degrades to "unlimited", which is the safe
/// direction for the student, while rejecting would be indistinguishable from
/// the hub deliberately switching the quota off.
#[must_use]
pub const fn sane_allowance_mb(raw: u64) -> u64 {
    if raw > MAX_SANE_ALLOWANCE_MB {
        MAX_SANE_ALLOWANCE_MB
    } else {
        raw
    }
}

// `WARN_FRACTION` is the documented value the integer comparison above encodes.
// This pins the two together: if someone edits the constant without editing the
// comparison, the test below fails rather than the docs drifting.
#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;
    const ALLOWANCE: u64 = 5120 * MB; // 5 GiB
    const T0: u64 = 1_700_000_000_000;

    fn used(bytes: u64) -> Usage {
        Usage {
            used_bytes: bytes,
            window_start_ms: T0,
            warned: false,
        }
    }

    #[test]
    fn warn_fraction_matches_the_integer_comparison() {
        // The comparison in `classify` is `used*10 >= allowance*8`, i.e. 80%.
        let line = (ALLOWANCE as f64 * WARN_FRACTION) as u64;
        let just_under = used(line - 1).classify(Some(ALLOWANCE), T0);
        let at_line = used(line).classify(Some(ALLOWANCE), T0);
        assert!(
            matches!(just_under, AllowanceState::Within { .. }),
            "one byte under the warning line must not warn"
        );
        assert!(
            matches!(at_line, AllowanceState::Warning { .. }),
            "the warning line itself must warn"
        );
    }

    #[test]
    fn no_allowance_never_throttles() {
        // A paying tier, or a hub that predates the field. The whole point of
        // `None` meaning unlimited is that it cannot gate a paying user.
        let huge = used(500 * 1024 * MB);
        assert_eq!(huge.classify(None, T0), AllowanceState::Unlimited);
        // An explicit zero is the operator switching the quota off.
        assert_eq!(huge.classify(Some(0), T0), AllowanceState::Unlimited);
    }

    #[test]
    fn exactly_at_the_allowance_is_throttled() {
        assert!(used(ALLOWANCE).classify(Some(ALLOWANCE), T0).is_throttled());
        assert!(!used(ALLOWANCE - 1)
            .classify(Some(ALLOWANCE), T0)
            .is_throttled());
    }

    #[test]
    fn a_full_window_rolls_over_to_a_fresh_allowance() {
        let spent = used(ALLOWANCE);
        let rolled = spent.window(T0 + WINDOW_MS);
        assert_eq!(rolled.used_bytes, 0, "a new window starts empty");
        assert_eq!(rolled.window_start_ms, T0 + WINDOW_MS);
        assert!(!rolled.warned, "the warning must be re-armed for the new window");
        assert!(!rolled.classify(Some(ALLOWANCE), T0 + WINDOW_MS).is_throttled());
    }

    #[test]
    fn rollover_is_from_the_new_reading_not_the_old_start() {
        // A student away for three windows gets a full month from today, not a
        // window that expired two months ago.
        let stale = used(ALLOWANCE);
        let long_after = T0 + WINDOW_MS * 4;
        let rolled = stale.window(long_after);
        assert_eq!(rolled.window_start_ms, long_after);
    }

    #[test]
    fn one_byte_under_a_window_does_not_roll() {
        let spent = used(ALLOWANCE);
        let held = spent.window(T0 + WINDOW_MS - 1);
        assert_eq!(held.used_bytes, ALLOWANCE, "the count must survive");
        assert!(held.classify(Some(ALLOWANCE), T0 + WINDOW_MS - 1).is_throttled());
    }

    #[test]
    fn a_config_with_no_window_starts_one_without_counting_old_bytes() {
        // The migrated-config case: a count with no window start must not be
        // honoured, or the student spends an allowance they never had.
        let migrated = Usage {
            used_bytes: 999 * MB,
            window_start_ms: 0,
            warned: false,
        };
        let started = migrated.window(T0);
        assert_eq!(started.used_bytes, 0);
        assert_eq!(started.window_start_ms, T0);
    }

    #[test]
    fn recording_advances_the_window_before_adding() {
        // A byte moved in a new window lands in the new window, not on last
        // month's total.
        let spent = used(ALLOWANCE);
        let next = spent.record(T0 + WINDOW_MS, 5 * MB);
        assert_eq!(next.used_bytes, 5 * MB);
        assert_eq!(next.window_start_ms, T0 + WINDOW_MS);
    }

    #[test]
    fn recording_saturates_rather_than_wrapping() {
        // A wrap would silently reset the allowance to near-zero usage.
        let almost_full = used(u64::MAX - 1);
        let next = almost_full.record(T0, 1024);
        assert_eq!(next.used_bytes, u64::MAX);
        assert!(next.classify(Some(ALLOWANCE), T0).is_throttled());
    }

    #[test]
    fn the_warning_fires_and_can_be_marked_so_it_does_not_repeat() {
        let at_line = used(ALLOWANCE * 8 / 10);
        assert!(at_line.classify(Some(ALLOWANCE), T0).is_warning());
        assert!(!at_line.warned);
        assert!(at_line.mark_warned().warned);
        // Marking it does not change the classification — the banner still
        // shows; only the one-time popup is suppressed.
        assert!(at_line.mark_warned().classify(Some(ALLOWANCE), T0).is_warning());
    }

    #[test]
    fn percent_is_clamped_and_zero_allowance_is_not_a_divide_by_zero() {
        assert_eq!(used(0).percent_used(ALLOWANCE, T0), 0);
        assert_eq!(used(ALLOWANCE / 2).percent_used(ALLOWANCE, T0), 50);
        assert_eq!(used(ALLOWANCE * 3).percent_used(ALLOWANCE, T0), 100);
        assert_eq!(used(ALLOWANCE).percent_used(0, T0), 0);
    }

    /// A window start in the future must never underflow the elapsed-time maths.
    ///
    /// Without the skew repair, `now - start` wraps in release to an enormous
    /// value, which reads as "the window just rolled" on every call — the student
    /// gets a fresh allowance on every poll and the quota silently stops
    /// existing. The repair resets to `now` and clears the count.
    #[test]
    fn a_window_start_in_the_future_is_repaired_not_underflowed() {
        let skewed = Usage {
            used_bytes: 7 * MB,
            // An hour ahead: past the five-minute tolerance.
            window_start_ms: T0 + 60 * 60 * 1000,
            warned: true,
        };
        assert!(
            skewed.window_start_is_skewed(T0),
            "an hour ahead must read as skew"
        );

        let repaired = skewed.window(T0);
        assert_eq!(repaired.window_start_ms, T0, "the window restarts at now");
        assert_eq!(repaired.used_bytes, 0, "a window we cannot bound is not counted");
        assert!(!repaired.warned, "and the warning is re-armed");
    }

    /// A start a little ahead of now is ordinary clock noise, not skew.
    ///
    /// An NTP step or a timestamp written a second early must not reset a
    /// student's counter — the tolerance exists so the repair fires only on a
    /// real clock problem.
    #[test]
    fn a_slightly_ahead_window_start_is_tolerated() {
        let slightly_ahead = Usage {
            used_bytes: 7 * MB,
            window_start_ms: T0 + 30_000, // 30s, inside the tolerance
            warned: false,
        };
        assert!(!slightly_ahead.window_start_is_skewed(T0));
        let held = slightly_ahead.window(T0);
        assert_eq!(
            held.used_bytes,
            7 * MB,
            "a tolerated skew must not discard the count"
        );
    }

    /// An absurd allowance is clamped to a figure the arithmetic can trust.
    ///
    /// `u64::MAX` mebibytes saturates the byte conversion, and the saturated
    /// value then overflows `used * 10` in `classify` — so `>=` misreads every
    /// window as spent and throttles the student instantly and permanently. The
    /// clamp degrades toward "unlimited", which is the safe direction.
    #[test]
    fn an_absurd_allowance_is_clamped_toward_unlimited() {
        assert_eq!(sane_allowance_mb(u64::MAX), MAX_SANE_ALLOWANCE_MB);
        assert_eq!(sane_allowance_mb(MAX_SANE_ALLOWANCE_MB + 1), MAX_SANE_ALLOWANCE_MB);
        assert_eq!(sane_allowance_mb(5120), 5120, "a real value is untouched");
        assert_eq!(sane_allowance_mb(0), 0, "zero still means off");
    }

    /// The clamped allowance must not misclassify a fresh window as throttled.
    ///
    /// This is the end-to-end property the clamp exists for: at the raw absurd
    /// value the student is throttled, and after clamping they are not.
    #[test]
    fn a_clamped_absurd_allowance_does_not_throttle_a_fresh_window() {
        let fresh = used(0);
        let clamped = sane_allowance_mb(u64::MAX).saturating_mul(MB);
        assert_eq!(
            fresh.classify(Some(clamped), T0),
            AllowanceState::Within {
                used_bytes: 0,
                allowance_bytes: clamped,
            },
            "an absurd-but-clamped allowance must read as within, not throttled"
        );
    }
}
