//! The free tier's meter — the piece that turns the Core's lifetime byte totals
//! into a stored allowance window.
//!
//! # Why this module exists at all
//!
//! [`crate::chrysalis::usage`] was written, tested and documented as "BUILT" —
//! and the counter never moved, because [`Usage::record`] had **no caller**. The
//! pure module answered every question correctly about a number nobody ever
//! produced, and the usage bar read a permanent zero. This is the missing half:
//! the thing that consumes traffic and writes the window.
//!
//! The lesson is recorded in `check-consistency.sh` §23 as an assertion, because
//! a module with perfect tests and no caller is a failure mode that tests cannot
//! see. Testing `record` proves `record` works; only a guard on the *wiring*
//! can tell you the meter is connected.
//!
//! # What it does and does not do
//!
//! It does not decide what throttling means, or whether it applies — that stays
//! in `usage` and in the caller. It does not read a clock: `observe` is a pure
//! fold over totals, and the only time-dependent decision (`flush_due`) takes
//! `now_ms` from its caller.
//! It holds a baseline and an accumulator, and it flushes to disk on an interval
//! rather than per sample (the Core pushes 1 Hz, and `save_verge` rewrites YAML —
//! a 1 Hz write would be a disk-thrash bug that looks like working code).
//!
//! # Counting is deliberately conditional
//!
//! Two conditions, both of which must hold, and both of which were decided rather
//! than inherited:
//!
//! * **A free tier with an allowance.** A paying tier has no allowance; counting
//!   for it would build a number the UI must suppress, and a later downgrade
//!   would open with a spent meter the student never spent.
//! * **A tunnel that is actually carrying the student's traffic.** The Core's
//!   totals include the readiness and egress probes, and counting those would
//!   make "0 GB used" false on a device that had carried nothing. The caller
//!   passes this in; the meter never guesses it.
//!
//! When either condition is false the meter still **rebaselines** — the baseline
//! is advanced without billing — so the bytes moved while uncounted are never
//! billed later by a stale baseline. That distinction is the whole reason
//! [`crate::chrysalis::usage::billable_delta`] returns `Option<u64>` and not
//! `u64`.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;

use crate::chrysalis::usage::{Usage, billable_delta};

/// How many bytes may accumulate before the window is flushed to disk.
///
/// Sized so a metered 1 Mbps free tunnel flushes about once every 80 seconds
/// (1 Mbps ≈ 125 KB/s), which keeps YAML writes rare without risking much loss
/// on a crash. A student who closes the app mid-month loses at most this, and
/// the loss is in the student's favour.
pub const FLUSH_EVERY_BYTES: u64 = 10 * 1024 * 1024;

/// How often to force a flush regardless of bytes moved.
///
/// A slow tunnel (a few KB over a whole session) would otherwise never reach
/// [`FLUSH_EVERY_BYTES`] and the counter would live only in memory. One minute is
/// short enough that a crash costs little and long enough not to thrash the file.
pub const FLUSH_EVERY_MS: u64 = 60 * 1000;

/// The baseline and the unflushed accumulator.
///
/// A process-global singleton, like `core::manager::traffic_probe`'s sink, and
/// for the same reason: its writer is a background stream task, not a field of
/// any struct the app already threads around.
static METER: OnceLock<Meter> = OnceLock::new();

fn meter() -> &'static Meter {
    METER.get_or_init(Meter::default)
}

#[derive(Default)]
struct Meter {
    /// The last lifetime total this meter accepted, per direction. `None` until
    /// the first sample, so the first sample establishes a baseline instead of
    /// billing everything the Core has moved since it started.
    baseline: Mutex<Option<(u64, u64)>>,
    /// Bytes counted but not yet written to the config.
    pending: AtomicU64,
    /// When the pending bytes were last flushed, in this meter's own millis.
    last_flush_ms: AtomicU64,
    /// The allowance in force, mirrored from the hub's latest word.
    ///
    /// `0` means "no allowance applies" — a paying tier, or the operator
    /// switching the quota off — and is the default, so a meter that has never
    /// been told will not bill anyone. Mirrored here rather than read from the
    /// config on each sample because the sample path is synchronous (1 Hz, off a
    /// websocket) and the config reader is async; the heartbeat is the only
    /// thing that changes it, and it calls [`set_allowance_mb`].
    allowance_mb: AtomicU64,
}

/// Why a sample was not billed, for the log line and for the tests.
///
/// Named rather than a bare bool because "we counted nothing" has several
/// distinct causes and only one of them is a problem: an uncounted sample while
/// a paying tier is active is correct, and one while a free tunnel is Ready is
/// a defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Bytes were billed.
    Billed(u64),
    /// No baseline existed yet; this sample only established one.
    Baselined,
    /// The total went backwards (a Core restart) or did not move. The baseline
    /// was advanced and nothing was billed.
    Rebaselined,
    /// The meter was told not to count — a paying tier, or no tunnel carrying
    /// traffic. The baseline was advanced anyway, so nothing accumulates debt.
    NotCounting,
}

/// Folds one `/traffic` sample into the stored window.
///
/// `billable` is the caller's decision — free tier with an allowance, and a
/// tunnel actually carrying the student's traffic. The meter never re-derives
/// it, because those are facts about the subscription and the tunnel that this
/// module has no business guessing.
///
/// Returns what it did, so the caller can log a sentence on the interesting
/// cases (`Rebaselined` is how a Core restart becomes visible instead of silent).
pub fn observe(totals_up: u64, totals_down: u64, billable: bool) -> Disposition {
    meter().observe(totals_up, totals_down, billable)
}

impl Meter {
    fn observe(&self, up_total: u64, down_total: u64, billable: bool) -> Disposition {
        // The baseline is read and written under ONE lock scope, and the lock is
        // released before the `pending` atomic below. Clippy's
        // `significant_drop_tightening` is the reason for the explicit scope, and
        // the reason is real: holding it across the arithmetic would serialise
        // every sample against every other reader of a value that is a single
        // tuple.
        let delta = {
            let mut baseline = self.baseline.lock();
            let previous = *baseline;

            // Uncounted samples STILL move the baseline. This is the half that is
            // easy to get wrong: if a paying tier or a silent tunnel left the
            // baseline behind, the first sample after the tunnel became Ready
            // would bill every byte the Core had moved meanwhile — a month of data
            // charged to a second.
            if !billable {
                *baseline = Some((up_total, down_total));
                return Disposition::NotCounting;
            }

            let Some((prev_up, prev_down)) = previous else {
                // First billable sample: establish the baseline, bill nothing. The
                // Core has been up and moving bytes (probes, the hub call that got
                // us here) and none of that is the student's allowance.
                *baseline = Some((up_total, down_total));
                return Disposition::Baselined;
            };

            // `billable_delta` is the ONE implementation of the delta rules. It is
            // given `u64::MAX` as the allowance so its "step larger than the
            // allowance" guard cannot fire here: this function does not know the
            // real allowance, and the caller re-checks the total when it
            // classifies usage.
            //
            // The two directions are evaluated SEPARATELY because a one-way
            // transfer legitimately leaves one of them flat. But the Core's totals
            // reset **together** on a restart, so a decrease in EITHER direction
            // means the pair is suspect and both are rebaselined as a unit — which
            // is what stops the surviving direction from billing the whole gap one
            // sample later.
            //
            // There is deliberately no second decrease check here. An earlier
            // draft had one, and it made the restart test PASS while
            // `billable_delta` wrapped — a guard that could not fail. The rule lives
            // in one place, and the test is what proves it is wired in.
            let up = billable_delta(prev_up, up_total, u64::MAX);
            let down = billable_delta(prev_down, down_total, u64::MAX);

            // `None` from either direction means "not billable and the baseline
            // must be advanced" — a decrease (Core restart) or no movement. Advance
            // the whole pair, because a restart resets both.
            *baseline = Some((up_total, down_total));
            let delta = up.unwrap_or(0).saturating_add(down.unwrap_or(0));
            // Explicit, so the lock is released at the end of the arithmetic rather
            // than at the end of the enclosing statement — clippy's
            // `significant_drop_tightening`, and it is right: the `pending` atomic
            // below has no reason to be inside the critical section.
            drop(baseline);
            delta
        };

        if delta == 0 {
            return Disposition::Rebaselined;
        }

        self.pending.fetch_add(delta, Ordering::AcqRel);
        Disposition::Billed(delta)
    }

    fn since_flush(&self, now_ms: u64) -> u64 {
        let last = self.last_flush_ms.load(Ordering::Relaxed);
        if last == 0 {
            // Never flushed. Returning `now_ms - 0` would make `flush_due` true on
            // the very first sample and every poll until the first flush landed —
            // a config write per second against a window that has nothing in it.
            // The timer starts at the first flush attempt, not at the epoch.
            return 0;
        }
        now_ms.saturating_sub(last)
    }

    fn mark_flushed(&self, now_ms: u64) {
        self.last_flush_ms.store(now_ms, Ordering::Relaxed);
    }
}

/// Whether the accumulated bytes are due to be written to the config.
///
/// Exposed so the caller decides *when to await* rather than the meter doing I/O
/// from a background task on its own schedule — the flush must be able to be
/// awaited where the app already awaits config writes.
#[must_use]
pub fn flush_due(now_ms: u64) -> bool {
    let m = meter();
    m.pending.load(Ordering::Acquire) >= FLUSH_EVERY_BYTES || m.since_flush(now_ms) >= FLUSH_EVERY_MS
}

/// Serialises `flush`, process-wide.
///
/// Two concurrent flushes would each read the accumulator and then do a
/// read-modify-write of the same stored window: the second `save_verge` overwrites
/// the first's addition, and one chunk of the student's usage vanishes. That is
/// silent under-counting — the mirror image of the defect this module exists to
/// fix — so the whole read-add-write is held under one lock.
///
/// A **tokio** mutex, deliberately, not the `parking_lot` one the baseline uses:
/// this guard is held across `store::usage().await` and `store_usage(..).await`,
/// and holding a blocking mutex across an await point is unsound (`Send` futures
/// may resume on another thread). The baseline mutex above is only ever held for
/// a tuple copy, so it stays a synchronous lock.
static FLUSH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Debits from the accumulator exactly the bytes a flush wrote, and returns what
/// remains.
///
/// # Why this is a function and not an inline subtraction
///
/// This is where the original defect lived, in both directions:
///
/// * An earlier draft cleared the accumulator with `swap(0)` **before** awaiting
///   the store. A failed write then lost those bytes permanently — silent
///   under-counting, the mirror image of the reported bug.
/// * The obvious repair, `store(0)` after a successful write, erases a sample
///   that landed while the write was in flight, because `observe` had already
///   added it to the accumulator.
///
/// So the debit is `fetch_sub(written)`: it removes what was written and nothing
/// else. It is a free function taking the accumulator so a test can drive the
/// real arithmetic with its own `AtomicU64` — an earlier test re-implemented the
/// subtraction beside the code, passed against `store(0)`, and proved nothing.
fn debit_written(pending: &AtomicU64, written: u64) -> u64 {
    // Saturating at zero: with the flush lock held nothing can legitimately make
    // this underflow, but an underflow would wrap to ~1.8e19 and read as a
    // colossal allowance, so the impossible case is made unrepresentable.
    pending.fetch_sub(written, Ordering::AcqRel).saturating_sub(written)
}

/// Writes the accumulated bytes into the stored window and returns them.
///
/// The window is read, advanced by `record` (which also applies the rollover and
/// the future-dated-window repair), and written back — so the persisted shape and
/// the pure module's rules stay one implementation. Returns the number of bytes
/// that were folded in, which is `0` when nothing was pending.
///
/// # The accumulator is not cleared until the write succeeds
///
/// An earlier draft took the pending bytes with `swap(0)` *before* awaiting the
/// store, so a failed write lost them permanently and the allowance silently
/// under-counted. The bytes are now left in place and only debited after a
/// successful save — an over-count that self-corrects is far better than an
/// under-count nobody can see.
///
/// A failed write therefore retries: immediately once the accumulator reaches
/// [`FLUSH_EVERY_BYTES`], and otherwise on the next [`FLUSH_EVERY_MS`] tick,
/// because `mark_flushed` has already moved the timer on. Both paths keep the
/// bytes, which is the property that matters.
pub async fn flush(now_ms: u64) -> anyhow::Result<u64> {
    let _serialise = FLUSH_LOCK.lock().await;
    let m = meter();

    // Peek, do not take. See the doc comment above.
    let pending = m.pending.load(Ordering::Acquire);
    // Mark the attempt even on the zero path: the time bound is this timer's
    // purpose, and a meter that never reaches the byte threshold would otherwise
    // report "due" on every poll and write nothing forever.
    m.mark_flushed(now_ms);
    if pending == 0 {
        return Ok(0);
    }

    let stored = crate::chrysalis::store::usage().await;
    let next: Usage = stored.record(now_ms, pending);
    crate::chrysalis::store::store_usage(&next).await?;

    // Only now, and only what was actually written. See `debit_written` for why
    // this is a subtraction and not a store of zero.
    debit_written(&m.pending, pending);
    Ok(pending)
}

/// Forgets the baseline because the Core is gone.
///
/// Called when the Core starts or stops, for the same reason
/// `traffic_probe::clear` exists: these totals belong to a process that has
/// exited, and subtracting from them across a restart is exactly the wrap this
/// module is built to avoid. The **stored window is untouched** — the student's
/// consumed allowance is a fact about the month, not about this Core.
pub fn core_changed() {
    *meter().baseline.lock() = None;
}

/// Mirrors the hub's latest allowance into the meter.
///
/// `None` — a paying tier, or the quota switched off — is stored as `0`, which
/// [`observe`] reads as "do not bill". Called from the heartbeat that already
/// persists the allowance, so the two cannot disagree: the value the UI meters
/// against and the value the counter bills against are the same one.
///
/// Deliberately does **not** reset the stored window. A student who moves from
/// free to paying keeps their history, and a later return to free continues it
/// rather than appearing to have spent nothing.
pub fn set_allowance_mb(allowance_mb: Option<u64>) {
    meter().allowance_mb.store(allowance_mb.unwrap_or(0), Ordering::Release);
}

/// Whether a free-tier allowance is in force, which is the meter's own half of
/// the "should I bill this sample" question.
#[must_use]
pub fn has_allowance() -> bool {
    meter().allowance_mb.load(Ordering::Acquire) > 0
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Each test needs its own meter; the process-global one is shared state and
    /// would make these order-dependent. Built directly rather than through
    /// `meter()`.
    fn fresh() -> Meter {
        Meter::default()
    }

    const T0: u64 = 1_700_000_000_000;

    /// The bug this module exists to prevent, end to end: a Core restart must not
    /// bill the wrap.
    #[test]
    fn a_core_restart_bills_nothing() {
        let m = fresh();
        m.observe(0, 0, true);
        assert_eq!(m.observe(1000, 2000, true), Disposition::Billed(3000));
        // The Core restarts: both totals back to near zero.
        assert_eq!(m.observe(0, 0, true), Disposition::Rebaselined);
        // And the next sample bills normally from the new baseline — it does not
        // bill the entire gap since the old one.
        assert_eq!(m.observe(10, 20, true), Disposition::Billed(30));
    }

    /// A restart that lands on a NON-ZERO total still bills nothing.
    ///
    /// This is the case that proves the guard is real, and the one an earlier
    /// draft of this test missed: when both totals fall to exactly zero,
    /// `unwrap_or(0)` hides a wrapping subtraction and the test passes against a
    /// broken `billable_delta`. Here the upstream total drops from 1000 to 500 —
    /// backwards, but still a plausible-looking number — so a `wrapping_sub`
    /// bills ~1.8e19 bytes and marks the whole month spent on one sample.
    #[test]
    fn a_restart_landing_on_a_nonzero_total_does_not_bill_the_wrap() {
        let m = fresh();
        m.observe(1000, 1000, true);
        let after = m.observe(500, 900, true);
        assert_eq!(
            after,
            Disposition::Rebaselined,
            "a total going backwards must rebaseline, never bill (a wrap would bill ~1.8e19 B)"
        );
        assert_eq!(
            m.pending.load(Ordering::Acquire),
            0,
            "nothing may be pending after a restart"
        );
    }

    /// The first sample establishes a baseline and bills nothing.
    ///
    /// A free student's first `/traffic` sample arrives on a Core that has
    /// already moved bytes — the probes, the hub call, the handshake. None of it
    /// is their allowance, and billing it would open the meter at a non-zero
    /// value the student did not cause.
    #[test]
    fn the_first_sample_only_baselines() {
        let m = fresh();
        assert_eq!(m.observe(5_000_000, 9_000_000, true), Disposition::Baselined);
        assert_eq!(m.pending.load(Ordering::Acquire), 0);
    }

    /// Uncounted samples advance the baseline, so returning to counting bills
    /// only what moved afterwards.
    ///
    /// This is the subtle one: leaving the baseline behind while a paying tier is
    /// active would mean the first free sample after a downgrade bills every byte
    /// the Core had ever moved.
    #[test]
    fn uncounted_samples_move_the_baseline_without_billing() {
        let m = fresh();
        m.observe(0, 0, true);
        assert_eq!(m.observe(1_000_000, 1_000_000, false), Disposition::NotCounting);
        // Counting resumes: the 2 MB moved while not counting is NOT billed.
        assert_eq!(m.observe(1_000_100, 1_000_100, true), Disposition::Billed(200));
    }

    /// A flat direction alongside a moving one still bills the moving one.
    #[test]
    fn a_one_way_transfer_bills_the_moving_direction() {
        let m = fresh();
        m.observe(100, 100, true);
        assert_eq!(m.observe(100, 5000, true), Disposition::Billed(4900));
        assert_eq!(m.observe(4000, 5000, true), Disposition::Billed(3900));
    }

    /// Backwards on one direction only is still a restart, and bills nothing.
    #[test]
    fn a_single_direction_going_backwards_bills_nothing() {
        let m = fresh();
        m.observe(1000, 1000, true);
        assert_eq!(m.observe(1000, 10, true), Disposition::Rebaselined);
    }

    /// A sample that lands mid-flush must survive the flush's subtraction.
    ///
    /// `flush` clears the accumulator with `fetch_sub(pending)` rather than a
    /// store of zero precisely so this holds: a sample counted while the write was
    /// in flight has already been added to `pending` and would be erased by a
    /// store.
    #[test]
    fn a_sample_landing_during_a_flush_is_not_erased() {
        let m = fresh();
        m.observe(0, 0, true);
        m.observe(1000, 0, true);
        let written = m.pending.load(Ordering::Acquire);
        assert_eq!(written, 1000);
        // A sample arrives while the write is in flight.
        m.observe(1000, 500, true);
        // The flush's real debit runs against the accumulator.
        let remaining = debit_written(&m.pending, written);
        assert_eq!(
            remaining, 500,
            "the in-flight sample must survive the flush's subtraction"
        );
        assert_eq!(
            m.pending.load(Ordering::Acquire),
            500,
            "the accumulator must still hold the in-flight bytes, not zero"
        );
    }

    /// A fresh meter is NOT due to flush, even at a wall-clock time far from
    /// zero.
    ///
    /// This is the bug a one-line `now - last_flush` had: `last_flush` starts at
    /// `0`, so the elapsed time was the whole Unix epoch — always over the
    /// interval — and the first sample would have triggered a config write on
    /// every poll until something flushed. The timer starts at the first flush
    /// attempt, not at the epoch.
    #[test]
    fn a_fresh_meter_is_not_due_to_flush() {
        let m = fresh();
        assert_eq!(m.pending.load(Ordering::Acquire), 0);
        assert_eq!(m.since_flush(T0), 0, "a never-flushed meter has no elapsed time");
    }

    /// `flush_due` fires on the timer for a tunnel too slow to reach the byte
    /// threshold — otherwise a light user's counter would never reach the disk.
    #[test]
    fn flush_becomes_due_on_time_as_well_as_bytes() {
        let m = fresh();
        m.observe(0, 0, true);
        m.mark_flushed(T0);
        assert!(
            !m.since_flush(T0 + FLUSH_EVERY_MS - 1) >= FLUSH_EVERY_MS,
            "not yet due by time"
        );
        assert!(m.since_flush(T0 + FLUSH_EVERY_MS) >= FLUSH_EVERY_MS, "due by time");
    }
}
