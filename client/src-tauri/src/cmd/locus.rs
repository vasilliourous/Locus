//! Tauri commands for the Locus product surface.
//!
//! Every command is thin: it gathers inputs, calls into [`crate::locus`], and
//! returns a **typed** result. The typing is deliberate — the retired client
//! returned strings, so the UI could not tell "code already used on another
//! device" from "we could not reach the hub", and every distinct situation
//! collapsed into one unhelpful message. A student whose code is bound to their
//! old laptop needs the former, not "activation failed".
//!
//! The commands are also the seam where persistence and the core meet. Nothing
//! in `locus/` writes config or starts the core; this module owns that wiring so
//! the pure logic stays testable without an app handle.

use super::CmdResult;
use crate::config::Config;
use crate::core::CoreManager;
use crate::core::manager::traffic_probe;
use crate::locus::{activation, apply, contract, store};
use crate::utils::dirs;
use clash_verge_logging::{Type, logging};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Emitter as _;

/// How long a connect may spend waiting for the Core to become ready.
///
/// Bounds the wait so a Core that starts and then wedges cannot leave the button
/// spinning forever — the retired client's failure, where the only recovery was
/// to kill the app. The value is generous because the honest cost is dominated by
/// the engine dialling the server over a school network, and a premature timeout
/// would report failure for a tunnel that was about to work.
const CONNECT_READY_BUDGET: Duration = Duration::from_secs(30);

/// How often the readiness loop re-checks while a connect is in flight.
const CONNECT_READY_INTERVAL: Duration = Duration::from_millis(250);

/// Set by [`locus_cancel_connect`] to abandon an in-flight connect.
///
/// A plain flag rather than a channel: there is exactly one connect at a time
/// (the UI cannot double-fire it, and the loop below clears it), so the only
/// thing this needs to express is "stop", not "stop which one".
static CANCEL_CONNECT: AtomicBool = AtomicBool::new(false);

/// Reads and clears the cancellation request.
///
/// Clearing is part of reading, deliberately: the flag describes one in-flight
/// connect, and a value left behind would cancel the next one the student asked
/// for. Keeping it in one function means no caller can forget.
fn take_cancellation() -> bool {
    CANCEL_CONNECT.swap(false, Ordering::AcqRel)
}

/// Whether the app has an activation to work with, and what it is.
///
/// The single call the UI polls. Deliberately small and cheap: the activation
/// gate renders on every launch, and a slow first paint is the first thing a
/// student notices.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocusStatus {
    /// Whether this device has a code bound and accepted.
    pub activated: bool,
    /// The tier name (`eco`, `stealth`, `strike`), when known.
    pub tier: Option<String>,
    /// The device fingerprint, **truncated** — enough for support to correlate,
    /// not enough to be a useful identifier if someone screenshots it.
    pub device_id: String,
    /// Which platform build this is, so a support report says so without asking.
    pub platform: Option<String>,
    /// The running app version.
    pub version: String,
    /// Whether the tunnel's core is actually running right now.
    ///
    /// This is the authoritative answer to "is the VPN up?", read from the same
    /// run state the core manager publishes — **not** inferred by the UI from the
    /// fact that a connect command once returned `Ok`.
    ///
    /// It exists because the UI could not answer that question and guessed
    /// instead. `locus_status` carried only entitlement (`activated`), so after a
    /// successful connect the connection screen's periodic status re-read found
    /// `activated == true` and concluded "not connected" — the traffic panel the
    /// student had just earned flashed on for one frame and vanished, and every
    /// poll reset it again. A field the backend owns is the fix: the screen can
    /// now render what is true rather than what it last remembers deciding.
    ///
    /// `NotRunning` is the only "down" answer.
    pub connected: bool,

    /// Whether the core is up *and* ready to carry traffic.
    ///
    /// Deliberately separate from [`Self::connected`]. `connected` means the
    /// engine process exists; this means it has bound its ports and can route a
    /// packet. The gap between them is seconds long on a real connection, and
    /// reporting "connected" across it is what made the button lie: a student who
    /// pressed Connect and immediately re-checked their address was told they
    /// were not connected by a tunnel that was merely still dialling.
    ///
    /// The screen renders its state from THIS field. `connected` remains what it
    /// always was — an honest statement that a process is running — and the two
    /// disagreeing is meaningful rather than a bug.
    ///
    /// Since the egress fix, `ready` means the core is up **and a real request
    /// completed through the tunnel**. A core that is running on a machine with
    /// no usable uplink — or pointed at an unreachable server, or behind a
    /// captive portal — reports `ready: false` while `connected: true`. That is
    /// the honest answer, and the one the screen must not round up to
    /// "connected".
    pub ready: bool,

    /// Whether the core is up, regardless of whether traffic is flowing.
    ///
    /// This is the middle state between "nothing running" and "connected":
    /// `connected && coreUp && !ready` is precisely *the tunnel exists but the
    /// internet is not reachable through it*. It is what lets the screen say
    /// "connected, but no traffic" instead of either claiming success or
    /// claiming the app is off. `false` whenever the core is not serving.
    pub core_up: bool,

    /// The subscription state, already parsed — see [`SubscriptionStatus`].
    ///
    /// Pre-parsed rather than handed to the UI as a raw date string, because the
    /// "never invent a date" rule needs exactly one decision point. If the
    /// frontend received the raw string it would have to decide what an empty
    /// value means, and each screen could decide differently.
    pub subscription: SubscriptionStatus,

    /// Unix seconds of the last heartbeat the hub accepted, when there has been.
    ///
    /// Exposed so the Account screen can answer "how current is this?" — a
    /// student who has just renewed needs to know whether the app has actually
    /// confirmed it, and "expires in 30 days" reads identically whether it was
    /// confirmed a minute ago or a week ago. Without this the only honest answer
    /// to "did my renewal land?" is a guess.
    ///
    /// `None` means no beat has ever succeeded — which is a different statement
    /// from "confirmed zero seconds ago", and rendering one as the other would
    /// tell a fresh install its status was just verified.
    pub last_confirmed_at: Option<i64>,

    /// Whether the Core is observed to be moving bytes right now.
    ///
    /// The second route to `ready`, reported alongside it rather than folded into
    /// it, for the same reason `ready` is separate from `connected`: the two
    /// routes fail for different reasons and a support report has to say which
    /// one answered. `ready: true, traffic: false` is a tunnel proven by a delay
    /// test on an idle machine; `ready: true, traffic: true` is the student's own
    /// bytes moving. Both are connected, and only one of them can be wrong in the
    /// direction that produced the recurring report — so this field is what lets
    /// the next investigation tell them apart without a debug build.
    ///
    /// It is a *rate*, not the Core's lifetime total: see
    /// `core::manager::traffic_probe`.
    pub traffic_flowing: bool,
}

/// What the Account screen should say about the subscription.
///
/// A closed set of cases rather than an optional date, because "we do not know"
/// and "it has lapsed" are different answers with different copy, and collapsing
/// them into one nullable field is how a blank or wrong date reaches a paying
/// student.
///
/// `rename_all` on an enum renames the *variants*, not the fields inside them, so
/// the struct-variant fields need `rename_all_fields` to reach the frontend as
/// `daysRemaining`/`expiresAt`. Without it serde emits `days_remaining` and
/// `expires_at`, the TypeScript type reads `undefined` for both, and the UI
/// renders "{{count}} days" with an empty count — or nothing at all. A test
/// below pins the wire shape and fails if this attribute is dropped.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SubscriptionStatus {
    /// The hub has not told us. Render nothing about expiry.
    Unknown,
    /// The date has passed.
    Lapsed,
    /// Valid, with whole days remaining and the date for a tooltip.
    Active {
        /// Whole days remaining, rounded up. `0` is possible only mid-day.
        days_remaining: i64,
        /// Whether this is inside the renewal-warning window.
        urgent: bool,
        /// The hub's own string, so the UI can show the exact date on hover
        /// without re-deriving it.
        expires_at: String,
    },
    /// The hub refused this device and the entitlement was withdrawn.
    ///
    /// Its own state, ahead of [`Unknown`](Self::Unknown), because after a
    /// refusal the client legitimately knows *nothing* about a subscription — the
    /// code is gone — and reporting that as `Unknown` would be technically true
    /// and completely useless. "We have nothing to show you" and "your access was
    /// ended, and here is why" call for different things on screen, and the
    /// second is the one a student can act on.
    ///
    /// Carries the hub's own sentence, so a suspension and an expiry are not
    /// flattened into one message when the student's next step differs.
    Refused {
        /// The hub's reason, verbatim. Never paraphrased: an operator's wording
        /// ("contact your middleman") is an instruction, not a description.
        reason: String,
    },
}

/// Maps stored state to what the Account screen should say about the subscription.
///
/// Split out of the command so the rules can be tested without an app handle —
/// `locus_status` reads global config, and a test that needed that would be
/// testing the config layer instead of these decisions.
///
/// The two rules that matter, and why each is a deliberate choice:
///
///  * **Unactivated implies Unknown, never Lapsed.** A fresh install has no
///    expiry recorded. Reporting that as "expired" would greet a new student
///    whose code is perfectly valid with a message telling them it had run out.
///  * **An unparseable or absent date implies Unknown.** Same reason, one step
///    further on: a hub that predates the field, or a code with no expiry set,
///    must show nothing rather than a guess.
#[must_use]
fn classify_subscription(
    activated: bool,
    raw_expiry: Option<&str>,
    refusal_reason: Option<&str>,
) -> SubscriptionStatus {
    if !activated {
        // A device with no entitlement, that the hub has refused, is not a blank
        // slate: the refusal is the whole answer to "why can I not connect".
        // Checked BEFORE the unactivated fallback, because a refusal always
        // leaves the device unactivated — testing `activated` first would make
        // this state unreachable and the reason invisible.
        if let Some(reason) = refusal_reason.filter(|r| !r.trim().is_empty()) {
            return SubscriptionStatus::Refused {
                reason: reason.trim().to_owned(),
            };
        }
        return SubscriptionStatus::Unknown;
    }

    match crate::locus::expiry::parse(raw_expiry) {
        crate::locus::expiry::Expiry::Unknown => SubscriptionStatus::Unknown,
        crate::locus::expiry::Expiry::Lapsed => SubscriptionStatus::Lapsed,
        crate::locus::expiry::Expiry::InDays { days, .. } => SubscriptionStatus::Active {
            days_remaining: days,
            urgent: days <= crate::locus::expiry::EXPIRY_WARNING_DAYS,
            // The hub's own string, so the tooltip can show the exact date
            // without the client re-deriving and possibly shifting it.
            expires_at: raw_expiry.unwrap_or_default().to_owned(),
        },
    }
}

/// Reads the current product state.
///
/// Never fails: a status query that can error forces the UI to invent a state,
/// and the honest answer when we cannot read storage is "not activated".
#[tauri::command]
pub async fn locus_status() -> LocusStatus {
    let verge = Config::verge().await;
    let verge_data = verge.latest_arc();

    // The device id reported for support is the value the code is BOUND to
    // (`identity.device_id`), not the hardware fingerprint.
    //
    // It used to be `device::fingerprint()`, on the reasoning that a derived
    // value is available before activation and would not "blank on a fresh
    // install". But support correlates this against `bound_fingerprint` on the
    // code, and the two disagreed the moment the hardware hash drifted — the
    // operator saw a fingerprint the hub had never bound, and the student was
    // told their code belonged to another device. Reporting the binding id makes
    // this column, the hub's column and the recognition row name one device.
    //
    // It is resolved from the identity stores, so it is available before
    // activation too: a fresh device resolves a `Fresh`/`AppFallback` identity
    // and gets an id, exactly as it got a hardware fingerprint before.
    let fingerprint = resolve_identity().await.binding_id();
    let activation = store::read(&verge_data);

    let subscription = classify_subscription(
        activation.is_some(),
        verge_data.locus_expires_at.as_deref(),
        verge_data.locus_refusal_reason.as_deref(),
    );

    // Observed, not assumed. The readiness latch answers "was a start
    // recorded", which stays true for a Core that has since died or that never
    // got a usable
    // network — the source of the client reporting "connected" with no wifi and
    // through a window of zero traffic. This asks the Core, and revokes the latch
    // if it does not answer, so "connected" means a packet can move.
    //
    // Bounded internally (`probe::PROBE_TIMEOUT`), so it cannot hang the status
    // command the UI polls at 750 ms while a connect settles.
    let readiness = CoreManager::global().observe_readiness().await;

    // Read for the report, after readiness has been decided: `observe_readiness`
    // is what starts and feeds the stream, so reading it here means the field
    // describes the same observation the verdict above was reached from.
    let traffic_flowing = traffic_probe::is_active();

    LocusStatus {
        activated: activation.is_some(),
        tier: activation.as_ref().map(|a| a.tier.clone()),
        device_id: store::redact(&fingerprint),
        platform: contract::current_platform().map(str::to_owned),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        connected: core_is_running(&crate::core::runstate::RUN_STATE.state()),
        ready: readiness.is_ready(),
        core_up: readiness.is_core_up(),
        subscription,
        last_confirmed_at: verge_data.last_heartbeat_ok,
        traffic_flowing,
    }
}
/// Whether the tunnel's core is running, from the run state's running mode.
///
/// Split out for the same reason as [`connect_allowed`]: the mapping is the
/// behaviour, and a test can pin it without an app handle. `NotRunning` is the
/// single "down" answer; anything else means the core is up.
#[must_use]
const fn core_is_running(state: &crate::core::runstate::RunState) -> bool {
    !matches!(state.mode, crate::core::manager::RunningMode::NotRunning)
}

/// Validates an activation code **offline**.
///
/// Called as the student types, so a typo is caught without a request. This is
/// also what protects them from the hub's rate limit: `/api/activate` allows 5
/// attempts per 10 minutes per address, and a student who mistypes four times
/// would otherwise lock themselves out of the one screen that could explain the
/// mistake.
#[tauri::command]
pub async fn locus_validate_code(code: String) -> ValidateCodeResult {
    match activation::validate_code(&code) {
        Ok(canonical) => ValidateCodeResult {
            valid: true,
            canonical: Some(canonical),
            message: None,
        },
        Err(error) => ValidateCodeResult {
            valid: false,
            canonical: None,
            message: Some(error.to_string()),
        },
    }
}

/// The result of the offline code check.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateCodeResult {
    pub valid: bool,
    /// The canonical hyphenated form, so the UI can show what will be sent.
    pub canonical: Option<String>,
    pub message: Option<String>,
}

/// Asks the hub whether a code is usable, without binding anything.
///
/// Lets the student learn their code is real *before* committing to an
/// activation that binds their device. A transport failure is returned as an
/// error rather than as "your code is bad", because those are different
/// situations and only one of them is the student's fault.
#[tauri::command]
pub async fn locus_check_code(code: String) -> CmdResult<activation::CodeCheck> {
    // The lookup is advisory and binds nothing, but it must ask about the SAME
    // device the activation will bind — otherwise a code that is this device's
    // own binding would be reported as held by another. Use the binding id.
    let fingerprint = resolve_identity().await.binding_id();
    activation::lookup_code(&code, &fingerprint)
        .await
        .map_err(|error| super::coded_error("LOCUS_LOOKUP_FAILED", format!("{error:#}")))
}

/// What a first-launch recognition check concluded.
///
/// Deliberately not a `bool`. The three cases lead to different UI:
///   * [`Self::Recognised`] skips the code prompt entirely;
///   * [`Self::UnknownDevice`] shows the prompt with no comment;
///   * [`Self::Unavailable`] shows the prompt *and* can explain that we could not
///     reach the hub — which is not the student's fault and must not be reported
///     as "your device is not recognised".
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RecognitionResult {
    /// The hub knows this device and it holds a live entitlement.
    Recognised {
        tier: String,
        expires_at: Option<String>,
        /// Whether this device's identity survives a reinstall. The UI must not
        /// promise durability the install does not have.
        durable: bool,
    },
    /// The hub answered, and does not know this device.
    UnknownDevice,
    /// We could not ask. Not a refusal — see the type docs.
    Unavailable,
}

/// Resolves this device's identity, creating and persisting one if needed.
///
/// The order is the fix for the reported bug:
///
///   1. Build the identity from whatever is stored — machine store first, then
///      the app-config fallback — plus the machine id.
///   2. If it was freshly generated, persist it. The machine store is tried
///      first because it is the one that survives a reinstall; only if that
///      fails do we fall back to the app config, and the identity records which
///      happened so the UI can be honest about it.
///
/// Returned to the caller rather than cached in a `OnceLock` because the store
/// it resolved from is part of the value: a device whose identity lives only in
/// the app config is a materially different situation, and a process-lifetime
/// cache would flatten the two together.
async fn resolve_identity() -> crate::locus::identity::DeviceIdentity {
    let app_stored = {
        let verge = Config::verge().await;
        store::read_identity(&verge.latest_arc())
    };

    let identity = crate::locus::identity::resolve_with_stores(
        crate::locus::identity::read_machine_id(),
        app_stored,
    );

    // Persist into the app config whenever the identity is not machine-backed.
    // This covers two cases: a fresh identity the machine store refused, and an
    // identity read back from the app fallback (rewriting it is harmless and
    // keeps the record current).
    if !identity.store.survives_reinstall()
        && let Err(error) = store::store_identity(&identity).await
    {
        // Not fatal. The identity still works for this session; it simply
        // will not survive a restart. Logged rather than surfaced, because
        // a storage failure is not something a student can act on.
        logging!(
            warn,
            Type::Config,
            "[locus] could not persist the device identity to the app config: {error:#}"
        );
    }

    identity
}

/// Asks the hub whether this device already holds an entitlement.
///
/// This is what lets a reinstalling student skip the code prompt. It is safe to
/// call on every launch: it never binds, never activates, and never returns the
/// activation code — the hub authenticates on a verifier the device proves it
/// holds, and answers only whether *this* device has a live tier.
///
/// Every failure path falls through to the code prompt, so a student holding
/// their card is never worse off than before this existed. That is the property
/// that makes it safe to add: the only way recognition can hurt is if it wrongly
/// claims a device is known, and only the hub can say that.
#[tauri::command]
pub async fn locus_recognise() -> RecognitionResult {
    let identity = resolve_identity().await;

    match activation::recognise(&identity).await {
        Ok(activation::Recognition::Recognised {
            tier,
            expires_at,
            store: _,
            config,
            udp_relay,
            token,
        }) => {
            // Store the session token FIRST, because `runtime::start` reads it
            // back out to authenticate: without it the loop would refuse to
            // start, and the device would be connected but never checking in.
            if let Err(error) = store::store_device_token(&token, None).await {
                logging!(
                    warn,
                    Type::Cmd,
                    "[locus] recognised but could not store the session token: {error:#}"
                );
                // Not recognised after all: a device that cannot authenticate is
                // not a restored device, and reporting success would leave the
                // student with a working-looking app that never re-syncs.
                return RecognitionResult::UnknownDevice;
            }

            // Persist the entitlement. The code is EMPTY on purpose — this device
            // has none, which is the point of recognition. `store::read` is not
            // consulted here because a token-authenticated device is a shape it
            // predates; `locus_status` learns about it through the tier instead.
            if let Err(error) = store::store(&store::Activation {
                code: String::new(),
                tier: tier.clone(),
                fingerprint: identity.device_id.clone(),
            })
            .await
            {
                logging!(
                    warn,
                    Type::Cmd,
                    "[locus] could not store the recognised entitlement: {error:#}"
                );
                return RecognitionResult::UnknownDevice;
            }

            // A previous refusal is now history: the hub accepted this device.
            // Best-effort, so a failure here cannot undo a successful restore.
            if let Err(error) = store::clear_refusal().await {
                logging!(warn, Type::Cmd, "[locus] could not clear a previous refusal: {error:#}");
            }

            // The expiry, so the account screen shows the date immediately rather
            // than waiting for the first beat.
            if expires_at.is_some()
                && let Err(error) = store::record_expiry(expires_at.clone()).await
            {
                logging!(warn, Type::Cmd, "[locus] could not store the expiry: {error:#}");
            }

            // The tier's connection details, so Connect has something to dial.
            // Without a config the device is recognised but cannot connect, and
            // that is reported as "not recognised" rather than presented as a
            // working app that reaches nothing.
            let Some(tier_config) = config.as_ref() else {
                logging!(
                    warn,
                    Type::Cmd,
                    "[locus] recognised device has no tier config for {tier}; cannot restore"
                );
                return RecognitionResult::UnknownDevice;
            };
            if let Err(error) = store::store_tier_config(tier_config, udp_relay).await {
                logging!(
                    warn,
                    Type::Cmd,
                    "[locus] could not store the tier config: {error:#}"
                );
                return RecognitionResult::UnknownDevice;
            }
            match apply::apply_tier(tier_config, udp_relay).await {
                Ok(apply::ApplyOutcome::Rejected { reason }) => {
                    logging!(warn, Type::Cmd, "[locus] restored tier config rejected: {reason}");
                }
                Err(error) => {
                    logging!(warn, Type::Cmd, "[locus] could not apply the restored tier: {error:#}");
                }
                Ok(_) => {}
            }

            logging!(
                info,
                Type::Config,
                "[locus] device recognised; restored the {tier} entitlement"
            );
            RecognitionResult::Recognised {
                tier,
                expires_at,
                durable: identity.store.survives_reinstall(),
            }
        }
        Ok(activation::Recognition::NotRecognised) => RecognitionResult::UnknownDevice,
        Err(error) => {
            // A hub we cannot reach is NOT a refusal, and must not be reported
            // as one. The student is shown the code prompt either way, but only
            // this branch is a problem worth logging.
            logging!(
                warn,
                Type::Config,
                "[locus] could not check whether this device is recognised: {error:#}"
            );
            RecognitionResult::Unavailable
        }
    }
}

/// Activates this device, persists the result, and configures the tunnel.
///
/// The order matters and is the whole point of this function:
///
///   1. validate locally (free, and catches typos)
///   2. ask the hub
///   3. **persist only on success** — a half-written activation is worse than
///      none, because the UI would show "activated" on a device the hub has
///      never heard of
///   4. write the tier config and apply it
///
/// A failure at any step leaves the previous state untouched.
#[tauri::command]
pub async fn locus_activate(code: String) -> CmdResult<ActivationResult> {
    // The durable identity, resolved FIRST, because it is also what the code is
    // bound to. `identity.binding_id()` is the `device_id`, which is persisted —
    // so the value the hub binds is stable across launches and updates.
    //
    // This used to send `device::fingerprint()` (a hardware hash re-derived on
    // every launch) as the binding value *and* register the identity separately.
    // Those are two different keys, so the hub bound the code to one and stored
    // recognition against the other: a hardware change moved the fingerprint
    // without moving `device_id`, and the student's own code came back
    // `403 "Code bound to another device"`. Binding to the identity is the fix —
    // the binding and the recognition row now name the same device.
    //
    // See `DeviceIdentity::binding_id` for the full argument.
    let identity = resolve_identity().await;
    let fingerprint = identity.binding_id();
    let identity_wire = activation::IdentityWire {
        verifier: identity.verifier(),
        device_id: identity.device_id.clone(),
        store: activation::store_label(identity.store).to_owned(),
    };

    let outcome = activation::activate_with_identity(&code, &fingerprint, &identity_wire)
        .await
        .map_err(|error| super::coded_error("LOCUS_ACTIVATE_FAILED", format!("{error:#}")))?;

    match outcome {
        activation::ActivationOutcome::Activated {
            code,
            tier,
            config,
            udp_relay,
            expires_at,
            fingerprint: _,
        } => {
            // Persist BEFORE reporting success. A UI that shows "activated" on a
            // device the hub has never heard of is worse than a failure: the
            // student stops trying to fix it.
            store::store(&store::Activation {
                code: code.clone(),
                tier: tier.clone(),
                fingerprint: fingerprint.clone(),
            })
            .await
            .map_err(|error| super::coded_error("LOCUS_STORE_FAILED", format!("{error:#}")))?;

            // A previous refusal is now history: this device is entitled again.
            // Left in place it would keep telling a renewed student why their old
            // code was refused, which is a correct sentence about the wrong
            // situation. Best-effort — a failure here must not fail an
            // activation that has already succeeded.
            if let Err(error) = store::clear_refusal().await {
                logging!(warn, Type::Cmd, "[locus] could not clear a previous refusal: {error:#}");
            }

            // Record the expiry the hub just gave us, so the date is correct the
            // moment activation completes.
            //
            // Without this the only writer was the heartbeat, so a freshly
            // activated device showed no expiry until the first beat landed — the
            // "days to expiry never appears" report. Only written when the hub
            // actually sent a date: a hub that predates the field sends nothing,
            // and writing `None` would wipe a value we legitimately hold (the
            // same rule the heartbeat path follows). The heartbeat still
            // refreshes it, so a renewal keeps working without a restart.
            if expires_at.is_some()
                && let Err(error) = store::record_expiry(expires_at).await
            {
                logging!(warn, Type::Cmd, "[locus] could not store the subscription expiry: {error:#}");
            }

            // The tier's connection details, so Connect has something to dial.
            // Stored separately from the entitlement because a hub response can
            // refresh one without the other — but persisted here, in the same
            // operation, because a code with no config is a device that believes
            // it is activated and cannot connect.
            let config_applied = match &config {
                Some(tier_config) => {
                    store::store_tier_config(tier_config, udp_relay)
                        .await
                        .map_err(|error| super::coded_error("LOCUS_STORE_FAILED", format!("{error:#}")))?;

                    // Apply it now so the tunnel is ready to start. A failure
                    // here is reported in the result rather than failing the
                    // whole activation: the student IS activated, and telling
                    // them otherwise would send them to re-enter a code that
                    // already worked.
                    match apply::apply_tier(tier_config, udp_relay).await {
                        Ok(apply::ApplyOutcome::Applied) => true,
                        Ok(apply::ApplyOutcome::StagedOnly) => true,
                        Ok(apply::ApplyOutcome::Rejected { reason }) => {
                            logging!(warn, Type::Cmd, "[locus] tier config rejected: {reason}");
                            false
                        }
                        Err(error) => {
                            logging!(warn, Type::Cmd, "[locus] could not apply tier: {error:#}");
                            false
                        }
                    }
                }
                // The hub returned a success without a server config. That is a
                // hub-side fault (it would leave a "connected" device that cannot
                // reach anything), and it is worth saying so rather than
                // pretending activation fully succeeded.
                None => false,
            };

            // Start beating now rather than waiting for the next launch. A
            // freshly activated device is the one most likely to be used
            // immediately, and without this its entitlement would only start
            // refreshing on the next restart.
            crate::locus::runtime::start(store::Activation {
                code: code.clone(),
                tier: tier.clone(),
                fingerprint: fingerprint.clone(),
            })
            .await;

            Ok(ActivationResult {
                code,
                tier,
                udp_relay,
                config_applied,
                message: if config_applied {
                    "Activated".to_owned()
                } else {
                    "Activated, but the connection details could not be set up. \
                     Contact support before trying to connect."
                        .to_owned()
                },
            })
        }
        // Every other outcome is a definitive answer about the code, not an
        // error in our code. Returning them as failures would make the UI show
        // "something went wrong" when the truth is "your code is expired".
        other => Err(super::coded_error("LOCUS_ACTIVATION_REFUSED", describe_outcome(&other))),
    }
}

/// What a successful activation produced.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationResult {
    pub code: String,
    pub tier: String,
    pub udp_relay: bool,
    /// Whether the tunnel configuration was written and applied. False while the
    /// store and apply path are still being wired, so the UI can say so rather
    /// than implying the VPN is ready.
    pub config_applied: bool,
    pub message: String,
}

/// Turns a refusal into something worth showing a student.
///
/// Each arm is a distinct, actionable situation. "Activation failed" is what the
/// retired client said for all of them, and it is why support conversations
/// started from nothing.
fn describe_outcome(outcome: &activation::ActivationOutcome) -> String {
    use activation::ActivationOutcome as O;
    match outcome {
        O::Activated { .. } => "Activated".to_owned(),
        O::BoundToAnotherDevice => "This code is already activated on a different device. \
             Contact the person who sold it to you — they can move it to this one."
            .to_owned(),
        O::Suspended => "This code has been suspended. Contact the person who sold it to you.".to_owned(),
        O::Expired => "This code has expired. You will need a new one.".to_owned(),
        O::NotFound => "That code was not recognised. Check it against the card — it is easy \
             to mix up 0 and O, or 1 and I, which is why Locus codes leave them out."
            .to_owned(),
        // Written for a student, and it names the ACTION rather than the rule:
        // "one code per device" is a policy, but "ask whoever sold it to you and
        // they can move it" is something they can do. Distinct from
        // BoundToAnotherDevice, which points at the code rather than the machine.
        O::DeviceAlreadyActivated => "This device is already activated with a different code. \
             One code works on one device — contact the person who sold you this code and \
             they can move it onto this device for you."
            .to_owned(),
        O::RateLimited { .. } => "Too many attempts. Wait ten minutes and try again — the limit protects \
             everyone's codes from being guessed."
            .to_owned(),
        O::ServerError { message, .. } => {
            if message.is_empty() {
                "The Locus hub could not complete the activation. Try again in a moment.".to_owned()
            } else {
                message.clone()
            }
        }
    }
}

/// Connects the tunnel.
///
/// The four steps, in order, with the reason for each:
///
///   1. **Require an entitlement.** Connecting without one produces a tunnel the
///      hub will refuse at the next heartbeat, and a student staring at
///      "connected" while nothing works.
///   2. **Check TUN is actually possible before demanding it.** See below.
///   3. **Write and apply the tier config.** Idempotent: re-applying refreshes a
///      rotated password or a moved server.
///   4. **Turn TUN on, then start the core and confirm it is running.** A success
///      return from `start_core` is not proof; the run state is what the UI will
///      show, so that is what gets checked.
///
/// ## Why TUN capability is checked before anything is written
///
/// TUN is still always-on, by product decision — the retired client was TUN-only,
/// and a system-proxy mode would silently pass traffic the school network can
/// see. Nothing here makes Locus connect without it.
///
/// What changed is *when* we find out it is impossible. TUN needs privileges:
/// mihomo running as an ordinary user fails with `configure tun interface:
/// operation not permitted`. Verge models this as `tun_capable()` — elevated, or
/// a usable service. Forcing `enable_tun_mode = true` on a machine where neither
/// holds is what produced the dead end students hit: the preference was written,
/// `prepare_startup` then computed `service_required = true`, the service was
/// `NotInstalled`, so it returned `StartupDecision::Wait` and the core never
/// started at all. The UI showed "Core temporarily unavailable" — a message about
/// the core, for a problem that was never the core's.
///
/// The failure was made worse by ordering. `reconcile_startup_tun_availability`
/// runs at startup and deliberately turns TUN off so the core *can* start on the
/// a mode the core could not actually start in. Connect then set it straight back
/// on, re-arming the exact condition startup had just cleared, so the app started
/// cleanly and then walked into the wall on the first press of Connect.
///
/// So: refuse early, name the real obstacle, and leave the stored preference
/// alone when we refuse. A student who is told "TUN needs administrator rights"
/// can act on that; a student shown a spinning overlay cannot.
/// The refusal returned when TUN cannot start on this device.
///
/// Kept as a named constant so a test can assert its wording without running the
/// command, and so the one place a student is told about privileges is greppable.
/// It names **both** routes (install the service, or run elevated) because on
/// Linux either one is sufficient, and it says "administrator rights" rather than
/// "TUN" — the student does not know what TUN is, and naming our mechanism would
/// describe the implementation instead of the obstacle.
const TUN_UNAVAILABLE_MESSAGE: &str = "Locus needs administrator rights to create the VPN tunnel, \
     and the Locus service is not available on this device. Install the service from Settings, or \
     start Locus as an administrator, then try again.";

/// Whether a connect attempt may proceed on the given run state.
///
/// Split out from the command so the gate is testable without an app handle:
/// `tun_capable` is `is_admin || service_usable`, and the point of the gate is
/// that a machine with neither must be refused *before* anything is written.
#[must_use]
const fn connect_allowed(state: &crate::core::runstate::RunState) -> bool {
    state.tun_capable()
}

/// What the connection screen says when the subscription has lapsed.
///
/// Names the next action (contact the seller) and does not blame the student.
/// A date is appended by the caller when one is known — this is the part that is
/// always true.
const SUBSCRIPTION_LAPSED_MESSAGE: &str =
    "This Locus code has expired, so the VPN will not connect. Contact the person \
     who sold you the code to renew it, then activate again.";

/// Whether a stored expiry permits a new connection.
///
/// Split out so the rule is testable without an app handle, like
/// [`connect_allowed`]. The rule is deliberately narrow: **only a known past
/// date refuses.** `Unknown` allows, because a hub that has not reported a date
/// (or a code with none recorded) must never be read as expired — refusing on a
/// guess would cut off a valid subscription, which is worse than the odd lapsed
/// device slipping through.
///
/// This is the *new-connect* gate and it honours no grace period: a lapsed code
/// cannot start a tunnel. The grace period governs a tunnel that is already up
/// (see [`crate::locus::runtime`]), because dropping a student mid-session is a
/// different decision from letting them start one.
#[must_use]
fn subscription_allows_connect(raw_expiry: Option<&str>) -> bool {
    !crate::locus::expiry::parse(raw_expiry).is_lapsed()
}

#[tauri::command]
pub async fn locus_connect() -> CmdResult<ConnectionResult> {
    let verge = Config::verge().await;
    let activation = store::read(&verge.latest_arc())
        .ok_or_else(|| super::coded_error("LOCUS_NOT_ACTIVATED", "This device is not activated yet."))?;

    // Refuse a lapsed subscription BEFORE anything else, and before writing
    // anything.
    //
    // Checked here rather than left to the hub because this is the one gate the
    // client can apply with no network, and a lapsed code that is still sitting
    // in local storage would otherwise start a tunnel that works until the next
    // heartbeat happens to be refused. The hub also refuses (410), so this is
    // belt-and-braces — but a student who is offline should still be told
    // clearly, immediately, and in words, rather than watching a tunnel come up
    // and then die.
    //
    // The stored date is the one the hub last sent (refreshed every heartbeat),
    // so it is the hub's own value and not a local guess. A code whose expiry
    // the hub never reported parses to `Unknown` and is allowed through.
    {
        let expiry_raw = verge.latest_arc().locus_expires_at.clone();
        if !subscription_allows_connect(expiry_raw.as_deref()) {
            logging!(
                warn,
                Type::Cmd,
                "[locus] refusing to connect: the stored subscription expiry has lapsed"
            );
            return Err(super::coded_error("LOCUS_SUBSCRIPTION_LAPSED", SUBSCRIPTION_LAPSED_MESSAGE));
        }
    }

    // Ask the run state whether TUN can work here, before writing anything.
    //
    // This is the same predicate the manager uses, not a second opinion: if it
    // says TUN is impossible, `start_core` would have refused for the same
    // reason, and we prefer to explain that now rather than after a failed
    // start. Nothing is persisted on this path — a refusal must not leave
    // `enable_tun_mode` flipped on for the next launch to trip over.
    if !connect_allowed(&crate::core::runstate::RUN_STATE.state()) {
        // Log the EVIDENCE, not just the verdict.
        //
        // The refusal is the one place a student is told about privileges, and it
        // is indistinguishable across three quite different causes: the app is
        // genuinely not elevated, the service is not usable, or the elevation
        // *probe* is wrong. A previous bug was the third case, and because the
        // refusal carried only a sentence there was nothing in the log to
        // separate them — the investigation had to start from "run it as admin
        // and see", which cannot distinguish a wrong probe from a real absence of
        // privilege.
        //
        // `is_process_elevated()` is read fresh at the moment of refusal so the
        // logged value is the one the decision actually used.
        let admin = crate::utils::help::is_process_elevated();
        let health = &crate::core::runstate::RUN_STATE.state().health;
        logging!(
            warn,
            Type::Cmd,
            "[locus] refusing to connect: tun not available (elevated={admin}, service={health:?}). \
             If the app was started with administrator rights, this line is evidence the probe is wrong, not the privilege."
        );
        return Err(super::coded_error("LOCUS_TUN_NOT_AVAILABLE", TUN_UNAVAILABLE_MESSAGE));
    }

    // Turn TUN on BEFORE applying, so the generated config is built for TUN
    // rather than being rebuilt immediately afterwards. We only reach this once
    // the capability check above has established that TUN can actually start.
    {
        let verge = Config::verge().await;
        verge.edit_draft(|draft| {
            draft.enable_tun_mode = Some(true);
        });
        verge
            .data_arc()
            .save_file()
            .await
            .map_err(|error| super::coded_error("LOCUS_STORE_FAILED", format!("{error:#}")))?;
    }

    // Step 2 requires the tier config, which activation stored. Until the tier
    // payload is persisted alongside the entitlement, this reports honestly that
    // it cannot proceed rather than connecting with no proxy configured.
    let config = match store::tier_config(&activation.tier).await {
        Some(config) => config,
        None => {
            return Err(super::coded_error(
                "LOCUS_NO_TIER_CONFIG",
                format!(
                    "No connection details are stored for the {} tier yet. Re-open the app so activation can complete.",
                    activation.tier
                ),
            ));
        }
    };

    // The UDP-over-TCP decision is the hub's, stored at activation. Reading it
    // back rather than passing a literal is what makes the Strike tier's UDP
    // path real: with `false` hardcoded here, every tier's game and voice
    // traffic was sent as raw UDP onto a school network that drops it, and the
    // symptom was "games are broken on the tier I bought for games".
    let udp_relay = store::udp_relay().await;

    match apply::apply_tier(&config, udp_relay).await {
        Ok(apply::ApplyOutcome::Rejected { reason }) => {
            return Err(super::coded_error(
                "LOCUS_CONFIG_REJECTED",
                format!("The tunnel configuration was refused: {reason}"),
            ));
        }
        Ok(_) => {}
        Err(error) => {
            return Err(super::coded_error(
                "LOCUS_CONFIG_FAILED",
                format!("Could not write the tunnel configuration: {error:#}"),
            ));
        }
    }

    // Wait for the tunnel to actually be ready, rather than returning the moment
    // the process was spawned.
    //
    // `start_core` resolves once the engine has been launched; the engine then
    // needs seconds more to bind, dial the server and install routes. Reporting
    // "connected" at the first moment produced a button that said Connected while
    // traffic still left by the school's connection — and a student who checked
    // immediately got "not connected" from a tunnel that was merely not finished.
    //
    // The wait is on the OBSERVED readiness, not the latch. A latched start
    // returns the instant the launch call does, which is what let this command
    // resolve `connected: true` for a Core that had not bound anything — the
    // "connected with no wifi" report. `observe_readiness` asks the Core's
    // control API and only answers Ready when it replies, so the loop below waits
    // for evidence rather than for an intention.
    //
    // Since 2026-10-02 that evidence can also be the Core reporting bytes moving
    // on the student's own traffic (see `core::manager::traffic_probe`). That
    // matters most here, in the one place a student is watching a spinner: a
    // tunnel that is already carrying their traffic is ready by definition, and
    // this loop no longer waits for a delay test to agree before saying so.
    //
    // Bounded, deliberately: an unbounded wait is the retired client's forever
    // spinner. On timeout this is a RESULT, not an error, so the screen can say
    // "still starting" and keep the cancel affordance alive.
    let started = tokio::time::Instant::now();
    loop {
        match CoreManager::global().start_core().await {
            Ok(()) => {}
            Err(error) => {
                return Err(super::coded_error("LOCUS_CONNECT_FAILED", format!("{error:#}")));
            }
        }

        if CoreManager::global().observe_readiness().await.is_ready() {
            return Ok(ConnectionResult {
                connected: true,
                message: "Connected".to_owned(),
            });
        }

        // A cancelled attempt must not leave a half-started engine behind: the
        // stop is unconditional for the same reason `locus_disconnect` is.
        if take_cancellation() {
            let _ = CoreManager::global().stop_core().await;
            return Ok(ConnectionResult {
                connected: false,
                message: "Cancelled".to_owned(),
            });
        }

        if started.elapsed() >= CONNECT_READY_BUDGET {
            return Ok(ConnectionResult {
                connected: false,
                message: format!(
                    "The tunnel is taking longer than usual to start. It may still come up — \
                     check again in a moment ({}s elapsed).",
                    started.elapsed().as_secs()
                ),
            });
        }

        tokio::time::sleep(CONNECT_READY_INTERVAL).await;
    }
}

/// Asks an in-flight connect to stop and unwind.
///
/// The button is pressable while connecting *because* of this: a connect that
/// cannot be abandoned is the retired client's forever-loading failure, where
/// the only recovery was to kill the app.
#[tauri::command]
pub async fn locus_cancel_connect() -> CmdResult<()> {
    CANCEL_CONNECT.store(true, Ordering::Release);
    Ok(())
}

/// Disconnects the tunnel.
///
/// Stops the core **unconditionally**, even when we believe we are already
/// disconnected.
///
/// This is the direct fix for the retired client's worst state bug: it
/// early-returned when its own `connected` flag was false, so a disconnect that
/// raced the watchdog left the engine running untracked, and the next Connect
/// failed with "already running" — recoverable only by restarting the app.
/// `stop_core` is a safe no-op when nothing is running, so there is nothing to
/// save by checking first, and everything to lose.
#[tauri::command]
pub async fn locus_disconnect() -> CmdResult<ConnectionResult> {
    CoreManager::global()
        .stop_core()
        .await
        .map_err(|error| super::coded_error("LOCUS_DISCONNECT_FAILED", format!("{error:#}")))?;

    Ok(ConnectionResult {
        connected: false,
        message: "Disconnected".to_owned(),
    })
}

/// The result of a connect or disconnect.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionResult {
    pub connected: bool,
    pub message: String,
}

/// What the UI should show about updates right now.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// The running version.
    pub current_version: String,
    /// A version the hub has offered and that is worth prompting about, if any.
    ///
    /// Read from storage rather than fetched, so the prompt can appear on launch
    /// without waiting for a heartbeat — the offer is recorded when a beat
    /// delivers it, and this is what the popup reads.
    pub offered_version: Option<String>,
}

/// Reports the update state the prompt should render from.
///
/// Never fails and never touches the network: this is read during window
/// start-up, and a check that could block would delay the first paint.
#[tauri::command]
pub async fn locus_update_status() -> UpdateStatus {
    let verge = Config::verge().await;
    let offered = verge.latest_arc().locus_update_offered.clone();

    UpdateStatus {
        current_version: env!("CARGO_PKG_VERSION").to_owned(),
        offered_version: offered.map(Into::into),
    }
}

/// Forgets an offered update the student has dismissed or already installed.
#[tauri::command]
pub async fn locus_dismiss_update() -> CmdResult<()> {
    store::clear_update_offer()
        .await
        .map_err(|error| super::coded_error("LOCUS_STORE_FAILED", format!("{error:#}")))
}

/// Downloads and installs the offered update, reporting progress as it goes.
///
/// The download and the mandatory signature verification are the plugin's job —
/// `locus::update::install` drives it, and deliberately does not re-implement the
/// swap. The retired client hand-rolled one and destroyed an installation
/// (FIXES #22).
///
/// # Completion means "expect a restart" on Windows
///
/// The plugin launches the NSIS installer and then exits the app, so a successful
/// return may never happen. Callers must not treat a missing response as failure.
#[tauri::command]
pub async fn locus_install_update(app: tauri::AppHandle) -> CmdResult<()> {
    let hub = contract::HUB_URL;
    let current = env!("CARGO_PKG_VERSION");

    // START-OF-INSTALL. This line is the one whose absence made the reported
    // crash undiagnosable: the install path logged only on failure-to-check and
    // on success, so a process that died mid-download left nothing behind at
    // all. Every branch below now says so before it can block.
    logging!(info, Type::System, "[locus] update install requested (running {current})");

    let pending = crate::locus::update::install::PendingInstall::check(&app, hub, current)
        .await
        .map_err(|error| {
            // The plugin has to be registered for `updater_builder()` to resolve.
            // When it is not, this is where it shows up — with an opaque
            // "plugin not found" that reads like a network fault. Naming it means
            // a build that lost the registration says so, rather than presenting
            // as "updates are broken" again.
            let detail = format!("{error:#}");
            logging!(error, Type::System, "[locus] update check failed: {detail}");
            if detail.contains("updater") && detail.contains("not") {
                super::coded_error(
                    "LOCUS_UPDATE_UNAVAILABLE",
                    "The updater is not available in this build. Reinstall Locus from the \
                     installer, or report this to support.",
                )
            } else {
                super::coded_error("LOCUS_UPDATE_CHECK_FAILED", detail)
            }
        })?
        .ok_or_else(|| {
            logging!(
                warn,
                Type::System,
                "[locus] update install requested but the hub offers nothing to install"
            );
            super::coded_error("LOCUS_UPDATE_NONE", "No update is available to install.")
        })?;

    let installed_version = pending.version().to_owned();

    // The hub publishes the artifact's SHA-256 alongside the URL and signature,
    // and the plugin preserves it in `raw_json`. Without it we would have to
    // install bytes we cannot hash-check, so a missing one is refused here —
    // before a single byte is fetched — rather than discovered after.
    let Some(sha256) = pending.sha256().map(str::to_owned) else {
        logging!(
            error,
            Type::System,
            "[locus] the hub offered {installed_version} with no sha256; refusing to install"
        );
        return Err(super::coded_error(
            "LOCUS_UPDATE_NO_CHECKSUM",
            "The update the hub offered carries no checksum, so it cannot be verified. \
             Report this to support.",
        ));
    };

    let staging_dir = dirs::app_home_dir()
        .map(|root| root.join("update-staging"))
        .map_err(|error| super::coded_error("LOCUS_PATHS_FAILED", format!("{error:#}")))?;

    // A partial file from a previous attempt is dead weight and, on Windows,
    // something antivirus may still hold a handle on. Clear it before starting.
    crate::locus::update::apply::prune_staging_dir(&staging_dir);

    logging!(
        info,
        Type::System,
        "[locus] downloading update {installed_version} to {}",
        staging_dir.display()
    );

    // Progress is emitted rather than returned, because the download outlives the
    // command's usefulness to the UI: the popup needs a percentage while it runs,
    // not one number at the end.
    //
    // Throttled: the plugin's callback fires per network chunk, and emitting a
    // Tauri event for each one floods the webview on a fast connection. One event
    // per whole percent (plus the first and last) is what a progress bar needs.
    let progress_app = app.clone();
    let mut last_percent = u64::MAX;
    let on_progress = move |received: u64, total: Option<u64>| {
        let percent = total
            .filter(|total| *total > 0)
            .map(|total| received.saturating_mul(100) / total);
        if percent == Some(last_percent) {
            return;
        }
        if let Some(percent) = percent {
            last_percent = percent;
        } else {
            last_percent = u64::MAX;
        }
        let _ = progress_app.emit(
            "locus://update-progress",
            UpdateProgress {
                chunk_length: usize::try_from(received).unwrap_or(usize::MAX),
                content_length: total,
            },
        );
    };

    let ready = pending
        .download_to_file(&staging_dir, &sha256, on_progress)
        .await
        .map_err(|error| {
            let detail = format!("{error:#}");
            logging!(error, Type::System, "[locus] update download failed: {detail}");
            super::coded_error("LOCUS_UPDATE_DOWNLOAD_FAILED", detail)
        })?;

    logging!(
        info,
        Type::System,
        "[locus] update {installed_version} downloaded and verified; running the installer"
    );

    // Hand the verified bytes to the platform installer. On Windows this exits
    // the process, so the line below may never be reached — which is why the
    // success log is emitted *before* the call, not after.
    ready.install().map_err(|error| {
        let detail = format!("{error:#}");
        logging!(error, Type::System, "[locus] the installer refused the update: {detail}");
        super::coded_error("LOCUS_UPDATE_INSTALL_FAILED", detail)
    })?;

    logging!(
        info,
        Type::System,
        "[locus] update to {installed_version} installed; a restart may be required"
    );

    // The offer is spent. Clearing it stops the prompt reappearing for a version
    // the student now has, including after the restart the install may trigger.
    let _ = store::clear_update_offer().await;
    Ok(())
}

/// Progress of an in-flight update download, emitted as `locus://update-progress`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    /// Bytes downloaded so far, **cumulative** — not the size of one chunk.
    ///
    /// Cumulative rather than per-chunk on purpose: the UI computes a percentage
    /// from it directly, and a per-chunk value would need the frontend to keep a
    /// running total that could disagree with the backend's.
    pub chunk_length: usize,
    /// Total bytes, when the server advertised a length.
    ///
    /// `None` means the hub used a chunked response, and the UI should show an
    /// indeterminate bar rather than a stalled 0%.
    pub content_length: Option<u64>,
}

/// Where the update staging directory lives.
///
/// Exposed so the diagnostics screen can report it without duplicating the path
/// logic, and so a support conversation can ask for exactly one location.
#[tauri::command]
pub async fn locus_update_staging_dir() -> CmdResult<String> {
    let dir = dirs::app_home_dir()
        .map(|root| root.join("update-staging"))
        .map_err(|error| super::coded_error("LOCUS_PATHS_FAILED", format!("{error:#}")))?;
    Ok(dir.to_string_lossy().into_owned())
}

/// Reports the hub URL the client will talk to.
///
/// The UI shows this in diagnostics, and it is the first thing to check when
/// activation fails on a school network.
#[tauri::command]
pub async fn locus_hub_url() -> String {
    contract::HUB_URL.to_owned()
}

/// Asks the client to confirm its subscription with the hub right now.
///
/// The answer to "when does my subscription end?" is only ever the hub's, and the
/// client learns it on a heartbeat whose interval is a floor of five minutes (and
/// up to two hours after a run of failures). So a student who has just renewed,
/// or who wants to know whether a suspension has been lifted, had no way to ask
/// without waiting for a timer they cannot see.
///
/// This triggers one beat now, and **waits for it to finish**. It returns whether
/// a beat was started, not what it found: the result arrives through the ordinary
/// heartbeat path (which stops the tunnel on a refusal and refreshes the stored
/// date on success), so there is exactly one place that interprets the hub's
/// answer. Reporting a verdict here would be a second interpretation that could
/// disagree with the first.
///
/// # Why it waits rather than returning immediately (2026-09-30)
///
/// It used to return the moment the beat was *requested*, so the caller's pending
/// state lasted one IPC round trip — single-digit milliseconds — while the hub
/// round trip the student was waiting for had not begun. The button therefore
/// animated a state it had not observed: "Check status now" flicked to
/// "Checking…" and straight back, and the confirmation landed afterwards, from a
/// poll. Waiting makes the control's pending state bracket the work, which is the
/// only honest thing a progress indicator can do.
///
/// The wait is bounded by the beat's own request timeout, so an unreachable hub
/// delays the reply rather than hanging it.
#[tauri::command]
pub async fn locus_check_subscription() -> bool {
    crate::locus::runtime::beat_now_and_wait().await
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, reason = "tests assert by panicking")]
mod tests {
    use super::*;
    use activation::ActivationOutcome as O;
    use crate::core::manager::RunningMode;
    use crate::core::runstate::ServiceHealth;

    /// Every refusal must produce a sentence a student can act on.
    ///
    /// This is the direct counter to the retired client's behaviour, where all
    /// of these collapsed into one message. A test rather than a comment because
    /// "add a new outcome and forget to describe it" is exactly the kind of
    /// omission that reaches a student.
    #[test]
    fn every_refusal_is_described_actionably() {
        let outcomes = [
            O::BoundToAnotherDevice,
            O::Suspended,
            O::Expired,
            O::NotFound,
            O::RateLimited { retry_after_secs: None },
            O::ServerError {
                code: 500,
                message: String::new(),
            },
        ];

        for outcome in outcomes {
            let text = describe_outcome(&outcome);
            assert!(!text.is_empty(), "{outcome:?} produced no message");
            assert!(
                text.len() > 20,
                "{outcome:?} produced something too terse to act on: {text:?}"
            );
            // "failed" alone is the phrasing this exists to eliminate.
            assert!(
                !text.eq_ignore_ascii_case("activation failed"),
                "{outcome:?} fell back to the useless generic message"
            );
        }
    }

    /// Bound-to-another-device must name the fix, because the student cannot
    /// solve it themselves — an operator has to unbind the code.
    #[test]
    fn the_bound_message_points_at_the_person_who_can_help() {
        let text = describe_outcome(&O::BoundToAnotherDevice);
        assert!(
            text.to_lowercase().contains("sold") || text.to_lowercase().contains("contact"),
            "the message must say who can resolve it, got {text:?}"
        );
    }

    /// A hub-supplied message is shown verbatim: it is what a support
    /// conversation will quote, and rewording it would make the two disagree.
    #[test]
    fn a_hub_message_is_preferred_when_present() {
        let text = describe_outcome(&O::ServerError {
            code: 500,
            message: "Hub is in maintenance until 14:00".to_owned(),
        });
        assert_eq!(text, "Hub is in maintenance until 14:00");
    }

    /// Rate limiting must say to wait, not just to try again — retrying
    /// immediately makes it worse and the limit is 10 minutes.
    #[test]
    fn the_rate_limit_message_says_to_wait() {
        let text = describe_outcome(&O::RateLimited { retry_after_secs: None });
        assert!(
            text.to_lowercase().contains("wait") || text.to_lowercase().contains("minute"),
            "the message must tell the student to wait, got {text:?}"
        );
    }

    /// The device id shown in the UI must be truncated. It is a stable device
    /// identifier (the value a code is bound to), and a full one in a screenshot
    /// is a shareable one.
    ///
    /// This used to truncate `device::fingerprint()`. That value is no longer the
    /// binding id — the identity's `device_id` is — so the test now pins the
    /// redaction of the value the status command actually reports, which is what
    /// the UI renders. Testing a helper against a value the product no longer
    /// uses would read green while the rendered id was unredacted.
    #[test]
    fn status_reports_a_redacted_device_id() {
        let full = crate::locus::identity::DeviceIdentity {
            device_id: "0123456789abcdef0123456789abcdef".to_owned(),
            secret: String::new(),
            store: crate::locus::identity::Store::Machine,
        }
        .binding_id();
        let redacted = store::redact(&full);
        assert_ne!(redacted, full, "the device id must not be the full binding id");
        assert_eq!(redacted.len(), 12);
        assert!(full.starts_with(&redacted));
    }

    /// Build a run state for the connect gate.
    fn run_state(health: ServiceHealth, is_admin: bool, op_in_flight: bool) -> crate::core::runstate::RunState {
        crate::core::runstate::RunState {
            health,
            pending: None,
            mode: RunningMode::NotRunning,
            is_admin,
            op_in_flight,
        }
    }

    /// A machine with neither privileges nor a usable service must be refused.
    ///
    /// This is the exact configuration that produced the dead end: an ordinary
    /// user, service never installed, because TUN cannot start without one of
    /// them (`configure tun interface: operation not permitted`). Before this
    /// gate, connect wrote `enable_tun_mode = true` anyway, `start_core` then
    /// waited forever on a service that was never coming, and the student saw a
    /// message about the core rather than about privileges.
    #[test]
    fn connect_is_refused_when_tun_cannot_start() {
        let state = run_state(ServiceHealth::NotInstalled, false, false);
        assert!(
            !connect_allowed(&state),
            "an unprivileged machine with no service must not attempt TUN"
        );
    }

    /// Either route is sufficient: elevation alone, or a usable service alone.
    ///
    /// Pinned because `tun_capable` is `is_admin || service_usable`, and a future
    /// edit that tightened it to require *both* would lock out every student who
    /// was only ever going to use one of them.
    #[test]
    fn connect_is_allowed_with_either_privilege_or_service() {
        assert!(
            connect_allowed(&run_state(ServiceHealth::NotInstalled, true, false)),
            "running elevated must be enough on its own"
        );
        assert!(
            connect_allowed(&run_state(ServiceHealth::Ready, false, false)),
            "a ready service must be enough on its own"
        );
    }

    /// Any running mode means the core is up; only `NotRunning` means down.
    ///
    /// This is the mapping the connection screen relies on to decide whether to
    /// show the traffic panel. It was previously absent, and the screen guessed
    /// "disconnected" after every poll — which is why a successful connect
    /// flashed the panel for a single frame.
    ///
    /// Note what this does *not* claim: a started engine is not yet a tunnel
    /// carrying traffic, which is why the screen asks
    /// [`CoreManager::observe_readiness`] before it says "Connected".
    #[test]
    fn a_running_service_reads_as_connected() {
        let running = crate::core::runstate::RunState {
            mode: RunningMode::Service,
            ..run_state(ServiceHealth::NotInstalled, true, false)
        };
        assert!(core_is_running(&running), "a running service must read as connected");

        let stopped = run_state(ServiceHealth::NotInstalled, true, false);
        assert!(
            !core_is_running(&stopped),
            "NotRunning is the only down answer"
        );
    }

    /// A cancelled connect is remembered until the loop that reads it consumes it.
    ///
    /// The flag is a plain atomic with one writer (the UI) and one reader (the
    /// in-flight connect), so the risk is not tearing but *stickiness*: a flag
    /// left set would make the NEXT connect cancel itself immediately. The
    /// consume step is therefore part of the contract, and this pins it.
    #[test]
    fn a_cancelled_connect_is_consumed_exactly_once() {
        CANCEL_CONNECT.store(true, Ordering::Release);

        assert!(
            take_cancellation(),
            "the in-flight connect must see the request"
        );
        assert!(
            !take_cancellation(),
            "a cancellation must not leak into the next connect"
        );

        // And an unflagged connect is never cancelled.
        assert!(!take_cancellation());
    }

    /// A service operation in flight is not a usable service.
    ///
    /// This is the install-in-progress case: the service is `Ready` but an
    /// operation is mid-flight, so `service_usable` is false. Connecting then
    /// would race the operation, so the gate must hold.
    #[test]
    fn connect_is_refused_while_a_service_operation_is_in_flight() {
        let state = run_state(ServiceHealth::Ready, false, true);
        assert!(
            !connect_allowed(&state),
            "a service mid-operation must not be treated as usable"
        );
    }

    /// The refusal must name a route the student can actually take.
    ///
    /// Two independent things could make this useless: a message so terse it does
    /// not say what to do, or one that says "TUN" — our mechanism, not their
    /// problem. It must mention administrator access and where to get the fix.
    #[test]
    fn the_tun_refusal_is_actionable() {
        let text = TUN_UNAVAILABLE_MESSAGE;
        let lowered = text.to_lowercase();
        assert!(
            lowered.contains("administrator"),
            "must name the privilege that is missing, got {text:?}"
        );
        assert!(
            lowered.contains("settings") || lowered.contains("install"),
            "must say where the fix is, got {text:?}"
        );
        // The acronym specifically, as a word — not the substring, or "tunnel"
        // (which is the student-facing concept and belongs here) would match.
        assert!(
            !lowered.split(|c: char| !c.is_alphanumeric()).any(|word| word == "tun"),
            "must not name the TUN mechanism itself, got {text:?}"
        );
        // And it must still say what the thing they are getting *is*.
        assert!(
            lowered.contains("tunnel") || lowered.contains("vpn"),
            "must say what access they are getting, got {text:?}"
        );
    }

    /// User-facing messages must not carry collapsed or doubled whitespace.
    ///
    /// A literal run of spaces inside a `format!` string is invisible in review
    /// and reaches the student verbatim. This happened: the no-tier-config
    /// message shipped with 22 spaces mid-sentence — a manual re-wrap whose
    /// newline was replaced rather than removed — and nothing caught it, because
    /// no test looked at a message's *whitespace*, only at whether it mentioned
    /// the right words.
    ///
    /// Checked over the whole file rather than one constant, so a new message
    /// cannot reintroduce the class. Two spaces are allowed where they are
    /// conventional (after a sentence-ending period), which is why the rule is
    /// "no run of 3+", not "no double space".
    #[test]
    fn user_facing_messages_have_no_collapsed_whitespace() {
        let source = include_str!("locus.rs");
        let mut offenders = Vec::new();
        for (index, line) in source.lines().enumerate() {
            // Only string literals that are messages, not code or docs.
            let trimmed = line.trim_start();
            if trimmed.starts_with("///") || trimmed.starts_with("//") {
                continue;
            }
            let Some(start) = line.find('"') else { continue };
            let rest = &line[start + 1..];
            let Some(end) = rest.find('"') else { continue };
            let literal = &rest[..end];
            // The needle is built at runtime so this test's own source cannot
            // match it. A literal `"   "` here would make the test fail on
            // itself — which is exactly what happened on the first run, and is
            // a reminder that a self-scanning check must not contain its own
            // pattern.
            let needle = " ".repeat(3);
            if literal.contains(needle.as_str()) {
                offenders.push(format!("line {}: {:?}", index + 1, literal));
            }
        }
        assert!(
            offenders.is_empty(),
            "these message literals contain a run of 3 or more spaces \
             (a collapsed quote or a lost newline):\n{}",
            offenders.join("\n")
        );
    }

    /// An unactivated device has an UNKNOWN subscription, never a lapsed one.
    ///
    /// This is the fresh-install case. `locus_expires_at` is empty because no
    /// hub has ever spoken to this device; if that were classified as lapsed,
    /// the Account screen would tell a brand-new student their subscription had
    /// expired before they had activated anything.
    #[test]
    fn an_unactivated_device_has_an_unknown_subscription() {
        assert!(matches!(
            classify_subscription(false, None, None),
            SubscriptionStatus::Unknown
        ));
        // Even with a date present — a damaged record — activation wins, because
        // an expiry without an entitlement describes nothing.
        assert!(matches!(
            classify_subscription(false, Some("2030-01-01 00:00:00.000Z"), None),
            SubscriptionStatus::Unknown
        ));
    }

    /// A refusal is reported ahead of Unknown, and carries the hub's reason.
    ///
    /// This is the state a student lands in after a background heartbeat is
    /// refused: the entitlement is gone, so there is no subscription left to
    /// describe, and reporting that as UNKNOWN would be true and useless — it
    /// looks identical to a fresh install and explains nothing. The reason is the
    /// actionable part, and it must survive verbatim because the hub's wording is
    /// an instruction ("contact your middleman"), not a description.
    #[test]
    fn a_refused_device_is_told_why_it_was_refused() {
        match classify_subscription(false, None, Some("Account suspended — contact your middleman")) {
            SubscriptionStatus::Refused { reason } => {
                assert_eq!(reason, "Account suspended — contact your middleman");
            }
            other => panic!("a recorded refusal must be reported as Refused, got {other:?}"),
        }
    }

    /// The refusal must win over an unactivated device's UNKNOWN, or it can
    /// never be seen: a refusal always clears the code, so the unactivated branch
    /// is the one a refused device lands in.
    #[test]
    fn a_refusal_is_reachable_which_is_why_it_is_checked_first() {
        assert!(matches!(
            classify_subscription(false, None, Some("Code expired")),
            SubscriptionStatus::Refused { .. }
        ));
    }

    /// A blank or whitespace reason is not a reason. Treating one as truthy would
    /// render an empty explanation, which is worse than the honest UNKNOWN.
    #[test]
    fn a_blank_refusal_reason_falls_back_to_unknown() {
        for blank in ["", "   ", "\n"] {
            assert!(
                matches!(
                    classify_subscription(false, None, Some(blank)),
                    SubscriptionStatus::Unknown
                ),
                "a blank reason ({blank:?}) must not produce a Refused state"
            );
        }
    }

    /// An entitled device is not refused, whatever an old reason says.
    ///
    /// The refusal is cleared on activation, but this pins the invariant at the
    /// classifier too: a student who renewed must never be told their code was
    /// refused.
    #[test]
    fn an_activated_device_is_never_refused() {
        assert!(matches!(
            classify_subscription(true, Some("2030-01-01 00:00:00.000Z"), Some("Code expired")),
            SubscriptionStatus::Active { .. }
        ));
    }

    /// An activated device with no recorded date also reports UNKNOWN.
    ///    /// Happens with a hub that predates the heartbeat field, and with codes that
    /// have no expiry set at all. Neither is the student's fault and neither
    /// should render as an expiry.
    #[test]
    fn a_missing_date_on_an_activated_device_is_unknown() {
        assert!(matches!(
            classify_subscription(true, None, None),
            SubscriptionStatus::Unknown
        ));
        assert!(matches!(
            classify_subscription(true, Some(""), None),
            SubscriptionStatus::Unknown
        ));
        assert!(matches!(
            classify_subscription(true, Some("garbage"), None),
            SubscriptionStatus::Unknown
        ));
    }

    /// A past date on an activated device is LAPSED, and says so.
    #[test]
    fn a_past_date_is_reported_as_lapsed() {
        assert!(matches!(
            classify_subscription(true, Some("2020-01-01 00:00:00.000Z"), None),
            SubscriptionStatus::Lapsed
        ));
    }

    /// A future date is ACTIVE, and its urgency follows the warning window.
    ///
    /// The `urgent` flag is what the UI colours on, so it must agree with the
    /// one threshold constant rather than a second copy of the number.
    #[test]
    fn a_future_date_is_active_with_the_right_urgency() {
        let far = chrono::Utc::now() + chrono::Duration::days(60);
        let far_text = far.format("%Y-%m-%d %H:%M:%S%.fZ").to_string();
        match classify_subscription(true, Some(&far_text), None) {
            SubscriptionStatus::Active {
                days_remaining,
                urgent,
                expires_at,
            } => {
                assert!(days_remaining > crate::locus::expiry::EXPIRY_WARNING_DAYS);
                assert!(!urgent, "60 days out must not be urgent");
                // The exact string the hub sent, so the tooltip can show it.
                assert_eq!(expires_at, far_text);
            }
            other => panic!("expected Active, got {other:?}"),
        }
    }

    /// Inside the warning window, `urgent` is set.
    #[test]
    fn a_soon_expiry_is_marked_urgent() {
        let soon = chrono::Utc::now() + chrono::Duration::days(2);
        let text = soon.format("%Y-%m-%d %H:%M:%S%.fZ").to_string();
        match classify_subscription(true, Some(&text), None) {
            SubscriptionStatus::Active { urgent, .. } => assert!(
                urgent,
                "two days from expiry must be urgent"
            ),
            other => panic!("expected Active, got {other:?}"),
        }
    }

    /// A lapsed subscription must REFUSE a connect. This is the gate that stops
    /// the reported bug at its source: an expired code sat in local storage and
    /// the client happily started a tunnel with it.
    #[test]
    fn a_lapsed_subscription_refuses_a_connect() {
        assert!(
            !subscription_allows_connect(Some("2020-01-01 00:00:00.000Z")),
            "an expired code must not be allowed to connect"
        );
    }

    /// But a subscription we know nothing about must NOT refuse one.
    ///
    /// Every "no date" case is a valid reason to connect: a fresh install, a hub
    /// that predates the field, a code with no expiry set. Refusing on a guess
    /// would end subscriptions that are perfectly fine — the failure is silent
    /// and the student did nothing wrong.
    #[test]
    fn an_unknown_or_future_expiry_allows_a_connect() {
        assert!(
            subscription_allows_connect(None),
            "no stored date must not block a connect"
        );
        assert!(
            subscription_allows_connect(Some("")),
            "a blank date must not block a connect"
        );
        assert!(
            subscription_allows_connect(Some("not a date")),
            "an unparseable date must not block a connect"
        );

        let future = (chrono::Utc::now() + chrono::Duration::days(30))
            .format("%Y-%m-%d %H:%M:%S%.fZ")
            .to_string();
        assert!(
            subscription_allows_connect(Some(&future)),
            "a future expiry must allow a connect"
        );
    }

    /// The gate and the display must agree about what "lapsed" means, or the
    /// screen could show "expires in 3 days" while the button silently refuses.
    #[test]
    fn the_gate_and_the_display_agree_on_lapsed() {
        let past = "2020-01-01 00:00:00.000Z";
        assert!(matches!(
            classify_subscription(true, Some(past), None),
            SubscriptionStatus::Lapsed
        ));
        assert!(!subscription_allows_connect(Some(past)));
    }

    /// The serialized shape is what the frontend actually reads, so it is part
    /// of the contract — not an implementation detail.
    ///
    /// This pins the failure mode directly: serde's `rename_all` on an enum
    /// renames variants, NOT the fields inside struct variants, so without
    /// `rename_all_fields` the payload carried `days_remaining`/`expires_at`
    /// while `src/services/locus.ts` reads `daysRemaining`/`expiresAt`. The row
    /// then rendered "{{count}} days" with an empty count (or nothing), and the
    /// expiry was reported as simply not working — with the Rust tests all green,
    /// because none of them looked at the bytes.
    #[test]
    fn the_active_wire_shape_is_camel_case() {
        let value = serde_json::to_value(SubscriptionStatus::Active {
            days_remaining: 12,
            urgent: true,
            expires_at: "2030-09-19 00:00:00.000Z".to_owned(),
        })
        .expect("SubscriptionStatus must serialize");

        assert_eq!(
            value.get("state").and_then(serde_json::Value::as_str),
            Some("active")
        );
        assert!(
            value.get("daysRemaining").is_some(),
            "the frontend reads `daysRemaining`; got {value}"
        );
        assert!(
            value.get("expiresAt").is_some(),
            "the frontend reads `expiresAt`; got {value}"
        );
        assert!(
            value.get("days_remaining").is_none() && value.get("expires_at").is_none(),
            "snake_case field names must not reach the wire; got {value}"
        );
    }

    /// The refused variant carries its `reason` under the same rule.
    #[test]
    fn the_refused_wire_shape_is_camel_case() {
        let value = serde_json::to_value(SubscriptionStatus::Refused {
            reason: "Code suspended".to_owned(),
        })
        .expect("SubscriptionStatus must serialize");

        assert_eq!(
            value.get("state").and_then(serde_json::Value::as_str),
            Some("refused")
        );
        assert!(
            value.get("reason").is_some(),
            "`reason` is a single word and must survive; got {value}"
        );
    }
}
