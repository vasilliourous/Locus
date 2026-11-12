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
/// `LOCUS_ACTIVATION_TUNNEL_KEY` build-time environment variable.
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
const BOOTSTRAP_KEY: Option<&str> = option_env!("LOCUS_ACTIVATION_TUNNEL_KEY");

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
const BOOTSTRAP_KEY_PRESENT: &str = env!("LOCUS_ACTIVATION_TUNNEL_KEY_PRESENT");

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
    if key_in_build == build_script_says {
        format!(
            "activation tunnel: {}, listener port {} → filter port {}",
            if key_in_build { "available" } else { "NOT COMPILED IN" },
            TUNNEL_PORT,
            FILTER_PORT
        )
    } else {
        format!(
            "activation tunnel: INCONSISTENT BUILD — key present={key_in_build}, build script \
             reported {BOOTSTRAP_KEY_PRESENT}. The tunnel is {} in this binary; rebuild before \
             shipping it.",
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
    /// [`crate::locus::contract::ACTIVATION_TUNNEL_ADDR`] for why a name cannot
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
            server: crate::locus::contract::activation_tunnel_addr().to_owned(),
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
        let key = option_env!("LOCUS_FREE_TIER_KEY").filter(|key| !key.is_empty())?;
        Some(Self {
            server: crate::locus::contract::activation_tunnel_addr().to_owned(),
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
                tracing::debug!(target: "locus::activation_tunnel", "no bootstrap key compiled in; tunnel unavailable");
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
            let ss_hub = crate::locus::contract::ss_hub().await;
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
            target: "locus::activation_tunnel",
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
                    target: "locus::activation_tunnel",
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
    format!("{core}-{}{suffix}", env!("LOCUS_BUILD_TARGET"))
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
/// ([`crate::locus::contract::ACTIVATION_TUNNEL_ADDR`]) as the case that needs no
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
const GROUP_NAME: &str = "Locus Activation";

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
/// One failed attempt is a property of the network, not of that request, so it is
/// remembered: a second attempt in the same process is skipped and the caller
/// reports its original failure. Session-scoped on purpose — a process restart is
/// a reasonable moment to re-test, and a stale "it did not work" that never
/// expired would disable the feature for a student who later moved networks.
///
/// One static, read and written by both accessors below. Two `OnceLock`s would
/// initialise independently and the flag would never be observed as set — the
/// spawn-every-beat bug this exists to prevent, wearing a disable flag.
fn tunnel_failed_flag() -> &'static std::sync::atomic::AtomicBool {
    static FAILED: std::sync::OnceLock<std::sync::atomic::AtomicBool> = std::sync::OnceLock::new();
    FAILED.get_or_init(|| std::sync::atomic::AtomicBool::new(false))
}

fn tunnel_already_failed() -> bool {
    tunnel_failed_flag().load(std::sync::atomic::Ordering::Relaxed)
}

fn remember_tunnel_failure() {
    tunnel_failed_flag().store(true, std::sync::atomic::Ordering::Relaxed);
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
// transition observed in `locus_status`).
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
/// lock, and it failed exactly as a split lock does: `cargo test -p locus --lib`
/// ran this module's negative case concurrently with `tier.rs`'s positive one and
/// the assertion saw the other test's write. One flag needs one lock, in the
/// module that owns the flag.
#[cfg(test)]
pub(crate) fn preference_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
    GUARD.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
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

/// Starts the maintenance engine and holds it, unless one is already warm.
///
/// Returns whether a maintenance route exists afterwards. Called from app setup
/// and again when the tunnel drops; both are "make sure one is running", so this
/// is idempotent rather than a start/stop pair with its own state.
pub async fn warm_maintenance() -> bool {
    // Cheap check first: a warm engine needs no lock beyond this.
    if maintenance_proxy().is_some() {
        return true;
    }

    let Some(bootstrap) = Bootstrap::maintenance() else {
        // Once per warm attempt, which is once per status poll while the tunnel is
        // down — hence `debug`, not `warn`. A build without the key is a normal
        // branch build, and a warn would train readers to ignore the tag.
        tracing::debug!("[maintenance] no key, direct only");
        return false;
    };

    // Starting is slow and must not hold the slot lock across the await.
    let started = start_quietly(&bootstrap).await;

    let mut slot = maintenance_slot().lock().await;
    if slot.is_some() {
        // Another caller won the race. Theirs is already serving, so drop this
        // one rather than replacing a working listener with a second.
        return true;
    }
    *slot = started;
    log_warm(slot.as_ref())
}

/// Starts an engine without holding the slot, reporting failure rather than
/// returning it: "could not start" is a state this function records, not an
/// outcome a caller branches on.
async fn start_quietly(bootstrap: &Bootstrap) -> Option<Tunnel> {
    match Tunnel::start_with(bootstrap).await {
        Ok(tunnel) => tunnel,
        Err(error) => {
            tracing::warn!(
                target: "locus::activation_tunnel",
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

/// Stops the maintenance engine, if one is running.
///
/// Called when the plan tunnel is proven to carry traffic: the tunnel now carries
/// everything, so a second route through the free tier would be a second copy of
/// the same traffic at a worse rate.
pub async fn stop_maintenance() {
    let mut slot = maintenance_slot().lock().await;
    *slot = None;
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
pub async fn hub_client_for(hub: &str, timeout: crate::locus::hub::Timeout) -> crate::locus::hub::HubClient {
    let proxy = if prefer_maintenance() {
        match maintenance_proxy() {
            Some(url) => match reqwest::Proxy::all(&url) {
                Ok(proxy) => Some(proxy),
                Err(error) => {
                    tracing::warn!(
                        target: "locus::activation_tunnel",
                        "the maintenance proxy URL could not be used ({error:#}); using direct"
                    );
                    None
                }
            },
            None => None,
        }
    } else {
        None
    };

    crate::locus::hub::client_with_proxy(hub, timeout, proxy).await
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
/// [`crate::locus::hub::Failure::classify`] would report `Transport` and the older
/// tunnel retry never fires on the network this exists for.
pub fn should_retry_over_maintenance(error: &reqwest::Error) -> bool {
    error.status().is_none() && maintenance_proxy().is_some()
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
/// error rather than a reset ([`crate::locus::hub::Failure::classify`]), so keying
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
            target: "locus::hub",
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
pub async fn post<T, B>(path: &str, body: &B, timeout: crate::locus::hub::Timeout) -> Result<T>
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
        .user_agent(concat!("Locus-Client/", env!("CARGO_PKG_VERSION")))
        .proxy(proxy)
        // `no_proxy` must NOT be set here — it is what makes the direct client
        // bypass the system proxy, and here we are deliberately installing one.
        .build()
        .context("could not build the activation tunnel HTTP client")?;

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

    serde_json::from_str(&text).with_context(|| {
        format!("the activation tunnel returned something that is not a Locus response ({status}): {text}")
    })
}

/// The tunnel's per-request ceiling.
///
/// It inherits the direct path's timeout rather than inventing one: a tunnel
/// request that is slower than the direct one it replaced has failed at the only
/// thing it was for.
const fn timeout_seconds(timeout: crate::locus::hub::Timeout) -> Duration {
    Duration::from_secs(match timeout {
        crate::locus::hub::Timeout::Activation => 30,
        crate::locus::hub::Timeout::Heartbeat => 15,
        crate::locus::hub::Timeout::Download => 600,
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
    /// failure in 5 runs of `cargo test --lib --package locus`, in `debug`, with
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
            Some("MATCH,Locus Activation"),
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
        crate::locus::activation_tunnel::note_transport_failure(false);
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
        crate::locus::activation_tunnel::note_transport_failure(error.status().is_none());
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
    /// `locus_status` makes on each readiness transition — rather than a
    /// readiness value, because a test that injected readiness would assert the
    /// poll's wiring, not the lifecycle it triggers. The wiring itself is one `if`
    /// at `cmd/locus.rs`; the behaviour that can regress is here.
    ///
    /// LIMITATION, stated rather than hidden: the 750 ms vs 15 s poll interval
    /// lives in React (`use-connection.ts:84,95`) and cannot be exercised from
    /// Rust. What is proved is that a stop stops and a warm restarts; that a
    /// transition is noticed at all depends on the UI polling, which it does.
    #[tokio::test]
    async fn the_maintenance_engine_stops_and_restarts_across_a_lifecycle_transition() {
        let _serialised = slot_guard().await;

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
            server: crate::locus::contract::activation_tunnel_addr().to_owned(),
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

    #[test]
    fn there_is_no_maintenance_proxy_until_an_engine_is_held() {
        // Nothing started it in this binary's test process, so the honest answer
        // is `None`. If a future change starts one implicitly, this fails and the
        // routing rule's "cold" branch loses its only coverage.
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
        let compiled = option_env!("LOCUS_ACTIVATION_TUNNEL_KEY").is_some();
        assert_eq!(
            activation_tunnel_available(),
            compiled,
            "availability must match whether the environment variable was present at build time"
        );
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
    /// to activate regardless. But `post` set the session-wide `tunnel_failed_flag`
    /// on any transport failure, so a lookup that timed out made `post` refuse the
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
