//! The Locus wire contract: one definition of every name the hub and the client
//! must agree on.
//!
//! # Why this module exists
//!
//! The retired Wails client earned every entry here the hard way. Two failures
//! in particular are the reason this is a module rather than a habit:
//!
//! 1. **`uot_port` vs `server_port_uot`.** The hub sends the UoT endpoint as
//!    `uot_port` (the tier config JSON is passed through verbatim). The client
//!    declared only `server_port_uot`. Serde ignores unknown fields silently, so
//!    the value never landed, the port stayed `0`, and UDP-over-TCP — the whole
//!    mechanism behind the Strike gaming tier — was dead fleet-wide while both
//!    sides were individually "correct". Nothing caught it because no test
//!    asserted that the client parses the key the server emits.
//! 2. **`download_<platform>` → `update_<platform>`.** The `update_config`
//!    record and the heartbeat response deliberately use different names. The
//!    rename is historical and is now load-bearing: deployed clients read
//!    `update_linux`, so the hook must keep translating. "Simplifying" it breaks
//!    every installed client.
//!
//! Both are the same class of bug: a field name duplicated in three languages
//! (goja JS, Rust, Go) with nothing enforcing agreement. Everything that crosses
//! the wire is therefore named exactly once, here.

use serde::{Deserialize, Serialize};

/// The Locus hub.
///
/// Kept as a constant rather than a setting: the hub also owns the `server`
/// field in every tier config, so a hub move is a server-side change plus this
/// line, never a per-client setting. A client that cannot reach its hub cannot
/// activate, heartbeat, or update.
///
/// **"Never a per-client setting" is now "never a per-client *guess*".** The
/// value still has one compiled default, but a network can refuse that default
/// outright — N4L classifies `*.duckdns.org` as uncategorised and blocks it, which
/// makes activation, heartbeat AND the update path unreachable at once, so the
/// device cannot even fetch the build that would fix it. [`resolve_hub_url`] is
/// the escape hatch: an operator-set override, validated, with this constant as
/// the fallback.
///
/// # Why this moved off `*.duckdns.org` (3.3.2)
///
/// The duckdns name was blocked by N4L's filter, and that block could not be
/// escaped from the client side: `/api/activate` is the first call a device
/// makes, it happens *before* any heartbeat, and the heartbeat is the only
/// channel that could deliver [`LEGACY_HUB_URL`]'s replacement. The failure
/// prevented escaping the failure, exactly as `/api/release` and the
/// unauthenticated `/api/update` exist to prevent elsewhere.
///
/// The replacement is a name under a domain the operator owns, which is the one
/// property a dynamic-DNS subdomain cannot have: a registered name with a
/// reputation can be recategorised on request, and its certificate can be
/// renewed against a name that is not itself the thing being blocked.
///
/// **This is not a migration mechanism.** Deployed 3.3.1-and-earlier builds
/// keep dialling [`LEGACY_HUB_URL`] and cannot be told otherwise — which is why
/// the hub must keep answering on it. See [`LEGACY_HUB_URL`].
pub const HUB_URL: &str = "https://networkingguides.jadedns.uk";

/// The hub's **previous** name, kept alive on the server for the deployed fleet.
///
/// Every build before 3.3.2 has this string compiled in and no way to learn a
/// new one, because the only channel that carries a hub move is the heartbeat —
/// which requires a working hub to reach. So the hub serves the full API surface
/// on this name as well (`05-caddy.sh` puts both names on one site block), and
/// it is not a redirect: a 308 would need this name to present a valid
/// certificate for itself first, which is the thing that failed.
///
/// **Do not delete the server-side block while any earlier build is in the
/// field.** A device stranded on it is stuck at the activation gate with no
/// path to an update that would fix it — the same dead end as the retired
/// `already_used` lookup status.
///
/// It is referenced by tests and by the consistency checker, never by the
/// client's own call sites; keeping it named here makes the two-sided server
/// contract visible in the one module that owns wire names.
pub const LEGACY_HUB_URL: &str = "https://networkingguides.duckdns.org";

/// The hub base URL this run should use, given a stored override.
///
/// # Why this is a pure function with the override injected
///
/// The rule is *"honour a valid override, ignore anything else, never fail"*, and
/// the interesting cases are all the invalid ones. Taking `Option<&str>` means
/// every one of them can be exercised in a unit test with no config file, no app
/// handle and no network — which is the only way a rule about a *fallback* gets
/// tested at all, since the fallback is by definition the path nobody exercises
/// by hand.
///
/// # What it accepts
///
/// Only an absolute `https://` URL with a host. Everything else returns
/// [`HUB_URL`]:
///
///   * empty or whitespace — the normal state of every device;
///   * `http://` — a downgrade to plaintext for a credential-bearing API
///     (`/api/activate` carries the code, `/api/heartbeat` carries it too) is a
///     silent security regression, so it is refused rather than "helpfully"
///     upgraded;
///   * no scheme (`hub.example.com`) — ambiguous, and a typo here strands a
///     device with no way back;
///   * anything with no host, or with a path/query, because callers concatenate
///     `{hub}/api/...` and a stray path would silently rewrite the API root.
///
/// A trailing `/` is stripped, so `https://h.example/` and `https://h.example`
/// produce identical URLs. That is the one normalisation performed: without it
/// every call site would form `https://h.example//api/activate`, which some
/// servers 404 and some accept, which is the worst kind of difference.
#[must_use]
pub fn resolve_hub_url(override_url: Option<&str>) -> String {
    let Some(raw) = override_url else {
        return HUB_URL.to_owned();
    };
    let candidate = raw.trim();

    if candidate.is_empty() {
        return HUB_URL.to_owned();
    }

    let Ok(parsed) = url::Url::parse(candidate) else {
        return HUB_URL.to_owned();
    };

    if parsed.scheme() != "https" {
        return HUB_URL.to_owned();
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        return HUB_URL.to_owned();
    }
    if !matches!(parsed.path(), "" | "/") {
        return HUB_URL.to_owned();
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return HUB_URL.to_owned();
    }

    candidate.trim_end_matches('/').to_owned()
}

/// The hub URL for this run, read from the stored override.
///
/// The async wrapper around [`resolve_hub_url`]; every network call site goes
/// through this rather than through [`HUB_URL`], so there is exactly one place the
/// override can take effect and exactly one place to change if it ever needs to.
pub async fn hub_url() -> String {
    let verge = crate::config::Config::verge().await;
    let stored = verge.latest_arc().locus_hub_override.clone();
    resolve_hub_url(stored.as_deref())
}

/// Request body for `/api/activate` and `/api/code-lookup`.
///
/// `verifier`, `device_id` and `store` are the device's **durable identity**,
/// carried here so activation can REGISTER it — see `RegisterRequest` for why.
/// They are omitted entirely when absent so an older client's body is
/// byte-identical to what it sends today, and an older hub that ignores unknown
/// fields is unaffected.
#[derive(Debug, Clone, Serialize)]
pub struct CodeRequest<'a> {
    pub code: &'a str,
    pub fingerprint: &'a str,
    /// `sha256(secret)`, hex. Never the secret itself.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub verifier: &'a str,
    /// The device's non-secret name.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub device_id: &'a str,
    /// Where the client persisted its secret: `"machine"` or `"app"`.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub store: &'a str,
}

/// Request body for `/api/heartbeat`.
///
/// Extends [`CodeRequest`] with the session token, because a device restored by
/// recognition has no code to send. The hub accepts **either**: it resolves a
/// token to the code it stands for, and every enforcement rule then runs on that
/// code unchanged — so a token is an alternative way to name the entitlement,
/// never a weaker one.
///
/// `token` is emitted (empty when absent) rather than omitted, matching how the
/// hub reads it (`data.token || ""`), so an old hub that ignores the field
/// behaves exactly as it does today.
#[derive(Debug, Clone, Serialize)]
pub struct HeartbeatRequest<'a> {
    pub code: &'a str,
    pub fingerprint: &'a str,
    /// The session token, empty when the device authenticates with a code.
    pub token: &'a str,
}

// `/api/device-recognise` — REMOVED.
//
// `RecogniseRequest`, `RecogniseResponse` and the `Recognised` status enum all
// went with the device-recognition feature: the client no longer asks the hub
// whether it remembers a device, because nothing on either side identifies a
// device any more. A student's recovery path is the activation code, which the
// client persists to a machine-scoped store precisely so a reinstall does not
// lose it — see `crate::locus::credential`.

/// The tier's connection parameters, as the hub emits them.
///
/// The hub passes the `tier_configs.config` JSON through **verbatim**, so the
/// field names here are the stored ones. `uot_port` in particular must not be
/// renamed — see the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierConfig {
    pub server: String,
    pub server_port: u16,
    pub password: String,
    pub method: String,

    /// The optional UDP-over-TCP endpoint for this tier.
    ///
    /// **Frozen wire name.** When `> 0` *and* [`ActivateResponse::udp_relay`] is
    /// true, UDP is carried inside the TCP tunnel instead of being sent raw —
    /// which is the only way game/voice traffic survives a school network that
    /// drops UDP.
    ///
    /// The legacy spelling `server_port_uot` is accepted too, so a future
    /// server-side rename cannot break this build the way the original mismatch
    /// did. Accept both, emit neither: the client only ever reads this.
    #[serde(rename = "uot_port", alias = "server_port_uot", default)]
    pub uot_port: u16,
}

impl TierConfig {
    /// Whether this tier should carry UDP over TCP.
    ///
    /// Both halves are required. `udp_relay` alone is an operator editing a
    /// half-configured tier, and `uot_port` alone would point at a listener
    /// nothing advertises; pinning the condition in one place is what stops the
    /// two from being checked inconsistently.
    #[must_use]
    pub const fn uot_enabled(&self, udp_relay: bool) -> bool {
        udp_relay && self.uot_port > 0
    }
}

/// Response body for `POST /api/activate`.
#[derive(Debug, Clone, Deserialize)]
pub struct ActivateResponse {
    /// The hub's own status code, which is what is switched on — the HTTP status
    /// is only inspected separately for 429. See [`ActivationOutcome`].
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub device_fingerprint: Option<String>,
    #[serde(default)]
    pub server_config: Option<TierConfig>,
    #[serde(default)]
    pub udp_relay: bool,
    /// The code's expiry, as `/api/activate` now returns it.
    ///
    /// Optional and defaulted so an older hub that does not send it still
    /// deserializes — in that case the client learns the date on its first
    /// heartbeat instead, which is exactly the behaviour before this field
    /// existed. Present, it lets a freshly activated device show the expiry
    /// immediately rather than waiting for a beat.
    #[serde(default)]
    pub expires_at: Option<String>,
}

/// Response body for `POST /api/code-lookup`.
#[derive(Debug, Clone, Deserialize)]
pub struct LookupResponse {
    pub status: LookupStatus,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

/// The classification `/api/code-lookup` returns.
///
/// This endpoint exists so a student learns their code is wrong *before*
/// committing to an activation round-trip. It is read-only and never binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LookupStatus {
    /// The code exists and is ready to activate.
    Ok,
    /// The code exists and has not been redeemed yet.
    Unbound,
    /// The code has already been redeemed. Activation will still succeed — it
    /// restores the entitlement — so the screen says so rather than refusing.
    ///
    /// # Two wire names, deliberately
    ///
    /// A code is no longer tied to a device, so `BoundThisDevice`/`BoundOther`
    /// lost their meaning. But the hub cannot simply start sending a new name:
    /// an older client's enum is an allow-list whose `#[serde(other)]` catch-all
    /// is `Unknown`, and `Unknown` is `ready: false` — so a new status string is
    /// a **dead end** for every client already in the field, and a student stuck
    /// at the code prompt cannot update their way out of it. That happened:
    /// the hub sent `already_used` and deployed clients refused to activate at
    /// all (found live 2026-10-03).
    ///
    /// So the hub sends the frozen name `bound_this_device` — which every
    /// deployed build already maps to `ready: true` — and this build accepts it
    /// under the meaning that actually applies, alongside the descriptive alias
    /// `already_used` for any hub that has learned to send it.
    #[serde(alias = "bound_this_device")]
    AlreadyUsed,
    Suspended,
    Expired,
    NotFound,
    /// An older hub that lacks the route, or a transient failure.
    ///
    /// **Not a refusal.** The caller must fall back to attempting activation
    /// rather than treating this as "the code is bad": the hub is the authority
    /// and answers the same question at activate time. Reading this as "not
    /// ready" is exactly the dead end described above, so `classify_lookup`
    /// treats `Unknown` as *proceed*, not as *block*.
    #[serde(other)]
    Unknown,
}

/// Response body for `GET /api/release`.
///
/// The credential-free update manifest: it answers "what is the current public
/// release?" without an activation code. It exists because every other way of
/// learning about a new build runs through the heartbeat, which needs a bound
/// code — so a client whose build is broken *enough that activation fails* could
/// never learn that a fix had been published. The failure prevented escaping
/// the failure.
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseManifest {
    pub ok: bool,
    #[serde(default)]
    pub published: bool,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub platforms: std::collections::BTreeMap<String, ReleaseAsset>,
}

/// One platform's artifact in the public manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    pub url: String,
    #[serde(default)]
    pub sha256: String,
}

/// The platforms `update_config` and the release manifest use as keys.
///
/// **Frozen.** These strings are the contract: the heartbeat hook interpolates
/// them as `response.update_<key>` and the client matches on them.
pub const PLATFORM_LINUX: &str = "linux";
pub const PLATFORM_WINDOWS: &str = "windows";
pub const PLATFORM_MACOS_INTEL: &str = "macos_intel";
pub const PLATFORM_MACOS_ARM: &str = "macos_arm";

/// The artifact filename CI publishes for a platform.
///
/// **Frozen, and deliberately asymmetric.** The *platform key* says `macos_*`
/// while the *filename* says `darwin-*`. That is not an oversight: the hub's
/// release fetcher resolves these exact names from the GitHub Release, so
/// "tidying" one side to match the other breaks publishing for that platform.
///
/// Returns `None` on an unsupported platform rather than guessing, so a caller
/// cannot accidentally fetch another platform's binary.
#[must_use]
pub const fn artifact_for(platform: &str) -> Option<&'static str> {
    match platform.as_bytes() {
        b"linux" => Some("locus-linux-amd64"),
        b"windows" => Some("locus-windows-amd64.exe"),
        b"macos_intel" => Some("locus-darwin-amd64"),
        b"macos_arm" => Some("locus-darwin-arm64"),
        _ => None,
    }
}

/// The platform key for the running build, or `None` if unsupported.
#[must_use]
pub const fn current_platform() -> Option<&'static str> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        Some(PLATFORM_LINUX)
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        Some(PLATFORM_WINDOWS)
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        Some(PLATFORM_MACOS_ARM)
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        Some(PLATFORM_MACOS_INTEL)
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
    )))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The N4L report, as a rule.**
    ///
    /// A school network refused `networkingguides.duckdns.org` outright, which
    /// made activation, heartbeat and the update path unreachable at once — so
    /// the device could not fetch the build that would have fixed it. The escape
    /// hatch is a stored override; this pins what it must and must not accept.
    ///
    /// The valid cases are the easy half. The invalid ones are the reason the
    /// function exists: an override that is silently accepted in a broken form
    /// strands the device with no way back, because the thing it would use to
    /// recover is the thing it broke.
    #[test]
    fn a_hub_override_is_honoured_only_when_it_is_unambiguously_a_hub() {
        // ── Honoured ─────────────────────────────────────────────────────────
        assert_eq!(
            resolve_hub_url(Some("https://hub.example.org")),
            "https://hub.example.org"
        );
        // A trailing slash is the ONE normalisation: call sites concatenate
        // `{hub}/api/...`, and `https://h//api/...` is a different request.
        assert_eq!(
            resolve_hub_url(Some("https://hub.example.org/")),
            "https://hub.example.org"
        );
        // Surrounding whitespace is a paste artefact, not a decision.
        assert_eq!(
            resolve_hub_url(Some("  https://hub.example.org  ")),
            "https://hub.example.org"
        );

        // ── Falls back to the compiled default ───────────────────────────────
        for bad in [
            None,
            Some(""),
            Some("   "),
            // Plaintext for an API that carries the activation code in its body.
            Some("http://hub.example.org"),
            // No scheme: ambiguous, and a typo here is unrecoverable.
            Some("hub.example.org"),
            // A path would silently rewrite the API root every caller appends to.
            Some("https://hub.example.org/api"),
            // Query and fragment have no meaning for a base URL.
            Some("https://hub.example.org/?x=1"),
            Some("https://hub.example.org/#x"),
            // Not a URL at all.
            Some("not a url"),
        ] {
            assert_eq!(
                resolve_hub_url(bad),
                HUB_URL,
                "{bad:?} must not replace the compiled hub address"
            );
        }
    }

    /// The default itself must remain reachable-by-construction: absolute https.
    ///
    /// A compiled default that fails `resolve_hub_url`'s own rules would mean the
    /// fallback path is the one path nobody had validated.
    #[test]
    fn the_compiled_default_passes_the_same_rule() {
        assert_eq!(resolve_hub_url(Some(HUB_URL)), HUB_URL);
        assert!(HUB_URL.starts_with("https://"));
        assert!(!HUB_URL.ends_with('/'));
    }

    /// The regression that motivated this module: the hub emits `uot_port`, and
    /// a client that does not land it silently loses UDP-over-TCP for the whole
    /// Strike tier.
    #[test]
    fn the_hub_uot_key_is_parsed_under_its_frozen_name() {
        let live = r#"{
          "server": "networkingguides.duckdns.org",
          "server_port": 8445,
          "password": "3467ce5ae756c185f45be91fed2dbeb6",
          "method": "2022-blake3-aes-256-gcm",
          "uot_port": 8446
        }"#;

        let cfg: TierConfig = serde_json::from_str(live).expect("live tier config must parse");
        assert_eq!(
            cfg.uot_port, 8446,
            "the hub sends `uot_port`; failing to read it is the bug that made UoT dead fleet-wide"
        );
        assert!(cfg.uot_enabled(true), "udp_relay + a live port means UoT is on");
    }

    /// The legacy spelling stays accepted so a future server-side rename cannot
    /// break this build in the opposite direction.
    #[test]
    fn the_legacy_spelling_is_accepted_as_a_fallback() {
        let legacy = r#"{
          "server": "example.org", "server_port": 8443,
          "password": "x", "method": "2022-blake3-aes-256-gcm",
          "server_port_uot": 8446
        }"#;
        let cfg: TierConfig = serde_json::from_str(legacy).expect("legacy spelling must parse");
        assert_eq!(cfg.uot_port, 8446);
    }

    /// A tier with no UoT endpoint is the normal case for the free plan, and
    /// must not be an error.
    #[test]
    fn a_tier_without_uot_parses_and_reports_disabled() {
        let tcp_only = r#"{
          "server": "example.org", "server_port": 8443,
          "password": "x", "method": "2022-blake3-aes-256-gcm"
        }"#;
        let cfg: TierConfig = serde_json::from_str(tcp_only).expect("tcp-only tier must parse");
        assert_eq!(cfg.uot_port, 0);
        assert!(!cfg.uot_enabled(true), "no port means no UoT even if the flag is set");
        assert!(!cfg.uot_enabled(false), "no flag means no UoT even if a port exists");
    }

    /// Both halves of the UoT condition are required; an operator toggling one
    /// without the other is editing a half-configured tier.
    #[test]
    fn uot_needs_both_the_flag_and_a_port() {
        let cfg = TierConfig {
            server: "example.org".into(),
            server_port: 8445,
            password: "x".into(),
            method: "2022-blake3-aes-256-gcm".into(),
            uot_port: 8446,
        };
        assert!(cfg.uot_enabled(true));
        assert!(!cfg.uot_enabled(false));
    }

    /// The platform key ↔ filename asymmetry is a contract with the publish
    /// pipeline, not an oversight. Pinning it here means a "tidy-up" fails the
    /// build instead of breaking releases for one platform.
    #[test]
    fn artifact_names_keep_the_macos_darwin_asymmetry() {
        assert_eq!(artifact_for(PLATFORM_LINUX), Some("locus-linux-amd64"));
        assert_eq!(artifact_for(PLATFORM_WINDOWS), Some("locus-windows-amd64.exe"));
        assert_eq!(artifact_for(PLATFORM_MACOS_INTEL), Some("locus-darwin-amd64"));
        assert_eq!(artifact_for(PLATFORM_MACOS_ARM), Some("locus-darwin-arm64"));
    }

    /// An unknown platform must not silently resolve to another platform's
    /// binary — that would hand a Windows user a Linux executable.
    #[test]
    fn an_unknown_platform_has_no_artifact() {
        assert_eq!(artifact_for("freebsd"), None);
        assert_eq!(artifact_for("macos"), None);
    }

    /// The response shape the hub ACTUALLY returns for a redeemed code, and the
    /// descriptive alias, must both map to `AlreadyUsed`.
    ///
    /// The hub sends the frozen `bound_this_device` because deployed clients
    /// only accept known names (see the enum's own note); `already_used` is
    /// accepted too so a hub that learns the clearer name is not a breaking
    /// change for this build. Both must parse to the same variant.
    #[test]
    fn both_redeemed_status_names_map_to_the_same_variant() {
        for status in ["bound_this_device", "already_used"] {
            let live = format!(
                r#"{{"expires_at": "2027-09-19 00:00:00.000Z", "message": "This code has already been activated", "status": "{status}", "tier": "strike"}}"#
            );
            let parsed: LookupResponse = serde_json::from_str(&live).expect("the redeemed response shape must parse");

            assert_eq!(
                parsed.status,
                LookupStatus::AlreadyUsed,
                "misreading `{status}` would either refuse a student their own code \
                 (`bound_this_device` read as Unknown) or lose the reason entirely"
            );
            assert_eq!(parsed.tier.as_deref(), Some("strike"));
        }
    }

    /// Every status the hub emits must round-trip. An unrecognised one degrades
    /// to `Unknown`, which is safe (not "ready") but silently loses the reason —
    /// so a new status on the hub side should fail here rather than in the field.
    #[test]
    fn every_hub_status_maps_to_a_known_variant() {
        for (wire, expected) in [
            ("unbound", LookupStatus::Unbound),
            ("ok", LookupStatus::Ok),
            ("bound_this_device", LookupStatus::AlreadyUsed),
            ("already_used", LookupStatus::AlreadyUsed),
            ("suspended", LookupStatus::Suspended),
            ("expired", LookupStatus::Expired),
            ("not_found", LookupStatus::NotFound),
        ] {
            let json = format!(r#"{{"status":"{wire}"}}"#);
            let parsed: LookupResponse = serde_json::from_str(&json).expect("must parse");
            assert_eq!(parsed.status, expected, "{wire} mapped to the wrong variant");
        }
    }

    /// A brand-new status must degrade to `Unknown`, never to `Ok`. Guessing
    /// "ready" tells a student to activate a code that will be refused.
    #[test]
    fn an_unrecognised_status_degrades_safely() {
        let json = r#"{"status":"some_future_state"}"#;
        let parsed: LookupResponse = serde_json::from_str(json).expect("must still parse");
        assert_eq!(parsed.status, LookupStatus::Unknown);
    }

    /// The running build must be able to name its own platform, or the updater
    /// cannot choose an artifact.
    #[test]
    fn the_running_platform_is_known() {
        assert!(
            current_platform().is_some(),
            "unsupported build target: the updater would have no artifact to fetch"
        );
    }

    /// Every platform the hub can advertise must have an artifact, or a client
    /// on it is offered an update it can never fetch.
    #[test]
    fn every_platform_key_has_an_artifact() {
        for key in [
            PLATFORM_LINUX,
            PLATFORM_WINDOWS,
            PLATFORM_MACOS_INTEL,
            PLATFORM_MACOS_ARM,
        ] {
            assert!(artifact_for(key).is_some(), "no artifact for platform {key}");
        }
    }

    /// **The canonical name and the legacy name must stay distinguishable.**
    ///
    /// `LEGACY_HUB_URL` is never dialled by this client — it is here so the
    /// server-side commitment is recorded where wire names live, and so
    /// `check-consistency.sh` §32 can assert the hub still serves it. Two ways
    /// that arrangement silently stops working, and neither is visible at
    /// runtime:
    ///
    ///   * someone "tidies" the two constants together — then the record of what
    ///     the deployed fleet dials is gone, and a future cleanup removes the
    ///     server block that a thousand devices need;
    ///   * the legacy value is edited to the canonical one — the checker then
    ///     compares a name against itself and passes forever.
    ///
    /// Both are asserted here rather than left to review, because the failure
    /// they cause is a stranded fleet, and it surfaces months later.
    #[test]
    fn the_legacy_hub_name_is_distinct_and_still_a_hub_url() {
        assert_ne!(
            HUB_URL, LEGACY_HUB_URL,
            "the canonical and legacy hub names must differ — collapsing them \
             erases the record of what pre-3.3.2 builds dial"
        );
        assert!(
            LEGACY_HUB_URL.starts_with("https://"),
            "the legacy hub must be the https origin it was, not a rewritten form"
        );
        // The legacy name is a host only: no path, no trailing slash. The server
        // block is generated from it, and a stray path there would produce an
        // address Caddy treats as a non-host site.
        let host = LEGACY_HUB_URL.trim_start_matches("https://").trim_end_matches('/');
        assert!(!host.contains('/'), "the legacy hub must be a bare host, got {host}");
    }
}
