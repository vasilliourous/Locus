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

impl Tunnel {
    /// Starts an engine configured to send everything to the activation tunnel,
    /// and waits until its SOCKS listener is accepting.
    ///
    /// Returns `Ok(None)` when this build has no bootstrap key — not an error,
    /// because a build without the key is the normal case and the caller must
    /// fall through to reporting the original failure.
    pub async fn start() -> Result<Option<Self>> {
        let Some(key) = BOOTSTRAP_KEY.filter(|key| !key.is_empty()) else {
            tracing::debug!(target: "locus::activation_tunnel", "no bootstrap key compiled in; tunnel unavailable");
            return Ok(None);
        };

        let port = free_loopback_port()?;
        let dir = scratch_dir()?;
        let config = dir.join("activation-tunnel.yaml");
        let ss_hub = crate::locus::contract::ss_hub().await;
        std::fs::write(&config, render_config(port, &ss_hub, key)?)
            .with_context(|| format!("could not write the activation tunnel config to {}", config.display()))?;

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

/// Renders the engine document.
///
/// Kept as a pure function so the *generated document* — which is where a
/// missing `MATCH` rule or a second proxy would silently widen what the tunnel
/// can reach — is testable without spawning anything.
fn render_config(forward_port: u16, ss_hub: &str, key: &str) -> Result<String> {
    // Built as a value rather than a format string so a key containing YAML
    // metacharacters cannot break out of its scalar. The key is base64 today, so
    // this is defence in depth rather than a live hazard — but the value is a
    // credential and a quoting bug here would leak it into the wrong place.
    let proxies = serde_yaml_ng::Value::Sequence(vec![serde_yaml_ng::Value::Mapping(
        [
            ("name", serde_yaml_ng::Value::String("Activation-Tunnel".into())),
            ("type", serde_yaml_ng::Value::String("ss".into())),
            // The engine dials the hub's bootstrap listener at the hub's real
            // name. This was `127.0.0.1`, which is the loopback of **the student's
            // own machine** — the comment beside it claimed the engine rewrote the
            // address to the SS server's host, and mihomo does not do that. The
            // outbound connects to exactly the `server` it is given, so the tunnel
            // could only ever have worked against a listener running locally.
            //
            // `ss_hub` is `contract::ss_hub()`, derived from the resolved hub URL,
            // so a device moved to an alias dials the bootstrap listener at the
            // name it can reach. It is a NAME on purpose: the port presents no
            // hostname to the filter, and resolving it is the one lookup a
            // filtering network may still answer.
            ("server", serde_yaml_ng::Value::String(ss_hub.to_owned())),
            ("port", serde_yaml_ng::Value::Number(TUNNEL_PORT.into())),
            ("cipher", serde_yaml_ng::Value::String(TUNNEL_METHOD.into())),
            ("password", serde_yaml_ng::Value::String(key.to_owned())),
            // TCP only: every enrolment endpoint is a small request/response and
            // the hub's listener is configured `tcp_only`.
            ("udp", serde_yaml_ng::Value::Bool(false)),
        ]
        .into_iter()
        .map(|(k, v)| (serde_yaml_ng::Value::String(k.into()), v))
        .collect(),
    )]);

    let group_members = serde_yaml_ng::Value::Sequence(vec![serde_yaml_ng::Value::String("Activation-Tunnel".into())]);
    let groups = serde_yaml_ng::Value::Sequence(vec![serde_yaml_ng::Value::Mapping(
        [
            ("name", serde_yaml_ng::Value::String("Locus Activation".into())),
            ("type", serde_yaml_ng::Value::String("select".into())),
            ("proxies", group_members),
        ]
        .into_iter()
        .map(|(k, v)| (serde_yaml_ng::Value::String(k.into()), v))
        .collect(),
    )]);

    // A catch-all MATCH as the ONLY rule. There is no DIRECT outbound reachable
    // through it, so this engine cannot be used to reach anything except the
    // five-thousand-port listener, and the hub's allowlist bounds that further.
    let rules = serde_yaml_ng::Value::Sequence(vec![serde_yaml_ng::Value::String("MATCH,Locus Activation".into())]);

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

    #[test]
    fn the_config_carries_exactly_one_proxy_and_one_catch_all_rule() {
        let yaml = render_config(17999, "hub.example.org", "AAAA").expect("render");
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
        let yaml = render_config(17999, "hub.example.org", "AAAA").expect("render");
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
        let yaml = render_config(17999, "hub.example.org", "AAAA").expect("render");
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
        let yaml = render_config(17999, "hub.example.org", "AAAA").expect("render");
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
        let yaml = render_config(17999, "hub.example.org", hostile).expect("render");
        let parsed: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).expect("parse");
        assert_eq!(parsed["proxies"][0]["password"].as_str(), Some(hostile));
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
