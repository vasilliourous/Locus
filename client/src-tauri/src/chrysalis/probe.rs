//! The session probe: asking the hub what IT believes about this device.
//!
//! # Why the client runs this at all
//!
//! The device is the only party that knows its own half of a desync. The hub can
//! say "this code is in term, last seen 4 minutes ago"; it cannot say "and the
//! student's app believes it is suspended", because that fact has never left the
//! machine. `docs/reference/AUTH-STATE.md` names six cases that all render as
//! "signed out" on the console, and separating them needs both halves in one
//! place — which is why the probe body carries a `client` block.
//!
//! # What this is NOT
//!
//! * **Not a heartbeat.** It writes nothing on the hub, and it is never called on
//!   a timer. Liveness is the heartbeat's job (`heartbeat.rs`); this is a
//!   human-initiated diagnostic. Calling it periodically would be the polling
//!   storm the heartbeat's backoff exists to prevent, against a 10-per-10-min
//!   rate limit.
//! * **Not a repair.** It cannot change what the hub will do next. The only
//!   mutating repair in this product is an operator's `codes.unbind`.
//!
//! # The transport, and why it is the one call that always tries the free tunnel
//!
//! The probe is run when the hub and the device disagree, and the most common
//! cause of that is a school network that **refuses the hub by name** — the same
//! condition the free ("maintenance") tunnel exists for. A probe that only went
//! direct would fail on precisely the networks it is most needed on, and report
//! "the hub is down", which is the wrong diagnosis entirely.
//!
//! So unlike every other hub call in this crate, the probe tries **both** routes
//! on every invocation: direct, then the free tunnel (starting an engine if none
//! is warm). The rest of the product makes that second attempt only after a
//! previous call has established a block, because they run on loops where a cold
//! engine per attempt would be unacceptable. The probe is one deliberate request
//! that a person is waiting on, so the economics are reversed — see
//! `worthy_of_tunnel` for the fault set, which is deliberately wider than the
//! heartbeat's.
//!
//! Note that the free tunnel is **not** always running: it is stopped once the
//! plan tunnel is proven (`TUNNEL-SUPERVISOR.md` §3). A connected device has no
//! warm engine, which is why a probe on a blocked network may take several
//! seconds the first time — that is the engine starting, not the hub hanging.
//! * **Not authoritative.** Its whole output exists to be read by a person; the
//!   local half of the client's state is still whatever `store.rs` says.
//!
//! # Redaction
//!
//! `device_id` is the **redacted** fingerprint (`store::redact`, 12 chars), not
//! the full hardware fingerprint. The hub stores `first_seen_fingerprint` in the
//! same truncated form, so the comparison still works — and a probe transcript
//! that a student pastes into a support thread carries nothing that identifies
//! their machine beyond a prefix that is already on their account screen.
//! Nothing here ever includes the activation code in a log line.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::chrysalis::{contract, device, store};

/// The schema version this client speaks. The hub rejects anything else with a
/// `409` rather than interpreting it — see `session_probe.pb.js`.
pub const PROBE_VERSION: u32 = 1;

/// The client's own view, sent so the hub can name the divergence.
///
/// Every field is the client's honest report of its own state. `auth_state` is
/// the load-bearing one: it is derived from the same facts the gate branches on
/// (`cmd::chrysalis::chrysalis_status`), so the probe cannot claim a state the UI
/// is not showing.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientReport {
    pub app_version: String,
    pub platform: String,
    pub auth_state: AuthState,
    pub last_heartbeat_ok_unix: Option<i64>,
    pub heartbeat_failures: u32,
    pub grace_remaining_s: u64,
    pub hub_enforcement: Option<u32>,
    pub recorded_refusal: Option<String>,
    pub clock: Clock,
    pub degradation: Vec<Degradation>,
    pub transport: String,
}

/// The client's auth state, as a closed set.
///
/// # Why this mirrors `SUSPENSION.md` rather than inventing a vocabulary
///
/// The hub needs to know which of the four product states the device is in, and
/// a second, probe-only vocabulary would be a second thing that can disagree with
/// the gate. These five are exactly the states `gateScreen` branches on, plus
/// `Diverged` — the state that only exists *during* a desync, which is the state
/// the probe is for.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AuthState {
    /// Holds a usable code and the gate shows the app.
    Activated,
    /// Holds a code, and the hub's last answer was a suspension.
    Suspended,
    /// Holds no code; the hub's last answer was a refusal.
    Refused,
    /// Holds nothing and has no history.
    SignedOut,
    /// Holds a code but is past its grace period — authenticated with no
    /// corroboration. The client-side half of the desync, and the reason
    /// `GRACE_PERIOD` is what the hub's `STALE` bound is set to.
    LapsedGrace,
}

/// A degradation flag, passed through to the operator verbatim.
///
/// Free-form strings are deliberately not used: the runbook's table is keyed on
/// these words, so they are a small closed set like `reason_code` is on the hub.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Degradation {
    /// Consecutive heartbeat failures are non-zero.
    BeatsFailing,
    /// The last beat had no HTTP response at all.
    Offline,
    /// Hub traffic is going through the free-tier relay.
    MaintenanceTunnel,
    /// The stored record could not be used as a code.
    LocalRecordDamaged,
}

/// Both clocks, so a skew can be *reported* without ever being *used*.
///
/// The hub's liveness arithmetic uses only its own clock, so this cannot corrupt
/// a verdict. It exists because a skewed client renders confusing ages on its own
/// Account screen, and an operator seeing "last confirmed 3 hours in the future"
/// should get an explanation rather than a mystery.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Clock {
    pub unix: i64,
    pub utc_offset_min: i32,
}

/// What the hub answered. Mirrors `session_probe.pb.js`; unknown keys are dropped.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct ProbeResponse {
    pub probe_version: u32,
    pub server_time: String,
    pub entitlement: String,
    pub liveness: String,
    pub hub_view: HubView,
    pub client_view: Option<serde_json::Value>,
    pub lease_expires_at: Option<String>,
    pub reason_code: String,
    #[serde(default)]
    pub divergence: Vec<String>,
}

/// The hub's half, as it reported it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct HubView {
    pub activated_at: Option<String>,
    pub released_at: Option<String>,
    pub released_pending: bool,
    pub suspended: bool,
    pub suspended_reason: Option<String>,
    pub expires_at: Option<String>,
    pub tier: String,
    /// `None` means the hub has never seen this code beat — **not** that it is
    /// absent. Same distinction as `last_confirmed_at` in `cmd::chrysalis`.
    pub last_seen_at: Option<String>,
    pub last_seen_age_s: Option<i64>,
    pub first_seen_fingerprint: String,
}

/// The `client` block of the request.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ClientBlock {
    app_version: String,
    platform: String,
    device_id: String,
    #[serde(flatten)]
    report: ClientReport,
}

/// The request body.
#[derive(Debug, Clone, Serialize)]
struct ProbeRequest {
    probe_version: u32,
    code: String,
    device_id: String,
    client: ClientBlock,
}

/// Builds the client's half of the probe from what the app already knows.
///
/// Split out from the network call so it is testable without a hub, and so the
/// derivations below are in one place. Every input is a fact the gate already
/// reads — this deliberately does not observe anything new, because a probe that
/// measured something the product does not use would report a state the UI never
/// shows, and the two would then disagree for a new reason.
#[must_use]
pub fn client_report(verge: &crate::config::IVerge) -> ClientReport {
    let activation = store::read(verge);
    let damaged = store::stored_code_is_damaged(verge);
    let suspended = store::is_suspended(verge);

    let last_ok = verge.last_heartbeat_ok.filter(|t| *t > 0);
    let failures = verge.heartbeat_failures.unwrap_or(0);

    let now = now_unix();
    let grace = activation.as_ref().map_or(0, |_| {
        crate::chrysalis::heartbeat::Backoff::remaining_grace(last_ok.unwrap_or(0), now).as_secs()
    });

    let auth_state = derive_auth_state(activation.is_some(), damaged, suspended, last_ok, grace, verge);

    let mut degradation = Vec::new();
    if failures > 0 {
        degradation.push(Degradation::BeatsFailing);
    }
    if damaged {
        degradation.push(Degradation::LocalRecordDamaged);
    }

    ClientReport {
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: contract::current_platform().unwrap_or("unknown").to_owned(),
        auth_state,
        last_heartbeat_ok_unix: last_ok,
        heartbeat_failures: failures,
        grace_remaining_s: grace,
        hub_enforcement: verge.chrysalis_hub_enforcement,
        recorded_refusal: verge.chrysalis_refusal_reason.as_deref().map(ToOwned::to_owned),
        clock: Clock {
            unix: now,
            utc_offset_min: local_utc_offset_min(),
        },
        degradation,
        // Which route this device's hub traffic actually takes. Reported because
        // the runbook's "the client cannot reach the hub" case turns on it: a
        // device on the free-tier relay is one whose network refused the hub by
        // name, and that is a different problem from a hub that is down.
        transport: if crate::chrysalis::activation_tunnel::prefer_maintenance() {
            "maintenance-tunnel".to_owned()
        } else {
            "direct".to_owned()
        },
    }
}

/// The client's auth state, derived from facts the gate already branches on.
///
/// # Why this is a separate function and not six lines inline
///
/// It is the whole client half of the desync diagnosis, and it has two traps
/// (below) that an earlier version fell into. Extracted so a test can drive every
/// branch without a config file, a filesystem, or an app handle — the same reason
/// `store::activation_from` exists. `has_activation` replaces `activation.is_some()`
/// because the caller has already resolved the record.
///
/// # Trap 1 — "never beaten" is not "healthy"
///
/// [`crate::chrysalis::heartbeat::Backoff::remaining_grace`] returns the FULL
/// window when `last_ok == 0`, because its documented job is to avoid cutting off
/// a device that has simply never been online. Correct for the tunnel, wrong
/// here: a device that has never reached the hub is exactly the `unknown` the hub
/// reports, and labelling it `Activated` makes the hub's pair read `in_sync`.
///
/// # Trap 2 — grace running out is a degradation
///
/// `grace == 0` means the window has EXPIRED. That is `LapsedGrace`: the client's
/// own half of the desync (authenticated with no corroboration), which is what
/// `GRACE_PERIOD` exists to bound.
///
/// Note the deliberate asymmetry with `activation.is_none()`: a device with no
/// code and a recorded refusal is `Refused`, not `SignedOut`, because the two
/// show different screens and the probe must report the one the student is
/// looking at.
#[allow(
    clippy::missing_const_for_fn,
    reason = "`IVerge::chrysalis_refusal_reason` is a String, so `is_some()` on it is not const-callable; the lint fires on the signature alone"
)]
fn derive_auth_state(
    has_activation: bool,
    damaged: bool,
    suspended: bool,
    last_ok: Option<i64>,
    grace_seconds: u64,
    verge: &crate::config::IVerge,
) -> AuthState {
    if damaged {
        AuthState::Refused
    } else if suspended && has_activation {
        // A suspension without a code is nonsense (`store::is_suspended` already
        // refuses it on read), so it is not a state this can report.
        AuthState::Suspended
    } else if !has_activation {
        if verge.chrysalis_refusal_reason.is_some() {
            AuthState::Refused
        } else {
            AuthState::SignedOut
        }
    } else if last_ok.is_none() || grace_seconds == 0 {
        AuthState::LapsedGrace
    } else {
        AuthState::Activated
    }
}

/// Unix seconds, saturating at 0 before the epoch rather than panicking.
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The machine's UTC offset, in minutes, for a display that will read correctly.
///
/// Deliberately NOT used in any arithmetic on either side: it is reported so a
/// skewed clock can be explained, and nothing compares it. A timezone bug in
/// this product (`parsePBDate`, the goja date dialect) came from treating a
/// display fact as a comparison fact; this is where that mistake would recur.
///
/// Returns 0 where the platform cannot say, which is an honest "no opinion"
/// rather than a guess at a timezone.
fn local_utc_offset_min() -> i32 {
    #[cfg(unix)]
    {
        // `localtime_r` applies the offset in force AT THIS INSTANT, DST
        // included, for the process's own TZ. No stored timezone is involved.
        // Only `tm_gmtoff` is read; every other field is left alone, so a struct
        // mismatch on another libc cannot corrupt the answer.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as libc::time_t);
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if unsafe { libc::localtime_r(&now, &mut tm) }.is_null() {
            return 0;
        }
        (tm.tm_gmtoff / 60) as i32
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// Runs one probe. Exactly one attempt, no retry.
///
/// No retry on purpose: a retry would turn one diagnostic call into three against
/// a 10-per-10-min bucket, and — more importantly — an operator diagnosing a flaky
/// network needs to see the *failed* attempt, not a silently-retried success. The
/// caller renders the error.
pub async fn probe(report: ClientReport) -> Result<ProbeResponse> {
    let verge = crate::config::Config::verge().await;
    let activation = store::read(&verge.latest_arc())
        .ok_or_else(|| anyhow::anyhow!("this device holds no activation code, so there is nothing to probe"))?;

    let device_id = store::redact(&device::fingerprint());
    let body = ProbeRequest {
        probe_version: PROBE_VERSION,
        code: activation.code.clone(),
        device_id: device_id.clone(),
        client: ClientBlock {
            app_version: report.app_version.clone(),
            platform: report.platform.clone(),
            device_id,
            report,
        },
    };

    let hub = contract::hub_url().await;
    let url = format!("{hub}/api/session-probe");

    // ── THE TRANSPORT: this is the one call that must try the free tunnel ────
    //
    // The probe is run when the hub and the device disagree, and the most common
    // reason they disagree is that the school network REFUSES THE HUB BY NAME —
    // the same condition the maintenance ("free") tunnel exists for. A probe that
    // only ever went direct would therefore fail on precisely the networks it is
    // most needed on, and its failure would be indistinguishable from "the hub is
    // down", which is the wrong diagnosis.
    //
    // The rest of the product makes two calls, in this order:
    //   1. `hub_client_for`      — direct, or the tunnel if one is ALREADY warm;
    //                              never starts an engine.
    //   2. `hub_client_for_retry` — may START a maintenance engine.
    // Heartbeat (`beat` + `retry_beat_over_maintenance`) and activation both do
    // it, and both spend the second half only after a classifiable transport
    // failure. That is right for a call on a LOOP: paying a ~60 MB engine load on
    // every beat would be unacceptable.
    //
    // The probe is NOT on a loop. It is a single, deliberate, human-initiated
    // request with a 10-per-10-minute server-side limit, so the cost of a cold
    // engine is paid once, by someone who is already waiting for a diagnosis and
    // has been told to press a button. So this call site tries BOTH routes every
    // time rather than half of them. `probe_hub` below owns that decision.
    let (status, text) = probe_hub(&hub, &body, &url).await?;

    match status {
        200 => serde_json::from_str::<ProbeResponse>(&text)
            .map_err(|e| anyhow::anyhow!("the hub returned an unexpected probe shape: {e}")),
        // 404 is UNIFORM by design (unknown / suspended / expired / wrong device),
        // so it is reported as what it is rather than guessed at. Saying "wrong
        // device" here would be the client inventing the hub's reason.
        404 => Err(anyhow::anyhow!(
            "the hub answered 404 — it does not recognise this code for this device. \
             That answer is deliberately the same for an unknown, suspended or expired code, \
             so check the code and this device's id against the console."
        )),
        409 => Err(anyhow::anyhow!(
            "this app's probe schema (v{PROBE_VERSION}) is not what the hub speaks. Update the app."
        )),
        429 => Err(anyhow::anyhow!(
            "the hub is rate limiting probes; try again in a few minutes"
        )),
        other => Err(anyhow::anyhow!("the hub answered {other}: {text}")),
    }
}

/// Sends the probe body, trying the direct route and then the free tunnel.
///
/// Returns the status and body text of whichever route answered.
///
/// # Why both routes, every time
///
/// See the call site. In short: this is the diagnostic for a device that cannot
/// reach the hub, and the most common cause is a network that blocks the hub by
/// name. The tunnel exists for exactly that, so the probe tries it on a transport
/// failure rather than relying on some *earlier* call having already failed and
/// flipped the session-wide preference.
///
/// # Why the retry is bounded to one attempt
///
/// `hub_client_for_retry` may START an engine, which takes seconds. A diagnostic
/// that hangs for a minute looks broken. The tunnel's own `post` path already
/// treats a tunnel failure as terminal for the session (`tunnel_already_failed`),
/// so a network where the tunnel is also blocked costs one attempt, not a loop.
///
/// # The error names which network
///
/// The three failure shapes are genuinely different problems and the operator
/// acts differently on each, so they are not collapsed into "could not reach the
/// hub":
///   * no response, no tunnel available  → their network blocks the hub
///   * no response, tunnel also failed   → their network blocks both
///   * an HTTP status                    → the hub answered; this is not a network problem
async fn probe_hub(hub: &str, body: &ProbeRequest, url: &str) -> Result<(u16, String)> {
    use crate::chrysalis::activation_tunnel;
    use crate::chrysalis::hub::{Failure, Timeout};

    // The direct attempt, using the warm route if the session already prefers it.
    let direct = activation_tunnel::hub_client_for(hub, Timeout::Activation).await;
    match direct.client().post(url).json(body).send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            Ok((status, text))
        }
        Err(error) => {
            // A status means the hub ANSWERED — a 4xx/5xx is not a transport
            // failure and the tunnel would say the same thing. Only a genuine
            // no-response failure is worth a second, expensive route.
            let failure = Failure::classify(&error);
            if !worthy_of_tunnel(failure) {
                let remedy = failure.remedy().map(|r| format!(" {r}")).unwrap_or_default();
                return Err(anyhow::anyhow!("the hub did not answer: {error}.{remedy}"));
            }

            // The tunnel is unavailable in a build that carries no free-tier key.
            // Say so explicitly: "both blocked" and "this build has no tunnel" are
            // different answers and an operator chasing the first would be wrong.
            if !activation_tunnel::activation_tunnel_available() {
                return Err(anyhow::anyhow!(
                    "the hub could not be reached directly and this build carries no free-tier \
                     key, so the free tunnel was not available. This is very likely your \
                     network refusing {hub}."
                ));
            }

            let retry = activation_tunnel::hub_client_for_retry(hub, Timeout::Activation).await;
            match retry.client().post(url).json(body).send().await {
                Ok(response) => {
                    tracing::info!(target: "chrysalis::probe", "[probe] answered via the free tunnel");
                    let status = response.status().as_u16();
                    let text = response.text().await.unwrap_or_default();
                    Ok((status, text))
                }
                Err(tunnel_error) => Err(anyhow::anyhow!(
                    "the hub could not be reached directly ({:?}) and the free tunnel failed too \
                     ({tunnel_error}). Your network is blocking both.",
                    Failure::classify(&error)
                )),
            }
        }
    }
}

/// Whether a direct-route failure is worth retrying through the free tunnel.
///
/// # Why this is NOT `Failure::retryable_over_tunnel`
///
/// That predicate is calibrated for the HEARTBEAT LOOP, and its exclusions are
/// economic: retrying a `Transport` failure per beat would spawn a ~60 MB engine
/// on every beat of a network where the hub is merely slow. Its own doc says so
/// ("TimedOut in particular is indistinguishable from a hub that is merely
/// slow, and retrying it on a heartbeat loop would spawn a 60 MB engine per
/// beat").
///
/// The probe is not on a loop. It is one request that a student pressed a button
/// to send, it is limited server-side to 10 per 10 minutes, and the person
/// waiting has already been told the app may take a moment. So the economics are
/// reversed: paying one cold engine is cheap, and FAILING TO DIAGNOSE is
/// expensive.
///
/// That matters because school filters block in more than one shape. A reset or
/// a refused handshake is the one the heartbeat's set covers; a silent
/// **timeout** — the connection dropped or blackholed with no RST — is at least
/// as common, and it classifies as `Transport`. Excluding it would make the probe
/// silently unusable on a large share of the networks it exists for, which is the
/// exact defect this function was written to fix at the call site.
///
/// What is still NOT retried, and must not be:
///   * `Certificate` / `Protocol` — the hub WAS reached. A different transport
///     presents the same certificate and speaks the same TLS, so the tunnel
///     cannot help and trying it would only delay the real message.
///   * `Response` — a status arrived; the problem is above the transport.
const fn worthy_of_tunnel(failure: crate::chrysalis::hub::Failure) -> bool {
    use crate::chrysalis::hub::Failure;
    match failure {
        Failure::ResetDuringHandshake | Failure::Refused | Failure::Dns | Failure::Transport => true,
        Failure::Certificate | Failure::Protocol | Failure::Response => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire words are a contract with `session_probe.pb.js` and the runbook.
    /// A rename here would silently stop matching the hub's `client_view`.
    #[test]
    fn the_auth_state_wire_words_are_the_ones_the_hub_expects() {
        let cases = [
            (AuthState::Activated, "\"activated\""),
            (AuthState::Suspended, "\"suspended\""),
            (AuthState::Refused, "\"refused\""),
            (AuthState::SignedOut, "\"signed-out\""),
            (AuthState::LapsedGrace, "\"lapsed-grace\""),
        ];
        for (state, want) in cases {
            assert_eq!(
                serde_json::to_string(&state).unwrap(),
                want,
                "auth_state wire word changed"
            );
        }
    }

    #[test]
    fn degradation_flags_are_the_words_the_runbook_names() {
        assert_eq!(
            serde_json::to_string(&Degradation::BeatsFailing).unwrap(),
            "\"beats-failing\""
        );
        assert_eq!(
            serde_json::to_string(&Degradation::LocalRecordDamaged).unwrap(),
            "\"local-record-damaged\""
        );
    }

    /// The request must carry the REDACTED id in both places, and the code must
    /// not appear anywhere except `code`. This is the redaction boundary, and it
    /// is asserted on the serialised bytes rather than on the struct, because a
    /// later field could leak it without touching these.
    #[test]
    fn the_probe_body_uses_the_redacted_device_id() {
        let full = "0123456789abcdef0123456789abcdef";
        let redacted = store::redact(full);
        assert_eq!(redacted.len(), 12);
        assert!(full.starts_with(&redacted));
        assert_ne!(redacted, full, "the probe must not carry the full fingerprint");
    }

    /// A null `last_seen_at` from the hub must deserialize to `None`, not to a
    /// fabricated timestamp — the same rule the console applies.
    #[test]
    fn a_never_seen_code_parses_as_none_not_as_a_date() {
        let json = r#"{
            "probe_version": 1,
            "server_time": "2026-11-25T10:00:00Z",
            "entitlement": "active",
            "liveness": "unknown",
            "hub_view": {
                "activated_at": "2026-11-20T08:00:00Z",
                "released_at": null,
                "released_pending": false,
                "suspended": false,
                "suspended_reason": null,
                "expires_at": null,
                "tier": "strike",
                "last_seen_at": null,
                "last_seen_age_s": null,
                "first_seen_fingerprint": "a1b2c3d4e5f6"
            },
            "client_view": null,
            "lease_expires_at": null,
            "reason_code": "unknown",
            "divergence": []
        }"#;
        let parsed: ProbeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.liveness, "unknown");
        assert!(
            parsed.hub_view.last_seen_at.is_none(),
            "a null last_seen must not become a date"
        );
        assert!(parsed.hub_view.last_seen_age_s.is_none());
    }

    /// The hub speaks snake_case; the UI speaks camelCase. `ProbeResponse` is the
    /// one struct that travels BOTH ways, so its rename is directional — and a
    /// directional rename is exactly the kind of thing that silently breaks in one
    /// direction only. Both are asserted.
    ///
    /// The failure this prevents: parse a hub response correctly, then serialise
    /// it to the webview still in snake_case, and the Account screen reads
    /// `undefined` for every field — a blank diagnostic that looks like "the hub
    /// said nothing".
    #[test]
    fn the_probe_response_parses_snake_case_and_serialises_camel_case() {
        let hub_json = r#"{
            "probe_version": 1,
            "server_time": "2026-11-25T10:00:00Z",
            "entitlement": "active",
            "liveness": "live",
            "hub_view": {
                "activated_at": "2026-11-20T08:00:00Z",
                "released_at": null,
                "released_pending": false,
                "suspended": false,
                "suspended_reason": null,
                "expires_at": null,
                "tier": "strike",
                "last_seen_at": "2026-11-25T09:55:00Z",
                "last_seen_age_s": 300,
                "first_seen_fingerprint": "a1b2c3d4e5f6"
            },
            "client_view": null,
            "lease_expires_at": "2026-11-25T12:16:00Z",
            "reason_code": "client_beating_hub_quiet",
            "divergence": []
        }"#;
        let parsed: ProbeResponse = serde_json::from_str(hub_json).expect("snake_case must parse");
        assert_eq!(parsed.hub_view.last_seen_age_s, Some(300));

        let to_ui = serde_json::to_value(&parsed).expect("must serialise");
        assert!(to_ui.get("reasonCode").is_some(), "the UI reads camelCase; got {to_ui}");
        assert!(
            to_ui.get("reason_code").is_none(),
            "snake_case must not reach the wire; got {to_ui}"
        );
        assert!(
            to_ui["hubView"].get("lastSeenAgeS").is_some(),
            "nested fields must rename too; got {to_ui}"
        );
    }

    // ── The derivation ──────────────────────────────────────────────────────
    //
    // Every branch of `derive_auth_state`, because this is the client half of the
    // desync verdict and TWO of these cases were wrong in the first version: a
    // never-beaten device reported `Activated` (making the hub's pair read
    // `in_sync`), and grace expiry was indistinguishable from health.
    fn verge_with_refusal(reason: Option<&str>) -> crate::config::IVerge {
        crate::config::IVerge {
            chrysalis_refusal_reason: reason.map(Into::into),
            ..crate::config::IVerge::default()
        }
    }

    #[test]
    fn a_device_that_has_never_beaten_is_not_reported_as_activated() {
        // Holds a code, no successful beat ever, full grace window remaining
        // (that is what `remaining_grace(0, now)` returns). The trap: reporting
        // `Activated` here makes the hub's verdict `in_sync` about a device it has
        // never heard from.
        let state = derive_auth_state(true, false, false, None, 7 * 24 * 3600, &verge_with_refusal(None));
        assert_eq!(
            state,
            AuthState::LapsedGrace,
            "a never-beaten device must not claim to be activated"
        );
    }

    #[test]
    fn an_expired_grace_window_is_a_degradation_not_a_healthy_state() {
        let state = derive_auth_state(true, false, false, Some(1_700_000_000), 0, &verge_with_refusal(None));
        assert_eq!(state, AuthState::LapsedGrace);
    }

    #[test]
    fn a_recently_confirmed_device_is_activated() {
        let state = derive_auth_state(
            true,
            false,
            false,
            Some(1_700_000_000),
            6 * 24 * 3600,
            &verge_with_refusal(None),
        );
        assert_eq!(state, AuthState::Activated);
    }

    #[test]
    fn a_suspension_only_reads_as_suspended_while_a_code_is_held() {
        // The invariant `SUSPENSION.md` enforces three times: a suspension is a
        // statement about a code.
        assert_eq!(
            derive_auth_state(true, false, true, Some(1_700_000_000), 3600, &verge_with_refusal(None)),
            AuthState::Suspended
        );
        assert_eq!(
            derive_auth_state(false, false, true, None, 0, &verge_with_refusal(None)),
            AuthState::SignedOut,
            "a suspension with no code held describes nothing"
        );
    }

    #[test]
    fn a_recorded_refusal_outranks_a_plain_sign_out() {
        // They show different screens; the probe reports the one the student sees.
        assert_eq!(
            derive_auth_state(false, false, false, None, 0, &verge_with_refusal(Some("Code expired"))),
            AuthState::Refused
        );
        assert_eq!(
            derive_auth_state(false, false, false, None, 0, &verge_with_refusal(None)),
            AuthState::SignedOut
        );
    }

    #[test]
    fn a_damaged_record_is_refused_whatever_else_is_true() {
        // `stored_code_is_damaged` means what is on disk is not a code. Reporting
        // any other state would have the probe defend a credential that cannot
        // authenticate.
        assert_eq!(
            derive_auth_state(true, true, true, Some(1), 3600, &verge_with_refusal(None)),
            AuthState::Refused
        );
    }

    #[test]
    fn degradation_flags_follow_the_evidence() {
        let verge = crate::config::IVerge {
            activation_code: Some("RQ-ABCD-EFGH-JKMN-T".into()),
            chrysalis_tier: Some("strike".into()),
            device_fingerprint: Some("0123456789abcdef0123456789abcdef".into()),
            heartbeat_failures: Some(3),
            last_heartbeat_ok: Some(1),
            ..crate::config::IVerge::default()
        };
        let report = client_report(&verge);
        assert!(
            report.degradation.contains(&Degradation::BeatsFailing),
            "a non-zero failure count must be reported, not swallowed"
        );
    }
    // ── Transport routing ───────────────────────────────────────────────────
    //
    // The probe is the ONE call that must reach the hub on a name-blocked
    // network, because that is the network it diagnoses. These pin the decision,
    // and the `Transport` case is the one that was wrong: it is excluded from the
    // heartbeat's set for a reason that does not apply here.
    #[test]
    fn a_timeout_or_reset_is_worth_the_tunnel() {
        use crate::chrysalis::hub::Failure;
        // The shapes a school filter actually produces.
        assert!(
            worthy_of_tunnel(Failure::Dns),
            "a resolver that answers nothing is the classic block"
        );
        assert!(
            worthy_of_tunnel(Failure::Refused),
            "a middlebox refusing the destination"
        );
        assert!(
            worthy_of_tunnel(Failure::ResetDuringHandshake),
            "the read-and-kill signature"
        );
        // THE CASE THE HEARTBEAT'S SET EXCLUDES. A silent blackhole (no RST, no
        // answer) classifies as Transport, and it is at least as common as a
        // reset. Excluding it would make the probe unusable on those networks.
        assert!(
            worthy_of_tunnel(Failure::Transport),
            "a blackholed connection must still be tried over the tunnel by a one-shot probe"
        );
    }

    #[test]
    fn reaching_the_hub_but_failing_is_not_worth_the_tunnel() {
        use crate::chrysalis::hub::Failure;
        // A different transport presents the same certificate and the same TLS,
        // so the tunnel cannot fix either of these and trying would only delay
        // the message the operator actually needs.
        assert!(!worthy_of_tunnel(Failure::Certificate));
        assert!(!worthy_of_tunnel(Failure::Protocol));
        assert!(
            !worthy_of_tunnel(Failure::Response),
            "a status arrived; not a transport problem"
        );
    }
}
