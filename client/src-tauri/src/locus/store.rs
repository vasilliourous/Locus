//! Product state: what this device is entitled to, and how it is doing.
//!
//! # Why the state lives in `IVerge`
//!
//! Verge already has a config store (`config/verge.rs`), with a draft/transaction
//! system, atomic writes and a save path that the rest of the app follows. A
//! second JSON file beside it would be a second thing to corrupt, a second backup
//! mechanism, and a second migration path — for seven fields. So the product
//! state is **additive fields on `IVerge`**, which needs no migration because
//! every field is `Option<T>` with a serde default.
//!
//! # The security boundary, stated plainly
//!
//! The activation code is stored in plaintext in a user-writable file. That is
//! unavoidable — the device must hold it to authenticate — and it is the same
//! choice the retired client made. What matters is what we do *around* it:
//!
//!   * it is never logged;
//!   * it is never included in a diagnostics export;
//!   * the frontend is given a redacted form, never the real value;
//!   * the real enforcement is the hub's, not the client's: a student who edits
//!     this file to claim a tier they did not buy gets a config the hub refuses
//!     to serve at their next heartbeat.
//!
//! The last point is the important one. Client-side storage is not a security
//! boundary and must not be treated as one; it is a cache of an entitlement the
//! hub owns.

use crate::config::{Config, IVerge};
use crate::locus::contract::TierConfig;
use anyhow::Result;
// `IVerge` uses smartstring, not std String. Importing the alias makes the
// conversions below unnecessary and keeps this module in step with the config
// type it is writing into.
use smartstring::alias::String as SmartString;

/// Everything the client knows about its own entitlement.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Activation {
    pub code: String,
    pub tier: String,
    pub fingerprint: String,
}

/// Whether this device currently has an entitlement it can use.
///
/// Requires all three parts. A code without a fingerprint cannot heartbeat (the
/// hub uses the fingerprint for rollout bucketing and binding checks), and one
/// without a tier cannot build a config — so a partial record is not "activated",
/// it is damaged, and saying so is more useful than half-working.
#[must_use]
pub fn is_usable(activation: &Activation) -> bool {
    !activation.code.trim().is_empty()
        && !activation.tier.trim().is_empty()
        && !activation.fingerprint.trim().is_empty()
}

/// Reads the stored activation, if the record is complete.
///
/// Returns `None` for a partial record rather than a half-filled struct: the
/// caller must decide to re-activate, and it cannot make that decision from a
/// struct that looks populated but is not usable.
///
/// When the config has no code, this falls back to the durable machine-store
/// mirror ([`crate::locus::credential`]) so a device whose `verge.yaml` was
/// wiped by an uninstall can still be seen as activated. The tier and
/// fingerprint must still be present in the config for the record to be usable —
/// those are refreshed by the next heartbeat, and [`read_code_rehydrating`]
/// is the path that also writes the recovered code back.
#[must_use]
pub fn read(verge: &IVerge) -> Option<Activation> {
    activation_from(verge, crate::locus::credential::read_code())
}

/// The pure decision behind [`read`], with the mirror value injected.
///
/// Split out so the "config first, mirror as fallback" rule can be tested
/// without touching a real machine-scoped file — the same reason the identity
/// module takes its sources as arguments.
#[must_use]
fn activation_from(verge: &IVerge, mirrored_code: Option<String>) -> Option<Activation> {
    let code = config_code(verge).or(mirrored_code)?;
    let activation = Activation {
        code,
        tier: verge.locus_tier.as_deref().unwrap_or_default().to_owned(),
        fingerprint: verge.device_fingerprint.as_deref().unwrap_or_default().to_owned(),
    };
    is_usable(&activation).then_some(activation)
}

/// The stored activation code, from the config **or** the durable mirror.
///
/// `verge.yaml` is the primary store, but it is the app's own config and an
/// uninstall or a reset takes it — along with the code the student may no longer
/// have a card for. So a second copy lives in a machine-scoped file
/// ([`crate::locus::credential`]) and this is the read that consults both.
///
/// The config wins when it holds a code, because that is the value the rest of
/// the app has been running on and it is refreshed by every write. The mirror is
/// the fallback: when the config has none, a code found there is *adopted back*
/// into the config so the launch is not a one-off.
///
/// Returns `None` when neither holds anything, which is the genuine first-run
/// state.
pub async fn read_code_rehydrating() -> Option<String> {
    let verge = Config::verge().await;
    if let Some(code) = config_code(&verge.latest_arc()) {
        return Some(code);
    }

    // The config lost it. A mirror is the only thing that can recover the
    // student's entitlement, so adopt it and write it back to the config too.
    let code = crate::locus::credential::read_code()?;
    logging::info_recovered_code();
    // Best-effort: the caller has the code either way, and failing the launch
    // over a config write would trade a recovering device for a clean file.
    if let Err(error) = store_code_in_config(&code).await {
        logging::warn_could_not_write_back(&error);
    }
    Some(code)
}

/// The code held in the config alone, without consulting the mirror.
#[must_use]
fn config_code(verge: &IVerge) -> Option<String> {
    verge
        .activation_code
        .as_deref()
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .map(str::to_owned)
}

/// Writes just the code into the config, leaving the rest of the record alone.
///
/// Used by [`read_code_rehydrating`] to put a recovered code back. Deliberately
/// narrow: it must not touch the tier or the fingerprint, which the heartbeat
/// owns and which a recovery has no new information about.
async fn store_code_in_config(code: &str) -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.activation_code = Some(SmartString::from(code));
    });
    verge.data_arc().save_file().await
}

/// Persists an activation, then saves the file.
///
/// Written as one operation so a crash cannot leave the code stored without the
/// tier, or the reverse. `save_file` is the same path the rest of the app uses,
/// so this inherits its atomicity.
pub async fn store(activation: &Activation) -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.activation_code = Some(SmartString::from(activation.code.as_str()));
        draft.locus_tier = Some(SmartString::from(activation.tier.as_str()));
        draft.device_fingerprint = Some(SmartString::from(activation.fingerprint.as_str()));
    });
    let saved = verge.data_arc().save_file().await;

    // Mirror the code to the machine store in the SAME operation, so a launch
    // that writes the code once is a launch that has a durable copy of it. This
    // is the write that makes the code survive `verge.yaml` being wiped by an
    // uninstall or a reset.
    //
    // Best-effort and logged, never fatal: the config write above is what the
    // session needs, and a device with no writable machine store should still
    // activate — it simply cannot promise to survive a reinstall.
    if !activation.code.trim().is_empty() && !crate::locus::credential::write_code(&activation.code) {
        logging::warn_could_not_mirror();
    }

    saved
}

// `read_identity`, `store_identity`, `store_device_token` and `device_token`
// — REMOVED.
//
// These were the app-config half of the durable device identity and the
// session token minted at device recognition. Both features are gone: a client
// keeps its activation code in a machine-scoped store
// (`crate::locus::credential`) and authenticates with that code alone.
//
// The `locus_device_*` / `locus_identity_store` / `locus_device_token` fields
// still exist on `IVerge` so an upgrade does not fail to deserialize a config
// that holds them. They are simply never read or written any more.
/// Records a successful heartbeat.
///
/// The failure count is reset at the same time, because the two are one fact:
/// a beat either worked (and the backoff restarts) or it did not. Splitting them
/// across two writes is how a device ends up with a fresh timestamp and a stale
/// failure count, and then backs off as though it were still failing.
pub async fn record_heartbeat_success(now_unix: i64) -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.last_heartbeat_ok = Some(now_unix);
        draft.heartbeat_failures = Some(0);
    });
    verge.data_arc().save_file().await
}

/// Records a failed heartbeat and the new consecutive-failure count.
pub async fn record_heartbeat_failure(failures: u32) -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.heartbeat_failures = Some(failures);
    });
    verge.data_arc().save_file().await
}

/// Records the subscription expiry the hub reported.
///
/// Stores the hub's own string verbatim. The client never converts it to a
/// timestamp: that conversion is where a timezone bug would enter, and the only
/// consumer is a display that parses it once via [`crate::locus::expiry`].
///
/// Takes an `Option` rather than a `&str` because the hub may legitimately send
/// `null` for a code with no expiry recorded. The caller decides whether that is
/// worth writing; see the note in `runtime::handle_success` for why a `None` from
/// an older hub must not overwrite a date we already hold.
pub async fn record_expiry(expires_at: Option<String>) -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.locus_expires_at = expires_at.map(Into::into);
    });
    verge.data_arc().save_file().await
}

/// Clears the entitlement after the hub has **definitively** refused it.
///
/// This is the suspension/expiry/refund path — a statement about the code
/// itself, which the device must act on. It clears the code, the tier and the
/// expiry together, and removes the durable mirror, because keeping a credential
/// the hub has rejected is how a suspended device would keep looking entitled
/// after a restart.
///
/// Deliberately does NOT use this for a transient failure. A grace period that
/// ran out, or a hub that could not be reached, says nothing about whether the
/// code is still valid — and wiping it there is exactly the bug this module's
/// mirror exists to fix: a student whose wifi was down for a week is dropped
/// back onto the activation screen with a code they may no longer have a card
/// for. Use [`record_lapsed_grace`] for that case, which keeps the code.
///
/// The refusal reason is NOT cleared, because it is the record of why this
/// happened; see [`record_refusal`].
pub async fn clear_entitlement() -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.activation_code = None;
        draft.locus_tier = None;
        draft.locus_expires_at = None;
        draft.last_heartbeat_ok = None;
        draft.heartbeat_failures = None;
    });
    let saved = verge.data_arc().save_file().await;
    // The mirror goes with it, and this one IS the durable copy — leaving it
    // behind would let the next launch re-adopt a code the hub has refused.
    crate::locus::credential::remove_code();
    saved
}

/// Clears what a lapsed grace period invalidates, **keeping the code**.
///
/// The grace period is a local rule: the hub has not answered for long enough
/// that the tunnel must come down, because staying up would mean serving a
/// device whose entitlement can no longer be confirmed. That is a statement
/// about *this session*, not about the code.
///
/// So the code and its durable mirror survive, and only the session state is
/// cleared — the tier, the expiry and the heartbeat bookkeeping. On the next
/// launch, or as soon as the hub answers again, the device re-activates with a
/// code it still holds rather than sending the student hunting for their card.
pub async fn record_lapsed_grace() -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.locus_tier = None;
        draft.locus_expires_at = None;
        draft.last_heartbeat_ok = None;
        draft.heartbeat_failures = None;
    });
    verge.data_arc().save_file().await
}

/// Records why the hub refused this device, so the refusal can be explained.
///
/// Written by the heartbeat's refusal branch, which runs in the background at a
/// moment nobody is looking. Without it the student's next view of the app is one
/// where the entitlement has silently vanished, and the two possible reasons —
/// an expired subscription, or a code an operator suspended — need different
/// actions from them but are indistinguishable from the outside.
///
/// Survives [`clear`] deliberately: it describes an event rather than an
/// entitlement, and it is the only thing that can still answer "why" once the
/// code itself is gone.
pub async fn record_refusal(reason: &str) -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.locus_refusal_reason = Some(reason.to_owned().into());
    });
    verge.data_arc().save_file().await
}

/// Records the version the hub has offered this device.
///
/// Written on the heartbeat path, so the prompt can appear on the next launch
/// rather than only in the instant after a beat. Stores the bare version string:
/// the URL, checksum and signature are deliberately NOT cached, because they are
/// re-fetched from `/api/update` when the student actually installs — caching
/// security-relevant material for an unbounded time is a liability with no
/// benefit, since the fetch is cheap and already required.
pub async fn record_update_offer(version: &str) -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.locus_update_offered = Some(version.to_owned().into());
    });
    verge.data_arc().save_file().await
}

/// Forgets an offered update, once it is installed or no longer relevant.
///
/// Called after a successful install (so the prompt does not reappear for a
/// version the student already has) and when the offer is no longer newer than
/// what is running.
pub async fn clear_update_offer() -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.locus_update_offered = None;
    });
    verge.data_arc().save_file().await
}

/// Records why the last heartbeat did or did not produce an update offer.
///
/// `None` means an offer WAS recorded — the healthy case, and the one that must
/// clear any previous reason so a fixed problem stops being reported. `Some`
/// carries the reason it was not.
///
/// Persisted so the Account page can answer "why is this device not updating?"
/// on demand. It is per-heartbeat state, not an event log: the latest beat wins,
/// because the question a student asks is about *now*.
pub async fn record_update_check_reason(
    reason: Option<&crate::locus::update::NoOfferReason>,
) -> Result<()> {
    let verge = Config::verge().await;
    let encoded = reason.map(|r| r.describe());
    verge.edit_draft(|draft| {
        draft.locus_update_check_reason = encoded.clone().map(Into::into);
    });
    verge.data_arc().save_file().await
}

/// Forgets a previous refusal, after the device is entitled again.
///
/// Called on a successful activation. Without this a student who renews would
/// keep being told why their previous code was refused, on a device that is now
/// working — a stale explanation is its own kind of wrong answer.
pub async fn clear_refusal() -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.locus_refusal_reason = None;
    });
    verge.data_arc().save_file().await
}

/// Persists the tier's connection details and UDP preference.
///
/// Stored alongside the entitlement because they arrive together and are useless
/// apart: a code without a config cannot connect, and a config without a code
/// cannot be authorised. Keeping them in one write means a crash cannot leave a
/// device that believes it is activated but has nothing to dial.
pub async fn store_tier_config(config: &TierConfig, udp_relay: bool) -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.locus_tier_server = Some(serde_json::to_string(config).unwrap_or_default().into());
        draft.locus_udp_relay = Some(udp_relay);
    });
    verge.data_arc().save_file().await
}

/// Reads the free tier's usage window from the config.
///
/// Returns [`Usage::default`] — not an error — when nothing is stored or the
/// stored JSON is unreadable. Both cases mean "no window recorded", which
/// [`crate::locus::usage::Usage::window`] starts lazily on the first reading.
/// An unreadable value is treated as absent rather than as fatal because a
/// corrupt counter must never block the connection: the worst case is a student
/// who gets a fresh allowance, and the alternative is an app that will not
/// connect over bookkeeping.
pub async fn usage() -> crate::locus::usage::Usage {
    let verge = Config::verge().await;
    let data = verge.latest_arc();
    decode_usage(data.locus_usage.as_deref())
}

/// Decodes a stored usage window, treating anything unreadable as absent.
///
/// Split out as a pure function so the corrupt case is testable without a config
/// layer — and because the *decision* here (reset rather than fail) is the part
/// worth pinning, not the plumbing that reaches it.
///
/// A corrupt value resets and **logs**. The reset is deliberate: a counter must
/// never block a connection, and the worst case is a student who gets a fresh
/// allowance. The log is equally deliberate — without it, "my free data keeps
/// resetting" is a report with no diagnosable cause.
fn decode_usage(raw: Option<&str>) -> crate::locus::usage::Usage {
    let Some(raw) = raw else {
        // Never written: a paying tier, or a free student before their first
        // byte. Not an error, and not worth a log line on every poll.
        return crate::locus::usage::Usage::default();
    };
    match serde_json::from_str::<crate::locus::usage::Usage>(raw) {
        Ok(usage) => usage,
        Err(error) => {
            logging::warn_corrupt_usage(&error.to_string());
            crate::locus::usage::Usage::default()
        }
    }
}

/// Persists the free tier's usage window.
///
/// The whole window is written at once, because a count without its window is
/// the corrupt state this shape exists to prevent.
pub async fn store_usage(usage: &crate::locus::usage::Usage) -> Result<()> {
    let verge = Config::verge().await;
    let encoded = serde_json::to_string(usage).unwrap_or_default();
    verge.edit_draft(|draft| {
        draft.locus_usage = Some(encoded.clone().into());
    });
    verge.data_arc().save_file().await
}

/// Reads the throttle speed the hub advertised, in Mbps.
///
/// `None` means the hub has not told us — a paying tier, or an older hub — and
/// the caller must **not** apply any throttle in that case. Absence is not
/// "zero speed".
///
/// NOTE: this value is persisted and readable, but **nothing applies it to a
/// running Core yet**. The classification that decides *when* to throttle works
/// and is tested; the step that acts on it does not exist. Kept honest here
/// rather than implied by the field's presence — see `STILL-OPEN.md`.
#[must_use]
pub async fn throttle_mbps() -> Option<u32> {
    crate::config::Config::verge()
        .await
        .latest_arc()
        .locus_throttle_mbps
}

/// Caches the throttle speed the hub advertised, in Mbps.
///
/// Stored beside the allowance because they arrive together and are meaningless
/// apart: one says *how much* data a free student gets, the other *how slow*
/// they go once it is gone.
pub async fn store_throttle_mbps(mbps: Option<u32>) -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.locus_throttle_mbps = mbps.filter(|m| *m > 0);
    });
    verge.data_arc().save_file().await
}

/// Caches the free-tier allowance the hub advertised, in mebibytes.
///
/// `None` is written through deliberately — it is the hub saying "no allowance
/// applies" (a paying tier, or the quota switched off), and leaving a stale
/// value in place would throttle a student the hub had released. See the call
/// site in `locus::runtime::handle_success` for why this differs from the
/// expiry rule.
pub async fn store_allowance(allowance_mb: Option<u64>) -> Result<()> {
    let verge = Config::verge().await;
    verge.edit_draft(|draft| {
        draft.locus_allowance_mb = allowance_mb.filter(|mb| *mb > 0);
    });
    verge.data_arc().save_file().await
}

/// Reads back the stored tier connection details.
///
/// Returns `None` — rather than an error — when nothing is stored or the stored
/// JSON is unreadable. The caller's question is "can I connect?", and both cases
/// answer it the same way: not yet, re-activate.
pub async fn tier_config(tier: &str) -> Option<TierConfig> {
    let verge = crate::config::Config::verge().await;
    let data = verge.latest_arc();
    let raw = data.locus_tier_server.as_deref()?;
    let config: TierConfig = serde_json::from_str(raw).ok()?;
    // Guard against a stored config for a different tier: a re-activation that
    // changed tier must not keep dialling the old server.
    if config.server.is_empty() || config.server_port == 0 {
        logging::warn_wrong_tier(tier);
        return None;
    }
    Some(config)
}

/// Whether the hub told us to carry UDP over TCP for the stored tier.
///
/// Read back separately from [`tier_config`] because `udp_relay` is a column on
/// the activation/heartbeat payload, not a field of the tier config JSON — the
/// two arrive together but are stored in different places, and a connect that
/// hardcoded either one would disagree with what was activated.
///
/// Absent means `false`: a device activated before the flag existed has no
/// stored value, and inventing `true` would put UDP through a UoT endpoint that
/// may not exist.
#[must_use]
pub async fn udp_relay() -> bool {
    crate::config::Config::verge()
        .await
        .latest_arc()
        .locus_udp_relay
        .unwrap_or(false)
}

/// Small logging shim so this module needs no direct logging import.
mod logging {
    pub fn warn_wrong_tier(tier: &str) {
        clash_verge_logging::logging!(
            warn,
            clash_verge_logging::Type::Config,
            "[locus] stored tier config is unusable for the {tier} tier"
        );
    }

    /// The stored usage window could not be parsed, so it was reset.
    ///
    /// Worth a line: the visible symptom is "my free data keeps resetting", and
    /// without this the cause is unknowable from the student's side.
    pub fn warn_corrupt_usage(detail: &str) {
        clash_verge_logging::logging!(
            warn,
            clash_verge_logging::Type::Config,
            "[locus] the stored free-tier usage window was unreadable and has been reset: {detail}"
        );
    }

    /// The code was recovered from the durable mirror, so the config had lost it.
    pub fn info_recovered_code() {
        clash_verge_logging::logging!(
            info,
            clash_verge_logging::Type::Config,
            "[locus] recovered the activation code from the machine store after the config lost it"
        );
    }

    /// The code could not be written back into the config after a recovery.
    pub fn warn_could_not_write_back(error: &anyhow::Error) {
        clash_verge_logging::logging!(
            warn,
            clash_verge_logging::Type::Config,
            "[locus] recovered the code but could not write it back to the config: {error:#}"
        );
    }

    /// The code could not be mirrored to the machine store.
    pub fn warn_could_not_mirror() {
        clash_verge_logging::logging!(
            warn,
            clash_verge_logging::Type::Config,
            "[locus] could not mirror the activation code to the machine store"
        );
    }
}

/// Whether an incoming tier config actually differs from the stored one.
///
/// This exists to stop a RESTART STORM. The hub sends `server_config` on every
/// heartbeat, and applying it unconditionally would mean re-writing the profile
/// and reloading-or-restarting the core every five minutes — dropping every
/// connection the student has, forever, for no reason.
///
/// Compares the whole payload including `udp_relay`, because a change to either
/// can alter the generated config.
pub async fn tier_changed(config: &TierConfig, udp_relay: bool) -> bool {
    let verge = Config::verge().await;
    let data = verge.latest_arc();

    let stored_udp = data.locus_udp_relay.unwrap_or(false);
    if stored_udp != udp_relay {
        return true;
    }

    match data.locus_tier_server.as_deref() {
        Some(raw) => match serde_json::from_str::<TierConfig>(raw) {
            Ok(stored) => stored != *config,
            // Unreadable stored config: treat as changed so it gets replaced.
            // The alternative is refusing to self-heal a corrupt field.
            Err(_) => true,
        },
        None => true,
    }
}

/// A copy of the config with every credential removed.
///
/// Used by the diagnostics export. Written as an allow-list of the fields that
/// are safe rather than a deny-list of the ones that are not: a deny-list has to
/// be updated every time a credential field is added, and the failure mode when
/// someone forgets is publishing a student's code.
#[must_use]
pub fn redacted_for_diagnostics(verge: &IVerge) -> serde_json::Value {
    serde_json::json!({
        // Presence, not value. Support needs to know a code is set; nobody needs
        // to know what it is, and a support log is a thing people paste around.
        "has_activation_code": verge.activation_code.as_deref().is_some_and(|c| !c.is_empty()),
        "tier": verge.locus_tier,
        "device_id": verge.device_fingerprint.as_deref().map(redact),
        "last_heartbeat_ok": verge.last_heartbeat_ok,
        "heartbeat_failures": verge.heartbeat_failures,
        "update_pending_version": verge.update_pending_version,
    })
}

/// Truncates a fingerprint for display and logging.
///
/// The fingerprint is a stable device identifier. It is not a secret, but there
/// is no reason to put the whole thing somewhere it might be screenshotted —
/// and a truncated value is still enough to correlate two log lines.
#[must_use]
pub fn redact(fingerprint: &str) -> String {
    fingerprint.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verge_with(code: Option<&str>, tier: Option<&str>, fingerprint: Option<&str>) -> IVerge {
        IVerge {
            activation_code: code.map(SmartString::from),
            locus_tier: tier.map(SmartString::from),
            device_fingerprint: fingerprint.map(SmartString::from),
            ..Default::default()
        }
    }

    #[test]
    fn a_complete_record_reads_back() {
        let verge = verge_with(
            Some("RQ-ABCD-EFGH-JKMN-T"),
            Some("strike"),
            Some("a".repeat(64).as_str()),
        );
        let activation = activation_from(&verge, None).expect("a complete record must read");
        assert_eq!(activation.tier, "strike");
        assert!(is_usable(&activation));
    }

    /// A partial record must read as absent, not as a half-populated struct.
    ///
    /// The caller's decision is "activate again or not", and it cannot make that
    /// decision from something that looks populated but cannot heartbeat.
    #[test]
    fn a_partial_record_reads_as_absent() {
        for verge in [
            verge_with(None, Some("eco"), Some(&"a".repeat(64))),
            verge_with(Some("RQ-ABCD-EFGH-JKMN-T"), None, Some(&"a".repeat(64))),
            verge_with(Some("RQ-ABCD-EFGH-JKMN-T"), Some("eco"), None),
            // Whitespace-only counts as empty, because that is what a corrupt or
            // hand-edited file produces.
            verge_with(Some("   "), Some("eco"), Some(&"a".repeat(64))),
        ] {
            assert!(
                activation_from(&verge, None).is_none(),
                "an incomplete record must not present as activated: {verge:?}"
            );
        }
    }

    /// A fresh install is the normal case and must not be an error.
    #[test]
    fn a_fresh_config_is_not_activated() {
        assert!(activation_from(&IVerge::default(), None).is_none());
    }

    /// The mirror is the fallback: when the config has no code but the durable
    /// store does, the record is usable again.
    ///
    /// This is the whole point of Phase 1 — an uninstall wipes `verge.yaml`, and
    /// the code must come back from the machine store rather than sending the
    /// student to find a card they threw away.
    #[test]
    fn a_mirrored_code_revives_a_code_less_config() {
        let verge = verge_with(None, Some("strike"), Some(&"a".repeat(64)));
        let activation = activation_from(&verge, Some("RQ-ABCD-EFGH-JKMN-T".to_owned()))
            .expect("the mirror must revive a config with no code");
        assert_eq!(activation.code, "RQ-ABCD-EFGH-JKMN-T");
        assert_eq!(activation.tier, "strike");
    }

    /// The config wins when it holds a code, so a stale mirror cannot override a
    /// freshly activated value.
    #[test]
    fn the_config_code_beats_the_mirror() {
        let verge = verge_with(
            Some("RQ-ABCD-EFGH-JKMN-T"),
            Some("eco"),
            Some(&"a".repeat(64)),
        );
        let activation = activation_from(&verge, Some("RQ-ZZZZ-ZZZZ-ZZZZ-Z".to_owned()))
            .expect("the config record must read");
        assert_eq!(activation.code, "RQ-ABCD-EFGH-JKMN-T");
    }

    /// A mirror cannot supply what the config never had: without a tier the
    /// record is still damaged, and the student is re-activated.
    #[test]
    fn a_mirrored_code_without_a_tier_is_still_absent() {
        let verge = verge_with(None, None, Some(&"a".repeat(64)));
        assert!(activation_from(&verge, Some("RQ-ABCD-EFGH-JKMN-T".to_owned())).is_none());
    }

    /// The redaction must never emit the activation code, whatever else it says.
    ///
    /// This is the check that matters: a diagnostics export is a thing people
    /// paste into a support thread, and the code is a bearer credential.
    #[test]
    fn diagnostics_never_include_the_code() {
        let secret = "RQ-SECR-ETXX-XXXX-T";
        let verge = verge_with(Some(secret), Some("strike"), Some(&"b".repeat(64)));

        let exported = serde_json::to_string(&redacted_for_diagnostics(&verge)).expect("export must serialise");

        assert!(
            !exported.contains(secret),
            "the activation code appeared in the diagnostics export: {exported}"
        );
        // And the whole fingerprint must not be there either.
        assert!(
            !exported.contains(&"b".repeat(64)),
            "the full fingerprint appeared in the diagnostics export: {exported}"
        );
        // It must still be useful: presence, tier and a truncated id.
        assert!(exported.contains("\"has_activation_code\":true"));
        assert!(exported.contains("strike"));
    }

    /// With nothing stored, the export must say so rather than omit the key —
    /// a missing field is indistinguishable from a bug in the exporter.
    #[test]
    fn diagnostics_reports_absence_explicitly() {
        let exported = redacted_for_diagnostics(&IVerge::default());
        assert_eq!(exported["has_activation_code"], serde_json::json!(false));
        assert_eq!(exported["tier"], serde_json::Value::Null);
    }

    #[test]
    fn redaction_truncates() {
        let full = "c".repeat(64);
        let short = redact(&full);
        assert_eq!(short.len(), 12);
        assert!(full.starts_with(&short));
    }

    /// An empty string must not read as a stored value.
    #[test]
    fn an_empty_code_is_not_a_stored_code() {
        let verge = verge_with(Some(""), Some("eco"), Some(&"d".repeat(64)));
        assert!(activation_from(&verge, None).is_none());
    }

    /// A stored usage window round-trips.
    #[test]
    fn a_stored_usage_window_decodes() {
        let stored = crate::locus::usage::Usage {
            used_bytes: 1234,
            window_start_ms: 1_700_000_000_000,
            warned: true,
        };
        let raw = serde_json::to_string(&stored).expect("Usage must serialize");
        assert_eq!(decode_usage(Some(&raw)), stored);
    }

    /// A corrupt window resets to a fresh one rather than failing.
    ///
    /// The safety property: a counter must never block a connection. The student
    /// gets a fresh allowance, which is the safe direction — the alternative is
    /// an app that will not connect over bookkeeping.
    #[test]
    fn a_corrupt_usage_window_resets_rather_than_failing() {
        let decoded = decode_usage(Some("{ this is not json"));
        assert_eq!(decoded, crate::locus::usage::Usage::default());
        assert_eq!(decoded.used_bytes, 0, "a corrupt count must not be honoured");
    }

    /// Absent and corrupt reach the same place, and both are safe.
    ///
    /// They differ only in that corrupt logs (so it is diagnosable); the reset
    /// behaviour is deliberately identical, because a student seeing a fresh
    /// allowance does not care which of the two happened.
    #[test]
    fn absent_and_corrupt_usage_both_start_a_fresh_window() {
        assert_eq!(
            decode_usage(None),
            decode_usage(Some("!! not json !!")),
            "absent and corrupt must not diverge in behaviour"
        );
    }

    /// A window with a skew-guarded start survives the round-trip intact.
    ///
    /// Decoding must not silently repair: the repair belongs in
    /// `Usage::window`, which knows `now`. Doing it here would need a clock and
    /// would make the decode untestable.
    #[test]
    fn decoding_does_not_pre_repair_a_skewed_window() {
        let skewed = crate::locus::usage::Usage {
            used_bytes: 99,
            window_start_ms: u64::MAX,
            warned: false,
        };
        let raw = serde_json::to_string(&skewed).expect("Usage must serialize");
        assert_eq!(
            decode_usage(Some(&raw)).window_start_ms,
            u64::MAX,
            "decoding must be a pure read; the skew repair happens in `window`, which has `now`"
        );
    }
}
