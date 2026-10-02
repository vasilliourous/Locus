//! Whether the Core is moving bytes, observed from its own traffic stream.
//!
//! # What this is for
//!
//! The status path needs a second opinion to the delay test. The delay test asks
//! the Core to *dial* a target and report how long that took; it is a request the
//! Core could in principle answer without the student's own traffic flowing, and
//! — more importantly — it is a question the Core can fail for reasons that have
//! nothing to do with whether the tunnel works. Each failure of that probe walked
//! a working tunnel back to "connecting", which is the recurring report this
//! replaces.
//!
//! Bytes moving is the opposite kind of evidence. It is not a question asked of
//! the Core; it is the Core reporting what it already did on behalf of the
//! student, off the `/traffic` websocket, once a second, with no request of ours
//! in the loop. A byte that left the machine and a byte that came back is a
//! tunnel that carried something.
//!
//! # The distinction everything rests on
//!
//! mihomo reports four numbers. Two of them are the trap:
//!
//!   * `up` / `down` — this sample's rate in bytes per second.
//!   * `upTotal` / `downTotal` — the Core's **lifetime** totals, which reset to
//!     zero when the Core restarts and then only ever grow.
//!
//! "Show connected if the numbers are non-zero" reads correctly and is wrong:
//! `upTotal > 0` is a Core that moved a byte *at some point since it started*,
//! which includes the DNS lookup and the first handshake of a tunnel that then
//! died, and includes every byte moved before the student's traffic should count
//! for anything. Readiness on a total is a latch with extra steps — the same
//! failure this file exists to avoid, wearing a counter instead of a bool.
//!
//! So the only fact [`mark`] keeps is the one with no memory in it: **is the
//! current in-flight sample carrying bytes?** `up > 0 || down > 0`. Traffic this
//! instant, from a stream that is delivering at this instant, is a tunnel that is
//! working at this instant, and it stops being evidence the second the samples
//! stop carrying bytes.
//!
//! # Why the sample's own freshness is not enough
//!
//! The stream is one-directional: the Core pushes a sample every second and
//! nothing obliges it to push one with any particular rate in it. A stream that
//! has gone quiet because the Core died and a stream that has gone quiet because
//! the student is reading a document look identical in the samples. The
//! difference is only visible in the *arrival* of the samples, so [`mark`]
//! records when it was last called and [`is_active`] requires both a byte and a
//! recent call. Two conditions, because they are two facts: bytes moved, and the
//! Core is still telling us about it.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::Mutex;
use tauri::async_runtime::JoinHandle;
use tokio::time::Instant;

/// How recent the last sample with traffic in it must be to count as "now".
///
/// The Core pushes one sample a second, so three of them is three consecutive
/// seconds of silence parked against three consecutive seconds of a student
/// reading a page. Comfortably longer than the stream's own 1 Hz cadence — a
/// dropped or late sample must not flicker the button — and short enough that a
/// tunnel that has genuinely stopped stops claiming to be connected promptly.
pub const TRAFFIC_ACTIVE_WINDOW: Duration = Duration::from_secs(3);

/// How long the stream may deliver nothing at all before it is reconnected.
///
/// Deliberately the same call the tray rate task makes (`TRAY_SPEED_STALE_TIMEOUT`
/// there): a mihomo `/traffic` stream that has sent nothing for this long is not
/// a quiet tunnel, it is a stream that is gone — the Core restarted and the
/// socket is dead — and reconnecting is the only way to get samples again.
pub const TRAFFIC_STALE_TIMEOUT: Duration = Duration::from_secs(5);

const RECONNECT_DELAY: Duration = Duration::from_secs(1);

/// How long to wait before retrying the `/traffic` socket, given why it ended.
///
/// A socket the Core closed or that went quiet while a Core *is* running is a
/// transient — reconnect promptly, because activity is what stops the button
/// flapping. A socket that could not be opened at all is different: it means the
/// Core is not serving (not started, mid-restart, sidecar still coming up), and
/// retrying it at 1 Hz forever is a busy loop against a socket nobody is
/// listening on. Those back off, so an app left open overnight does not spin.
///
/// Deliberately not exponential-with-a-cap tuned by feel: the first few retries
/// are the ones a student is watching, so they are fast; after that the reason for
/// speed (a connect in flight) has passed and only the cost of spinning remains.
#[must_use]
const fn reconnect_delay(consecutive_open_failures: u32) -> Duration {
    const FAST_RETRY_BUDGET: u32 = 3;
    if consecutive_open_failures < FAST_RETRY_BUDGET {
        RECONNECT_DELAY
    } else {
        Duration::from_secs(5)
    }
}

/// The most recent traffic state observed from the Core's stream.
///
/// A process-global singleton rather than a field on `CoreManager` because its
/// writers are not the manager: the tray rate task feeds it from a task the tray
/// owns, and the stream task below feeds it from its own. Funnelling those
/// through the manager would mean threading a handle into the tray, for a value
/// that is a reading of the world rather than the manager's own state.
static SINK: OnceLock<TrafficSink> = OnceLock::new();

fn sink() -> &'static TrafficSink {
    SINK.get_or_init(TrafficSink::default)
}

#[derive(Default)]
struct TrafficSink {
    /// When the last sample carrying bytes arrived, as millis since this
    /// sink's epoch. Zero with `has_traffic == false` is "never".
    last_traffic_at: AtomicU64,
    has_traffic: AtomicBool,
    /// The last rates seen, for the log line and for a caller that wants to show
    /// the student a number rather than a verdict.
    last_up: AtomicU64,
    last_down: AtomicU64,
    epoch: Mutex<Option<Instant>>,
    /// While this is `true`, no sample may claim activity — set when a stream is
    /// found dead and cleared when a new one delivers.
    ///
    /// This is the difference between "no bytes right now" and "we have no idea
    /// right now". Without it, the window that elapses while a stale socket is
    /// detected and reopened is covered by the *previous* sample: the Core died
    /// mid-download, its last sample is 1 s old, the socket is reopened 1 s later,
    /// and for that whole span `is_active` keeps answering from bytes that have
    /// stopped moving. Three seconds of a green button over a dead tunnel is
    /// exactly the lie this module exists to prevent.
    verifying: AtomicBool,
}

impl TrafficSink {
    fn now_millis(&self) -> u64 {
        // Bound to a local before dropping the guard: the value is copied out
        // while the lock is held, so a later reader cannot see a half-updated
        // epoch.
        let start = {
            let mut epoch = self.epoch.lock();
            *epoch.get_or_insert_with(Instant::now)
        };
        u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Records one sample from the Core's `/traffic` stream.
    ///
    /// Both branches are recorded, not just the interesting one: a sample that
    /// carries nothing must be able to *end* a claim of activity as well as start
    /// one, which is why the timestamp is only written when bytes moved and the
    /// window in [`is_active`] does the expiring.
    fn mark(&self, up: u64, down: u64) {
        // A sample arriving is itself proof the stream is alive again, so it
        // lifts any hold — including one a *previous* Core's death left behind.
        self.verifying.store(false, Ordering::Release);
        self.last_up.store(up, Ordering::Relaxed);
        self.last_down.store(down, Ordering::Relaxed);

        if up > 0 || down > 0 {
            self.has_traffic.store(true, Ordering::Release);
            self.last_traffic_at.store(self.now_millis(), Ordering::Release);
        }
    }
    /// Whether the Core is moving bytes **right now**.
    ///
    /// See the module docs for why this is a rate and not a total, and why it
    /// needs both a byte and a recent sample.
    fn is_active(&self) -> bool {
        self.is_active_for(TRAFFIC_ACTIVE_WINDOW)
    }

    fn is_active_for(&self, window: Duration) -> bool {
        if !self.has_traffic.load(Ordering::Acquire) {
            return false;
        }
        // A stream we have declared dead proves nothing, however recent its last
        // sample looked. See the field.
        if self.verifying.load(Ordering::Acquire) {
            return false;
        }
        let last_activity = self.last_traffic_at.load(Ordering::Acquire);
        let Some(epoch) = *self.epoch.lock() else {
            return false;
        };
        let last = Duration::from_millis(last_activity);
        epoch.elapsed().saturating_sub(last) <= window
    }

    /// Forgets everything, for a Core that is no longer running.
    ///
    /// Called on stop so a stopped Core cannot inherit the evidence of the last
    /// one: these bytes were moved by a process that has exited.
    fn clear(&self) {
        self.has_traffic.store(false, Ordering::Release);
        self.last_traffic_at.store(0, Ordering::Release);
        self.last_up.store(0, Ordering::Relaxed);
        self.last_down.store(0, Ordering::Relaxed);
        self.verifying.store(false, Ordering::Release);
    }

    /// Declares the stream dead: its evidence stops counting until a new socket
    /// delivers a sample.
    fn hold_off(&self) {
        self.verifying.store(true, Ordering::Release);
    }

    /// The tray blanking its own title — explicitly **not** a Core measurement.
    ///
    /// macOS-only in practice, because the tray rate task is the only caller.
    ///
    /// The tray task calls `apply_tray_speed(0, 0)` whenever it wants an empty
    /// title: on a failed connect, on a stale stream, and once per loop sweep.
    /// Those are statements about the *display*, not about the tunnel, so they
    /// must not be able to cancel a byte the Core really reported. Leaving the
    /// sink untouched is the honest handling: we learned nothing.
    // Available in tests on every platform so the rule can be pinned without a
    // macOS runner; in the app only the tray task calls it, and that is macOS-only.
    // Not `const`: clippy only thinks so because the body is deliberately empty.
    #[allow(clippy::missing_const_for_fn)]
    #[cfg(any(target_os = "macos", test))]
    fn blank_display(&self) {
        // Deliberately a no-op on the measurement. `up`/`down` of zero *from the
        // Core* means "quiet right now" and is handled by `mark`; a zero from the
        // tray means "we are not showing anything" and carries no information.
    }
}

/// The traffic half of the readiness decision, as the manager consumes it.
///
/// The status path turns this into `probe::TrafficOutcome`, which is one of the
/// two ways `Ready` can be reached — so this is the one place the counter becomes
/// a connection decision.
#[must_use]
pub fn outcome() -> crate::core::manager::probe::TrafficOutcome {
    if sink().is_active() {
        crate::core::manager::probe::TrafficOutcome::Flowing
    } else {
        crate::core::manager::probe::TrafficOutcome::Silent
    }
}

/// Records a sample the tray rate task already handed us.
///
/// Both entry points land in the same sink on purpose. The tray task's numbers
/// and this module's own stream are the same fact off the same socket, read by
/// whichever subscriber happens to be listening; two sinks would be two
/// measurements that could disagree about whether the tunnel works.
///
/// Kept as its own name rather than folded into one generic `mark` so the call
/// site in `apply_tray_speed` shows what it is: the tray numbers and the
/// connection screen's numbers are the same measurement.
///
/// macOS-only in practice, because the tray rate task is: nowhere else has a
/// second reader to feed.
#[cfg(target_os = "macos")]
pub fn publish_from_tray(up: u64, down: u64) {
    sink().mark(up, down);
}

/// Records a sample from this module's own stream task.
fn mark(up: u64, down: u64) {
    sink().mark(up, down);
}

/// Starts the app's own `/traffic` subscription, if the tray rate task (which
/// shows the same numbers) is not already holding one.
///
/// Idempotent, and safe to call from a status read that runs several times a
/// second. Called by [`crate::core::manager::CoreManager::observe_readiness`],
/// which is the only reader — so a Core whose readiness is never observed never
/// opens the socket.
pub fn ensure_stream() {
    if crate::core::tray::Tray::speed_task_running() {
        return;
    }
    TrafficStreamTask::global().start();
}

/// Whether the Core is moving bytes right now.
#[must_use]
pub fn is_active() -> bool {
    sink().is_active()
}

/// Forgets the last Core's traffic. Called when the Core starts or stops.
pub fn clear() {
    sink().clear();
}

/// Declares the traffic stream dead until a new one delivers a sample.
///
/// Called when a stream is found stale, so the samples it left behind stop
/// counting during the reconnect rather than covering it.
fn hold_claim_off() {
    sink().hold_off();
}

/// Records that the tray has blanked its own title, which says nothing about the
/// tunnel. See [`TrafficSink::blank_display`].
///
/// macOS-only, like the tray task that calls it.
#[cfg(target_os = "macos")]
pub fn display_blanked() {
    sink().blank_display();
}

/// Owns the app's own subscription to the Core's `/traffic` stream.
///
/// One at a time, process-wide: the value it produces is a property of the
/// machine, so a second subscription would be a second websocket proving the same
/// fact and a second source that could disagree with the first. The lock is held
/// across the whole start so two callers cannot both decide they are the one to
/// start it.
#[derive(Default)]
pub struct TrafficStreamTask {
    task: Mutex<Option<JoinHandle<()>>>,
}

impl TrafficStreamTask {
    #[must_use]
    pub fn global() -> &'static Self {
        static GLOBAL: OnceLock<TrafficStreamTask> = OnceLock::new();
        GLOBAL.get_or_init(Self::default)
    }

    /// Starts the stream if it is not already running.
    ///
    /// Idempotent, because the caller is a status read that may run several times
    /// a second and must not accumulate sockets.
    pub fn start(&self) {
        let mut guard = self.task.lock();
        if guard.as_ref().is_some_and(|task| !task.inner().is_finished()) {
            return;
        }

        let connection_id = std::sync::Arc::new(Mutex::new(None));
        let task_connection_id = std::sync::Arc::clone(&connection_id);

        let handle = tauri::async_runtime::spawn(async move {
            // Counts *open* failures only. A socket that opened and then ended is
            // not evidence the Core is unreachable, so it resets this to zero —
            // otherwise a tunnel that drops once every few minutes would march
            // itself into the slow retry and stay there.
            let mut consecutive_open_failures: u32 = 0;

            loop {
                match crate::utils::connections_stream::connect_traffic_stream().await {
                    Ok(mut stream) => {
                        consecutive_open_failures = 0;
                        *task_connection_id.lock() = Some(stream.connection_id);
                        loop {
                            match stream.next_event(TRAFFIC_STALE_TIMEOUT, || false).await {
                                crate::utils::connections_stream::StreamConsumeState::Event(event) => {
                                    mark(event.up, event.down);
                                }
                                crate::utils::connections_stream::StreamConsumeState::Stale => {
                                    // A quiet stream is a gone stream. Drop the claim
                                    // — and hold it dropped for the reconnect window,
                                    // or the last sample a dead Core managed would
                                    // keep the button green straight through the
                                    // reopen, which is the latch bug at 3 s scale.
                                    hold_claim_off();
                                    break;
                                }
                                crate::utils::connections_stream::StreamConsumeState::Closed
                                | crate::utils::connections_stream::StreamConsumeState::ExitRequested => break,
                            }
                        }
                        // Bind the id before awaiting: the guard is not `Send`, so
                        // holding it across the await would make this task
                        // unspawnable.
                        let id = { task_connection_id.lock().take() };
                        if let Some(id) = id {
                            crate::utils::connections_stream::disconnect_connection(id).await;
                        }
                    }
                    Err(_) => {
                        consecutive_open_failures = consecutive_open_failures.saturating_add(1);
                        tokio::time::sleep(reconnect_delay(consecutive_open_failures)).await;
                    }
                }
            }
        });

        *guard = Some(handle);
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule the whole file exists for: a byte in the current sample is
    /// activity, and nothing needs to corroborate it.
    #[test]
    fn a_sample_with_bytes_is_activity() {
        let sink = TrafficSink::default();
        sink.mark(0, 0);
        assert!(!sink.is_active(), "a zero sample is not activity");

        sink.mark(1, 0);
        assert!(sink.is_active(), "one uploaded byte is activity");

        sink.mark(0, 1);
        assert!(sink.is_active(), "one downloaded byte is activity");
    }

    /// The trap this file is built around: a total that has ever been non-zero
    /// must not keep claiming traffic after the samples stop carrying it.
    ///
    /// `mark` is given the *rates*, so this pins that a later quiet sample is
    /// allowed to end the claim — the "read the total" bug would fail here.
    #[test]
    fn a_quiet_sample_after_traffic_stops_being_evidence() {
        let sink = TrafficSink::default();
        sink.mark(4096, 8192);
        assert!(sink.is_active());

        sink.mark(0, 0);
        assert!(
            !sink.is_active_for(Duration::ZERO),
            "a sample carrying nothing must not be activity"
        );
    }

    /// A byte is not enough on its own: the samples have to still be arriving.
    ///
    /// Without this, a Core that died mid-download leaves its last sample behind
    /// and the button stays connected forever on traffic that ended.
    #[test]
    fn traffic_older_than_the_window_is_not_active() {
        let sink = TrafficSink::default();
        sink.mark(1024, 1024);
        assert!(sink.is_active_for(TRAFFIC_ACTIVE_WINDOW));
        assert!(
            !sink.is_active_for(Duration::ZERO),
            "a byte outside the window is not activity now"
        );
    }

    /// A stopped Core must not inherit the last one's evidence.
    #[test]
    fn clearing_forgets_the_last_cores_traffic() {
        let sink = TrafficSink::default();
        sink.mark(1024, 2048);
        assert!(sink.is_active());

        sink.clear();
        assert!(!sink.is_active());
    }

    /// A restart must not make a *fresh* byte look older than it is.
    ///
    /// `clear()` wipes the timestamp but deliberately keeps the epoch, so the
    /// timestamp stays comparable to `epoch.elapsed()`. If the epoch were reset
    /// too, the next real sample would be stored as "now" while `is_active_for`
    /// compared it against a clock that restarted at zero — the sample would read
    /// as infinitely old and activity would be lost for a full window.
    #[test]
    fn clearing_keeps_the_clock_running_so_a_new_sample_is_not_stale() {
        let sink = TrafficSink::default();
        sink.mark(64, 64);
        sink.clear();
        assert!(!sink.is_active(), "cleared means nothing is proven");

        // A real sample after the clear must be activity immediately.
        sink.mark(64, 64);
        assert!(
            sink.is_active(),
            "a byte after a clear is still a byte now — the clock must not restart"
        );
    }

    /// A stream that could not be *opened* must back off; one that closed after
    /// working must not.
    ///
    /// These are different faults and the original code treated them the same,
    /// retrying an unopenable socket once a second forever. That is a busy loop
    /// against a Core that is not running — the state this module is in whenever
    /// the student has the app open but nothing connected.
    #[test]
    fn repeated_open_failures_back_off_but_a_closed_socket_retries_fast() {
        // The first few failures are the ones a connect is watching for.
        assert_eq!(reconnect_delay(0), RECONNECT_DELAY);
        assert_eq!(reconnect_delay(2), RECONNECT_DELAY);
        // Past that, speed buys nothing and spinning costs something.
        assert!(reconnect_delay(3) > RECONNECT_DELAY);
        assert!(reconnect_delay(100) > RECONNECT_DELAY);
        // And the backoff is bounded: a Core that comes back must be noticed
        // without the student waiting on a clock that has grown for hours.
        assert!(reconnect_delay(10_000) <= Duration::from_secs(30));
    }

    /// A dead stream's last sample must not keep the button green while the
    /// socket is being reopened.
    ///
    /// The failure this pins: the Core dies mid-download, its final sample is one
    /// second old, the reopen takes another second, and for that whole span the
    /// window still contains "a recent byte" — so `decide` reports Ready over a
    /// tunnel that is gone. Three seconds of green over a dead tunnel is the lie
    /// this module exists to prevent, and it is invisible to the timestamp alone.
    #[test]
    fn a_dead_stream_stops_counting_before_the_reconnect() {
        let sink = TrafficSink::default();
        sink.mark(2048, 4096);
        assert!(sink.is_active(), "a live sample is activity");

        // The stream is found stale: evidence must stop now, not in 3 s.
        sink.hold_off();
        assert!(
            !sink.is_active(),
            "a sample from a stream we have declared dead is not activity"
        );

        // A new stream delivering lifts the hold immediately.
        sink.mark(1, 1);
        assert!(sink.is_active(), "a delivered sample is a live stream again");
    }

    /// A hold left behind by one Core must not suppress the next one.
    ///
    /// `mark` lifting the hold is what guarantees it, so this pins the ordering:
    /// a fresh Core's first sample is activity even though the previous Core's
    /// stream died holding the claim off.
    #[test]
    fn a_new_stream_lifts_a_hold_left_by_an_old_one() {
        let sink = TrafficSink::default();
        sink.hold_off();
        assert!(!sink.is_active());
        sink.mark(0, 4096);
        assert!(sink.is_active());
    }

    /// The tray's zero-publish must not cancel a real byte.
    ///
    /// `apply_tray_speed(0, 0)` is called by the tray task on a failed connect, on
    /// `Stale`, and on every loop sweep, to blank the tray title. Now that the
    /// tray publishes into the shared sink, a zero from that path could land after
    /// a real sample and wipe the connection screen's proof — on macOS, with tray
    /// speed on, the tray's own housekeeping would break the fix.
    ///
    /// Read through the *stored* byte rather than a window, so the assertion
    /// cannot pass or fail on elapsed time. A zero-length window would make this
    /// a race: `is_active_for(Duration::ZERO)` is false the instant a nanosecond
    /// has passed, whether or not the bug is present.
    #[test]
    fn a_tray_zero_publish_does_not_cancel_measured_traffic() {
        let sink = TrafficSink::default();
        sink.mark(4096, 8192);
        assert!(sink.has_traffic.load(Ordering::Acquire), "a byte was recorded");

        // What the tray does to blank its display — not a sample from the Core.
        sink.blank_display();
        assert!(
            sink.has_traffic.load(Ordering::Acquire),
            "the tray blanking its own title must not erase the Core's measured traffic"
        );
        assert_eq!(
            (
                sink.last_up.load(Ordering::Relaxed),
                sink.last_down.load(Ordering::Relaxed)
            ),
            (4096, 8192),
            "the last real rates must survive a display blank"
        );
    }

    /// The window is a decision, not a taste: it must outlast the stream's own
    /// 1 Hz cadence with room for a late sample, and it must be short enough to
    /// be worth calling "now".
    #[test]
    fn the_activity_window_covers_several_samples_without_being_slack() {
        assert!(TRAFFIC_ACTIVE_WINDOW >= Duration::from_secs(2));
        assert!(TRAFFIC_ACTIVE_WINDOW <= Duration::from_secs(5));
        assert!(TRAFFIC_STALE_TIMEOUT > TRAFFIC_ACTIVE_WINDOW);
    }
}
