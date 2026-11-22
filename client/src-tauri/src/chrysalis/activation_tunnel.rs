//! The client half of the pre-activation activation tunnel.
//!
//! # The problem this solves
//!
//! A student on a school network could not activate. Measured from that network
//! on 2026-10-29: `https://<hub>/api/health` on 443 gets the TLS ClientHello sent
//! and then **"Connection reset by peer"**, and 8443 spoken to in plaintext gets
//! the filter's own `503 Application Control Violation`. Both blocks fire because
//! a **hostname is visible** — 443 in the TLS SNI, 8443 in the Host header. The
//! student's existing tunnel works because a Shadowsocks stream presents no name
//! to anything.
//!
//! Activation is what *delivers* the tunnel credentials, so there is no tunnel to
//! ride before activating. The hub grows a bootstrap listener on `:8453`
//! (`server/modules/09-activation-tunnel.sh`) carrying only the enrolment
//! endpoints; this module is the client that rides it.
//!
//! # Shape
//!
//! ```text
//! post_json / beat
//!      │ 1. direct HTTPS — the normal path
//!      ▼
//!   Failure::ResetDuringHandshake?
//!      │ yes                              │ no
//!      ▼                                  ▼
//!   tunnel request                     report as today
//!      │
//!      ▼
//!   mihomo (SOCKS, loopback) ──SS──▶ hub:8453 ──▶ allowlisted endpoints
//!      │
//!      └─ HTTP to http://127.0.0.1:<forward_port>
//! ```
//!
//! # Why the engine carries it rather than a Rust Shadowsocks client
//!
//! The engine is already vendored, already audited for this key format, and
//! already proven against the hub's listener by
//! `client/src-tauri/tests/tunnel_end_to_end.rs`. A second SS2022 implementation
//! in Rust would be a second place for the cipher, the key format and the
//! UDP-over-TCP revision to disagree — the exact class of defect that took the
//! UDP fleet down once already. See `docs/reference/ACTIVATION-TUNNEL.md` §6.
//!
//! # Why the engine is a SEPARATE instance from the Core
//!
//! The Core is the student's tunnel, and before activation it does not exist —
//! that is the whole problem. It also runs only after the Service is installed,
//! which can require elevation the app may not have on a fresh install. This
//! instance:
//!
//!   * listens on loopback only, on an ephemeral port;
//!   * has exactly one proxy and one rule (`MATCH`), so it cannot be used as a
//!     general-purpose proxy even if something found the port;
//!   * is started on demand and **killed on drop**, so it does not outlive the
//!     call that needed it;
//!   * has no TUN, no DNS hijack and no service, so it never collides with the
//!     Core or with mihomo's port allocation.
//!
//! # What this module does NOT do
//!
//! It is not the update path. Tunnelling updates is a separate decision with its
//! own signature surface. Scope is `/api/activate`, `/api/code-lookup` and
//! `/api/heartbeat` — exactly the hub's allowlist.

use anyhow::{Context as _, Result, anyhow};
use serde::{Serialize, de::DeserializeOwned};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The hub port the bootstrap listener occupies. Mirrors
/// `ACTIVATION_TUNNEL_PORT` in `server/modules/09-activation-tunnel.sh`; the
/// agreement is asserted by `check-consistency.sh` §40.
///
/// This is the port the SS outbound DIALS to reach the listener. It is **not** the
/// destination of a request — that is [`FILTER_PORT`].
pub const TUNNEL_PORT: u16 = 8453;

/// The port the hub's **path filter** listens on, inside the hub.
///
/// This is the destination a tunnelled request must name, and it is deliberately
/// not the same value as [`TUNNEL_PORT`].
///
/// # Why the destination is the filter and not the listener
///
/// `ssserver` has **no inbound forwarding**. Measured on 2026-11-01 against
/// `shadowsocks-rust` 1.25.0: it decrypts the stream and then connects to
/// whatever destination the client named, logging
/// `tcp tunnel <peer> -> <requested destination>`. The `local_addr`/`local_port`
/// keys in its config are `sslocal`'s SOCKS listen options, not a server-side
/// forward, and `ssserver` ignores them.
///
/// So the path filter is reached because the CLIENT asks for its address, which is
/// what makes the filter — and therefore the allowlist that bounds a public
/// bootstrap key — actually sit in the path. Naming the SS port here would loop the
/// decrypted stream back at the listener that terminates it.
///
/// `ACTIVATION_FORWARD_PORT` in `server/modules/09-activation-tunnel.sh`;
/// `check-consistency.sh` §40 asserts the two agree.
pub const FILTER_PORT: u16 = 18090;

/// The bootstrap key, compiled in.
///
/// **Absent by default, and absent in every build made from a clean checkout.**
/// The value lives in `server/secrets.env.age` on the operator's machine (see
/// `docs/operate/SECRETS-MANAGEMENT.md`); CI injects it as the
/// `CHRYSALIS_ACTIVATION_TUNNEL_KEY` build-time environment variable.
///
/// It is read through `option_env!` rather than `env!` on purpose: a missing key
/// must make the **tunnel** unavailable, not break the **build**. A client
/// without it is exactly as capable as every client shipped before this feature.
/// `activation_tunnel_available()` is what callers check.
///
/// **The value reaching this constant is `build.rs`'s job** (rewritten
/// 2026-11-06). It re-emits the environment variable as a `rustc-env` with an
/// explicit `cargo:rerun-if-env-changed`, so a key that appears after a build has
/// already cached its objects forces the rebuild. Reading the variable directly
/// here left the const stale across a cached build, which is how a release could
/// be built with a key in the environment and ship without one.
///
/// A tracked credential is a build failure — `check-consistency.sh` §29 guards
/// it, and this constant is one of the things it reads.
///
/// **This value is not secret in the usual sense.** It is shipped in every
/// binary that has it, so it must be assumed public. What makes that acceptable
/// is the hub's allowlist and rate limit, not the key — see the module docs in
/// `09-activation-tunnel.sh`.
const BOOTSTRAP_KEY: Option<&str> = option_env!("CHRYSALIS_ACTIVATION_TUNNEL_KEY");

/// Whether the build that produced THIS binary carried a bootstrap key.
///
/// Stamped by `build.rs`, and deliberately separate from
/// [`BOOTSTRAP_KEY`]: the key itself must never be rendered anywhere, but the
/// *fact* that it is missing has to be readable from a shipped artifact. A
/// release built without it cannot use the one transport that exists for a
/// network blocking the hub by name, and nothing else in the binary says so —
/// which is exactly how three releases went out keyless.
///
/// Rendered by [`build_stamp`] and logged at startup.
const BOOTSTRAP_KEY_PRESENT: &str = env!("CHRYSALIS_ACTIVATION_TUNNEL_KEY_PRESENT");

/// The hub's free-tier listener — the `free` plan's own shadowsocks port.
///
/// Mirrors `write_config "eco" 8443 "ECO_PASS" "tcp_only"` in
/// `server/modules/02-shadowsocks.sh:279`. Unlike [`TUNNEL_PORT`] this listener is
/// a **general relay**: it forwards to whatever destination the client names, and
/// the only limit on it is the tier's tc cap. That is what makes it usable as the
/// maintenance tunnel and as the `dialer-proxy` hop for the plan outbound.
const FREE_TIER_PORT: u16 = 8443;

/// The SS2022 cipher, matching `SS_METHOD` in the server modules.
const TUNNEL_METHOD: &str = "2022-blake3-aes-256-gcm";

/// How long to wait for the engine's SOCKS listener to accept a connection.
///
/// Generous: the engine is a 60 MB binary on a school machine that may be
/// loading it from a cold page cache. A timeout here is not an error the student
/// can act on, so it must be long enough to be rare.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// How often to poll for readiness while starting.
const STARTUP_POLL: Duration = Duration::from_millis(50);

/// Whether this build can use the tunnel at all.
///
/// False when the bootstrap key was not compiled in. Public so a call site can
/// avoid even attempting the fallback, and so the activation screen can say
/// something true rather than pretending the tunnel exists.
#[must_use]
pub fn activation_tunnel_available() -> bool {
    BOOTSTRAP_KEY.is_some_and(|key| !key.is_empty())
}

/// Whether this build carries the **maintenance** tunnel's key.
///
/// The companion to [`activation_tunnel_available`], and deliberately separate:
/// the two tunnels use different credentials against different listeners, and a
/// build can have one and not the other. Nothing reported that before 2026-11-19 —
/// `Bootstrap::maintenance()` simply returned `None` and the routing rule took the
/// direct branch, which on a censored network is indistinguishable from the
/// network refusing everything. See `build_stamp`.
#[must_use]
pub fn maintenance_tunnel_available() -> bool {
    option_env!("CHRYSALIS_FREE_TIER_KEY").is_some_and(|key| !key.is_empty())
}

/// What the build script said about the free-tier key.
///
/// Same two-way agreement as [`BOOTSTRAP_KEY_PRESENT`], and for the same reason:
/// a cached rebuild whose environment changed without this crate being rebuilt is
/// the one condition where the source and the build script disagree.
const FREE_TIER_KEY_PRESENT: &str = env!("CHRYSALIS_FREE_TIER_KEY_PRESENT");

/// One line describing this build's bootstrap capability, for the support log.
///
/// The key is never rendered — only whether one was compiled in, and whether the
/// build script agrees. Those two can disagree for exactly one reason: a cached
/// build whose environment changed without this crate being rebuilt. That is the
/// condition `build.rs`'s `rerun-if-env-changed` removes, and this line is what
/// would have made it visible instead of it arriving as "the tunnel never
/// triggers".
#[must_use]
pub fn build_stamp() -> String {
    let key_in_build = activation_tunnel_available();
    let build_script_says = BOOTSTRAP_KEY_PRESENT == "1";
    // The maintenance tunnel's key, on the SAME line since 2026-11-19.
    //
    // Why it belongs here rather than in its own message: on the network these
    // exist for, the two failures are indistinguishable to a student and were
    // previously indistinguishable in a log too. Measured 2026-11-19 — the
    // maintenance tunnel DOES work from the school network when driven by hand, so
    // "no key in this build" is a real, presently-possible explanation for the app
    // failing where a hand-built config succeeds, and nothing said so.
    let maintenance_in_build = maintenance_tunnel_available();
    let maintenance_script_says = FREE_TIER_KEY_PRESENT == "1";
    let maintenance = if maintenance_in_build == maintenance_script_says {
        if maintenance_in_build {
            "available"
        } else {
            "NOT COMPILED IN"
        }
    } else {
        "INCONSISTENT BUILD — rebuild before shipping it"
    };
    if key_in_build == build_script_says {
        format!(
            "activation tunnel: {}, maintenance tunnel: {}, listener port {} → filter port {}",
            if key_in_build { "available" } else { "NOT COMPILED IN" },
            maintenance,
            TUNNEL_PORT,
            FILTER_PORT
        )
    } else {
        format!(
            "activation tunnel: INCONSISTENT BUILD — key present={key_in_build}, build script \
             reported {BOOTSTRAP_KEY_PRESENT}. The tunnel is {} in this binary; rebuild before \
             shipping it. (maintenance tunnel: {maintenance})",
            if key_in_build { "usable" } else { "unusable" }
        )
    }
}

/// A running activation-tunnel engine. Killed on drop.
///
/// The `Drop` impl is the contract: this process must not outlive the request
/// that started it. Leaking one would leave a SOCKS proxy on a student's machine
/// for the rest of the session, and on Windows would keep the binary's file
/// handle open, breaking the next update's overwrite of the sidecar.
pub struct Tunnel {
    child: Option<Child>,
    /// The loopback port the engine's SOCKS listener was configured to use.
    forward_port: u16,
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // `kill` on an already-exited child is an error we do not care about;
            // the goal is that it is gone, whichever path got there.
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The credentials one bootstrap engine dials with.
///
/// # Why this is a descriptor and not two functions
///
/// Two tunnels exist and both are the same engine with the same shape: the
/// **activation** tunnel (`:8453`, `ACTIVATION_TUNNEL_KEY`) for the three
/// enrolment endpoints, and the **maintenance** tunnel (`:8443`, `ECO_PASS`) for
/// hub calls while the plan tunnel cannot be built. They differ in exactly three
/// values — address, port, key — so they are one `Tunnel` parameterised by this,
/// not two implementations. A second implementation would be a second place for
/// the cipher, the key format and the SOCKS configuration to disagree, which is
/// the defect class this whole module was written to stop.
#[derive(Debug, Clone)]
pub struct Bootstrap {
    /// The host the engine dials. The compiled address, not the hub's name — see
    /// [`crate::chrysalis::contract::ACTIVATION_TUNNEL_ADDR`] for why a name cannot
    /// be relied on here.
    pub server: String,
    /// The listener's port on that host.
    pub port: u16,
    /// The SS2022 key the listener holds.
    pub key: String,
}

impl Bootstrap {
    /// The activation tunnel: the hub's `:8453` listener, allowlist-bound to the
    /// three enrolment endpoints.
    ///
    /// Returns `None` when this build carries no key. Not an error: a build
    /// without it is the normal case for a branch build, and every caller must
    /// fall through to reporting the original failure rather than pretending the
    /// tunnel exists.
    #[must_use]
    pub fn activation() -> Option<Self> {
        let key = BOOTSTRAP_KEY.filter(|key| !key.is_empty())?;
        Some(Self {
            server: crate::chrysalis::contract::activation_tunnel_addr().to_owned(),
            port: TUNNEL_PORT,
            key: key.to_owned(),
        })
    }

    /// The maintenance tunnel: the hub's free-tier relay on `:8443`.
    ///
    /// # Why this is a DIFFERENT credential from [`Self::activation`]
    ///
    /// `:8453`'s forwarder admits exactly three paths (`ALLOWED` in
    /// `server/modules/09-activation-tunnel.sh:196`) and refuses everything else
    /// with an empty 403. It is a bootstrap for enrolment, not a relay, so it
    /// cannot carry a plan's TCP stream — chaining the plan outbound through it
    /// is what its allowlist exists to prevent. `:8443` is a general relay and is
    /// the one that can. The two keys are different values against different
    /// listeners; neither can stand in for the other.
    ///
    /// Returns `None` when this build carries no free-tier key, on the same terms
    /// as [`Self::activation`].
    #[must_use]
    pub fn maintenance() -> Option<Self> {
        let key = option_env!("CHRYSALIS_FREE_TIER_KEY").filter(|key| !key.is_empty())?;
        Some(Self {
            server: crate::chrysalis::contract::activation_tunnel_addr().to_owned(),
            port: FREE_TIER_PORT,
            key: key.to_owned(),
        })
    }
}

impl Tunnel {
    /// Starts an engine configured to send everything to the activation tunnel,
    /// and waits until its SOCKS listener is accepting.
    ///
    /// Returns `Ok(None)` when this build has no bootstrap key — not an error,
    /// because a build without the key is the normal case and the caller must
    /// fall through to reporting the original failure.
    pub async fn start() -> Result<Option<Self>> {
        match Bootstrap::activation() {
            Some(bootstrap) => Self::start_with(&bootstrap).await,
            None => {
                tracing::debug!(target: "chrysalis::activation_tunnel", "no bootstrap key compiled in; tunnel unavailable");
                Ok(None)
            }
        }
    }

    /// Starts an engine for an arbitrary [`Bootstrap`], and waits until its SOCKS
    /// listener is accepting.
    ///
    /// The single implementation both tunnels share. The body is the activation
    /// tunnel's original; only the credential, the destination port and the
    /// config stem come from `bootstrap`.
    pub async fn start_with(bootstrap: &Bootstrap) -> Result<Option<Self>> {
        let key = bootstrap.key.as_str();
        let destination_port = bootstrap.port;

        let port = free_loopback_port()?;
        let dir = scratch_dir()?;
        // Named per destination port so a maintenance engine and an activation
        // engine never overwrite each other's config while both are alive.
        let config = dir.join(format!("bootstrap-{destination_port}.yaml"));
        // The activation tunnel has two candidates (the hub's name first, then the
        // compiled address) because a DNS-filtering network can refuse the name.
        // The maintenance tunnel has the address only: it is built from the
        // compiled constant by construction, so there is no name to fall back
        // from. Both are one destination listener, which is why this stays one
        // outbound and one group — see `Target` on why that is not a widened
        // surface.
        let targets = if destination_port == TUNNEL_PORT {
            let ss_hub = crate::chrysalis::contract::ss_hub().await;
            bootstrap_targets(&ss_hub, bootstrap.server.as_str())
        } else {
            vec![Target {
                name: format!("hub-{destination_port}"),
                server: bootstrap.server.clone(),
            }]
        };
        std::fs::write(&config, render_config(port, &targets, key, destination_port)?)
            .with_context(|| format!("could not write the activation tunnel config to {}", config.display()))?;
        tracing::debug!(
            target: "chrysalis::activation_tunnel",
            targets = targets.len(),
            // Names and addresses only — never the key. This is the line that
            // makes "the fallback ran and both candidates failed" distinguishable
            // in a support log from "the fallback only ever had a name".
            first = %targets.first().map(|t| t.server.as_str()).unwrap_or(""),
            "activation tunnel bootstrap candidates"
        );

        // Resolved through the same accessor every other call site uses, so the
        // configured Core and this bootstrap engine cannot diverge.
        let core = crate::config::Config::verge().await.latest_arc().get_valid_clash_core();
        let engine = engine_path(&core)?;
        let child = Command::new(&engine)
            .arg("-d")
            .arg(&dir)
            .arg("-f")
            .arg(&config)
            .stdin(Stdio::null())
            // Both streams are discarded rather than inherited: the engine is
            // chatty at info level and this is a bootstrap path that must not
            // spam a student's log when it works. Failures are reported through
            // the request error instead.
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("could not start the activation tunnel engine at {}", engine.display()))?;

        let mut tunnel = Self {
            child: Some(child),
            forward_port: port,
        };

        // Wait for the listener. The engine binds the mixed port last, after it
        // has parsed the config, so a successful TCP connect means it is ready to
        // serve — a file on disk would prove nothing.
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        while Instant::now() < deadline {
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                tracing::info!(
                    target: "chrysalis::activation_tunnel",
                    port,
                    "activation tunnel engine is listening"
                );
                return Ok(Some(tunnel));
            }
            // If the process died, waiting out the full timeout is pointless and
            // misleading.
            if let Some(child) = tunnel.child.as_mut()
                && child.try_wait().ok().flatten().is_some()
            {
                return Err(anyhow!(
                    "the activation tunnel engine exited before it began listening \
                     (check that mihomo can write to {})",
                    dir.display()
                ));
            }
            tokio::time::sleep(STARTUP_POLL).await;
        }

        Err(anyhow!(
            "the activation tunnel engine did not begin listening within {}s",
            STARTUP_TIMEOUT.as_secs()
        ))
    }

    /// The SOCKS address to point a proxied HTTP client at.
    #[must_use]
    pub fn proxy_address(&self) -> String {
        format!("socks5h://127.0.0.1:{}", self.forward_port)
    }

    /// The plain-HTTP base the tunnel presents: the destination the engine dials
    /// is the hub's bootstrap listener, and the path is whatever the caller asked
    /// for.
    ///
    /// A caller must build `{base}{path}`; the port and host are fixed by the
    /// tunnel, never by `contract::hub_url()`, because the whole point is that
    /// the hub's *name* must not appear on the wire.
    #[must_use]
    pub fn http_base(&self) -> String {
        // The destination the hub dials. It is the PATH FILTER, not the SS
        // listener — see `FILTER_PORT` for the measurement that settles this. The
        // engine resolves it inside the SS stream, so nothing here is a DNS lookup
        // from the client, and the hub's name never appears on the wire.
        format!("http://127.0.0.1:{FILTER_PORT}")
    }
}

/// A usable loopback port.
///
/// Binding to port 0 and reading the port back is the only way to be sure the
/// port is free at the moment of the check. It is released immediately and the
/// engine binds it a moment later; a race is possible in principle and would show
/// up as the engine failing to start, which is reported rather than hidden.
fn free_loopback_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .context("could not allocate a loopback port for the activation tunnel")?;
    let port = listener
        .local_addr()
        .context("could not read the allocated loopback port")?
        .port();
    drop(listener);
    Ok(port)
}

/// A directory the engine may use, inside the app's own data directory.
///
/// Not the system temp directory: on Windows that is frequently a different
/// volume with restrictive ACLs, and mihomo writes `cache.db` beside its config.
fn scratch_dir() -> Result<PathBuf> {
    let base = crate::utils::dirs::app_home_dir().context("could not resolve the application data directory")?;
    let dir = base.join("activation-tunnel");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("could not create the activation tunnel directory at {}", dir.display()))?;
    Ok(dir)
}

/// The engine binary.
///
/// Tauri resolves an `externalBin` sidecar **relative to the running
/// executable's own directory**, and `core::validate::sidecar_is_missing` probes
/// for it exactly there. This follows the same rule so the two cannot disagree
/// about where the engine is:
///
///   1. beside the executable — the packaged layout;
///   2. `src-tauri/sidecar/` under the crate — the development layout, where the
///      binary has been fetched by `scripts/prebuild.mjs` but not yet packed.
///
/// Deliberately **not** a `PATH` lookup. A student machine that happens to have
/// an unrelated `mihomo` on `PATH` must not be the binary this code runs with a
/// bootstrap credential in its config.
fn engine_path(core: &str) -> Result<PathBuf> {
    let suffix = std::env::consts::EXE_SUFFIX;

    let beside = std::env::current_exe()
        .context("could not resolve the running executable")?
        .with_file_name(format!("{core}{suffix}"));
    if beside.is_file() {
        return Ok(beside);
    }

    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("sidecar")
        .join(development_sidecar_name(core, suffix));
    if dev.is_file() {
        return Ok(dev);
    }

    Err(anyhow!(
        "could not find the engine sidecar {core:?} for the activation tunnel — looked beside \
         the executable ({}) and in the development sidecar directory ({})",
        beside.display(),
        dev.display()
    ))
}

/// The built sidecar's file name, which carries a target triple.
///
/// `externalBin` names the binary without a triple; the built artifact adds one.
/// This helper exists so the development path can find it without this module
/// hard-coding a second copy of the platform list.
fn development_sidecar_name(core: &str, suffix: &str) -> String {
    // The triple this crate was compiled for, emitted by `build.rs`. Derived rather
    // than hard-coded so a cross-compile names the artifact the build actually
    // produces.
    format!("{core}-{}{suffix}", env!("CHRYSALIS_BUILD_TARGET"))
}

/// One place the bootstrap listener may be dialled.
///
/// # Why this is a list and not a hostname
///
/// The tunnel exists for a network that blocks the hub **by name**. Configuring
/// its outbound with a hostname therefore leaves the fallback depending on the
/// one thing the network controls: the engine must resolve `ss_hub` before it can
/// dial anything, and a resolver that answers nothing answers nothing for the
/// fallback either. That is the reasoning `Failure::Dns` was added for, and it is
/// why that variant is deliberately not retryable over the tunnel today.
///
/// So the outbound is built from an **ordered** set of targets — the resolved
/// name first (it is the one an operator moving the hub can change without a
/// release), then the compiled address
/// ([`crate::chrysalis::contract::ACTIVATION_TUNNEL_ADDR`]) as the case that needs no
/// resolver at all.
///
/// # Why more than one is not "a second proxy"
///
/// The security property of this engine is that it has exactly ONE possible
/// destination: every member of the group dials the same hub listener with the
/// same bootstrap key, and the path filter behind it is what bounds the key. An
/// outbound added here that named anything else would widen what a public
/// credential reaches, which is why `the_config_carries_exactly_one_destination`
/// asserts that every member's `server` is in this candidate list and nothing
/// else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// What the engine dials: a hostname or an IP literal.
    pub server: String,
    /// What the list calls it, and what mihomo shows in its own logs.
    pub name: String,
}

impl Target {
    fn new(name: &str, server: &str) -> Self {
        Self {
            server: server.to_owned(),
            name: name.to_owned(),
        }
    }
}

/// The bootstrap targets, in the order they are tried.
///
/// Pure, and taking its inputs by value, so the ordering rule is testable without
/// a network, a config directory or an engine.
#[must_use]
pub fn bootstrap_targets(ss_hub: &str, addr: &str) -> Vec<Target> {
    let mut targets = vec![Target::new(TARGET_BY_NAME, ss_hub)];
    // Only when it is actually a different value: a hostname duplicated under two
    // names would make the engine retry the same failed lookup, which reads in a
    // support log as "the fallback ran and also failed" when it never left the
    // first candidate.
    if !addr.trim().is_empty() && addr != ss_hub {
        targets.push(Target::new(TARGET_BY_ADDRESS, addr));
    }
    targets
}

/// The proxy name of the first (named) bootstrap target.
///
/// Kept stable so the group's default member — and therefore the name an operator
/// sees in the engine log — does not change when a second candidate is added.
const TARGET_BY_NAME: &str = "Activation-Tunnel";

/// The proxy name of the address fallback.
const TARGET_BY_ADDRESS: &str = "Activation-Tunnel-Direct";

/// The group every request is routed through.
///
/// **Must differ from every member name**, which is the loop trap this client has
/// already paid for (`tier.rs::GROUP_NAME`): mihomo treats a group whose name
/// equals a proxy inside it as a reference loop and refuses the whole config —
/// which is valid YAML, so only running the engine catches it.
const GROUP_NAME: &str = "Chrysalis Activation";

/// What the group's health check probes.
///
/// It is the PATH FILTER on the bootstrap port, not the listener and not the hub:
/// the outbound under test is the one that dials the listener, and a probe that
/// went anywhere else would measure a different link. The filter refuses this path
/// with an empty 403 (it is not on the allowlist), and `url-test` treats any HTTP
/// answer as alive — so the probe proves the whole chain carries bytes without
/// depending on an endpoint a public bootstrap key may reach.
const GROUP_TEST_URL: &str = "http://127.0.0.1:18090/api/health";

/// Renders the engine document.
///
/// Kept as a pure function so the *generated document* — which is where a
/// missing `MATCH` rule, a second destination or an unreachable group would
/// silently break (or widen) what the tunnel can do — is testable without
/// spawning anything.
fn render_config(forward_port: u16, targets: &[Target], key: &str, destination_port: u16) -> Result<String> {
    if targets.is_empty() {
        anyhow::bail!("the activation tunnel needs at least one bootstrap target");
    }

    // Built as a value rather than a format string so a key containing YAML
    // metacharacters cannot break out of its scalar. The key is base64 today, so
    // this is defence in depth rather than a live hazard — but the value is a
    // credential and a quoting bug here would leak it into the wrong place.
    let proxies = serde_yaml_ng::Value::Sequence(
        targets
            .iter()
            .map(|target| {
                serde_yaml_ng::Value::Mapping(
                    [
                        ("name", serde_yaml_ng::Value::String(target.name.clone())),
                        ("type", serde_yaml_ng::Value::String("ss".into())),
                        // The engine dials the hub's bootstrap listener at the hub's real
                        // name. This was `127.0.0.1`, which is the loopback of **the student's
                        // own machine** — the comment beside it claimed the engine rewrote the
                        // address to the SS server's host, and mihomo does not do that. The
                        // The engine dials the hub's bootstrap listener at one of the
                        // candidates in `bootstrap_targets`, in order. This was `127.0.0.1`,
                        // which is the loopback of **the student's own machine** — the comment
                        // beside it claimed the engine rewrote the address to the SS server's
                        // host, and mihomo does not do that. The outbound connects to exactly
                        // the `server` it is given, so the tunnel could only ever have worked
                        // against a listener running locally.
                        //
                        // The FIRST candidate is `contract::ss_hub()`, derived from the
                        // resolved hub URL, so a device moved to an alias dials the bootstrap
                        // listener at the name it can reach. The SECOND is the compiled
                        // address, for the case the name cannot survive: a DNS-filtering
                        // network. Both dial the same listener with the same key; see
                        // `Target` for why an ordered list is not a widened surface.
                        ("server", serde_yaml_ng::Value::String(target.server.clone())),
                        ("port", serde_yaml_ng::Value::Number(destination_port.into())),
                        ("cipher", serde_yaml_ng::Value::String(TUNNEL_METHOD.into())),
                        ("password", serde_yaml_ng::Value::String(key.to_owned())),
                        // TCP only: every enrolment endpoint is a small request/response and
                        // the hub's listener is configured `tcp_only`.
                        ("udp", serde_yaml_ng::Value::Bool(false)),
                    ]
                    .into_iter()
                    .map(|(k, v)| (serde_yaml_ng::Value::String(k.into()), v))
                    .collect(),
                )
            })
            .collect(),
    );

    // The group lists EVERY candidate, in order, and its default is the first —
    // so an ordinary network behaves exactly as it did when there was one member,
    // and the address is only ever reached by a student whose name lookup failed.
    //
    // `url-test` rather than `select`: the engine is never driven by a UI, so a
    // `select` group would stay on its first member forever and the address
    // fallback would be dead weight in the document. `url-test` probes both and
    // uses whichever answers. The probe target is the hub's own health endpoint on
    // the bootstrap port, which the far side's allowlist refuses — deliberately:
    // a refused probe still proves the outbound can carry traffic, and it keeps
    // the check from depending on an endpoint the bootstrap key is allowed to
    // reach.
    let group_members = serde_yaml_ng::Value::Sequence(
        targets
            .iter()
            .map(|target| serde_yaml_ng::Value::String(target.name.clone()))
            .collect(),
    );
    let groups = serde_yaml_ng::Value::Sequence(vec![serde_yaml_ng::Value::Mapping(
        [
            ("name", serde_yaml_ng::Value::String(GROUP_NAME.into())),
            ("type", serde_yaml_ng::Value::String("url-test".into())),
            ("url", serde_yaml_ng::Value::String(GROUP_TEST_URL.into())),
            // Long enough that a healthy bootstrap listener is not replaced by a
            // slower-but-working address every interval, short enough that a dead
            // name does not cost every request its own timeout.
            ("interval", serde_yaml_ng::Value::Number(300.into())),
            ("timeout", serde_yaml_ng::Value::Number(2000.into())),
            ("proxies", group_members),
        ]
        .into_iter()
        .map(|(k, v)| (serde_yaml_ng::Value::String(k.into()), v))
        .collect(),
    )]);

    // A catch-all MATCH as the ONLY rule. There is no DIRECT outbound reachable
    // through it, so this engine cannot be used to reach anything except the
    // five-thousand-port listener, and the hub's allowlist bounds that further.
    let rules = serde_yaml_ng::Value::Sequence(vec![serde_yaml_ng::Value::String(format!("MATCH,{GROUP_NAME}"))]);

    let mut root = serde_yaml_ng::Mapping::new();
    let mut put = |key: &str, value: serde_yaml_ng::Value| {
        root.insert(serde_yaml_ng::Value::String(key.into()), value);
    };
    put("mixed-port", serde_yaml_ng::Value::Number(forward_port.into()));
    put("allow-lan", serde_yaml_ng::Value::Bool(false));
    put("bind-address", serde_yaml_ng::Value::String("127.0.0.1".into()));
    put("mode", serde_yaml_ng::Value::String("rule".into()));
    put("ipv6", serde_yaml_ng::Value::Bool(false));
    // No external controller: nothing needs to drive this engine, and leaving the
    // default controller enabled would open another loopback port with an HTTP
    // API that can switch proxies.
    put("external-controller", serde_yaml_ng::Value::String(String::new()));
    put("proxies", proxies);
    put("proxy-groups", groups);
    put("rules", rules);

    serde_yaml_ng::to_string(&serde_yaml_ng::Value::Mapping(root))
        .context("could not serialise the activation tunnel config")
}

/// Whether a tunnel attempt has already failed in this process.
///
/// A heartbeat runs on a loop. On a network that blocks the hub **and** tolerates
/// nothing on the bootstrap port, every beat would spawn a 60 MB engine, wait for
/// it to fail, and do it again after `Backoff` — repeatedly, for the life of the
/// session, on a student's machine.
///
/// A failed attempt IS a property of the network, so attempts are counted rather
/// than acted on immediately — see [`TUNNEL_MAX_ATTEMPTS`] for why one failure was
/// the wrong threshold. Session-scoped: a process restart is a reasonable moment to
/// re-test.
/// How many failed attempts this session has spent, and when the last one was.
///
/// # Why a counter replaced a boolean (2026-11-19)
///
/// This used to be one flag: the first failed attempt disabled the tunnel for the
/// rest of the process. The reasoning — "one failure is a property of the network,
/// not of that request" — is right about the *network* and wrong about the
/// *session*, for a reason specific to what this tunnel is for:
///
///   * A student on the school network mistypes a code. The lookup fails, the
///     tunnel records a failure, and the **correction** they type next is refused
///     before it is attempted. They see the same error twice and conclude the app
///     is broken, on the one network where this is the only route.
///   * The engine can fail to start for reasons that are transient and local — a
///     cold page cache, a slow disk — and those are exactly the conditions on a
///     school laptop.
///
/// So the rule is now: retry, but a bounded number of times and not in a tight
/// loop. [`TUNNEL_MAX_ATTEMPTS`] failures disable it for the session as before; a
/// success clears the count. A heartbeat loop cannot spin, because attempts only
/// accumulate and reset on success.
fn tunnel_failure_state() -> &'static tokio::sync::Mutex<(u8, Instant)> {
    static STATE: std::sync::OnceLock<tokio::sync::Mutex<(u8, Instant)>> = std::sync::OnceLock::new();
    STATE.get_or_init(|| tokio::sync::Mutex::new((0, Instant::now())))
}

/// Failures allowed in one session before the tunnel is given up on.
///
/// Three, not one: enough for a mistyped code to be corrected and for one transient
/// engine-start failure, few enough that a network which genuinely refuses the
/// bootstrap cannot spawn an engine per beat for the life of the session.
const TUNNEL_MAX_ATTEMPTS: u8 = 3;

/// How long after the last failure a retry becomes available again.
///
/// The backoff, separate from the attempt cap: three failures in ten seconds is a
/// broken network, three failures spread over a minute is a student correcting a
/// typo. Both are allowed to differ.
const TUNNEL_RETRY_BACKOFF: Duration = Duration::from_secs(20);

fn tunnel_already_failed() -> bool {
    // `try_lock` so this stays callable from a non-async context and never blocks a
    // request behind another. A contended lock means another call is mid-failure,
    // which is itself a reason to answer "yes, it has failed".
    let Ok(state) = tunnel_failure_state().try_lock() else {
        return true;
    };
    let (attempts, last) = *state;
    attempts >= TUNNEL_MAX_ATTEMPTS && last.elapsed() < TUNNEL_RETRY_BACKOFF
}

fn remember_tunnel_failure() {
    if let Ok(mut state) = tunnel_failure_state().try_lock() {
        state.0 = state.0.saturating_add(1);
        state.1 = Instant::now();
    }
}

/// Clears the failure count after a call that succeeded.
///
/// The other half of the bounded-retry rule. Without it the counter only ever
/// climbs, so three failures spread across a long session — a typo, a sleep/wake,
/// a Wi-Fi change — would eventually disable the tunnel for a network that had been
/// working. Resetting on success makes the cap mean "three in a row", which is the
/// thing that actually indicates a broken network.
fn forget_tunnel_failure() {
    if let Ok(mut state) = tunnel_failure_state().try_lock() {
        state.0 = 0;
        state.1 = Instant::now();
    }
}

/// How long a running bootstrap engine is kept for the next call.
///
/// Enrolment makes two requests back to back, and a heartbeat may follow within
/// seconds of a sign-in. Long enough to cover both, short enough that the engine
/// does not sit on a student's machine through a whole session — which was the
/// argument for spawning per call in the first place, and it is still right.
const TUNNEL_IDLE_TTL: Duration = Duration::from_secs(60);

/// The shared engine slot, and when it was last used.
///
/// A single slot rather than a pool: there is exactly one hub, one port and one
/// credential, so a second engine would be a second copy of the same SOCKS proxy.
fn tunnel_slot() -> &'static tokio::sync::Mutex<Option<(Tunnel, Instant)>> {
    static SLOT: std::sync::OnceLock<tokio::sync::Mutex<Option<(Tunnel, Instant)>>> = std::sync::OnceLock::new();
    SLOT.get_or_init(|| tokio::sync::Mutex::new(None))
}

/// What a caller needs from a live engine.
///
/// Deliberately **not** the `Tunnel` itself: the engine now outlives the call, so
/// handing out ownership would let a caller's `Drop` kill a process the next call
/// still needs. These two strings are the whole interface, and neither can stop
/// the engine.
#[derive(Debug, Clone)]
pub struct Endpoint {
    /// `socks5h://127.0.0.1:<port>` — what the HTTP client's proxy points at.
    pub proxy: String,
    /// `http://127.0.0.1:<FILTER_PORT>` — the destination, inside the SS stream.
    pub http_base: String,
}

/// A live engine for this call, starting one only if none is warm.
///
/// Returns `Ok(None)` when this build has no bootstrap key. An engine idle past
/// [`TUNNEL_IDLE_TTL`] is dropped and replaced, so the process never accumulates
/// one and never keeps a SOCKS proxy alive for free.
///
/// # Why the engine is reused at all
///
/// Two reasons, and the second is a defect fix rather than an optimisation:
///
///   1. **Cost.** The engine is a 60 MB sidecar; the first call on a school laptop
///      pays a cold page-cache load.
///   2. **Correctness.** [`remember_tunnel_failure`] is session-scoped and is set
///      by a *transport* failure. With a fresh engine per call, a timeout on the
///      advisory `/api/code-lookup` set the flag and then refused `/api/activate`
///      without attempting it — so a hiccup on the request whose answer the screen
///      explicitly does not depend on (`pages/activation.tsx`: "the lookup is
///      advisory") blocked the one that enrols the student.
async fn endpoint() -> Result<Option<Endpoint>> {
    // A warm engine is handed back without holding the lock across anything
    // awaitable: `Endpoint` is two strings, so the caller never touches the
    // engine itself. The guard is scoped because `MutexGuard` is a significant
    // `Drop` and clippy is right to insist it does not outlive its critical
    // section.
    let warm = {
        let mut slot = tunnel_slot().lock().await;
        match slot.as_mut() {
            Some((tunnel, last_used)) if last_used.elapsed() < TUNNEL_IDLE_TTL => {
                *last_used = Instant::now();
                Some(Endpoint {
                    proxy: tunnel.proxy_address(),
                    http_base: tunnel.http_base(),
                })
            }
            // Stale: drop it now (its `Drop` kills the process) and fall through
            // to a fresh start.
            Some(_) => {
                *slot = None;
                None
            }
            None => None,
        }
    };
    if let Some(endpoint) = warm {
        return Ok(Some(endpoint));
    }

    // Starting is slow (a 60 MB sidecar) and must not hold the slot lock, or a
    // concurrent call would block on it for the whole start-up.
    let Some(tunnel) = Tunnel::start().await? else {
        return Ok(None);
    };
    let endpoint = Endpoint {
        proxy: tunnel.proxy_address(),
        http_base: tunnel.http_base(),
    };
    // Scoped for the same reason as above: the guard is a significant `Drop` and
    // must not outlive the store it performs.
    {
        let mut slot = tunnel_slot().lock().await;
        *slot = Some((tunnel, Instant::now()));
    }
    Ok(Some(endpoint))
}

/// Drops the warm engine, if any, so its process is killed now.
///
/// Called when a request fails at the transport level: the engine is what dialled
/// through the tunnel, and leaving it running would mean the next attempt reused a
/// listener whose only known property is that it just failed.
async fn drop_warm_engine() {
    {
        let mut slot = tunnel_slot().lock().await;
        *slot = None;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// The maintenance tunnel
//
// A SECOND engine, over the same code path, carrying the free tier's relay
// (`:8443`) and holding it open rather than letting it idle out. It exists so
// that a device on a network which blocks the hub by name can still reach the
// hub's API and still establish its plan tunnel.
//
// It is deliberately NOT a supervisor: there is one handle, one latch and no
// health checks. Whether it should be running is decided by the two calls below,
// made from the places that already know (app setup, and the readiness
// transition observed in `chrysalis_status`).
// ─────────────────────────────────────────────────────────────────────────────

/// The held maintenance engine.
///
/// Separate from [`tunnel_slot`] on purpose. That slot belongs to the
/// per-request activation tunnel and expires; this one is held for as long as the
/// device needs a route, so sharing the slot would have the TTL logic fighting
/// the lifecycle. Two slots, two lifetimes, no shared state to reason about.
fn maintenance_slot() -> &'static tokio::sync::Mutex<Option<Tunnel>> {
    static SLOT: std::sync::OnceLock<tokio::sync::Mutex<Option<Tunnel>>> = std::sync::OnceLock::new();
    SLOT.get_or_init(|| tokio::sync::Mutex::new(None))
}

/// Whether this session should prefer the maintenance route.
///
/// **What it means: "a direct hub call failed without an HTTP status."** It is a
/// *preference*, not a claim that the route works — which is why the routing rule
/// also consults [`maintenance_proxy`] before relying on it. Set once and never
/// cleared: the network has not changed, and re-testing direct on every call is
/// exactly the failure this exists to avoid.
fn prefer_maintenance_flag() -> &'static std::sync::atomic::AtomicBool {
    static PREFER: std::sync::OnceLock<std::sync::atomic::AtomicBool> = std::sync::OnceLock::new();
    PREFER.get_or_init(|| std::sync::atomic::AtomicBool::new(false))
}

/// See [`prefer_maintenance_flag`].
#[must_use]
pub fn prefer_maintenance() -> bool {
    prefer_maintenance_flag().load(std::sync::atomic::Ordering::Relaxed)
}

/// Records that direct has failed with no HTTP status, for the rest of the session.
pub fn remember_prefer_maintenance() {
    prefer_maintenance_flag().store(true, std::sync::atomic::Ordering::Relaxed);
}

/// The one lock that serialises every test touching the routing preference.
///
/// **Lives here, beside the flag, and is used by every module that touches it.**
/// It started as two guards — one in this module, one in `tier.rs` — each
/// serialising its own tests and neither excluding the other's. That is a split
/// lock, and it failed exactly as a split lock does: `cargo test -p chrysalis --lib`
/// ran this module's negative case concurrently with `tier.rs`'s positive one and
/// the assertion saw the other test's write. One flag needs one lock, in the
/// module that owns the flag.
#[cfg(test)]
pub(crate) fn preference_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
    GUARD.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The retry decision with the transport-failure half supplied, for a test.
///
/// [`should_retry_over_maintenance`] takes a `&reqwest::Error`, which a test cannot
/// construct without performing a failing request — and this rule must be testable
/// without the network, because the condition it exists for (no engine warm yet) is
/// exactly the one a unit test can arrange and a network test cannot. Passing the
/// `no_status` half in directly keeps the rule itself in one place: this is the
/// same expression, not a parallel copy.
#[cfg(test)]
pub(crate) fn should_retry_over_maintenance_for_tests(no_status: bool) -> bool {
    no_status && maintenance_tunnel_available()
}

/// Clears the bounded-retry counter. **Tests only**, for the same reason
/// [`forget_prefer_maintenance_for_tests`] exists: the state is process-wide and
/// `cargo test` shares one process across concurrent tests.
#[cfg(test)]
pub(crate) fn clear_tunnel_failures_for_tests() {
    forget_tunnel_failure();
}

/// Clears the preference. **Tests only.**
///
/// The flag is process-wide and set for the life of the process by design, so a
/// test that sets it would otherwise leak into every later test in the binary —
/// and `cargo test` runs them concurrently by default. A test that exercises the
/// switched route takes this crate's `serial`-style guard, sets the flag, asserts,
/// and clears it, so the default remains observable.
#[doc(hidden)]
pub fn forget_prefer_maintenance_for_tests() {
    prefer_maintenance_flag().store(false, std::sync::atomic::Ordering::Relaxed);
}

/// The maintenance engine's SOCKS URL, when one is warm.
///
/// The one accessor every routing decision reads. `None` means "no route through
/// maintenance right now" — either this build has no free-tier key, the engine was
/// stopped at TUN-up, or it has not started yet. Callers must treat `None` as
/// "use direct", not as an error.
#[must_use]
pub fn maintenance_proxy() -> Option<String> {
    let slot = maintenance_slot().try_lock().ok()?;
    slot.as_ref().map(Tunnel::proxy_address)
}

/// The maintenance engine's SOCKS URL, **starting one if none is warm.**
///
/// # Why this exists, and what it fixes (2026-11-19)
///
/// [`maintenance_proxy`] answers one question — "is one warm *right now*" — and it
/// answers `None` in three different situations that the old routing code treated
/// identically:
///
///   * this build has no free-tier key;
///   * the engine has not started yet;
///   * the engine was stopped at TUN-up.
///
/// The retry path only ever read it, never started it. So a student who submits a
/// code before the background warm has finished — the normal case on a cold launch,
/// where the warm is racing a 60 MB sidecar load — reaches
/// `retry_over_maintenance`, finds `None`, and sends the retry **direct**, where it
/// fails with exactly the failure that triggered it. The log then shows a retry
/// that failed for no stated reason, and the student sees the same error as before.
///
/// This measured as a real possibility on 2026-11-19: the tunnel demonstrably
/// carries enrolment from the school network when the config is driven by hand, so
/// "the app cannot" is a defect of this path, not of the network.
///
/// Starting on demand is deliberately the *retry's* behaviour and not the startup
/// path's: [`warm_maintenance`] still runs at setup so the engine is usually already
/// there, and this only pays the 60 MB load in the case where waiting was not an
/// option. It cannot block the failure it is handling for longer than
/// [`STARTUP_TIMEOUT`], and a failure to start is reported rather than swallowed.
pub async fn maintenance_proxy_or_start() -> Option<String> {
    if let Some(proxy) = maintenance_proxy() {
        return Some(proxy);
    }

    // No key means no amount of waiting will help; say so rather than spending the
    // startup timeout discovering it.
    if Bootstrap::maintenance().is_none() {
        tracing::warn!(
            target: "chrysalis::activation_tunnel",
            "the maintenance tunnel cannot be started: this build carries no free-tier key \
             (see build_stamp at startup), so the retry was not attempted"
        );
        return None;
    }

    // A real hub call needs a route, and a student who has just moved networks (home
    // to school, or the reverse) must not be refused because the budget was spent on
    // the previous one. The cap exists to stop a tight background loop, not to
    // refuse a foreground request, so this path resets it.
    //
    // The background loop still stops after `MAINTENANCE_START_ATTEMPTS`; the
    // difference is that a retry only happens when there is an actual request to
    // carry, which is the event that tells us the network may have changed.
    clear_failed_starts();

    tracing::info!(
        target: "chrysalis::activation_tunnel",
        "no maintenance engine was warm at the moment it was needed; starting one now"
    );
    if !warm_maintenance().await {
        tracing::warn!(
            target: "chrysalis::activation_tunnel",
            "the maintenance engine could not be started, so the retry did not happen"
        );
        return None;
    }
    maintenance_proxy()
}

/// Why the maintenance engine must not be started right now, if it must not.
///
/// Returns the reason for the log, or `None` when starting is appropriate. Every
/// refusal is a *state*, not an error:
///
///   * **already warm** — nothing to do, and starting a second would replace a
///     working listener;
///   * **draining** — a handover is stopping the engine on purpose. Starting here
///     would fight it: the drain would kill the process this just spawned;
///   * **budget spent** — a network that refuses the bootstrap attempted a start on
///     every warm call before this existed: roughly every 25 s for the life of the
///     session, each costing the 10 s `STARTUP_TIMEOUT`. That was bounded in *rate*
///     but not in *total*, so a student's laptop kept loading a 60 MB binary
///     forever. A few attempts cover a transient local failure (cold page cache, a
///     slow disk); a network that refuses the bootstrap will refuse it on the fourth
///     as surely as on the first.
#[must_use]
fn refusal_to_start() -> Option<String> {
    if maintenance_proxy().is_some() {
        return Some("an engine is already warm; not starting a second".to_owned());
    }
    if is_draining() {
        return Some("a handover is draining the maintenance engine; not starting another".to_owned());
    }
    if start_attempts_exhausted() {
        return Some(format!(
            "the maintenance engine has failed to start {MAINTENANCE_START_ATTEMPTS} times this \
             session; not trying again until a hub call retries on demand"
        ));
    }
    if Bootstrap::maintenance().is_none() {
        return Some("no free-tier key in this build; direct only".to_owned());
    }
    None
}

/// Starts the maintenance engine and holds it, unless one is already warm.
///
/// Returns whether a maintenance route exists afterwards. Called from app setup
/// and again when the tunnel drops; both are "make sure one is running", so this
/// is idempotent rather than a start/stop pair with its own state.
pub async fn warm_maintenance() -> bool {
    // Every reason not to start, in one place. Extracted so this function stays
    // within the crate's cognitive-complexity budget and so the refusals read as a
    // list rather than as a chain of early returns interleaved with the work.
    //
    // The answer is NOT simply "false on refusal". This function returns whether a
    // ROUTE EXISTS, and one refusal — "already warm" — means yes. Collapsing the
    // two made a functioning engine report failure, which `maintenance_proxy_or_start`
    // reads as "the start did not happen" and logs as such. The distinction is one
    // line and was wrong in the first version of this refactor.
    if let Some(reason) = refusal_to_start() {
        tracing::debug!(target: "chrysalis::activation_tunnel", "{reason}");
        return maintenance_proxy().is_some();
    }

    let Some(bootstrap) = Bootstrap::maintenance() else {
        // Unreachable in practice — `refusal_to_start` already excludes it — but
        // kept as the total form rather than an `expect`: a build without the key is
        // a normal branch build, and a panic here would be a worse failure than the
        // silent direct route.
        return false;
    };

    // Starting is slow and must not hold the slot lock across the await.
    let started = start_quietly(&bootstrap).await;

    let mut slot = maintenance_slot().lock().await;
    if slot.is_some() {
        // Another caller won the race. Theirs is already serving, so drop this one
        // rather than replacing a working listener with a second.
        //
        // `started` is moved out of scope here and its `Drop` kills the child, so
        // the losing engine is not leaked — that is what makes returning early
        // safe. And the answer is `true` because a route genuinely exists: the
        // OTHER caller's engine is in the slot and serving.
        return true;
    }
    *slot = started;
    log_warm(slot.as_ref())
}

/// Start attempts allowed in one session.
///
/// Three, matching `TUNNEL_MAX_ATTEMPTS` on the request path. A start costs
/// `STARTUP_TIMEOUT` (10 s) plus the engine load, so this bounds a refusing network
/// to about 30 s of wasted work rather than an unbounded sequence of them.
pub const MAINTENANCE_START_ATTEMPTS: u8 = 3;

fn start_attempt_counter() -> &'static std::sync::atomic::AtomicU8 {
    static COUNT: std::sync::OnceLock<std::sync::atomic::AtomicU8> = std::sync::OnceLock::new();
    COUNT.get_or_init(|| std::sync::atomic::AtomicU8::new(0))
}

fn start_attempts_exhausted() -> bool {
    start_attempt_counter().load(std::sync::atomic::Ordering::SeqCst) >= MAINTENANCE_START_ATTEMPTS
}

/// Records one FAILED start attempt.
///
/// Success is deliberately not recorded. A working engine must never inch the
/// session towards the cap, or a student who reconnects three times would disable a
/// route that was working perfectly — the same mistake the old "one failure
/// disables the session" flag made, one level down.
fn record_failed_start() {
    start_attempt_counter().fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// Clears the failure count after a start that succeeded.
///
/// The other half of the bounded rule, for the same reason as
/// `forget_tunnel_failure`: the cap means "three failures in a row", not "three
/// ever".
fn clear_failed_starts() {
    start_attempt_counter().store(0, std::sync::atomic::Ordering::SeqCst);
}

/// Starts an engine without holding the slot, reporting failure rather than
/// returning it: "could not start" is a state this function records, not an
/// outcome a caller branches on.
async fn start_quietly(bootstrap: &Bootstrap) -> Option<Tunnel> {
    match Tunnel::start_with(bootstrap).await {
        Ok(tunnel) => {
            // A start that worked resets the budget, so the cap cannot accumulate
            // across a session in which the route mostly works.
            clear_failed_starts();
            tunnel
        }
        Err(error) => {
            record_failed_start();
            tracing::warn!(
                target: "chrysalis::activation_tunnel",
                "could not start the maintenance tunnel: {error:#}"
            );
            None
        }
    }
}

/// Announces the engine that is now serving, and answers whether one is.
///
/// Separate from [`warm_maintenance`] so that function stays within this crate's
/// cognitive-complexity budget; the split is along "decide" versus "report".
fn log_warm(slot: Option<&Tunnel>) -> bool {
    let Some(tunnel) = slot else {
        return false;
    };
    tracing::info!("[maintenance] engine started proxy={}", tunnel.proxy_address());
    true
}

// ─────────────────────────────────────────────────────────────────────────────
// The supervisor
// ─────────────────────────────────────────────────────────────────────────────

/// When the maintenance engine may be stopped, as a state machine.
///
/// # Why this exists (2026-11-19)
///
/// The lifecycle used to be a two-branch `if` evaluated on every status poll:
/// ready → stop, otherwise → warm. That is unstable for a reason that is easy to
/// miss, because the signal it reads is *good*: `probe::decide` reaches
/// `Readiness::Ready` only on evidence that a payload moved (a completed egress
/// round trip, or bytes on the Core's `/traffic` stream) — Core-up is explicitly
/// not evidence.
///
/// The problem is that `Ready` is **instantaneous**. It is recomputed per poll
/// from the *current* sample, and returns to `Connecting` the moment a sample
/// shows no bytes — which a page that finished loading, or a lull between two
/// requests, produces routinely. So one unlucky sample stopped the engine, the
/// next poll restarted it, and a student on the network this exists for watched
/// the only route to the hub cycle on and off.
///
/// The states and the reasons for each transition are in
/// `docs/reference/TUNNEL-SUPERVISOR.md` §3. The short version: `Ready` must be
/// **sustained** for [`TUNNEL_HOLD`] before the engine is stopped, and stopping is
/// a distinct state so that "traffic resumed mid-handover" can cancel it without
/// having touched the engine at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Supervised {
    /// No engine; one is being started, or the last start failed.
    Warming,
    /// Engine up, the plan tunnel has not proven itself. Carries all app traffic.
    Carrying,
    /// Engine STILL up, but `Ready` has held long enough to start the handover.
    /// The engine is not stopped until this state is reached, and it survives any
    /// poll on which `Ready` is lost.
    Handover,
    /// Engine stopped; the plan tunnel carries the app's traffic.
    Stopped,
}

/// How long `Ready` must persist before the engine is stopped.
///
/// Chosen against the two poll intervals: 10 s is an order of magnitude longer
/// than the 750 ms connecting poll, so a single sample cannot cause a transition,
/// and shorter than the 15 s idle poll, so a handover completes within one tick.
///
/// This is a reasoned value, not a measured one. If the handover still cycles on
/// the school network, this constant is the thing to change — and only after a log
/// shows the cycle, so the number is not adjusted to hide some other fault.
pub const TUNNEL_HOLD: Duration = Duration::from_secs(10);

/// What the supervisor reads on each poll. Pure inputs, so the machine is
/// testable without a Core, a network or an engine.
#[derive(Debug, Clone, Copy)]
pub struct Observations {
    /// The plan tunnel is carrying traffic *right now* (`Readiness::Ready`).
    pub ready_now: bool,
    /// An engine is warm.
    pub engine_up: bool,
}

/// The supervisor's own state, separate from the engine's.
#[derive(Debug, Clone, Copy)]
pub struct Supervisor {
    state: Supervised,
    /// When `ready_now` first became true in the current run of true samples.
    ready_since: Option<Instant>,
}

impl Supervisor {
    /// A supervisor at startup: nothing warm, nothing proven.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: Supervised::Warming,
            ready_since: None,
        }
    }

    /// The current state, for reporting.
    #[must_use]
    pub const fn state(self) -> Supervised {
        self.state
    }

    /// Feeds one poll, and answers with the action to take.
    ///
    /// # The rule, in one place
    ///
    /// The timestamp of a true `ready_now` is tracked across calls, so the hold is
    /// measured against **elapsed wall time**, not a sample count: a count would
    /// mean a different real duration at the 750 ms connecting interval than at
    /// the 15 s idle one, which is precisely the kind of hidden coupling that made
    /// the old rule unstable.
    ///
    /// A false sample **clears** the timestamp, so the hold must be earned
    /// consecutively. That is what makes a flapping `Ready` harmless: it can never
    /// accumulate ten seconds' worth of scattered trues.
    pub fn observe(&mut self, seen: Observations, now: Instant) -> Action {
        if !seen.ready_now {
            self.ready_since = None;
        } else if self.ready_since.is_none() {
            self.ready_since = Some(now);
        }

        let held = self
            .ready_since
            .is_some_and(|since| now.duration_since(since) >= TUNNEL_HOLD);

        // Every arm returns its own action, so each transition reads as one
        // statement rather than as a fall-through to a shared tail. The alternative —
        // a single `Action` at the bottom — is what clippy flags as "all match arms
        // end with the same expression", and it is genuinely worse here: the action
        // IS the difference between the arms.
        match self.state {
            Supervised::Warming | Supervised::Stopped => {
                if seen.engine_up {
                    // An engine is already up (the on-demand retry can start one
                    // before the first poll after a failure). Adopt it rather than
                    // replacing a working listener with a second.
                    self.state = Supervised::Carrying;
                    return Action::Keep;
                }
                // The plan tunnel is proven and nothing is warm, so nothing needs
                // starting. Without this, a device that launches while already
                // connected would start an engine the next poll stops.
                if held {
                    self.state = Supervised::Stopped;
                    return Action::None;
                }
                Action::Start
            }
            Supervised::Carrying => {
                if held {
                    // The hold is met: begin the handover. The engine stays up,
                    // because this is not a decision to stop yet.
                    self.state = Supervised::Handover;
                    return Action::BeginHandover;
                }
                Action::Keep
            }
            Supervised::Handover => {
                if !seen.ready_now {
                    // Traffic was flowing and has stopped again, or the sample that
                    // opened the handover was the only true one. Cancel WITHOUT
                    // having stopped the engine — the transition this state exists
                    // to make possible.
                    self.state = Supervised::Carrying;
                    return Action::Keep;
                }
                if held {
                    self.state = Supervised::Stopped;
                    return Action::Stop;
                }
                Action::Keep
            }
        }
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

/// The process's supervisor, and the only place its state is stored.
///
/// A mutex rather than an atomic because the state is a struct with a timestamp:
/// two atomics would let a reader observe the state from one poll and the timestamp
/// from another, which is a torn read of exactly the pair the hold rule needs to be
/// consistent.
fn supervisor_slot() -> &'static tokio::sync::Mutex<Supervisor> {
    static SLOT: std::sync::OnceLock<tokio::sync::Mutex<Supervisor>> = std::sync::OnceLock::new();
    SLOT.get_or_init(|| tokio::sync::Mutex::new(Supervisor::new()))
}

/// The supervisor's current state, for the status report.
pub async fn supervised_state() -> Supervised {
    supervisor_slot().lock().await.state()
}

/// Whether a detached drain is running.
///
/// Read by the next poll so a second `Stop` while one is draining does not start a
/// second drain on the same slot.
fn drain_running_flag() -> &'static std::sync::atomic::AtomicBool {
    static FLAG: std::sync::OnceLock<std::sync::atomic::AtomicBool> = std::sync::OnceLock::new();
    FLAG.get_or_init(|| std::sync::atomic::AtomicBool::new(false))
}

/// Starts the engine on a detached task, so no poll ever waits for it.
///
/// The re-entrancy guard is `warm_maintenance`'s own slot check plus
/// `is_draining`; a second dispatch while the first is still loading finds the slot
/// empty and starts a SECOND engine. That is prevented here by a flag rather than
/// left to a race between two detached tasks.
fn dispatch_start() {
    fn starting_flag() -> &'static std::sync::atomic::AtomicBool {
        static FLAG: std::sync::OnceLock<std::sync::atomic::AtomicBool> = std::sync::OnceLock::new();
        FLAG.get_or_init(|| std::sync::atomic::AtomicBool::new(false))
    }

    if starting_flag()
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_err()
    {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let started = warm_maintenance().await;
        starting_flag().store(false, std::sync::atomic::Ordering::SeqCst);
        if !started {
            tracing::debug!(
                target: "chrysalis::activation_tunnel",
                "the detached maintenance start did not produce an engine"
            );
        }
    });
}

/// Drains and stops on a detached task, so no poll ever waits for it.
fn dispatch_drained_stop() {
    if drain_running_flag()
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_err()
    {
        // A drain is already in flight. The next poll will see the engine gone.
        return;
    }

    tauri::async_runtime::spawn(async move {
        stop_maintenance_drained().await;
        drain_running_flag().store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

/// Feeds one observation to the supervisor and performs the action it asks for.
///
/// **This is the whole lifecycle**, replacing the two-branch `if` in `cmd/chrysalis.rs`.
/// Returning the state keeps the caller's report and the decision it just made
/// reading from the same observation, rather than from two independently timed
/// reads.
///
/// The engine actions are performed here rather than returned to the caller: the
/// decision and its effect belong together, or a future caller can act on a stale
/// decision while a poll has already moved the machine on.
pub async fn supervise(ready_now: bool) -> Supervised {
    let engine_up = maintenance_proxy().is_some();

    // The lock is held only across the decision, never across the engine start or
    // stop: both are slow (a 60 MB process), and holding it would make every
    // concurrent status poll wait for them.
    let (action, state) = {
        let mut supervisor = supervisor_slot().lock().await;
        let action = supervisor.observe(Observations { ready_now, engine_up }, Instant::now());
        (action, supervisor.state())
    };

    // **Nothing here may block the caller for longer than a status poll.**
    //
    // `supervise` runs inside `chrysalis_status`, which the UI polls every 750 ms while
    // a connect settles. A `Start` costs the engine load (up to `STARTUP_TIMEOUT`)
    // and a drained `Stop` costs up to `DRAIN_TIMEOUT` (45 s) — so awaiting either
    // here would stall the status command, and with it the button, for as long as
    // the engine takes. That is a worse student-visible fault than the handover it
    // was meant to make graceful.
    //
    // Both are therefore DISPATCHED and not awaited. Each is already idempotent
    // (`warm_maintenance` checks the slot, the drain sets a flag the next poll
    // reads), so a second poll arriving mid-flight is a no-op rather than a second
    // engine.
    match action {
        Action::Start => {
            dispatch_start();
        }
        Action::BeginHandover => {
            // Deliberately does NOT stop the engine. The next polls decide, and a
            // sample showing traffic again returns to `Carrying` with the engine
            // never having been touched.
            tracing::info!(
                target: "chrysalis::activation_tunnel",
                "the plan tunnel has carried traffic for {}s; beginning the maintenance handover",
                TUNNEL_HOLD.as_secs()
            );
        }
        Action::Stop => {
            tracing::info!(
                target: "chrysalis::activation_tunnel",
                "handover complete: the plan tunnel is carrying traffic, draining and stopping the \
                 maintenance engine"
            );
            // Drained, never the hard stop: a hub request in flight — an update
            // download can hold the stream for up to 600 s — must not be killed by a
            // handover it has nothing to do with. Dispatched, not awaited, so the
            // drain cannot stall the status poll that started it.
            dispatch_drained_stop();
        }
        Action::Keep | Action::None => {}
    }

    state
}

/// What the caller must do after [`Supervisor::observe`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Make sure an engine is running.
    Start,
    /// Leave the engine exactly as it is.
    Keep,
    /// Announce that the handover has begun. The engine is NOT stopped.
    BeginHandover,
    /// Stop the engine now — the plan tunnel has carried traffic for the hold.
    Stop,
    /// Nothing to do; the plan tunnel is proven and no engine is needed.
    None,
}

/// Stops the maintenance engine, if one is running.
///
/// Called when the plan tunnel is proven to carry traffic: the tunnel now carries
/// everything, so a second route through the free tier would be a second copy of
/// the same traffic at a worse rate.
///
/// # This is a HARD stop, and callers should prefer [`stop_maintenance_drained`]
///
/// It takes the engine out of the slot and drops it, and `Tunnel::drop` sends
/// `kill()` to the child — SIGKILL. A request already handed to that SOCKS listener
/// dies with it. Kept public because there are paths (app shutdown, a test) where
/// that is exactly what is wanted; the lifecycle path uses the drained form.
pub async fn stop_maintenance() {
    let mut slot = maintenance_slot().lock().await;
    *slot = None;
}

/// Stops the engine only once no request is using it, waiting up to
/// [`DRAIN_TIMEOUT`].
///
/// # Why the hard stop was a defect (2026-11-19)
///
/// `Tunnel::drop` kills the child. With no drain, a handover that landed during a
/// hub call cut that call off mid-stream — and the window is not small: the
/// per-request ceiling is 30 s for activation, 15 s for a heartbeat and **600 s for
/// an update download** (`timeout_seconds`). So a student who started an update and
/// then watched the plan tunnel come up could have the download killed at any
/// point in ten minutes, with the failure surfacing as a transport error that then
/// triggers a *retry* — which, since the engine is now gone, starts a new one. A
/// handover that caused a restart is the cycle this whole supervisor exists to
/// prevent.
///
/// The rule: stop accepting new work, let what is in flight finish, then stop.
/// New work is refused by [`gated_proxy`], which returns `None` for the whole
/// duration so a caller goes direct rather than into a listener about to close.
///
/// # On timeout
///
/// A request that outlives [`DRAIN_TIMEOUT`] is abandoned and the engine stopped
/// anyway. The alternative — waiting indefinitely — lets one hung download hold a
/// 60 MB process on a student's machine for its full 600 s ceiling, which is
/// worse. The abandoned request fails exactly as it would have under the old hard
/// stop, so the timeout is not a new failure mode; it is the old one, bounded.
pub async fn stop_maintenance_drained() {
    // Refuse new work FIRST, and do it before the wait: a caller that arrives
    // during the drain must not be handed a listener that is about to close.
    set_draining(true);

    let deadline = Instant::now() + DRAIN_TIMEOUT;
    while in_flight_count() > 0 && Instant::now() < deadline {
        tokio::time::sleep(DRAIN_POLL).await;
    }

    let outstanding = in_flight_count();
    if outstanding > 0 {
        tracing::warn!(
            target: "chrysalis::activation_tunnel",
            outstanding,
            "stopping the maintenance engine with {outstanding} request(s) still in flight after \
             {}s; they will fail as they would have without the drain",
            DRAIN_TIMEOUT.as_secs()
        );
    }

    // The flag is cleared by a guard, not a trailing statement: a panic anywhere
    // above (a poisoned lock, a task cancelled mid-await) would otherwise leave
    // `draining` set forever, and every later request would take the direct route
    // for the life of the process — the route silently disabled by the mechanism
    // meant to hand it over cleanly. That is the same class of failure as the
    // one-failure latch this work already removed, and it must not be reintroduced
    // one level down.
    let _clear = ClearDrainingOnDrop;

    let mut slot = maintenance_slot().lock().await;
    *slot = None;
}

/// Clears the draining flag when it goes out of scope, panic or not.
struct ClearDrainingOnDrop;

impl Drop for ClearDrainingOnDrop {
    fn drop(&mut self) {
        set_draining(false);
    }
}

/// How long a drain waits for in-flight requests before stopping anyway.
///
/// Longer than the activation (30 s) ceiling, so an enrolment in progress is never
/// cut; shorter than the 600 s download ceiling, so one hung update cannot pin a
/// 60 MB process. A download that is still running at 45 s is abandoned — see
/// [`stop_maintenance_drained`] for why that is the right trade.
pub const DRAIN_TIMEOUT: Duration = Duration::from_secs(45);

/// How often the drain re-checks the in-flight count.
const DRAIN_POLL: Duration = Duration::from_millis(100);

fn draining_flag() -> &'static std::sync::atomic::AtomicBool {
    static FLAG: std::sync::OnceLock<std::sync::atomic::AtomicBool> = std::sync::OnceLock::new();
    FLAG.get_or_init(|| std::sync::atomic::AtomicBool::new(false))
}

fn set_draining(draining: bool) {
    draining_flag().store(draining, std::sync::atomic::Ordering::SeqCst);
}

/// Whether a handover has begun and the engine must take no new requests.
#[must_use]
pub fn is_draining() -> bool {
    draining_flag().load(std::sync::atomic::Ordering::SeqCst)
}

fn in_flight_counter() -> &'static std::sync::atomic::AtomicUsize {
    static COUNTER: std::sync::OnceLock<std::sync::atomic::AtomicUsize> = std::sync::OnceLock::new();
    COUNTER.get_or_init(|| std::sync::atomic::AtomicUsize::new(0))
}

fn in_flight_count() -> usize {
    in_flight_counter().load(std::sync::atomic::Ordering::SeqCst)
}

/// Marks one request as using the maintenance engine, for the life of the guard.
///
/// A guard rather than a pair of calls: an early `return` or a `?` in the request
/// path would otherwise leak the count, and a leaked count means the drain waits
/// for its full timeout on every handover.
struct InFlight;

impl InFlight {
    fn enter() -> Self {
        in_flight_counter().fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        in_flight_counter().fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// The hub client a call should use **first**, given this session's routing
/// preference.
///
/// This is the whole routing rule, in one function, called from the three hub call
/// sites (`activation::post_json`, `heartbeat::beat`, `update::apply`). It is
/// deliberately not a trait or a policy object: the rule is three lines and the
/// only thing it needs from a caller is that the caller reports a transport
/// failure back through [`note_transport_failure`].
///
/// # The rule
///
///   * not preferring maintenance → **direct**
///   * preferring it, and an engine is warm → **through maintenance**
///   * preferring it, nothing warm → **direct**
///
/// The third case is what stops the preference becoming a stuck route. After the
/// plan tunnel is up, [`stop_maintenance`] has stopped the engine, so every call
/// goes direct at full speed even though the session still prefers maintenance —
/// the flag records what the network did, not what the route is.
pub async fn hub_client_for(hub: &str, timeout: crate::chrysalis::hub::Timeout) -> crate::chrysalis::hub::HubClient {
    let proxy = match proxy_if_warm() {
        Some(url) => match reqwest::Proxy::all(&url) {
            Ok(proxy) => Some(proxy),
            Err(error) => {
                tracing::warn!(
                    target: "chrysalis::activation_tunnel",
                    "the maintenance proxy URL could not be used ({error:#}); using direct"
                );
                None
            }
        },
        None => None,
    };

    crate::chrysalis::hub::client_with_proxy(hub, timeout, proxy).await
}

/// The same client, but **starts a maintenance engine if none is warm.**
///
/// The retry path's constructor, and the reason it is a separate function rather
/// than a flag on [`hub_client_for`]: the direct call must never pay a 60 MB engine
/// load on a network that does not need it, and the retry has already established
/// that it does. Keeping them apart makes that asymmetry visible at the call site
/// instead of hidden in a boolean.
pub async fn hub_client_for_retry(
    hub: &str,
    timeout: crate::chrysalis::hub::Timeout,
) -> crate::chrysalis::hub::HubClient {
    let proxy = match maintenance_proxy_or_start().await {
        Some(url) => match reqwest::Proxy::all(&url) {
            Ok(proxy) => Some(proxy),
            Err(error) => {
                tracing::warn!(
                    target: "chrysalis::activation_tunnel",
                    "the maintenance proxy URL could not be used ({error:#}); using direct"
                );
                None
            }
        },
        None => None,
    };

    crate::chrysalis::hub::client_with_proxy(hub, timeout, proxy).await
}

/// The warm maintenance proxy, whenever the route should be used for a hub call.
///
/// # The rule, and why it is two conditions rather than one (2026-11-19)
///
/// `prefer_maintenance()` alone was too narrow. It is set only *after* a direct
/// call has already failed with no HTTP status — so the very first hub call of a
/// session always went direct, discovered the block by failing, and then switched.
/// On a network that blocks the domain by category that discovery is guaranteed
/// and costs a round trip every launch.
///
/// The second condition is the requirement stated plainly: **if a maintenance
/// engine is up, the app's hub traffic goes through it.** The supervisor
/// (`Supervised::Carrying` / `Handover`) is what decides whether an engine is up;
/// while one is, it is carrying the app's traffic without exception, and there is
/// no reason to make a request take the route that is known to fail first.
///
/// This deliberately does NOT widen what the tunnel carries to the student's own
/// traffic. Before a plan tunnel exists there is nothing to carry it, and the free
/// tier is capped at 1 Mbps — routing a student's browsing through it would be a
/// worse connection than none. See `TUNNEL-SUPERVISOR.md` §3.3.
fn proxy_if_warm() -> Option<String> {
    gated_proxy(prefer_maintenance())
}

/// The warm proxy, refusing while a handover drain is in progress.
///
/// # Why the gate is here and not at the call sites
///
/// Three call sites read this, and a gate duplicated three times is three places
/// for "did we check draining?" to drift — the same argument that put the retry
/// rule in one function. Putting it at the bottom means a new caller cannot forget
/// it, which matters because the failure it prevents (a request handed to a
/// listener that is about to be killed) is silent and looks like a network fault.
///
/// `prefer` is passed in rather than read, because `prefer_maintenance()` is a
/// session-level fact while draining is a moment-level one; the two are ANDed here
/// so the distinction stays visible.
fn gated_proxy(prefer: bool) -> Option<String> {
    // The warm proxy is read ONCE. Reading it twice — once to test and once to
    // return — is how a gate ends up admitting a request the second read no longer
    // supports, and it also made the function untestable: with no engine in a test
    // process the two branches returned `None` either way, so an assertion here
    // passed whether or not the gate existed. `admit` takes the proxy as a value
    // so the gate itself can be exercised without an engine.
    let warm = maintenance_proxy();
    admit(prefer, warm, is_draining())
}

/// The gate, as a pure function of its three inputs.
///
/// Separated from [`gated_proxy`] so a test can supply a warm proxy it does not
/// have. Reading the process's real slot made the interesting case untestable: no
/// engine in a test binary means every branch returns `None`, and a guard written
/// against that passes even when the gate is removed —
/// which is exactly what happened the first time this was written.
fn admit(prefer: bool, warm: Option<String>, draining: bool) -> Option<String> {
    if draining {
        // Deliberately silent: this is the NORMAL path during a handover, and a log
        // line per request would bury the one line that says it started.
        return None;
    }
    // Two independent reasons to use a warm engine, and both are real:
    //
    //   * `prefer` — this session has seen a transport failure, so the route is
    //     known to be needed;
    //   * the slot is warm — the supervisor keeps an engine up while the plan
    //     tunnel is unproven, and while it is up it carries the app's hub traffic
    //     without exception (the 2026-11-19 requirement).
    //
    // The second is why `warm` is matched rather than consumed: its presence is
    // itself the signal.
    if !prefer && warm.is_none() {
        return None;
    }
    warm
}

/// Whether this failure is one the maintenance route should carry.
///
/// The single predicate the three call sites share, so the rule cannot drift
/// between them: **no HTTP status at all** means nothing answered, which is what a
/// network blocking the hub by name produces. A 4xx/5xx is an ANSWER and never
/// qualifies — the server is reachable, and switching routes would only make a
/// working call slower.
///
/// Deliberately not a classifier variant: a TLS-inspecting appliance truncates the
/// stream, which arrives as a decode error rather than a reset, so
/// [`crate::chrysalis::hub::Failure::classify`] would report `Transport` and the older
/// tunnel retry never fires on the network this exists for.
pub fn should_retry_over_maintenance(error: &reqwest::Error) -> bool {
    // Deliberately NOT `maintenance_proxy().is_some()` any more (2026-11-19).
    //
    // That extra clause looked like a cheap guard — "do not promise a retry we
    // cannot perform" — and it was the defect. It asked whether an engine was warm
    // *at this instant*, which on a cold launch is no, so the single transport this
    // network needs was refused before anything could start it. The engine is
    // started on demand now (`maintenance_proxy_or_start`), so the question is
    // whether this build CAN form the route, not whether it happens to be warm.
    //
    // A build with no key still answers false, which is right: no retry, and the
    // startup stamp is what explains why.
    error.status().is_none() && maintenance_tunnel_available()
}

/// Records the outcome and answers whether this failure should be retried over
/// the maintenance route.
///
/// The single call a hub site makes in its error arm, so the three sites cannot
/// drift in either half of the rule: record what the network did, then decide.
pub fn note_failure_and_decide_retry(error: &reqwest::Error) -> bool {
    note_transport_failure(error.status().is_none());
    should_retry_over_maintenance(error)
}

/// Logs a retry's outcome, and returns what the caller should do with it.
///
/// One place for the two lines, because three call sites emitting their own copy
/// is three places for the message to drift. Takes the `Result` rather than a
/// bool so the failure branch can name the reason without the caller forming it.
///
/// `status()` is logged rather than the message: a transport failure's chain can
/// carry the hub URL, and this line is meant to be pasted into a report.
pub fn log_retry_outcome<T>(result: &Result<T, reqwest::Error>) -> bool {
    match result {
        Ok(_) => {
            tracing::info!("[hub] retry via maintenance ok");
            true
        }
        Err(error) => {
            tracing::warn!("[hub] retry via maintenance failed err={:?}", error.status());
            false
        }
    }
}

/// Records the outcome of one hub call, so the NEXT call picks the right route.
///
/// `no_status` means "the request produced no HTTP response at all" — a transport
/// failure, whatever it was classified as. That is the signal the maintenance
/// tunnel exists for, and it is deliberately NOT a classifier variant: a school
/// appliance that inspects TLS truncates the stream, which arrives as a decode
/// error rather than a reset ([`crate::chrysalis::hub::Failure::classify`]), so keying
/// on the classifier would miss the case this was built for.
///
/// Called from every call site, and cheap: two atomics.
pub fn note_transport_failure(no_status: bool) {
    // Logged on the false→true transition only. A heartbeat runs on a loop, and
    // every beat after the switch arrives here with the flag already set — logging
    // each one would bury the single line that says when the network changed.
    if no_status && !prefer_maintenance() {
        remember_prefer_maintenance();
        tracing::warn!(
            target: "chrysalis::hub",
            "direct failed (no status), switching to maintenance"
        );
    }
}

/// Sends `body` to `path` **through** the tunnel and parses the response.
///
/// Mirrors `activation::post_json`'s contract: a non-2xx status still carries a
/// parseable body (the hub answers 4xx with a JSON `message`), so the body is
/// parsed regardless of status and only unreadable responses become errors.
///
/// The engine is SHARED between calls and torn down when it has been idle (see
/// [`reusable_tunnel`]). It used to be spawned per call, which was wrong in two
/// ways that only show up on the network this exists for:
///
///   * Enrolment is TWO requests — the advisory `/api/code-lookup`, then
///     `/api/activate` (`pages/activation.tsx`). Each would pay the full engine
///     start on a 60 MB binary loaded from a cold page cache on a school laptop.
///   * Worse, the first call's failure marks the session as "the tunnel does not
///     work", so the second call — the one that actually matters — is refused
///     BEFORE it is attempted. A transient failure on the advisory lookup
///     therefore blocked activation outright.
pub async fn post<T, B>(path: &str, body: &B, timeout: crate::chrysalis::hub::Timeout) -> Result<T>
where
    T: DeserializeOwned,
    B: Serialize + Sync,
{
    if tunnel_already_failed() {
        return Err(anyhow!(
            "the activation tunnel already failed once in this session, so it was not retried"
        ));
    }

    let Some(tunnel) = endpoint().await? else {
        return Err(anyhow!(
            "no activation tunnel is compiled into this build, so the retry was not attempted"
        ));
    };

    let proxy = reqwest::Proxy::all(&tunnel.proxy).context("could not configure the tunnel proxy")?;
    let client = reqwest::Client::builder()
        // Through the SS stream there is no TLS to verify and no name to resolve;
        // the security boundary is the allowlist on the far side, which refuses
        // anything outside it with an empty 403.
        .timeout(timeout_seconds(timeout))
        .user_agent(concat!("Chrysalis-Client/", env!("CARGO_PKG_VERSION")))
        .proxy(proxy)
        // `no_proxy` must NOT be set here — it is what makes the direct client
        // bypass the system proxy, and here we are deliberately installing one.
        .build()
        .context("could not build the activation tunnel HTTP client")?;

    // Counted for the drain in `stop_maintenance_drained`, for the whole life of
    // this request. The guard is dropped on every path out of this function,
    // including the `?`s below, so a failed request cannot leak the count and make
    // the next handover wait out its full timeout.
    let _in_flight = InFlight::enter();

    let url = format!("{}{path}", tunnel.http_base);
    let response = match client.post(&url).json(body).send().await {
        Ok(response) => response,
        Err(error) => {
            // The failure is remembered AND the warm engine is dropped.
            //
            // Remembering alone was not enough: the engine stays in the slot, so a
            // retry that reuses it would retry the same broken SOCKS listener. The
            // network has not changed, so neither should the process we dial
            // through.
            remember_tunnel_failure();
            drop_warm_engine().await;
            return Err(error)
                .with_context(|| format!("could not reach the hub through the activation tunnel ({url})"));
        }
    };

    let status = response.status();
    let text = response
        .text()
        .await
        .with_context(|| format!("the activation tunnel returned an unreadable response ({status})"))?;

    // The stream carried a real message from the hub, so the network allows this
    // transport. Clearing the count is what makes the cap mean "three IN A ROW":
    // without it the failures accumulate across a whole session and eventually
    // disable a route that has been working.
    forget_tunnel_failure();

    serde_json::from_str(&text).with_context(|| {
        format!("the activation tunnel returned something that is not a Chrysalis response ({status}): {text}")
    })
}

/// The tunnel's per-request ceiling.
///
/// It inherits the direct path's timeout rather than inventing one: a tunnel
/// request that is slower than the direct one it replaced has failed at the only
/// thing it was for.
const fn timeout_seconds(timeout: crate::chrysalis::hub::Timeout) -> Duration {
    Duration::from_secs(match timeout {
        crate::chrysalis::hub::Timeout::Activation => 30,
        crate::chrysalis::hub::Timeout::Heartbeat => 15,
        crate::chrysalis::hub::Timeout::Download => 600,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialises the tests that touch the process-wide warm-engine slot.
    ///
    /// WHY THIS EXISTS. `tunnel_slot()` is a single static, and three tests
    /// arrange and assert on it: `a_warm_engine_is_reused_…`,
    /// `a_stale_engine_is_not_reused`, and anything that lets `endpoint()` fail
    /// (which calls `drop_warm_engine()`). `cargo test` runs them CONCURRENTLY by
    /// default, so one test's `drop_warm_engine()` can clear the slot between the
    /// next test's setup and its assertion.
    ///
    /// That was not theoretical. Measured on this machine before the guard: 1
    /// failure in 5 runs of `cargo test --lib --package chrysalis`, in `debug`, with
    /// the message "a warm engine must be handed back" — plus a second sighting
    /// in `--profile fast-release`. The same suite passed on the other runs, which
    /// is exactly how a flake reaches CI and gets called a broken build.
    ///
    /// A mutex rather than `--test-threads=1`: serialising the whole binary would
    /// slow every run to fix two tests, and the setting lives in CI rather than
    /// here, where the reason would be lost.
    ///
    /// `tokio::sync::Mutex`, not `std::sync::Mutex`. Both tests are async and hold
    /// the guard across `.await` points (they call `endpoint().await` while
    /// serialised), which is a deadlock hazard with a blocking guard — and clippy
    /// rejects it outright under `await_holding_lock`, which this crate enables.
    /// The async mutex suspends instead of blocking the executor thread.
    /// Serialises the tests that force the process-wide routing preference.
    ///
    /// Separate from [`slot_guard`] because these tests are synchronous: a
    /// `tokio` mutex would need a runtime this module's plain `#[test]` cases do
    /// not have. `std::sync::Mutex` is safe here because no guard outlives its
    /// test and none is held across an `.await`.
    use super::preference_guard;

    async fn slot_guard() -> tokio::sync::MutexGuard<'static, ()> {
        static GUARD: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        // No poisoning: a panic in one guarded test must not turn the other into a
        // confusing secondary failure. The panic is already reported on its own.
        GUARD.lock().await
    }

    /// The single-candidate shape most tests want, built through the real
    /// builder rather than hand-constructed, so a change to how candidates are
    /// produced cannot leave these tests asserting against a fossil.
    fn named_only() -> Vec<Target> {
        bootstrap_targets("hub.example.org", "")
    }

    /// **Every bootstrap candidate must dial the same listener, with the same key,
    /// on the same port — and the group's name must not be one of them.**
    ///
    /// WHY THIS EXISTS. The engine that carries enrolment is a complete proxy. Its
    /// safety rests on it being able to reach exactly one destination, because the
    /// credential in it ships in every binary and must be assumed public. Adding a
    /// second candidate opens the first real hole in that argument: a future edit
    /// that pointed one member at a different host (a second hub, an operator's
    /// test box, a `direct` outbound) would make a public credential a route to
    /// somewhere nobody intended, and nothing at runtime would say so.
    ///
    /// The second half is the trap this client has already paid for: mihomo
    /// refuses a config whose group shares a name with a member, and the document
    /// is valid YAML either way, so only running the engine catches it.
    ///
    /// Falsified by construction: point `TARGET_BY_ADDRESS` at `"example.com"` and
    /// the first loop fails; rename `GROUP_NAME` to `TARGET_BY_NAME` and the last
    /// assertion fails.
    #[test]
    fn every_bootstrap_candidate_dials_the_one_listener_and_nothing_else() {
        let targets = bootstrap_targets("hub.example.org", "203.0.113.7");
        assert_eq!(targets.len(), 2, "the name and the address are both candidates");

        let yaml = render_config(17999, &targets, "AAAA", TUNNEL_PORT).expect("render");
        let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).expect("parse");
        let proxies = parsed["proxies"].as_sequence().expect("proxies is a sequence");
        assert_eq!(proxies.len(), targets.len(), "one outbound per candidate");

        let allowed: Vec<String> = targets.iter().map(|t| t.server.clone()).collect();
        for proxy in proxies {
            let server = proxy["server"].as_str().unwrap_or_default();
            assert!(
                allowed.iter().any(|candidate| candidate == server),
                "{server:?} is not one of the bootstrap candidates; a route beside the \
                 hub's listener makes a shipped credential a proxy to somewhere else"
            );
            assert_eq!(proxy["port"].as_u64(), Some(u64::from(TUNNEL_PORT)));
            assert_eq!(
                proxy["password"].as_str(),
                Some("AAAA"),
                "same credential, one listener"
            );
            assert_eq!(proxy["cipher"].as_str(), Some(TUNNEL_METHOD));
        }

        let group = &parsed["proxy-groups"][0];
        let members = group["proxies"].as_sequence().expect("a member list");
        assert_eq!(members.len(), targets.len(), "every candidate must be usable");
        let group_name = group["name"].as_str().unwrap_or_default();
        for member in members {
            let member = member.as_str().unwrap_or_default();
            assert_ne!(
                group_name, member,
                "the group must not share a name with a member: mihomo rejects the \
                 whole document as a reference loop, and it is valid YAML, so only \
                 running the engine catches it"
            );
        }
    }

    /// **The address candidate exists precisely because the name may not resolve.**
    ///
    /// `Failure::Dns` is not retryable over the tunnel on the grounds that the
    /// fallback's engine resolves the same name — a true statement about the old
    /// document and the reason a DNS-filtering network had no route at all. The
    /// fix is that the second candidate is an address literal, so the engine can
    /// dial without a lookup.
    ///
    /// FALSIFIED before keeping: with `bootstrap_targets` returning only the named
    /// target (the pre-2026-11-09 document), the second assertion fails.
    #[test]
    fn an_address_candidate_exists_so_a_dns_block_cannot_strand_enrolment() {
        let targets = bootstrap_targets("hub.example.org", "203.0.113.7");
        let last = targets.last().expect("at least one candidate");
        assert!(
            last.server.parse::<std::net::IpAddr>().is_ok(),
            "the last candidate must be an IP literal: it is the one that needs no \
             resolver, and the network this exists for is the one that filters DNS"
        );

        // A duplicate under a second name would make the engine retry the same
        // failed lookup and read in a log as "the fallback ran and also failed".
        let collapsed = bootstrap_targets("203.0.113.7", "203.0.113.7");
        assert_eq!(collapsed.len(), 1, "one server, one candidate — not two names for it");

        // An empty build-time value degrades to name-only, which is the pre-fix
        // behaviour and must be possible without producing a nonsense outbound.
        assert_eq!(bootstrap_targets("hub.example.org", "").len(), 1);
    }

    #[test]
    fn the_config_carries_exactly_one_proxy_and_one_catch_all_rule() {
        let yaml = render_config(17999, &named_only(), "AAAA", TUNNEL_PORT).expect("render");
        let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).expect("parse");

        let proxies = parsed["proxies"].as_sequence().expect("proxies is a sequence");
        assert_eq!(proxies.len(), 1, "one outbound, so nothing else can be reached");

        let rules = parsed["rules"].as_sequence().expect("rules is a sequence");
        assert_eq!(rules.len(), 1, "one rule, so there is no path we did not choose");
        assert_eq!(
            rules[0].as_str(),
            Some("MATCH,Chrysalis Activation"),
            "the single rule must be a catch-all to our group — a DIRECT rule here \
             would turn the bootstrap tunnel into a general proxy"
        );
    }

    #[test]
    fn the_config_dials_the_hub_and_not_the_local_machine() {
        let yaml = render_config(17999, &named_only(), "AAAA", TUNNEL_PORT).expect("render");
        let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).expect("parse");

        assert_eq!(
            parsed["proxies"][0]["server"].as_str(),
            Some("hub.example.org"),
            "the SS outbound must dial the HUB. It used to say `127.0.0.1` — the \
             student's own loopback — under a comment claiming mihomo rewrote it to the \
             server's host. It does not: the outbound connects to exactly this value, so \
             the tunnel could only ever have worked against a local listener, and every \
             real build failed with nothing to show for it but 'the tunnel never helps'"
        );
        assert!(
            !parsed["proxies"][0]["server"]
                .as_str()
                .unwrap_or_default()
                .starts_with("127."),
            "a loopback server here means the tunnel dials the student's own machine"
        );
    }

    #[test]
    fn the_config_dials_the_bootstrap_port_and_nothing_else() {
        let yaml = render_config(17999, &named_only(), "AAAA", TUNNEL_PORT).expect("render");
        let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).expect("parse");
        let proxy = &parsed["proxies"][0];

        assert_eq!(proxy["type"].as_str(), Some("ss"));
        assert_eq!(
            proxy["port"].as_u64(),
            Some(u64::from(TUNNEL_PORT)),
            "the engine must dial the activation tunnel's port; changing this in one \
             place and not the other is how the UDP fleet went down"
        );
        assert_eq!(proxy["cipher"].as_str(), Some(TUNNEL_METHOD));
        assert_eq!(
            proxy["udp"].as_bool(),
            Some(false),
            "the hub's listener is tcp_only; advertising udp here would make the engine \
             offer a path that cannot answer"
        );
    }

    #[test]
    fn the_listener_is_loopback_only_and_has_no_controller() {
        let yaml = render_config(17999, &named_only(), "AAAA", TUNNEL_PORT).expect("render");
        let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).expect("parse");

        assert_eq!(parsed["allow-lan"].as_bool(), Some(false));
        assert_eq!(parsed["bind-address"].as_str(), Some("127.0.0.1"));
        assert_eq!(
            parsed["external-controller"].as_str(),
            Some(""),
            "an enabled controller is a second loopback HTTP API that can switch proxies"
        );
        assert_eq!(parsed["mixed-port"].as_u64(), Some(17999));
    }

    #[test]
    fn a_key_with_yaml_metacharacters_stays_one_scalar() {
        // Not a real key. The point is that the renderer quotes it correctly, so a
        // future key format cannot break out of its scalar or leak into a sibling.
        let hostile = "a: b\nc: d #not-a-comment";
        let yaml = render_config(17999, &named_only(), hostile, TUNNEL_PORT).expect("render");
        let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).expect("parse");
        assert_eq!(parsed["proxies"][0]["password"].as_str(), Some(hostile));
    }

    /// The reqwest `socks` feature is OFF by default, and its absence is SILENT:
    /// `Proxy::all("socks5h://…")` still parses (it only builds a URL), and
    /// without the feature `connect_via_proxy` skips the socks arms
    /// (`connect.rs:788-793`) and speaks **HTTP** to the SOCKS port. That fails at
    /// request time as a generic transport error, which is indistinguishable from
    /// the network being blocked — so the bootstrap would be blamed on the school
    /// and never work anywhere.
    ///
    /// This asserts the one thing that catches that: a client with a `socks5h`
    /// proxy still builds. It is a construction check, not a connection — no
    /// listener is needed, and none is started.
    #[test]
    fn the_socks5h_proxy_actually_speaks_socks() {
        // A CONSTRUCTION assertion cannot catch this, and the first version of
        // this test proved it: `Client::builder().build()` only moves the proxy
        // matcher into an `Arc` (`reqwest/src/async_impl/client.rs:416-420`), so
        // it succeeds with the feature OFF. The scheme is interpreted per
        // connection, in `connect_via_proxy` (`connect.rs:788-793`), where the
        // `#[cfg(feature = "socks")]` match is skipped and the SOCKS URL is dialled
        // as an **HTTP** proxy instead.
        //
        // So the assertion has to observe the wire. This binds a listener, points
        // a `socks5h` proxy at it, makes one request, and reads the FIRST BYTE the
        // client sends:
        //
        //   * `0x05` — a SOCKS5 greeting. The feature is on and the proxy is real.
        //   * anything else (an HTTP request line starts `GET`/`CONN`) — the URL
        //     was treated as an HTTP proxy, which is exactly the silent misroute.
        //
        // The listener never completes a handshake; the request is expected to
        // fail. The single byte is the whole assertion.
        use std::io::Read as _;

        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind a probe listener");
        let port = listener.local_addr().expect("probe addr").port();
        listener.set_nonblocking(true).expect("nonblocking probe listener");
        let handle = std::thread::spawn(move || {
            // Bounded, because a missing feature can make the client never connect
            // at all. Without this the test HANGS instead of failing, and a hang in
            // CI cost four minutes before it was cancelled by hand.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut first = [0u8; 1];
                        return stream.read_exact(&mut first).map(|()| first[0]).ok();
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            return None;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    Err(_) => return None,
                }
            }
        });

        // The runtime must exist BEFORE the client is built: reqwest's builder
        // resolves some of its state against the ambient reactor.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let _enter = rt.enter();

        let proxy = reqwest::Proxy::all(format!("socks5h://127.0.0.1:{port}")).expect("proxy url parses");
        let client = reqwest::Client::builder()
            .proxy(proxy)
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("client builds");
        // The request must fail: nothing on the probe listener answers. The
        // result is irrelevant; what the listener saw is the assertion.
        let _ = rt.block_on(client.get("http://example.com/").send());

        let first = handle.join().expect("probe thread");
        assert_eq!(
            first,
            Some(0x05),
            "the probe listener saw {:?} rather than a SOCKS5 greeting (0x05). Either the client \
             never connected at all, or reqwest's `socks` feature is OFF and the socks5h:// proxy \
             was dialled as an HTTP proxy — the silent misroute this test exists to catch.",
            first
        );
    }

    /// The maintenance engine's SOCKS URL is what every routing decision reads,
    /// and it must be `None` whenever no engine is held.
    ///
    /// `None` is not an error: the routing rule treats it as "use direct", which
    /// is the state on every network that is not blocking the hub. An accessor
    /// that panicked or blocked here would turn "no maintenance route" into a
    /// failed hub call.
    /// **The load-bearing test for item 2.** The switch must fire on the ABSENCE
    /// of an HTTP status and must NOT fire when a status arrived.
    ///
    /// Run against a real local HTTP server, because the whole point is what
    /// `reqwest` reports when a server answers versus when nothing does. A mock
    /// would assert my own assumption about `status().is_none()` instead of the
    /// dependency's behaviour — and that assumption is exactly what a previous
    /// version of this change got wrong (it keyed on the classifier, which a
    /// TLS-inspecting appliance does not trip).
    ///
    /// Both halves in one test, in order, so the negative case runs while the flag
    /// is still false and the positive case is what changes it.
    ///
    /// **Shown failing, both ways:** dropping the `no_status &&` from
    /// `note_transport_failure` fails the positive half; making it `if false &&`
    /// fails it too. Re-creating the classifier shape (switching on any error,
    /// not on the missing status) fails the negative half. All three were run
    /// during review; they are not in CI because each is a temporary edit.
    #[test]
    fn a_response_with_a_status_never_switches_but_no_response_does() {
        use std::io::{Read as _, Write as _};

        let _serialised = preference_guard();
        forget_prefer_maintenance_for_tests();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");

        // ── Case 1: the server ANSWERS with 503 ──
        //
        // A 503 is the hub being briefly unavailable, not the network blocking it.
        // Switching routes here would be wrong AND useless: the maintenance tunnel
        // reaches the same server.
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream
                    .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            }
        });

        let answered = rt.block_on(async {
            reqwest::Client::new()
                .get(format!("http://127.0.0.1:{port}/api/heartbeat"))
                .send()
                .await
        });
        let _ = handle.join();
        let answered = answered.expect("a 503 is a response, not an error");
        assert_eq!(answered.status().as_u16(), 503);
        // Asserted on the flag AND on the rule, so that breaking either one fails
        // here: a rule that ignored the status, and a `note_transport_failure`
        // that recorded an answered request.
        // The decision a call site makes for an ANSWERED request: the flag must
        // stay false. Written through the same entry point the call sites use, so
        // the test follows the rule rather than restating it.
        crate::chrysalis::activation_tunnel::note_transport_failure(false);
        assert!(
            !prefer_maintenance(),
            "a request the server ANSWERED must not switch the session to the maintenance route"
        );

        // ── Case 2: nothing is listening ──
        //
        // The port was bound and released, so the connection is refused rather
        // than left hanging: `status()` is `None`, which is the condition.
        let closed_port = {
            let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind");
            let p = probe.local_addr().expect("addr").port();
            drop(probe);
            p
        };
        let refused = rt.block_on(async {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("client")
                .get(format!("http://127.0.0.1:{closed_port}/api/heartbeat"))
                .send()
                .await
        });
        let error = refused.expect_err("nothing answered, so this must be an error");
        assert!(error.status().is_none(), "a refused connection carries no HTTP status");

        // The DECISION is asserted without the "is a route warm" conjunct,
        // because no engine runs in a test process. `status().is_none()` is the
        // whole of the rule's first half; the warm half is asserted separately by
        // `there_is_no_maintenance_proxy_until_an_engine_is_held`, and keeping them
        // apart is what makes this test able to FAIL — asserting the combined
        // predicate here would pass for the wrong reason whenever no engine is
        // warm, which is always in a unit test.
        assert!(
            error.status().is_none(),
            "a refused connection is the signal: no HTTP status"
        );
        crate::chrysalis::activation_tunnel::note_transport_failure(error.status().is_none());
        assert!(
            prefer_maintenance(),
            "a request that produced no HTTP status at all must switch the session to the maintenance route"
        );

        forget_prefer_maintenance_for_tests();
    }

    /// **Item 4**: the maintenance engine stops when the plan tunnel carries
    /// traffic and comes back when it does not.
    ///
    /// This drives `stop_maintenance`/`warm_maintenance` — the two calls
    /// `chrysalis_status` makes on each readiness transition — rather than a
    /// readiness value, because a test that injected readiness would assert the
    /// poll's wiring, not the lifecycle it triggers. The wiring itself is one `if`
    /// at `cmd/chrysalis.rs`; the behaviour that can regress is here.
    ///
    /// LIMITATION, stated rather than hidden: the 750 ms vs 15 s poll interval
    /// lives in React (`use-connection.ts:84,95`) and cannot be exercised from
    /// Rust. What is proved is that a stop stops and a warm restarts; that a
    /// transition is noticed at all depends on the UI polling, which it does.
    #[tokio::test]
    async fn the_maintenance_engine_stops_and_restarts_across_a_lifecycle_transition() {
        let _serialised = slot_guard().await;
        // Start from a KNOWN state (2026-11-19). The attempt budget and the drain
        // flag are process-wide, so a test that leaves either set makes this one
        // fail for reasons that have nothing to do with the lifecycle it checks.
        clear_failed_starts();
        set_draining(false);

        // No free-tier key in a test build, so a real engine cannot start. The
        // assertion is therefore on the slot, which is what both the lifecycle and
        // `maintenance_proxy()` read — a listener test would need the key.
        let warmed = warm_maintenance().await;
        assert!(
            !warmed,
            "with no free-tier key compiled in, warming must report no route rather than pretending"
        );
        assert_eq!(
            maintenance_proxy(),
            None,
            "nothing is held, so the rule sees a cold slot"
        );

        stop_maintenance().await;
        assert_eq!(
            maintenance_proxy(),
            None,
            "stopping an already-stopped engine is not an error"
        );

        // The transition that matters at TUN-up: the slot is emptied, so the next
        // `hub_client_for` takes the direct branch even though the session may
        // still prefer maintenance. That is the whole reconciliation.
        assert_eq!(maintenance_proxy(), None);
    }

    /// **U1**: the maintenance engine is configurable without a plan tunnel, and
    /// its generated document dials `:8443` — not the activation port.
    ///
    /// The full start cannot be exercised here: `Bootstrap::maintenance()` reads a
    /// build-time key this test binary does not carry (`option_env!`), so
    /// `start_with` would be handed `None` before any process was spawned. What
    /// CAN be pinned is the part that would silently be wrong — the destination
    /// port and the config stem — because `render_config` took its port from a
    /// constant until item 9's review changed it.
    #[test]
    fn the_maintenance_config_dials_the_free_tier_port() {
        let bootstrap = Bootstrap {
            server: crate::chrysalis::contract::activation_tunnel_addr().to_owned(),
            port: FREE_TIER_PORT,
            key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned(),
        };
        let target = Target {
            name: "hub-8443".to_owned(),
            server: bootstrap.server.clone(),
        };
        let yaml = render_config(17999, &[target], &bootstrap.key, bootstrap.port).expect("render");
        let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).expect("parse");

        assert_eq!(
            parsed["proxies"][0]["port"].as_u64(),
            Some(u64::from(FREE_TIER_PORT)),
            "the maintenance outbound must dial the free-tier port, not the activation port"
        );
        assert_eq!(FREE_TIER_PORT, 8443, "server/modules/02-shadowsocks.sh:279");
        assert_ne!(
            FREE_TIER_PORT, TUNNEL_PORT,
            "the two tunnels must not collide on one listener"
        );
    }

    #[tokio::test]
    async fn there_is_no_maintenance_proxy_until_an_engine_is_held() {
        // The slot must be SERIALISED, not merely observed (2026-11-19). This test
        // used to assert a cold slot with no lock, on the reasoning that "nothing
        // started one in this process". That is true only if nothing else is
        // running — and `cargo test` runs concurrently, so a test that warms the
        // slot made this fail for a reason that had nothing to do with the property
        // it checks. Clearing first makes the precondition explicit rather than
        // assumed, which is the same fix the slot's other users already needed.
        let _serialised = slot_guard().await;
        // `stop_maintenance`, NOT `drop_warm_engine`. They clear different slots:
        // `drop_warm_engine` clears `tunnel_slot` (the activation engine) while this
        // test is about `maintenance_slot`. Using the wrong one left the fake engine
        // from `install_fake_warm_engine` in place and made this assertion fail for a
        // reason unrelated to what it checks — which is how the leak above was found.
        stop_maintenance().await;
        // Nothing held, so the honest answer is `None`. If a future change starts an
        // engine implicitly, this fails and the routing rule's "cold" branch loses
        // its only coverage.
        assert_eq!(
            maintenance_proxy(),
            None,
            "no maintenance engine is running, so the routing rule must see a cold slot"
        );
    }

    /// The preference starts false and is what makes the rule take the direct
    /// branch. Asserted as its own case so a default that flips fails here rather
    /// than as a mysterious extra request somewhere else.
    #[test]
    fn the_session_prefers_direct_until_a_failure_is_recorded() {
        let _serialised = preference_guard();
        forget_prefer_maintenance_for_tests();
        assert!(!prefer_maintenance(), "a fresh session must try direct first");
    }

    /// The build stamp must name BOTH tunnels, not just the activation one.
    ///
    /// # Why this is a test and not just a log line
    ///
    /// Added 2026-11-19. The stamp reported the activation key alone, so a build
    /// could ship with the activation tunnel working and the maintenance tunnel
    /// dead — and the support log would say "available", which is TRUE ABOUT THE
    /// WRONG TUNNEL. On the network both exist for, that is the difference between
    /// "this network blocks us" and "this build cannot", and it read as the former.
    ///
    /// Measured the same day: the maintenance tunnel DOES work from that network
    /// when driven by hand through the generated config. So the app failing where a
    /// hand-built config succeeds is a live possibility, and this line is the fact
    /// that settles it.
    ///
    /// The assertion is on the WORD, not on the value: the value depends on whether
    /// a test binary carries the key, which is not something a test can control.
    /// What must never regress is that the maintenance tunnel is reported at all.
    #[test]
    fn the_build_stamp_reports_the_maintenance_tunnel_too() {
        let stamp = build_stamp();
        assert!(
            stamp.contains("maintenance tunnel:"),
            "the build stamp must name the maintenance tunnel, or a build without its key is \
             indistinguishable from a network that refuses everything. Got: {stamp}"
        );
        assert!(
            stamp.contains("activation tunnel:"),
            "the activation tunnel must still be reported. Got: {stamp}"
        );
    }

    /// **The retry must not be refused just because no engine is warm yet.**
    ///
    /// Added 2026-11-19, and this is the defect that could strand a student on the
    /// one network the feature exists for. The old rule was
    /// `error.status().is_none() && maintenance_proxy().is_some()` — a cheap guard
    /// meaning "do not promise a retry we cannot perform". It asked whether an
    /// engine was warm *at that instant*. On a cold launch the warm is still racing
    /// a 60 MB sidecar load, so the answer was no, the retry was skipped, the
    /// request went direct, and it failed with exactly the failure that triggered
    /// it. The student saw the original error twice.
    ///
    /// The assertion is that the decision depends on the BUILD, not on the clock.
    /// It is written to fail against the old conjunct: force the slot empty, and the
    /// old code answers `false` where this demands `true`.
    ///
    /// FALSIFIED before keeping: re-adding `&& maintenance_proxy().is_some()` makes
    /// this red. Verified by running it that way.
    #[tokio::test]
    async fn a_retry_is_decided_by_the_build_not_by_whether_an_engine_is_warm() {
        let _serialised = slot_guard().await;
        // Arrange the condition that broke: nothing warm.
        drop_warm_engine().await;
        assert!(
            maintenance_proxy().is_none(),
            "arranging this test requires an empty slot — another test leaked a warm engine"
        );

        // The property under test is that the two answers are INDEPENDENT when the
        // build carries a key. A build without one legitimately refuses, which the
        // startup stamp explains.
        if maintenance_tunnel_available() {
            assert!(
                should_retry_over_maintenance_for_tests(true),
                "a transport failure must be retryable even with no engine warm, or a cold \
                 launch can never use the only transport a censored network allows"
            );
        } else {
            assert!(
                !should_retry_over_maintenance_for_tests(true),
                "a build with no free-tier key must not promise a retry it cannot perform"
            );
        }
    }

    /// **One failed attempt must not disable the tunnel for the whole session.**
    ///
    /// Added 2026-11-19. The old rule was a single boolean: the first failure set
    /// it, and every later `post` refused before doing anything. On the school
    /// network that meant a mistyped activation code — whose lookup fails, setting
    /// the flag — disabled the CORRECTION the student typed next. They saw the same
    /// error twice and had to restart the app to get a third attempt, on the one
    /// network where this is the only route to the hub.
    ///
    /// This pins the bounded rule: fewer than [`TUNNEL_MAX_ATTEMPTS`] failures leaves
    /// the tunnel usable, and a success clears the count so the cap means "three in
    /// a row" rather than "three ever".
    ///
    /// FALSIFIED before keeping: restoring the boolean (disable on first failure)
    /// makes the first assertion red.
    #[tokio::test]
    async fn a_first_failure_does_not_disable_the_tunnel_for_the_session() {
        // Serialised: this touches the process-wide failure counter.
        let _serialised = slot_guard().await;
        clear_tunnel_failures_for_tests();

        assert!(!tunnel_already_failed(), "a fresh session must be able to try");
        remember_tunnel_failure();
        assert!(
            !tunnel_already_failed(),
            "one failure must not disable the tunnel — a mistyped code would lock out the \
             correction the student types next"
        );

        // Reaching the cap DOES disable it, so a refusing network cannot spawn an
        // engine per heartbeat for the life of the session.
        for _ in 1..TUNNEL_MAX_ATTEMPTS {
            remember_tunnel_failure();
        }
        assert!(
            tunnel_already_failed(),
            "reaching TUNNEL_MAX_ATTEMPTS must stop the retries"
        );

        // And a success re-opens it, so the cap cannot accumulate across a session.
        forget_tunnel_failure();
        assert!(
            !tunnel_already_failed(),
            "a successful call must clear the count, or three failures spread over a day would \
             eventually disable a working route"
        );
    }

    // ── The supervisor's transitions ──
    //
    // Pure: `Supervisor::observe` takes the two booleans and an `Instant`, so every
    // transition can be driven at a fake clock with no Core, no engine and no
    // network. That is deliberate — the defect these guard is a *timing* defect, and
    // a test that needed a real 10-second wait would either be slow or be written
    // with a shortened constant, which would stop testing the shipped value.

    fn at(base: Instant, millis: u64) -> Instant {
        base + Duration::from_millis(millis)
    }

    /// **A flapping `Ready` must never stop the engine.**
    ///
    /// This is the defect the supervisor exists for, and the shape of the assertion
    /// is the point: the old rule acted on ONE true sample, so a run of scattered
    /// trues — a page finishing, a lull, a load — stopped and restarted the only
    /// route to the hub.
    ///
    /// Driven at the real poll cadence: many true/false samples, none of them
    /// contiguous for `TUNNEL_HOLD`, spread over far longer than `TUNNEL_HOLD` in
    /// wall time. A sample-COUNT rule would pass this and still be wrong; an
    /// elapsed-time rule with a non-clearing timestamp would also pass this and
    /// still be wrong. Only clearing the timestamp on a false sample passes, which
    /// is what makes this a real guard rather than a restatement.
    #[test]
    fn a_flapping_ready_signal_never_stops_the_engine() {
        let base = Instant::now();
        let mut supervisor = Supervisor::new();
        // Get to Carrying first: an engine exists and the plan tunnel is unproven.
        supervisor.observe(
            Observations {
                ready_now: false,
                engine_up: true,
            },
            at(base, 0),
        );
        assert_eq!(supervisor.state(), Supervised::Carrying);

        // 60 s of alternating samples at 750 ms — the connecting poll interval.
        for i in 1..=80u64 {
            let ready_now = i % 2 == 0;
            let action = supervisor.observe(
                Observations {
                    ready_now,
                    engine_up: true,
                },
                at(base, i * 750),
            );
            assert_ne!(
                action,
                Action::Stop,
                "a flapping Ready stopped the engine at sample {i} ({ready_now}); scattered true \
                 samples must never accumulate into a handover"
            );
            assert_eq!(
                supervisor.state(),
                Supervised::Carrying,
                "a flapping Ready must never leave Carrying at sample {i}"
            );
        }
    }

    /// `Ready` held for `TUNNEL_HOLD` does stop it, and stops it exactly once.
    #[test]
    fn a_sustained_ready_signal_completes_the_handover() {
        let base = Instant::now();
        let mut supervisor = Supervisor::new();
        supervisor.observe(
            Observations {
                ready_now: false,
                engine_up: true,
            },
            at(base, 0),
        );

        // The run of true samples STARTS here. The hold is measured from the first
        // true sample of the run, not from when the test started looking — which is
        // the property a sample-COUNT rule gets wrong and this one gets right.
        let almost = u64::try_from(TUNNEL_HOLD.as_millis()).expect("hold fits") - 1;
        assert_eq!(
            supervisor.observe(
                Observations {
                    ready_now: true,
                    engine_up: true
                },
                at(base, 0)
            ),
            Action::Keep,
            "the first true sample begins the run and cannot itself satisfy the hold"
        );
        assert_eq!(
            supervisor.observe(
                Observations {
                    ready_now: true,
                    engine_up: true
                },
                at(base, almost)
            ),
            Action::Keep,
            "the handover must not begin before TUNNEL_HOLD has elapsed"
        );

        // The hold is met: begin, without stopping.
        assert_eq!(
            supervisor.observe(
                Observations {
                    ready_now: true,
                    engine_up: true
                },
                at(base, almost + 2)
            ),
            Action::BeginHandover,
            "reaching TUNNEL_HOLD must begin the handover"
        );
        assert_eq!(supervisor.state(), Supervised::Handover);

        // And the stop happens on the next poll, once, from Handover.
        assert_eq!(
            supervisor.observe(
                Observations {
                    ready_now: true,
                    engine_up: true
                },
                at(base, almost + 1000)
            ),
            Action::Stop,
            "the handover must finish with exactly one Stop"
        );
        assert_eq!(supervisor.state(), Supervised::Stopped);
    }

    /// **Traffic resuming mid-handover cancels it with the engine never touched.**
    ///
    /// The transition that proves no traffic was dropped, and the reason `Handover`
    /// is a state rather than a timer inside `Carrying`: a timer would have to be
    /// cancelled by the same function that set it, and the cancel path would be
    /// implicit. Here the cancel is an explicit, asserted edge.
    #[test]
    fn traffic_resuming_mid_handover_cancels_it_without_stopping() {
        let base = Instant::now();
        let mut supervisor = Supervisor::new();
        supervisor.observe(
            Observations {
                ready_now: false,
                engine_up: true,
            },
            at(base, 0),
        );
        let hold = u64::try_from(TUNNEL_HOLD.as_millis()).expect("hold fits");

        // The run starts at t=0, so the hold is satisfied by `hold` milliseconds of
        // consecutive true samples.
        supervisor.observe(
            Observations {
                ready_now: true,
                engine_up: true,
            },
            at(base, 0),
        );
        supervisor.observe(
            Observations {
                ready_now: true,
                engine_up: true,
            },
            at(base, hold),
        );
        assert_eq!(supervisor.state(), Supervised::Handover);

        // The plan tunnel stalls before the stop could be taken.
        let action = supervisor.observe(
            Observations {
                ready_now: false,
                engine_up: true,
            },
            at(base, hold + 500),
        );
        assert_eq!(
            action,
            Action::Keep,
            "cancelling a handover must not stop or start anything"
        );
        assert_eq!(
            supervisor.state(),
            Supervised::Carrying,
            "a stalled plan tunnel must return to Carrying with the engine still up"
        );
    }

    /// An engine already warm is adopted rather than restarted.
    ///
    /// The on-demand retry (`maintenance_proxy_or_start`) can start one before the
    /// first poll after a failure. Without this, `Warming` would ask for a `Start`
    /// on top of a perfectly good engine and replace a working listener.
    #[test]
    fn an_engine_already_running_is_adopted_not_restarted() {
        let mut supervisor = Supervisor::new();
        assert_eq!(
            supervisor.observe(
                Observations {
                    ready_now: false,
                    engine_up: true
                },
                Instant::now()
            ),
            Action::Keep,
            "a warm engine must be adopted, not started again"
        );
        assert_eq!(supervisor.state(), Supervised::Carrying);
    }

    /// A proven plan tunnel with no engine needs no engine.
    ///
    /// The edge that would otherwise start an engine the next poll stops: a device
    /// that launches while already connected.
    #[test]
    fn a_proven_plan_tunnel_without_an_engine_starts_nothing() {
        let base = Instant::now();
        let mut supervisor = Supervisor::new();
        let hold = u64::try_from(TUNNEL_HOLD.as_millis()).expect("hold fits");
        // The run starts at t=0, then persists past the hold.
        supervisor.observe(
            Observations {
                ready_now: true,
                engine_up: false,
            },
            at(base, 0),
        );
        assert_eq!(
            supervisor.observe(
                Observations {
                    ready_now: true,
                    engine_up: false
                },
                at(base, hold)
            ),
            Action::None,
            "a plan tunnel that is already carrying traffic needs no maintenance engine"
        );
        assert_eq!(supervisor.state(), Supervised::Stopped);
    }

    // ── The drain ──

    /// **New requests must go direct during a drain, not into a closing listener.**
    ///
    /// Before this, `stop_maintenance` dropped the engine and `Drop` sent
    /// SIGKILL. A request handed to that SOCKS port in the same moment died
    /// mid-stream, and — worse — a transport failure now triggers a retry, which
    /// starts a NEW engine, which the next poll stops again. A handover that causes
    /// a restart is the cycle the whole supervisor exists to prevent.
    ///
    /// FALSIFIED: removing the `is_draining()` guard in `gated_proxy` makes this
    /// red on the first assertion.
    #[test]
    fn a_draining_engine_takes_no_new_requests() {
        let _serialised = preference_guard();
        set_draining(false);
        assert!(!is_draining());

        // A WARM proxy is supplied explicitly. The first version of this test used
        // `gated_proxy`, which reads the process's real (empty) slot — so every
        // branch returned `None` and the assertion passed even with the gate
        // deleted. That is the failure mode this file's own rules warn about, so it
        // is recorded here rather than quietly fixed.
        let warm = Some("socks5h://127.0.0.1:9999".to_owned());

        assert_eq!(
            admit(true, warm.clone(), false),
            warm,
            "a warm engine with a preference must be used — the positive case the gate must not break"
        );
        assert_eq!(
            admit(false, warm.clone(), false),
            warm,
            "a warm engine is used even without a recorded failure: while it is up it carries the \
             app's hub traffic without exception"
        );
        assert_eq!(admit(false, None, false), None, "no engine means no route");

        assert_eq!(
            admit(true, warm.clone(), true),
            None,
            "DURING A DRAIN a caller must go direct, not be handed a listener about to be killed — \
             even when the session prefers the route"
        );
        assert_eq!(
            admit(false, warm, true),
            None,
            "a drain refuses new work whether or not the route is preferred"
        );

        set_draining(true);
        assert!(
            gated_proxy(true).is_none(),
            "the real accessor must also refuse while draining"
        );
        set_draining(false);
        assert!(
            !is_draining(),
            "the drain must clear, or the route is disabled for the process"
        );
    }

    /// `warm_maintenance` answers "does a route exist", not "did I start one".
    ///
    /// Added 2026-11-19 after a refactor got it wrong: the extraction of
    /// `refusal_to_start` turned every refusal into `false`, including "already
    /// warm" — so a perfectly good engine reported itself absent and
    /// `maintenance_proxy_or_start` logged "the start did not happen" over a working
    /// route. A status function's answer and a work function's answer are different
    /// questions, and the refactor conflated them.
    ///
    /// FALSIFIED: making the refusal branch return a bare `false` makes this red.
    #[tokio::test]
    async fn a_warm_engine_reports_a_route_even_though_nothing_was_started() {
        let _serialised = slot_guard().await;
        // Arrange the only state that distinguishes the two answers. There is no
        // engine to spawn in a test binary, so the warm slot is faked with a
        // placeholder whose `Drop` would kill a child it does not have — see
        // `Tunnel::child` being `None`.
        install_fake_warm_engine().await;
        assert!(
            maintenance_proxy().is_some(),
            "arranging this test requires a warm slot"
        );
        assert!(
            warm_maintenance().await,
            "a warm engine means a route EXISTS; reporting false here makes a working route look \
             absent to every caller"
        );
        // The MAINTENANCE slot, not `drop_warm_engine` (which clears the activation
        // one). Leaving the fake in place makes every later test see a warm engine.
        stop_maintenance().await;
        assert!(
            maintenance_proxy().is_none(),
            "this test must not leave a warm engine behind for the next one"
        );
    }

    /// The in-flight counter must return to zero, including on an early exit.
    ///
    /// A leaked count makes every later handover wait out the full
    /// `DRAIN_TIMEOUT` (45 s) for a request that finished long ago.
    #[test]
    fn the_in_flight_count_returns_to_zero_after_each_request() {
        let base = in_flight_count();
        {
            let _first = InFlight::enter();
            assert_eq!(in_flight_count(), base + 1);
            {
                let _second = InFlight::enter();
                assert_eq!(in_flight_count(), base + 2);
            }
            assert_eq!(
                in_flight_count(),
                base + 1,
                "a nested request releasing must decrement exactly once"
            );
        }
        assert_eq!(
            in_flight_count(),
            base,
            "the count must return to its starting value, or the next drain waits out its timeout"
        );
    }

    /// A start that succeeded must reset the attempt budget.
    ///
    /// Without the reset the cap means "three failures EVER", so a student who
    /// reconnects a few times across a session would find the route refused while
    /// it was working perfectly — the same mistake the old one-failure latch made.
    #[test]
    fn a_successful_start_clears_the_attempt_budget() {
        clear_failed_starts();
        for _ in 0..MAINTENANCE_START_ATTEMPTS {
            record_failed_start();
        }
        assert!(
            start_attempts_exhausted(),
            "reaching the cap must stop background attempts"
        );

        clear_failed_starts();
        assert!(
            !start_attempts_exhausted(),
            "a successful start must reset the budget, or the cap becomes 'three ever'"
        );
    }

    /// The budget must actually be reached — a guard that can never trip is not a cap.
    #[test]
    fn the_start_budget_is_reachable_and_bounded() {
        clear_failed_starts();
        assert!(!start_attempts_exhausted());
        for expected in 1..MAINTENANCE_START_ATTEMPTS {
            record_failed_start();
            assert!(
                !start_attempts_exhausted(),
                "attempt {expected} must not exhaust a budget of {MAINTENANCE_START_ATTEMPTS}"
            );
        }
        record_failed_start();
        assert!(
            start_attempts_exhausted(),
            "the budget of {MAINTENANCE_START_ATTEMPTS} must be exhaustible"
        );
        clear_failed_starts();
    }

    /// A response WITH a status is not a transport failure: the hub answered, so
    /// the network is not blocking it and the route must not change.
    #[test]
    fn a_response_with_a_status_does_not_switch_the_route() {
        let _serialised = preference_guard();
        forget_prefer_maintenance_for_tests();

        note_transport_failure(false);
        assert!(
            !prefer_maintenance(),
            "an answered request (even a 4xx/5xx) must not switch this session to the maintenance route"
        );

        note_transport_failure(true);
        assert!(
            prefer_maintenance(),
            "a request that produced no HTTP status at all is what the maintenance route exists for"
        );

        forget_prefer_maintenance_for_tests();
    }

    #[test]
    fn availability_tracks_whether_a_key_was_compiled_in() {
        // This asserts the SHAPE of the rule rather than a value, because the
        // value depends on how the test binary was built. A build without the key
        // must report unavailable, which is what stops the fallback from being
        // attempted and then failing confusingly.
        let compiled = option_env!("CHRYSALIS_ACTIVATION_TUNNEL_KEY").is_some();
        assert_eq!(
            activation_tunnel_available(),
            compiled,
            "availability must match whether the environment variable was present at build time"
        );
    }

    /// Puts a warm `Tunnel` in the maintenance slot without spawning anything.
    ///
    /// `child: None` is what makes this free: `Drop` has nothing to kill, so a test
    /// can occupy the slot and later clear it. Shared by the tests that need to
    /// distinguish "a route exists" from "I started one".
    async fn install_fake_warm_engine() {
        let mut slot = maintenance_slot().lock().await;
        *slot = Some(Tunnel {
            child: None,
            forward_port: 17998,
        });
    }

    #[test]
    fn the_tunnel_targets_plain_http_on_the_bootstrap_port() {
        let tunnel = Tunnel {
            child: None,
            forward_port: 17999,
        };
        assert_eq!(
            tunnel.http_base(),
            format!("http://127.0.0.1:{FILTER_PORT}"),
            "the far side is plain HTTP inside the SS stream — a https:// base would \
             try to handshake TLS with the path filter, which speaks HTTP — and the \
             destination must be the FILTER, not the SS listener: `ssserver` dials \
             whatever the client names, so naming the listener would loop"
        );
        assert_ne!(
            FILTER_PORT, TUNNEL_PORT,
            "the destination and the listener must not be the same port; if they ever \
             coincide the stream is dialled back at the listener that terminates it"
        );
        assert!(tunnel.proxy_address().starts_with("socks5h://127.0.0.1:"));
    }

    /// **Enrolment is two requests, and the first one must not disable the second.**
    ///
    /// `pages/activation.tsx` calls `/api/code-lookup` and then `/api/activate`.
    /// The lookup's own comment says its failure is advisory — the screen goes on
    /// to activate regardless. But `post` recorded a session-wide failure on any
    /// transport failure, so a lookup that timed out made `post` refuse the
    /// activation before starting an engine. The one call that enrols the student
    /// was disabled by the one whose answer nothing depends on.
    ///
    /// This pins the two halves of the fix: the flag is only set for transport
    /// failures (a hub that ANSWERED must never set it), and a warm engine is
    /// reused rather than costing a second 60 MB spawn.
    ///
    /// FALSIFIED before keeping: making `post` spawn per call (the old shape) fails
    /// the reuse assertion; setting the flag on a parsed 4xx response fails the
    /// "an answer is not a failure" assertion.
    #[tokio::test]
    async fn a_warm_engine_is_reused_so_two_requests_cost_one_start() {
        // Serialised against the other tests that arrange the same static slot;
        // see `slot_guard`. Without this, a concurrent `drop_warm_engine()` clears
        // the entry between the setup below and the assertion.
        let _serialised = slot_guard().await;
        // The slot starts empty in a fresh test process; force the state a second
        // call would see, without spawning anything.
        {
            let mut slot = tunnel_slot().lock().await;
            *slot = Some((
                Tunnel {
                    child: None,
                    forward_port: 17998,
                },
                Instant::now(),
            ));
        }

        let first = endpoint().await.expect("no error expected");
        let second = endpoint().await.expect("no error expected");
        let first = first.expect("a warm engine must be handed back");
        let second = second.expect("a warm engine must be handed back");

        assert_eq!(
            first.proxy, second.proxy,
            "a second request must reuse the running engine — spawning one per call costs a \
             60 MB start on a school laptop for every enrolment step"
        );
        assert_eq!(first.http_base, format!("http://127.0.0.1:{FILTER_PORT}"));

        // Clean up: the test holds a slot with no child process, and leaving it
        // would make every later test in this binary see a "warm" engine.
        drop_warm_engine().await;
    }

    /// The failure flag is cleared by `drop_warm_engine`'s sibling path but is a
    /// process-wide static, so this asserts the property that matters rather than
    /// the flag's value: a fresh engine is the ONLY thing a retry may reuse.
    #[tokio::test]
    async fn a_stale_engine_is_not_reused() {
        // Same static slot as the reuse test; see `slot_guard`.
        let _serialised = slot_guard().await;
        {
            let mut slot = tunnel_slot().lock().await;
            *slot = Some((
                Tunnel {
                    child: None,
                    forward_port: 17997,
                },
                // Older than the TTL, so it must be discarded.
                Instant::now() - TUNNEL_IDLE_TTL - Duration::from_secs(1),
            ));
        }

        // `endpoint()` will try to start a real engine here and fail (there is no
        // sidecar in a unit test), which is the observable proof that the stale
        // entry was NOT handed back. What must not happen is it being reused.
        let result = endpoint().await;
        if let Ok(Some(entry)) = result {
            assert_ne!(
                entry.proxy, "socks5h://127.0.0.1:17997",
                "an engine idle past its TTL must be dropped, not handed to the next call"
            );
        }
        drop_warm_engine().await;
    }
}
