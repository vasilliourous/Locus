//! Turning a tier payload into the mihomo configuration the core runs.
//!
//! # The shape of the problem
//!
//! The hub hands the client a tier: a server, a port, a password, a cipher, and
//! optionally a UDP-over-TCP endpoint. Mihomo needs a *config document* with a
//! proxy, a proxy group and rules. This module is the translation, and only the
//! translation.
//!
//! # What this module deliberately does NOT do
//!
//! It does not write the runtime config, apply it, validate it, restart the
//! core, or decide how the core is hosted. Verge already does all of that, well,
//! in `core::manager::config`:
//!
//! ```text
//! update_config_forced() -> validate_and_apply()
//!                        -> apply_config() | apply_config_by_service()
//!                        -> reload_or_restart()
//! ```
//!
//! That path brings config validation, the privileged-staging decision and
//! reload-vs-restart policy for free, and every one of those was a source of
//! field bugs in the retired client. A second apply path would re-earn all of
//! them, so this module stops at producing a document.
//!
//! # Where the document goes
//!
//! Into the same profile slot Verge already uses (`app_profiles_dir` + the
//! `profiles.yaml` current pointer). That means the whole `enhance` pipeline —
//! merge, script, rules, DNS, TUN settings — keeps working unmodified, and there
//! is exactly one code path from "a tier arrived" to "the core is running it".

use crate::locus::contract::TierConfig;
use serde_yaml_ng::{Mapping, Value};

/// A mihomo proxy document for one tier, plus the metadata the UI needs.
#[derive(Debug, Clone)]
pub struct TierProfile {
    /// The profile document, ready to be written as YAML.
    pub yaml: String,
    /// The proxy group's name, so the UI can refer to it.
    pub group_name: String,
}

/// The name of the generated proxy. Kept stable so a config refresh replaces it
/// rather than accumulating duplicates.
pub const PROXY_NAME: &str = "Locus";

/// The name of the generated proxy group.
///
/// **Must differ from [`PROXY_NAME`].** mihomo treats a group whose name matches
/// a proxy inside it as a reference loop and refuses the entire configuration
/// with:
///
/// ```text
/// loop is detected in ProxyGroup, please check following ProxyGroups: [Locus]
/// ```
///
/// which surfaces on the device as a tunnel that never starts. Verified against
/// the real mihomo sidecar with `-t`, which is the only check that catches this —
/// the document is valid YAML either way.
pub const GROUP_NAME: &str = "Locus Auto";

/// The name of the UDP-over-TCP outbound, when a tier has one.
pub const UOT_PROXY_NAME: &str = "Locus-UoT";

/// Every outbound a tier profile can define, in the order
/// [`proxy_group`] lists them.
///
/// This exists so the egress probe can ask about each member **individually**
/// instead of about the group. mihomo's group delay tests only the group's
/// currently selected member, and the default selection is the first entry — so a
/// probe that asked the group could be answered by an outbound the student's
/// traffic never uses, and report a working tunnel as dead (see
/// `core/manager/probe.rs`).
///
/// It is deliberately the same two names the profile builder uses rather than a
/// hand-written list: a name here that the profile never defines would make the
/// probe ask about a proxy mihomo does not have, and the compiler cannot catch
/// that for a string key. `every_outbound_the_probe_asks_about_is_one_the_profile_defines`
/// pins the agreement against both tier shapes.
///
/// A TCP-only tier defines only [`PROXY_NAME`]; asking about [`UOT_PROXY_NAME`]
/// there is harmless — mihomo answers a non-2xx for an unknown proxy, which the
/// per-member route returns as the zero sentinel, so it reads as "this member
/// failed" and the probe moves on to one that exists. It is *not* harmless in the
/// other direction, which is why the test checks every name is defined rather
/// than checking only that some are.
pub const OUTBOUND_NAMES: &[&str] = &[PROXY_NAME, UOT_PROXY_NAME];

/// Builds the mihomo document for a tier.
///
/// `udp_relay` comes from the activation/heartbeat payload rather than the tier
/// config, because it is a separate column on the hub. It is passed in so the
/// one place that decides whether UoT is active stays
/// [`TierConfig::uot_enabled`].
#[must_use]
pub fn build_profile(config: &TierConfig, udp_relay: bool) -> TierProfile {
    let mut proxies: Vec<Value> = vec![shadowsocks_proxy(
        PROXY_NAME,
        config,
        config.server_port,
        // UDP is only carried by this outbound when there is no UoT endpoint;
        // with one, UDP is pinned to the dedicated outbound below instead.
        !config.uot_enabled(udp_relay),
    )];

    // A tier with UoT gets a second outbound that speaks udp-over-tcp, and a
    // rule pinning UDP to it. TCP keeps using the normal proxy, so the working
    // path never changes — only UDP traffic takes the new route. That
    // additivity is what makes UoT safe to enable on an existing deployment.
    if config.uot_enabled(udp_relay) {
        let mut uot = shadowsocks_proxy(UOT_PROXY_NAME, config, config.uot_port, true);
        if let Value::Mapping(map) = &mut uot {
            insert(map, "udp-over-tcp", Value::Bool(true));
            // Pin the protocol revision explicitly rather than letting the
            // engine default it.
            //
            // The hub's listener is sing-box, which accepts either magic domain
            // (`sp.udp-over-tcp.arpa` = v1, `sp.v2.udp-over-tcp.arpa` = v2). v2
            // is what the live deployment actually negotiates — the server log
            // records `inbound connection to sp.v2.udp-over-tcp.arpa:0` — and it
            // is the revision with the connection-reuse and framing fixes that
            // matter for many small datagrams.
            //
            // Naming it is not a functional change today; it removes a silent
            // dependency on the engine's default, so a future mihomo that shifts
            // that default cannot quietly move the fleet onto the older framing.
            insert(map, "udp-over-tcp-version", Value::Number(2.into()));
        }
        proxies.push(uot);
    }

    let group = proxy_group(config.uot_enabled(udp_relay));

    // UDP is pinned to the UoT outbound FIRST, then everything else falls
    // through to the group. Order is load-bearing: mihomo takes the first
    // matching rule, so a catch-all before this one would swallow game traffic
    // before it could be routed over TCP.
    //
    // This rule used to be produced by `uot_udp_rules` but never inserted into
    // the document, which left the profile relying on the group's default
    // member — and the default member is the TCP proxy, built with `udp: false`
    // because UoT owns UDP. The result was a tier that advertised UDP-over-TCP
    // and never used it: the exact shape of failure this codebase has hit
    // before (FIXES 29 / the hardcoded `udp_relay`).
    let mut rules: Vec<Value> = uot_udp_rules(config, udp_relay)
        .into_iter()
        .map(Value::String)
        .collect();

    // Point everything else at the group rather than a bare proxy, so the rules
    // and the UI have one stable name to refer to.
    rules.push(Value::String(format!("MATCH,{GROUP_NAME}")));

    let mut root = Mapping::new();
    insert(&mut root, "proxies", Value::Sequence(proxies));
    insert(&mut root, "proxy-groups", Value::Sequence(vec![group]));
    insert(&mut root, "rules", Value::Sequence(rules));

    let yaml = serde_yaml_ng::to_string(&Value::Mapping(root))
        .unwrap_or_else(|_| String::from("# Locus: failed to serialise the tier profile\n"));

    TierProfile {
        yaml,
        group_name: GROUP_NAME.to_owned(),
    }
}

/// Builds one shadowsocks outbound.
fn shadowsocks_proxy(name: &str, config: &TierConfig, port: u16, udp: bool) -> Value {
    let mut proxy = Mapping::new();
    insert(&mut proxy, "name", Value::String(name.into()));
    insert(&mut proxy, "type", Value::String("ss".into()));
    insert(&mut proxy, "server", Value::String(config.server.clone()));
    insert(&mut proxy, "port", Value::Number(port.into()));
    insert(&mut proxy, "cipher", Value::String(config.method.clone()));
    insert(&mut proxy, "password", Value::String(config.password.clone()));
    insert(&mut proxy, "udp", Value::Bool(udp));
    Value::Mapping(proxy)
}

/// Builds the proxy group, with UDP traffic pinned to the UoT outbound when the
/// tier has one.
///
/// The `network: udp` rule is what actually routes game and voice traffic over
/// TCP, and it lives here rather than in a global rule list so it travels with
/// the tier that defines the endpoint. A rule referencing a proxy that does not
/// exist makes mihomo refuse the whole config, so UoT and its rule are added
/// together or not at all.
fn proxy_group(has_uot: bool) -> Value {
    let mut group = Mapping::new();
    insert(&mut group, "name", Value::String(GROUP_NAME.into()));
    insert(&mut group, "type", Value::String("select".into()));

    // Order decides the default. A `select` group with no explicit choice uses
    // its FIRST member, and the UoT outbound is the only member that can carry
    // game UDP through a UDP-blocked network. Listing the plain TCP proxy first
    // therefore defaulted every Strike client to a profile whose UDP went to a
    // proxy built with `udp: false` — UDP-over-TCP advertised and unused.
    //
    // The UoT member goes first when it exists; TCP-only tiers are unaffected
    // (they have one member, and it is the correct one).
    let mut members = Vec::new();
    if has_uot {
        members.push(Value::String(UOT_PROXY_NAME.into()));
    }
    members.push(Value::String(PROXY_NAME.into()));
    insert(&mut group, "proxies", Value::Sequence(members));

    Value::Mapping(group)
}

/// The rules that pin UDP to the UoT outbound.
///
/// Separate from [`build_profile`] because it is applied by the profile's rule
/// chain rather than the profile document, and returning it explicitly keeps the
/// dependency between "UoT exists" and "the UDP rule exists" visible at the call
/// site.
///
/// # Why the UoT path is a separate, non-resolving rule
///
/// The UoT outbound is **one TCP connection** carrying every UDP flow
/// multiplexed over it, so a stall in that connection (a retransmit after loss)
/// blocks everything behind it. That head-of-line blocking is the mechanism
/// behind the "rubberbanding" reported on small-packet games, and its severity
/// scales with how much unrelated traffic shares the one connection.
///
/// `no-resolve` keeps the rule from performing a DNS lookup just to evaluate
/// itself. Without it, matching a UDP rule can trigger a resolution of the
/// destination — an operation that reaches the network through the very path
/// being matched, which is both wasteful and circular for a rule whose whole
/// purpose is to route packets that already have addresses.
///
/// The DNS-hijack in the profile still sends port 53 to the engine's resolver,
/// so ordinary name resolution does not depend on this rule at all.
#[must_use]
pub fn uot_udp_rules(config: &TierConfig, udp_relay: bool) -> Vec<String> {
    if !config.uot_enabled(udp_relay) {
        return Vec::new();
    }

    vec![format!("NETWORK,udp,{UOT_PROXY_NAME},no-resolve")]
}

/// Inserts a key, replacing any existing value.
fn insert(map: &mut Mapping, key: &str, value: Value) {
    map.insert(Value::String(key.into()), value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tcp_only_tier() -> TierConfig {
        TierConfig {
            server: "networkingguides.duckdns.org".into(),
            server_port: 8443,
            password: "shared-secret".into(),
            method: "2022-blake3-aes-256-gcm".into(),
            uot_port: 0,
        }
    }

    fn strike_tier() -> TierConfig {
        TierConfig {
            server: "networkingguides.duckdns.org".into(),
            server_port: 8445,
            password: "strike-secret".into(),
            method: "2022-blake3-aes-256-gcm".into(),
            uot_port: 8446,
        }
    }

    /// The generated document must be valid YAML, or the core refuses to start
    /// and the student sees an opaque engine error.
    #[test]
    fn produces_valid_yaml() {
        let profile = build_profile(&tcp_only_tier(), false);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("the tier profile must be valid YAML");
        assert!(parsed.get("proxies").is_some());
        assert!(parsed.get("proxy-groups").is_some());
        assert!(parsed.get("rules").is_some());
    }

    /// The proxy must carry the credentials the hub handed us, or the tunnel
    /// dials and is rejected.
    #[test]
    fn the_proxy_carries_the_tier_credentials() {
        let profile = build_profile(&tcp_only_tier(), false);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");

        let proxy = &parsed["proxies"][0];
        assert_eq!(proxy["name"], "Locus");
        assert_eq!(proxy["type"], "ss");
        assert_eq!(proxy["server"], "networkingguides.duckdns.org");
        assert_eq!(proxy["port"], 8443);
        assert_eq!(proxy["cipher"], "2022-blake3-aes-256-gcm");
        assert_eq!(proxy["password"], "shared-secret");
    }

    /// A tier without UoT must be a single plain proxy: adding a second
    /// outbound that points at a port nothing listens on sends game traffic
    /// into a black hole until it times out.
    #[test]
    fn a_tier_without_uot_has_no_uot_outbound() {
        let profile = build_profile(&tcp_only_tier(), false);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");

        assert_eq!(parsed["proxies"].as_sequence().map(Vec::len), Some(1));
        assert!(
            uot_udp_rules(&tcp_only_tier(), false).is_empty(),
            "a tier with no UoT endpoint must not get a UDP rule pointing at it"
        );
    }

    /// A tier with UoT gets a second outbound on the UoT port, with
    /// `udp-over-tcp` enabled — this is the entire mechanism behind the Strike
    /// gaming tier.
    #[test]
    fn a_tier_with_uot_gets_a_udp_over_tcp_outbound() {
        let profile = build_profile(&strike_tier(), true);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");

        let proxies = parsed["proxies"].as_sequence().expect("proxies list");
        assert_eq!(proxies.len(), 2, "UoT adds exactly one outbound");

        let uot = &proxies[1];
        assert_eq!(uot["name"], "Locus-UoT");
        assert_eq!(
            uot["port"], 8446,
            "the UoT outbound must use uot_port, not the main port"
        );
        assert_eq!(
            uot["udp-over-tcp"], true,
            "without udp-over-tcp this is just another raw UDP outbound"
        );
    }

    /// The UoT protocol revision is named explicitly rather than left to the
    /// engine's default.
    ///
    /// The hub's listener negotiates v2 (`sp.v2.udp-over-tcp.arpa` in the live
    /// server log). Pinning it here means a future mihomo that changes its
    /// default cannot silently move the fleet back to the older framing.
    #[test]
    fn the_uot_outbound_names_its_protocol_version() {
        let profile = build_profile(&strike_tier(), true);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");

        assert_eq!(
            parsed["proxies"][1]["udp-over-tcp-version"], 2,
            "the UoT outbound must pin the protocol revision the hub speaks"
        );
    }

    /// The group must default to the UoT outbound, because a `select` group with
    /// no explicit choice uses its FIRST member — and the plain TCP proxy is
    /// built with `udp: false`. Listing it first defaulted every Strike client
    /// to a profile whose UDP went to a member that cannot carry it.
    #[test]
    fn the_group_defaults_to_the_uot_outbound_when_it_exists() {
        let profile = build_profile(&strike_tier(), true);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");

        let members: Vec<&str> = parsed["proxy-groups"][0]["proxies"]
            .as_sequence()
            .expect("group members")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(
            members.first().copied(),
            Some(UOT_PROXY_NAME),
            "a select group defaults to its first member, which must be the UoT outbound"
        );
    }

    /// The UoT endpoint is an extra listener, not a replacement. Pointing the
    /// ordinary TCP path at it would move all traffic onto the UoT port, which
    /// only speaks the UoT framing.
    #[test]
    fn the_main_proxy_still_uses_the_standard_port() {
        let profile = build_profile(&strike_tier(), true);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");

        assert_eq!(
            parsed["proxies"][0]["port"], 8445,
            "TCP must keep using server_port even when a tier has UoT"
        );
    }

    /// Both halves of the UoT condition are required. `udp_relay` without a port
    /// is a half-configured tier, and following it would build an outbound on
    /// port 0.
    #[test]
    fn udp_relay_alone_does_not_enable_uot() {
        let profile = build_profile(&strike_tier(), false);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");

        assert_eq!(
            parsed["proxies"].as_sequence().map(Vec::len),
            Some(1),
            "the flag without a port must not add an outbound"
        );
        assert!(uot_udp_rules(&strike_tier(), false).is_empty());
    }

    /// A port without the flag is equally incomplete.
    #[test]
    fn a_port_alone_does_not_enable_uot() {
        let mut tier = strike_tier();
        tier.uot_port = 8446;
        assert!(!tier.uot_enabled(false));
        assert!(uot_udp_rules(&tier, false).is_empty());
    }

    /// When UoT is on, the rule must name the UoT outbound, not the group or the
    /// main proxy — otherwise UDP goes out raw and the school network drops it.
    #[test]
    fn the_udp_rule_pins_traffic_to_the_uot_outbound() {
        let rules = uot_udp_rules(&strike_tier(), true);
        assert_eq!(rules.len(), 1, "a UoT tier produces exactly one UDP rule");
        assert!(
            rules[0].contains(UOT_PROXY_NAME),
            "the rule must name the UoT outbound, got {rules:?}"
        );
    }

    /// The UDP rule must actually reach the generated document.
    ///
    /// It did not, once: `uot_udp_rules` existed and was tested, but nothing
    /// inserted its output into the profile, so the shipped config advertised
    /// UDP-over-TCP and never routed anything to it. A rule that exists only in
    /// a function is not a rule.
    #[test]
    fn the_udp_rule_is_present_in_the_generated_document() {
        let profile = build_profile(&strike_tier(), true);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");

        let rules = parsed["rules"].as_sequence().expect("rules list");
        let rendered: Vec<&str> = rules.iter().filter_map(Value::as_str).collect();

        assert!(
            rendered.iter().any(|rule| rule.starts_with("NETWORK,udp,")),
            "the UDP rule must be in the document, got {rendered:?}"
        );
    }

    /// The UDP rule must come BEFORE the catch-all, or the catch-all wins and
    /// game traffic never reaches the UoT outbound.
    #[test]
    fn the_udp_rule_precedes_the_catch_all() {
        let profile = build_profile(&strike_tier(), true);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");
        let rules: Vec<&str> = parsed["rules"]
            .as_sequence()
            .expect("rules list")
            .iter()
            .filter_map(Value::as_str)
            .collect();

        let udp_at = rules.iter().position(|rule| rule.starts_with("NETWORK,udp,"));
        let match_at = rules.iter().position(|rule| rule.starts_with("MATCH,"));
        assert!(
            udp_at.is_some() && match_at.is_some() && udp_at < match_at,
            "the UDP rule must precede the catch-all, got {rules:?}"
        );
    }

    /// The group must offer the UoT outbound when it exists, or the rule's
    /// target is unreachable from the group and mihomo rejects the config.
    #[test]
    fn the_group_lists_the_uot_outbound_when_it_exists() {
        let with_uot = build_profile(&strike_tier(), true);
        let parsed: Value = serde_yaml_ng::from_str(&with_uot.yaml).expect("valid YAML");
        let mut members = parsed["proxy-groups"][0]["proxies"]
            .as_sequence()
            .expect("group members")
            .iter()
            .filter_map(|v| v.as_str());
        assert!(members.any(|member| member == UOT_PROXY_NAME));

        let without = build_profile(&tcp_only_tier(), false);
        let parsed: Value = serde_yaml_ng::from_str(&without.yaml).expect("valid YAML");
        let members = parsed["proxy-groups"][0]["proxies"]
            .as_sequence()
            .expect("group members")
            .iter()
            .filter_map(|v| v.as_str());
        assert!(
            !members.into_iter().any(|member| member == UOT_PROXY_NAME),
            "a group listing a proxy that does not exist makes mihomo refuse the config"
        );
    }

    /// The proxy group and the proxy inside it must NOT share a name.
    ///
    /// mihomo reads a group whose name matches a member as a reference loop and
    /// refuses the whole configuration:
    ///
    ///     loop is detected in ProxyGroup, please check following ProxyGroups: [Locus]
    ///
    /// This is invisible in YAML terms — the document is perfectly valid — and
    /// surfaces only on the device, as a tunnel that never starts. It was caught
    /// by running the real mihomo sidecar with `-t` against generated output,
    /// which is why the rule is pinned here as a test rather than a comment.
    #[test]
    fn the_group_name_does_not_collide_with_its_members() {
        assert_ne!(
            GROUP_NAME, PROXY_NAME,
            "a group named the same as a proxy inside it is a reference loop to mihomo"
        );
        assert_ne!(GROUP_NAME, UOT_PROXY_NAME);

        for udp_relay in [false, true] {
            let config = if udp_relay { strike_tier() } else { tcp_only_tier() };
            let profile = build_profile(&config, udp_relay);
            let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");

            let group_name = parsed["proxy-groups"][0]["name"]
                .as_str()
                .expect("group must have a name");
            let members = parsed["proxy-groups"][0]["proxies"]
                .as_sequence()
                .expect("group members")
                .iter()
                .filter_map(|v| v.as_str());

            assert!(
                !members.into_iter().any(|member| member == group_name),
                "group {group_name:?} lists itself, which mihomo rejects as a loop"
            );
        }
    }

    /// Every name the egress probe asks about must be either an outbound the
    /// profile defines, or one the probe can safely be told does not exist.
    ///
    /// The probe queries mihomo by proxy name (`/proxies/<name>/delay`), so a
    /// name in [`OUTBOUND_NAMES`] that `build_profile` never writes is a request
    /// for a proxy that does not exist. That is *safe* on the per-member route —
    /// mihomo answers a non-2xx, the plugin maps it to the zero sentinel, the
    /// probe reads it as "this member failed" and tries the next one — but it is
    /// only safe because the probe treats an unusable answer as a *member*
    /// failure rather than a verdict on the tunnel.
    ///
    /// So this test pins both halves of that reasoning:
    ///
    ///  1. the tier's own outbound ([`PROXY_NAME`]) is always defined **and**
    ///     listed in the group — if the probe's first candidate did not exist,
    ///     every check would pay a wasted round trip before finding a real one;
    ///  2. the optional outbound is defined exactly when the tier enables UoT, so
    ///     the two lists cannot drift apart.
    ///
    /// A string key gives the compiler nothing to check, which is why it is
    /// checked here.
    #[test]
    fn every_outbound_the_probe_asks_about_is_one_the_profile_defines() {
        assert!(
            OUTBOUND_NAMES.contains(&PROXY_NAME),
            "the probe must ask about the tier's own outbound first, or every \
             check wastes a round trip on a proxy that may not exist"
        );
        assert!(
            OUTBOUND_NAMES.contains(&UOT_PROXY_NAME),
            "the probe must ask about the UoT outbound, or a tier whose traffic \
             goes over UoT could read as having no egress"
        );

        for udp_relay in [false, true] {
            let config = if udp_relay { strike_tier() } else { tcp_only_tier() };
            let profile = build_profile(&config, udp_relay);
            let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");

            let defined: Vec<&str> = parsed["proxies"]
                .as_sequence()
                .expect("proxies list")
                .iter()
                .filter_map(|p| p["name"].as_str())
                .collect();

            // The tier's own proxy is unconditional.
            assert!(
                defined.contains(&PROXY_NAME),
                "every tier defines {PROXY_NAME:?}; got {defined:?}"
            );
            // UoT is defined exactly when the tier enables it, so this pair
            // cannot silently disagree with `uot_enabled`.
            let expected_uot = config.uot_enabled(udp_relay);
            assert_eq!(
                defined.contains(&UOT_PROXY_NAME),
                expected_uot,
                "the profile's UoT outbound presence must match `uot_enabled` \
                 (udp_relay={udp_relay}, defined={defined:?})"
            );

            // Everything defined is reachable: a member outside the group is a
            // proxy the student's traffic can never use, which would make an
            // "egress proven" answer misleading.
            let members: Vec<&str> = parsed["proxy-groups"][0]["proxies"]
                .as_sequence()
                .expect("group members")
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            for name in &defined {
                assert!(
                    members.contains(name),
                    "outbound {name:?} is defined but not listed in the group"
                );
            }
        }
    }

    /// The MATCH rule must name the GROUP, not the proxy. Naming the proxy would
    /// bypass the group entirely and take the UDP pinning with it, so a Strike
    /// student would silently get raw UDP — the exact thing the tier exists to
    /// avoid.
    #[test]
    fn the_match_rule_routes_through_the_group() {
        let profile = build_profile(&strike_tier(), true);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");
        let rules: Vec<&str> = parsed["rules"]
            .as_sequence()
            .expect("rules list")
            .iter()
            .filter_map(Value::as_str)
            .collect();

        // The catch-all is the LAST rule, not rule 0: the UDP pin is emitted
        // first so game traffic is claimed before anything general can take it.
        let rule = rules.last().expect("at least one rule");
        assert!(
            rule.contains(GROUP_NAME),
            "the catch-all rule must name the group ({GROUP_NAME}), got {rule:?}"
        );
    }

    /// Regenerating from the same tier must produce the same document. If it did
    /// not, every heartbeat would look like a config change and restart the core
    /// every five minutes.
    #[test]
    fn generation_is_deterministic() {
        let first = build_profile(&strike_tier(), true);
        let second = build_profile(&strike_tier(), true);
        assert_eq!(first.yaml, second.yaml);
    }

    /// The generated document must survive a YAML round trip with the
    /// credentials intact — passwords are hex-ish but could contain characters
    /// that need quoting.
    #[test]
    fn credentials_survive_a_yaml_round_trip() {
        let mut tier = tcp_only_tier();
        tier.password = "p@ss:word#with\"specials".into();

        let profile = build_profile(&tier, false);
        let parsed: Value = serde_yaml_ng::from_str(&profile.yaml).expect("valid YAML");
        assert_eq!(parsed["proxies"][0]["password"], tier.password);
    }
}
