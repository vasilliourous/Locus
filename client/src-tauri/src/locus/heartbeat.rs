//! The heartbeat: the client's periodic check-in with the hub.
//!
//! Ports `legacy/wails-client/internal/heartbeat/heartbeat.go`, including its
//! constants — those were tuned against real school networks, not chosen by
//! feel.
//!
//! Three jobs, in order of importance:
//!
//! 1. **Suspension.** An operator can suspend a code at any time, and the next
//!    heartbeat is how the device finds out. Without this the client would keep
//!    working after a refund.
//! 2. **Config refresh.** The hub can push new connection parameters (a moved
//!    server, a rotated password, a tier change) so existing installs self-heal
//!    with no re-activation.
//! 3. **The update signal.** Updates are advertised through the heartbeat, not
//!    a separate poll, gated server-side by a per-device rollout percentage.

use crate::locus::contract::{HeartbeatRequest, HUB_URL, TierConfig};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

/// How often to beat once things are healthy.
pub const MIN_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Longest gap between beats, however many failures have accumulated.
pub const MAX_INTERVAL: Duration = Duration::from_secs(2 * 60 * 60);

/// How long the tunnel keeps working without a successful heartbeat.
///
/// This is what makes a suspension eventually bite even if the device never
/// gets a clean heartbeat, and it is the window an operator has to react before
/// a paying student is cut off by an outage. Bounded by expiry being enforced
/// on every heartbeat.
pub const GRACE_PERIOD: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Per-attempt timeout. School networks are slow, not absent.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Proportions of jitter applied to each interval.
///
/// Without it, every client that was activated in the same class period beats
/// in lockstep — and a hub that is briefly behind sees a thundering herd the
/// moment it recovers.
const JITTER: f64 = 0.1;

/// A tolerant `Option<u64>`: anything that is not a non-negative integer becomes
/// `None` instead of failing the deserialization.
///
/// Used for the **advisory** free-tier fields. Their whole class is "a number
/// the hub sends to help the client meter itself"; none of them is load-bearing
/// for a connection, and none of them can be trusted to be well-formed by a
/// client that did not send them. A negative, a float or a string therefore
/// degrades to "no allowance", never to "this beat is malformed" — because
/// failing the beat would also discard the `server_config`, the `expires_at`
/// and the update signal, which have nothing to do with the free tier.
///
/// This is deliberately the opposite of the rule for `uot_port` and the other
/// frozen wire names, where a wrong value *should* be loud: those are needed to
/// connect, and a silent `None` there is the bug that killed UoT fleet-wide
/// (`FIXES.md` 29). Advisory data tolerates; contract data does not.
fn tolerant_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match raw {
        Some(serde_json::Value::Number(n)) => n.as_u64(),
        _ => None,
    })
}

/// A tolerant `Option<u32>`, for [`tolerant_u64`]'s reason.
fn tolerant_u32<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match raw {
        Some(serde_json::Value::Number(n)) => n.as_u64().and_then(|v| u32::try_from(v).ok()),
        _ => None,
    })
}

/// The subset of the heartbeat response the client acts on.
#[derive(Debug, Clone, Deserialize)]
pub struct HeartbeatResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub server_config: Option<TierConfig>,
    #[serde(default)]
    pub udp_relay: bool,

    /// When this code lapses, exactly as the hub stores it.
    ///
    /// **Optional, and absence is meaningful.** A hub that predates the field
    /// sends nothing, and a code with no expiry recorded sends `null`; both
    /// deserialise to `None`. The UI must render nothing in that case rather
    /// than guessing "expired" — a fabricated expiry date is worse than a
    /// missing one, because it is a false claim about the student's account that
    /// support then has to argue with.
    ///
    /// Not reformatted here either: it is parsed with [`crate::locus::expiry::parse`]
    /// at the point of display, so there is one parser and one dialect.
    #[serde(default)]
    pub expires_at: Option<String>,

    /// The free tier's monthly allowance, in **mebibytes**, sent by the hub.
    ///
    /// **Advisory, and absent for a paying tier.** The hub has no per-user
    /// accounting (one shared password per tier means free users are
    /// indistinguishable on the wire), so the client counts its own bytes —
    /// see [`crate::locus::usage`]. It is a wire field rather than a constant
    /// precisely so the operator can change the allowance without shipping a
    /// release.
    ///
    /// **Absence means unlimited, not zero.** A paying tier omits this key, and
    /// so does an older hub that predates the free tier. Reading a missing
    /// allowance as "no bytes allowed" would silently throttle every user of a
    /// client built from this tree, so [`Self::allowance_bytes`] maps `None` to
    /// `None` and the callers treat that as "no quota to enforce".
    ///
    /// **A malformed value must not fail the whole beat.** `deserialize_with`
    /// drops a value that is not a non-negative integer (a negative count, a
    /// float, a string) to `None` rather than erroring, because this field is
    /// ADVISORY: a hub that sent a bad allowance must still be able to hand this
    /// device its config, its expiry and its update signal. Without this, one
    /// bad free-tier field would turn every beat into `Unreachable` — a paying
    /// student's tunnel quietly stops refreshing over a number that does not
    /// apply to them.
    #[serde(default, deserialize_with = "tolerant_u64")]
    pub free_allowance_mb: Option<u64>,

    /// The speed a free user drops to once the allowance is spent, in Mbps.
    ///
    /// The decided behaviour is *throttled further, not cut off*
    /// (`docs/business/04-tiers.md` §4.4.5): the app stays usable so the
    /// student does not uninstall it, while the paid tier becomes obviously
    /// worth it. Absent alongside a missing allowance.
    ///
    /// Tolerant of a malformed value for the same reason as
    /// [`Self::free_allowance_mb`] — see that field's note.
    #[serde(default, deserialize_with = "tolerant_u32")]
    pub free_throttle_mbps: Option<u32>,

    /// The advertised version, present only when this device is inside the
    /// rollout bucket. Absence is the normal case, not an error.
    #[serde(default)]
    pub update_available: Option<String>,

    /// Per-platform URL and checksum fields. Named `update_*` on the wire while
    /// the record stores `download_*`; the hub translates, and that rename is
    /// frozen. See `contract`.
    #[serde(default)]
    pub update_linux: Option<String>,
    #[serde(default)]
    pub update_windows: Option<String>,
    #[serde(default)]
    pub update_macos_intel: Option<String>,
    #[serde(default)]
    pub update_macos_arm: Option<String>,
    #[serde(default)]
    pub update_sha256_linux: Option<String>,
    #[serde(default)]
    pub update_sha256_windows: Option<String>,
    #[serde(default)]
    pub update_sha256_macos_intel: Option<String>,
    #[serde(default)]
    pub update_sha256_macos_arm: Option<String>,

    /// The legacy single-platform fields, kept for records that predate
    /// per-platform support. They describe the **Linux** binary only, which is
    /// why they are a fallback and never the first choice.
    #[serde(default)]
    pub update_url: Option<String>,
    #[serde(default)]
    pub update_sha256: Option<String>,
}

impl HeartbeatResponse {
    /// The download URL for a platform, falling back to the legacy field.
    ///
    /// The fallback is the historical bug in miniature: a record that only sets
    /// the legacy fields advertises the **Linux** binary to every platform, so
    /// Windows and macOS would download an ELF and fail the checksum. It is
    /// kept because a hub row may still be in that state, but the fork's own
    /// publishes always write per-platform values.
    #[must_use]
    pub fn download_url(&self, platform: &str) -> Option<&str> {
        let per_platform = match platform {
            crate::locus::contract::PLATFORM_LINUX => self.update_linux.as_deref(),
            crate::locus::contract::PLATFORM_WINDOWS => self.update_windows.as_deref(),
            crate::locus::contract::PLATFORM_MACOS_INTEL => self.update_macos_intel.as_deref(),
            crate::locus::contract::PLATFORM_MACOS_ARM => self.update_macos_arm.as_deref(),
            _ => None,
        };
        per_platform.or(self.update_url.as_deref())
    }

    /// The checksum for the artifact [`Self::download_url`] would fetch.
    #[must_use]
    pub fn download_sha256(&self, platform: &str) -> Option<&str> {
        let per_platform = match platform {
            crate::locus::contract::PLATFORM_LINUX => self.update_sha256_linux.as_deref(),
            crate::locus::contract::PLATFORM_WINDOWS => self.update_sha256_windows.as_deref(),
            crate::locus::contract::PLATFORM_MACOS_INTEL => self.update_sha256_macos_intel.as_deref(),
            crate::locus::contract::PLATFORM_MACOS_ARM => self.update_sha256_macos_arm.as_deref(),
            _ => None,
        };
        per_platform.or(self.update_sha256.as_deref())
    }

    /// Whether the hub reported a healthy account.
    ///
    /// A response with any other status is not a successful beat: treating it as
    /// one would reset the backoff and keep a suspended client looking healthy.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.status == "ok"
    }

    /// The free tier's monthly allowance as bytes, or `None` for "no quota".
    ///
    /// `None` means **unlimited**, which is the answer for every paying tier and
    /// for any hub that predates the free tier. It is deliberately not `Some(0)`:
    /// a zero allowance would throttle every client built from this tree, and an
    /// unknown key must never gate a paying user's traffic. An explicit
    /// `free_allowance_mb = 0` is treated the same way — the operator's way of
    /// turning the quota off without a release.
    ///
    /// The figure is **clamped** before conversion ([`crate::locus::usage::sane_allowance_mb`]).
    /// An absurd value straight off the wire saturates the multiply below, and
    /// the saturated byte count then overflows `used * 10` in the classifier —
    /// which misreads every window as spent and throttles the student instantly
    /// and permanently. Clamping degrades toward "unlimited" instead.
    #[must_use]
    pub fn allowance_bytes(&self) -> Option<u64> {
        self.free_allowance_mb
            .filter(|mb| *mb > 0)
            .map(|mb| crate::locus::usage::sane_allowance_mb(mb) * crate::locus::usage::MIB)
    }
}

/// What a single beat did, so the caller can react.
#[derive(Debug, Clone)]
pub enum BeatOutcome {
    /// The account is in good standing; the payload may carry a config refresh
    /// and/or an update signal.
    Ok(Box<HeartbeatResponse>),
    /// The hub refused: suspended, expired, unknown, or a credential it would
    /// not accept. The device is no longer entitled, and the tunnel should come
    /// down.
    ///
    /// There used to be a separate [`StaleCredential`] arm for a 401 — the
    /// signal that a recognised device's session token had lapsed, which the
    /// supervisor answered by re-recognising. Tokens are gone with device
    /// recognition: a device always authenticates with its code, so a 401 can
    /// only mean the code itself is not accepted, which is a refusal.
    Refused { reason: String },
    /// A transport failure. The account is *not* known to be bad — the tunnel
    /// keeps working through the grace period.
    Unreachable { reason: String },
}

/// What a heartbeat authenticates with: the activation code.
///
/// This was a two-variant enum (a code, or a session token minted at device
/// recognition). Recognition is gone, and with it the token: a device always
/// holds a code now, and `crate::locus::credential` is what keeps it from being
/// lost. The type is kept as a newtype rather than being replaced by a bare
/// `String` so the redaction and formatting rules below stay attached to the
/// value everywhere it is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential(String);

impl Credential {
    /// Wraps a stored activation code.
    #[must_use]
    pub fn code(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    /// The credential's value, whichever kind it is.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A redacted form for logs — never the whole value.
    ///
    /// A code is a bearer credential, so it is not logged in full. Twelve
    /// characters is enough to correlate two log lines and not enough to use.
    #[must_use]
    pub fn redacted(&self) -> String {
        self.as_str().chars().take(12).collect()
    }
}

/// How many consecutive failures have happened, and therefore how long to wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    consecutive_failures: u32,
}

impl Backoff {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            consecutive_failures: 0,
        }
    }

    pub const fn reset(&mut self) {
        self.consecutive_failures = 0;
    }

    pub const fn record_failure(&mut self) {
        // Saturating: past MAX_INTERVAL the delay is capped anyway, and
        // overflowing on a very long outage would be a buffer-overflow class
        // bug in a place nobody is looking.
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
    }

    #[must_use]
    pub const fn failures(self) -> u32 {
        self.consecutive_failures
    }

    /// The next delay: `MIN * 2^failures`, capped at `MAX`.
    ///
    /// The exponent is clamped before the shift so a long outage cannot
    /// overflow the multiplication.
    #[must_use]
    pub fn delay(self) -> Duration {
        if self.consecutive_failures == 0 {
            return MIN_INTERVAL;
        }
        let exponent = self.consecutive_failures.min(20);
        let scaled = MIN_INTERVAL.saturating_mul(1_u32 << exponent);
        scaled.min(MAX_INTERVAL)
    }

    /// The delay with ±10% jitter applied.
    #[must_use]
    pub fn jittered_delay(self) -> Duration {
        let base = self.delay().as_secs_f64();
        // A cheap deterministic-ish spread derived from the clock. Jitter does
        // not need to be cryptographic; it needs to differ between devices.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let unit = f64::from(seed % 1000) / 1000.0; // 0.0..1.0
        let offset = base * JITTER * (2.0 * unit - 1.0); // -10%..+10%
        Duration::from_secs_f64((base + offset).max(1.0))
    }

    /// How much of the grace period is left, given the last success.
    ///
    /// `last_ok == 0` means the device has never had a successful heartbeat, so
    /// the full window applies from first launch. Returning zero there would cut
    /// off a device that has simply never been online.
    #[must_use]
    pub const fn remaining_grace(last_ok: i64, now: i64) -> Duration {
        if last_ok == 0 {
            return GRACE_PERIOD;
        }
        let elapsed = now.saturating_sub(last_ok);
        if elapsed < 0 {
            return GRACE_PERIOD;
        }
        GRACE_PERIOD.saturating_sub(Duration::from_secs(elapsed as u64))
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

/// Sends one heartbeat. Makes exactly one attempt.
///
/// `credential` is the activation code this device holds. `token` is sent empty:
/// it is a frozen wire field kept for deployed clients that still read it, and
/// nothing mints one any more.
pub async fn beat(credential: &Credential, fingerprint: &str) -> BeatOutcome {
    let body = HeartbeatRequest {
        code: credential.as_str(),
        fingerprint,
        token: "",
    };

    let client = match reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .pool_idle_timeout(Duration::from_secs(30))
        .user_agent(concat!("Locus-Client/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return BeatOutcome::Unreachable {
                reason: format!("could not build the hub client: {error}"),
            };
        }
    };

    let response = match client.post(format!("{HUB_URL}/api/heartbeat")).json(&body).send().await {
        Ok(response) => response,
        Err(error) => {
            return BeatOutcome::Unreachable {
                reason: format!("could not reach the hub: {error}"),
            };
        }
    };

    let status = response.status();
    let code = status.as_u16();
    let text = match response.text().await {
        Ok(text) => text,
        Err(error) => {
            return BeatOutcome::Unreachable {
                reason: format!("hub returned an unreadable body ({status}): {error}"),
            };
        }
    };

    classify_response(code, &text)
}

/// Decides what a response means, from its status code and body. Pure, so the
/// whole mapping is testable without a network.
///
/// The rule that matters: a **definite** answer about the account is a refusal,
/// and a status the client does not recognise is a transport failure it may
/// retry. Getting a status into the wrong bucket is not cosmetic — a refusal
/// filed as `Unreachable` is retried forever and the tunnel stays up, which is
/// how an expired code kept working.
///
/// Refusals are 403 (suspended / bound elsewhere), 404 (unknown code) and **410
/// (expired)**. 410 was missing, so an expired code's refusal was retried as
/// though it were a flaky network — half of why nothing ever expired.
///
/// **401 is deliberately its own arm, not a refusal.** A 401 from this endpoint
/// means the credential is not one the hub will accept, which for a
/// code-authenticated device means the code itself — an unknown code answers
/// 404, so a 401 here is treated as a refusal and the device is torn down.
/// There is no token to renew any more: tokens went with device recognition.
#[must_use]
fn classify_response(code: u16, text: &str) -> BeatOutcome {
    let message = || {
        serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| value.get("message").and_then(|m| m.as_str()).map(str::to_owned))
            .unwrap_or_else(|| format!("the hub refused this device ({code})"))
    };

    // 401 is included with the other refusals. It used to be the token path's
    // signal to renew; with tokens gone it means the hub will not accept this
    // code, which is a refusal like any other.
    if matches!(code, 401 | 403 | 404 | 410) {
        return BeatOutcome::Refused { reason: message() };
    }

    if !(200..300).contains(&code) {
        return BeatOutcome::Unreachable {
            reason: format!("the hub returned {code}"),
        };
    }

    match serde_json::from_str::<HeartbeatResponse>(text) {
        Ok(parsed) if parsed.is_ok() => BeatOutcome::Ok(Box::new(parsed)),
        Ok(parsed) => BeatOutcome::Unreachable {
            reason: format!("the hub reported status {:?}", parsed.status),
        },
        Err(error) => BeatOutcome::Unreachable {
            reason: format!("the hub returned an unexpected shape: {error}"),
        },
    }
}

/// The heartbeat loop's control handle.
///
/// Owns the running task so the app can start it once after activation and stop
/// it cleanly on exit. A loop that outlived the app would keep a suspended code
/// looking alive, and a loop that started before activation would beat with no
/// code.
pub struct HeartbeatLoop {
    stop: Arc<Notify>,
    /// Wakes the loop out of its sleep so it beats immediately.
    ///
    /// Needed because the interval is a *floor* (5 minutes, and up to 2 hours
    /// after failures), so without this the only way to learn the account's
    /// current state is to wait for the timer. A student who has just renewed —
    /// or an operator who has just suspended a code — should not have to wait for
    /// a backoff that was computed for a network problem that has long since
    /// passed.
    beat_now: Arc<Notify>,
    /// Completions of a beat that was asked for by [`HeartbeatLoop::beat_now`].
    ///
    /// # Why a second primitive, rather than reusing `beat_now`
    ///
    /// `beat_now` only *wakes the loop*. Its `notify_one` returns as soon as the
    /// value is stored, so a caller that awaits nothing learns nothing — and the
    /// UI did exactly that: it cleared its "Checking…" flag the moment the IPC
    /// command returned, which is before the request was even sent. The button
    /// therefore animated a state it had not observed, in single-digit
    /// milliseconds, while the hub round trip that the student is actually waiting
    /// for had not begun. That is the "flicks from state to state so fast it seems
    /// off" report.
    ///
    /// So the request and the completion are separate signals. A caller that only
    /// wants to shorten the wait uses `beat_now` and returns immediately (the
    /// tray, the periodic nudge); a caller that must *report* the outcome awaits
    /// [`HeartbeatLoop::beat_now_and_wait`].
    ///
    /// This is not a queue: `Notify::notify_waiters` wakes whoever is waiting at
    /// that instant, so a beat completing with nobody listening is simply not
    /// observed. That is the intended semantic — the completion is an event for
    /// whoever asked, not a durable record. The durable record is the stored
    /// expiry, which `locus_status` reads.
    beat_done: Arc<Notify>,
    task: tokio::task::JoinHandle<()>,
}

impl HeartbeatLoop {
    /// Runs `on_outcome` after every beat until stopped.
    ///
    /// The callback is the seam the UI and the tunnel use: it receives the
    /// refresh and the update signal, and decides what to apply. Keeping policy
    /// out of here is what stops this module from needing to know about the core
    /// manager.
    pub fn start<F>(credential: Credential, fingerprint: String, on_outcome: F) -> Self
    where
        F: Fn(BeatOutcome) + Send + 'static,
    {
        let stop = Arc::new(Notify::new());
        let stop_signal = Arc::clone(&stop);
        let beat_now = Arc::new(Notify::new());
        let beat_now_signal = Arc::clone(&beat_now);
        let beat_done = Arc::new(Notify::new());
        let beat_done_signal = Arc::clone(&beat_done);

        let task = tokio::spawn(async move {
            let mut backoff = Backoff::new();

            loop {
                let outcome = beat(&credential, &fingerprint).await;

                match &outcome {
                    BeatOutcome::Ok(_) => backoff.reset(),
                    // A refusal is terminal for this run: the account is not in
                    // good standing, and hammering the hub will not change it.
                    //
                    // The completion signal is raised here rather than shared with
                    // the path below, so the terminal case stays a single obvious
                    // exit — a caller waiting on a manual check needs the wake on
                    // a refusal too, since that is an answer to the question it
                    // asked.
                    BeatOutcome::Refused { .. } => {
                        on_outcome(outcome);
                        wake_waiting_beat(&beat_done_signal);
                        break;
                    }
                    BeatOutcome::Unreachable { .. } => backoff.record_failure(),
                }

                on_outcome(outcome);
                // Raised AFTER `on_outcome`, so a caller awaiting this can rely on
                // everything a completed beat changes — the stored expiry, the
                // applied config — having already happened. Signalling first would
                // hand the UI a fresh flag over stale state, which is the same lie
                // in a new place.
                wake_waiting_beat(&beat_done_signal);

                let delay = backoff.jittered_delay();
                tokio::select! {
                    () = stop_signal.notified() => break,
                    // A beat was asked for. Wake now and beat on the next
                    // iteration, resetting nothing: the backoff is left alone so
                    // a manual check does not silently paper over a genuine
                    // outage by pretending the schedule was met.
                    () = beat_now_signal.notified() => {}
                    () = tokio::time::sleep(delay) => {}
                }
            }
        });

        Self {
            stop,
            beat_now,
            beat_done,
            task,
        }
    }

    /// Asks the loop to beat as soon as it can, without waiting out its delay.
    ///
    /// A no-op if a beat is already in flight or imminent; the point is to stop a
    /// *long* wait, not to queue extra requests. Deliberately does not reset the
    /// backoff, so this cannot be used to keep a failing device hammering the hub.
    ///
    /// Returns immediately. Use [`Self::control`] and
    /// [`HeartbeatControl::beat_now_and_wait`] when the caller has something to
    /// *report* about the outcome.
    pub fn beat_now(&self) {
        self.beat_now.notify_one();
    }

    /// Stops the loop and waits for it to finish.
    pub async fn stop(self) {
        self.stop.notify_one();
        let _ = self.task.await;
    }

    /// A cloneable handle for asking the loop to beat and waiting for it.
    ///
    /// Separate from `self` because `HeartbeatLoop` owns the task's
    /// `JoinHandle` and therefore cannot be cloned — while the *waiting* caller
    /// sits in an async command that must not hold the loop's mutex across a
    /// network round trip. The handle carries only the two notifiers.
    #[must_use]
    pub fn control(&self) -> HeartbeatControl {
        HeartbeatControl {
            beat_now: Arc::clone(&self.beat_now),
            beat_done: Arc::clone(&self.beat_done),
        }
    }
}

/// The cloneable half of a running loop: request a beat, wait for it, hear about
/// it.
///
/// Holds neither the task nor the stop signal, so it cannot stop or restart the
/// loop — it can only ask it to do the thing it already does.
#[derive(Clone)]
pub struct HeartbeatControl {
    beat_now: Arc<Notify>,
    beat_done: Arc<Notify>,
}

impl HeartbeatControl {
    /// Asks the loop to beat as soon as it can. Returns immediately.
    pub fn beat_now(&self) {
        self.beat_now.notify_one();
    }

    /// Asks the loop to beat, and resolves once that beat has completed and its
    /// outcome has been applied.
    ///
    /// Registration happens **before** the request, or a fast completion could
    /// land in the window between the two and be missed — leaving the caller
    /// asleep until the *next* beat, minutes later.
    pub async fn beat_now_and_wait(&self) {
        let done = self.beat_done.notified();
        tokio::pin!(done);
        done.as_mut().enable();

        self.beat_now.notify_one();
        done.await;
    }
}

/// Wakes anything awaiting a beat completion, without storing a permit.
///
/// `notify_waiters` rather than `notify_one`: a completion is a broadcast to
/// whoever is currently waiting, and nobody is owed a completion that happened
/// while they were not listening. `notify_one` would leave a permit behind, so a
/// later `beat_now_and_wait` could resolve on a *previous* beat's completion and
/// report success for a request that had not been sent — the precise bug this
/// signal exists to remove.
fn wake_waiting_beat(signal: &Arc<Notify>) {
    signal.notify_waiters();
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, reason = "tests assert by panicking")]
mod tests {
    use super::*;

    /// A refusal status must be a refusal, whatever the account problem behind
    /// it. **410 (expired) is the one that matters most**, because it is what the
    /// hub sends for a lapsed code and it was the one missing — so an expired
    /// device's refusal was retried forever as `Unreachable` and the tunnel
    /// stayed up.
    ///
    /// This is the client half of "expiry does not work"; the hub half is the
    /// missing check in `heartbeat.pb.js`. Neither alone fixes it.
    #[test]
    fn terminal_statuses_are_refusals_not_retries() {
        for code in [403, 404, 410] {
            let outcome = classify_response(code, r#"{"message":"Code expired"}"#);
            assert!(
                matches!(outcome, BeatOutcome::Refused { .. }),
                "status {code} is a definite answer and must not be retried, got {outcome:?}"
            );
        }
    }

    /// The reason is carried through from the hub's body, so a student is told
    /// *why* — "Code expired" is actionable, "refused" is not.
    #[test]
    fn a_refusal_carries_the_hubs_own_reason() {
        let outcome = classify_response(410, r#"{"code":410,"message":"Code expired"}"#);
        match outcome {
            BeatOutcome::Refused { reason } => assert_eq!(reason, "Code expired"),
            other => panic!("expected Refused, got {other:?}"),
        }
    }

    /// A transport failure must NOT be a refusal: cutting off a paying student
    /// because the school network dropped a packet is the failure the grace
    /// period exists to prevent.
    #[test]
    fn server_errors_are_retried_not_refused() {
        for code in [500, 502, 503, 429] {
            let outcome = classify_response(code, "");
            assert!(
                matches!(outcome, BeatOutcome::Unreachable { .. }),
                "status {code} is a transient failure, not a refusal, got {outcome:?}"
            );
        }
    }

    /// The success path still parses, and a 200 whose body says otherwise is not
    /// a successful beat.
    #[test]
    fn a_200_with_a_non_ok_status_is_not_success() {
        let ok = classify_response(200, r#"{"status":"ok"}"#);
        assert!(matches!(ok, BeatOutcome::Ok(_)), "a well-formed ok beat must parse");

        let not_ok = classify_response(200, r#"{"status":"paused"}"#);
        assert!(
            matches!(not_ok, BeatOutcome::Unreachable { .. }),
            "a 200 that does not claim ok must not be treated as success"
        );
    }

    /// These constants are tuned against real school networks. Changing one by
    /// feel changes how quickly a suspended student loses access and how much
    /// load a recovering hub takes.
    #[test]
    fn intervals_match_the_deployed_contract() {
        assert_eq!(MIN_INTERVAL, Duration::from_secs(300), "5 minutes");
        assert_eq!(MAX_INTERVAL, Duration::from_secs(7200), "2 hours");
        assert_eq!(GRACE_PERIOD, Duration::from_secs(604_800), "7 days");
    }

    /// Backoff doubles from the minimum and never exceeds the maximum, however
    /// long the outage lasts.
    #[test]
    fn backoff_doubles_then_caps() {
        let mut backoff = Backoff::new();
        assert_eq!(backoff.delay(), MIN_INTERVAL);

        backoff.record_failure();
        assert_eq!(backoff.delay(), MIN_INTERVAL * 2);

        backoff.record_failure();
        assert_eq!(backoff.delay(), MIN_INTERVAL * 4);

        for _ in 0..50 {
            backoff.record_failure();
        }
        assert_eq!(backoff.delay(), MAX_INTERVAL, "a long outage must cap, not overflow");
    }

    /// A success must return the interval to the minimum, or a device that
    /// recovered would keep beating hours apart.
    #[test]
    fn success_resets_the_backoff() {
        let mut backoff = Backoff::new();
        for _ in 0..5 {
            backoff.record_failure();
        }
        assert!(backoff.delay() > MIN_INTERVAL);

        backoff.reset();
        assert_eq!(backoff.delay(), MIN_INTERVAL);
        assert_eq!(backoff.failures(), 0);
    }

    /// Jitter must stay near the base delay: large enough to spread a fleet,
    /// small enough that the interval is still what it claims to be.
    #[test]
    fn jitter_stays_within_ten_percent() {
        let mut backoff = Backoff::new();
        backoff.record_failure();
        let base = backoff.delay().as_secs_f64();

        for _ in 0..200 {
            let jittered = backoff.jittered_delay().as_secs_f64();
            assert!(
                jittered >= base * 0.89 && jittered <= base * 1.11,
                "jittered {jittered} drifted outside ±10% of {base}"
            );
        }
    }

    /// A device that has never beaten gets the full window, not zero — it has
    /// simply never been online.
    #[test]
    fn a_device_that_never_beats_gets_the_whole_grace_period() {
        assert_eq!(Backoff::remaining_grace(0, 1_000_000), GRACE_PERIOD);
    }

    #[test]
    fn grace_counts_down_from_the_last_success() {
        let now = 1_000_000;
        let day = 86_400;

        assert_eq!(Backoff::remaining_grace(now, now), GRACE_PERIOD);
        assert_eq!(
            Backoff::remaining_grace(now - day, now),
            GRACE_PERIOD - Duration::from_secs(day as u64)
        );
    }

    /// Once the window has elapsed the answer is zero, not an underflow — a
    /// wrapped grace period would grant unlimited access.
    #[test]
    fn an_expired_grace_period_is_zero_not_negative() {
        let now = 1_000_000;
        let long_ago = now - 10_000_000;
        assert_eq!(Backoff::remaining_grace(long_ago, now), Duration::ZERO);
    }

    /// A clock that jumps backwards (NTP correction, timezone change) must not
    /// read as "grace expired".
    #[test]
    fn a_backwards_clock_does_not_expire_the_grace_period() {
        let now = 1_000_000;
        let future = now + 5_000;
        assert_eq!(Backoff::remaining_grace(future, now), GRACE_PERIOD);
    }

    const PLATFORM: &str = "linux";

    /// The per-platform field must win over the legacy one.
    #[test]
    fn per_platform_fields_take_precedence() {
        let response = HeartbeatResponse {
            status: "ok".into(),
            tier: None,
            server_config: None,
            udp_relay: false,
            expires_at: None,
            free_allowance_mb: None,
            free_throttle_mbps: None,
            update_available: Some("2.0.0".into()),
            update_linux: Some("https://hub/updates/2.0.0/locus-linux-amd64".into()),
            update_windows: Some("https://hub/updates/2.0.0/locus-windows-amd64.exe".into()),
            update_macos_intel: None,
            update_macos_arm: None,
            update_sha256_linux: Some("aaa".into()),
            update_sha256_windows: Some("bbb".into()),
            update_sha256_macos_intel: None,
            update_sha256_macos_arm: None,
            update_url: Some("https://hub/updates/2.0.0/locus-linux-amd64".into()),
            update_sha256: Some("legacy".into()),
        };

        assert_eq!(
            response.download_url(PLATFORM),
            Some("https://hub/updates/2.0.0/locus-linux-amd64")
        );
        assert_eq!(
            response.download_sha256(PLATFORM),
            Some("aaa"),
            "the per-platform hash must win over the legacy single hash"
        );
    }

    /// A record carrying only the legacy fields is the documented hazard: it
    /// describes the Linux binary, so a Windows client would fetch an ELF. The
    /// fallback exists for compatibility, so it must be *observable*.
    #[test]
    fn the_legacy_fallback_is_used_only_when_the_platform_field_is_absent() {
        let response = HeartbeatResponse {
            status: "ok".into(),
            tier: None,
            server_config: None,
            udp_relay: false,
            expires_at: None,
            free_allowance_mb: None,
            free_throttle_mbps: None,
            update_available: Some("2.0.0".into()),
            update_linux: None,
            update_windows: None,
            update_macos_intel: None,
            update_macos_arm: None,
            update_sha256_linux: None,
            update_sha256_windows: None,
            update_sha256_macos_intel: None,
            update_sha256_macos_arm: None,
            update_url: Some("https://hub/updates/2.0.0/locus-linux-amd64".into()),
            update_sha256: Some("legacy".into()),
        };

        assert_eq!(
            response.download_url(crate::locus::contract::PLATFORM_WINDOWS),
            Some("https://hub/updates/2.0.0/locus-linux-amd64"),
            "the fallback returns the linux URL even for windows — which is why the \
             publish path must always write per-platform fields"
        );
    }

    /// A response the hub considers unhealthy is not a successful beat.
    #[test]
    fn a_non_ok_status_is_not_a_success() {
        let response = HeartbeatResponse {
            status: "degraded".into(),
            tier: None,
            server_config: None,
            udp_relay: false,
            expires_at: None,
            free_allowance_mb: None,
            free_throttle_mbps: None,
            update_available: None,
            update_linux: None,
            update_windows: None,
            update_macos_intel: None,
            update_macos_arm: None,
            update_sha256_linux: None,
            update_sha256_windows: None,
            update_sha256_macos_intel: None,
            update_sha256_macos_arm: None,
            update_url: None,
            update_sha256: None,
        };
        assert!(!response.is_ok());
    }

    /// The live hub response shape, captured from the deployed server, must
    /// parse — including `uot_port` nested inside `server_config`.
    #[test]
    fn parses_a_live_hub_response() {
        let live = r#"{
          "status": "ok",
          "server_time": "2026-09-19T10:09:42.945Z",
          "tier": "strike",
          "server_config": {
            "server": "networkingguides.duckdns.org",
            "server_port": 8445,
            "password": "3467ce5ae756c185f45be91fed2dbeb6",
            "method": "2022-blake3-aes-256-gcm",
            "uot_port": 8446
          },
          "udp_relay": true
        }"#;

        let response: HeartbeatResponse = serde_json::from_str(live).expect("the live response shape must parse");

        assert!(response.is_ok());
        assert_eq!(response.tier.as_deref(), Some("strike"));

        let config = response.server_config.expect("server_config must parse");
        assert_eq!(config.server_port, 8445);
        assert_eq!(
            config.uot_port, 8446,
            "nested uot_port must survive deserialisation or the Strike tier loses UDP"
        );
        assert!(config.uot_enabled(response.udp_relay));
    }

    /// A malformed allowance must not fail the whole beat.
    ///
    /// The allowance is ADVISORY. If a bad free-tier number made the response
    /// unparseable, the beat would be discarded as `Unreachable` — and with it
    /// the `server_config`, the `expires_at` and any update signal. A paying
    /// student's tunnel would quietly stop refreshing over a field that does not
    /// even apply to them.
    #[test]
    fn a_malformed_allowance_does_not_fail_the_beat() {
        for bad in ["-1", "5.5", "\"5120\"", "null", "{}"] {
            let json = format!(
                r#"{{"status":"ok","tier":"free","free_allowance_mb":{bad},"free_throttle_mbps":{bad}}}"#
            );
            let response: HeartbeatResponse = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("a bad allowance ({bad}) must not fail the beat: {e}"));
            assert!(response.is_ok(), "the beat itself is still good ({bad})");
            assert_eq!(
                response.free_allowance_mb, None,
                "a malformed allowance reads as absent, not as a number ({bad})"
            );
            assert_eq!(response.allowance_bytes(), None);
            assert_eq!(
                response.tier.as_deref(),
                Some("free"),
                "the rest of the payload survives ({bad})"
            );
        }
    }

    /// A well-formed allowance still parses, including zero.
    ///
    /// The tolerance must not swallow real values — the failure mode of an
    /// over-eager guard is that it silently disables the feature it protects.
    #[test]
    fn a_well_formed_allowance_still_parses() {
        let json = r#"{"status":"ok","tier":"free","free_allowance_mb":5120,"free_throttle_mbps":1}"#;
        let response: HeartbeatResponse =
            serde_json::from_str(json).expect("a good allowance must parse");
        assert_eq!(response.free_allowance_mb, Some(5120));
        assert_eq!(response.free_throttle_mbps, Some(1));
        assert_eq!(response.allowance_bytes(), Some(5120 * 1024 * 1024));

        // Zero is the operator switching the quota off, and must survive as a
        // real value — not be treated as malformed.
        let off = r#"{"status":"ok","tier":"free","free_allowance_mb":0}"#;
        let response: HeartbeatResponse =
            serde_json::from_str(off).expect("zero must parse");
        assert_eq!(response.free_allowance_mb, Some(0));
        assert_eq!(
            response.allowance_bytes(),
            None,
            "zero means no quota, not zero bytes allowed"
        );
    }

    /// An absurd allowance is clamped before it becomes bytes.
    ///
    /// Straight off the wire, `u64::MAX` saturates the multiply and the saturated
    /// value then overflows `used * 10` in the classifier, which misreads every
    /// window as spent — the student is throttled instantly and permanently.
    /// Clamping degrades toward "unlimited", the safe direction.
    #[test]
    fn an_absurd_allowance_is_clamped_to_a_usable_byte_count() {
        let json = format!(
            r#"{{"status":"ok","tier":"free","free_allowance_mb":{}}}"#,
            u64::MAX
        );
        let response: HeartbeatResponse =
            serde_json::from_str(&json).expect("an absurd allowance must still parse");
        let bytes = response.allowance_bytes().expect("a quota is still applied");
        assert_eq!(
            bytes,
            crate::locus::usage::MAX_SANE_ALLOWANCE_MB * crate::locus::usage::MIB,
            "the absurd value must be clamped, not saturated into nonsense"
        );
    }

    /// A beat requested through the control handle must **resolve**, not return
    /// immediately.
    ///
    /// This is the guard for the "Check status now" flicker. The old path used
    /// `Notify::notify_one` alone, which returns as soon as the value is stored —
    /// so the UI cleared its pending state in single-digit milliseconds while the
    /// hub round trip it was reporting on had not begun. The test asserts the
    /// command's own contract: it resolves only after a completion is signalled.
    ///
    /// Written against the raw signal rather than a live loop, because the point
    /// is the *waiting* behaviour, not the network. The loop's own completion
    /// signalling is covered by `a_completed_beat_wakes_a_waiter`.
    #[tokio::test]
    async fn beat_now_and_wait_does_not_resolve_before_completion() {
        let beat_now = Arc::new(Notify::new());
        let beat_done = Arc::new(Notify::new());
        let control = HeartbeatControl {
            beat_now: Arc::clone(&beat_now),
            beat_done: Arc::clone(&beat_done),
        };

        // Race the wait against a short timer. The wait must NOT win: nothing has
        // signalled completion yet.
        let waiter = tokio::spawn(async move { control.beat_now_and_wait().await });

        let early = tokio::time::timeout(std::time::Duration::from_millis(50), waiter)
            .await
            .is_ok();
        assert!(
            !early,
            "beat_now_and_wait resolved before any completion was signalled; \
             this is the flicker bug — the caller would report success for a beat \
             that had not happened"
        );

        // Now signal completion, as the loop does.
        beat_done.notify_waiters();

        // And confirm the request itself was actually made.
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), beat_now.notified())
                .await
                .is_ok(),
            "beat_now_and_wait must request a beat, not merely wait for one"
        );
    }

    /// The order matters: the request must be issued before the wait begins, and
    /// a completion signalled *after* registration must reach the waiter.
    #[tokio::test]
    async fn a_completed_beat_wakes_a_waiter() {
        let beat_now = Arc::new(Notify::new());
        let beat_done = Arc::new(Notify::new());
        let control = HeartbeatControl {
            beat_now: Arc::clone(&beat_now),
            beat_done: Arc::clone(&beat_done),
        };

        let signaller = tokio::spawn(async move {
            // Wait for the request, then report completion, as the loop does.
            beat_now.notified().await;
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            beat_done.notify_waiters();
        });

        let waited = tokio::time::timeout(std::time::Duration::from_millis(1000), control.beat_now_and_wait()).await;

        assert!(
            waited.is_ok(),
            "a signalled completion must reach the waiter; a missed wake means the \
             caller sleeps until the next beat, minutes later"
        );
        signaller.await.expect("signaller task must finish");
    }

    /// `notify_waiters` leaves no permit behind, so a completion that happens with
    /// nobody listening cannot be mistaken for a later request's completion.
    ///
    /// This is why the signal is `notify_waiters` and not `notify_one`: a stored
    /// permit would let a subsequent `beat_now_and_wait` resolve on a *previous*
    /// beat, reporting success for a request that had not been sent.
    #[tokio::test]
    async fn a_completion_with_no_waiter_is_not_replayed() {
        let beat_now = Arc::new(Notify::new());
        let beat_done = Arc::new(Notify::new());
        let control = HeartbeatControl {
            beat_now: Arc::clone(&beat_now),
            beat_done: Arc::clone(&beat_done),
        };

        // A completion happens while nothing is waiting.
        beat_done.notify_waiters();

        // A later wait must still block until a NEW completion is signalled.
        let waiter = tokio::spawn(async move { control.beat_now_and_wait().await });
        let early = tokio::time::timeout(std::time::Duration::from_millis(50), waiter)
            .await
            .is_ok();
        assert!(
            !early,
            "a stale completion was replayed to a later waiter; the control would \
             report a check that never ran"
        );

        beat_done.notify_waiters();
    }
}
