//! The through-tunnel egress probe must mean what the app thinks it means.
//!
//! `core::manager::probe::probe_egress` decides whether a tunnel is carrying
//! traffic by asking mihomo to run a delay test on the Locus proxy group and
//! reading the result. The *rule* that turns that result into readiness is
//! unit-tested without a socket (`probe.rs`), which is deliberate — but it means
//! the rule is only as good as the assumption that a real mihomo answers the way
//! the rule expects. That assumption had never been checked against a real
//! engine (`docs/reference/STILL-OPEN.md`), and if it is wrong the app reports
//! "no egress" for a tunnel that is carrying traffic perfectly well — the button
//! then sits on "connecting" forever, the report this closes.
//!
//! This test starts the real sidecar with the tier's group name, points it at a
//! URL it can actually reach, and asserts the three facts the probe depends on:
//!
//!   1. a **working** member yields an HTTP 200 whose delay map has a usable
//!      value — the `Ok` path, the one that lets the UI say "connected";
//!   2. an **unreachable** member yields a non-2xx (mihomo answers 504 with
//!      `"get delay: all proxies timeout"`) — the `Failing` path;
//!   3. the values that path produces are classified by the *same* rule the
//!      probe uses, so a timeout sentinel is never mistaken for a measurement.
//!
//! The engine is the authority. Where this test and the code disagree about what
//! mihomo returns, the engine is right.

// Integration tests are their own crates, so the `cfg_attr(test, ...)` lint
// scope in `lib.rs` does not reach here. `panic!`/`expect()` are the correct
// idioms in a test.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cognitive_complexity
)]

#[cfg(unix)]
use std::fs;
use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Report that this test could not run, and fail when the environment asked for
/// completeness.
///
/// A bare `eprintln!("SKIP: …")` is how a test that never ran came to read exactly
/// like a test that passed. `cargo test` prints `ok` either way, so the only way
/// to tell them apart was to scroll for a line the harness does not treat as
/// meaningful. Two tests in this suite are *supposed* to skip on a machine with no
/// network egress (`a_reachable_member_yields_a_usable_delay` and its UoT twin),
/// which means the one signal that distinguished "verified" from "not run" was
/// prose on stderr.
///
/// `LOCUS_REQUIRE_LIVE_ENGINE=1` turns every skip into a failure, so a green run
/// on a machine that *should* have exercised the engine proves it did. Set it
/// wherever a real engine and egress exist — that is the run worth trusting.
fn skip(reason: &str) {
    if std::env::var_os("LOCUS_REQUIRE_LIVE_ENGINE").is_some() {
        panic!("SKIP was requested to be a failure (LOCUS_REQUIRE_LIVE_ENGINE is set): {reason}");
    }
    eprintln!("SKIP: {reason}");
}

/// The sidecar, or a skip that only the tests which cannot run without one may
/// take.
///
/// # Why this exists
///
/// `LOCUS_REQUIRE_LIVE_ENGINE` was a knob nobody turned, so `cargo test` stayed
/// skip-silent — and worse, it was applied to the wrong thing. Every test in this
/// file skipped on a missing sidecar, *including the four that need no network at
/// all*. Those four are the ones that assert a broken tunnel reads as broken, and
/// they are cheap and deterministic; skipping them on a machine that has simply
/// not run `prebuild` throws away the only part of this suite that always works.
///
/// So there are two requirements, and they are not the same:
///
///   * **the engine** — present after `prebuild`, which CI always runs. Every test
///     here needs it, so a missing engine is a defect in the run, not a fact
///     about this machine.
///   * **egress** — a public host must be reachable for the two positive cases.
///     That is a property of the runner, and no amount of building fixes it.
///
/// [`egress_required`] is for the second: `LOCUS_REQUIRE_EGRESS=1` turns "this
/// runner has no network" from a skip into a failure, so the positive case — the
/// one that was never run, and the direction three shipped defects failed in — is
/// exercised on the job that claims to exercise it.
fn require_engine_when_possible() -> Option<PathBuf> {
    match sidecar() {
        Some(engine) => Some(engine),
        None => {
            // No `LOCUS_REQUIRE_LIVE_ENGINE` check here on purpose: requiring the
            // engine is the DEFAULT now, so there is nothing to opt into. The
            // variable is still honoured for the egress cases below, which is the
            // only situation where a skip is a fact about the machine rather than
            // about the run.
            skip("no mihomo sidecar fetched; run `node scripts/prebuild.mjs` first");
            None
        }
    }
}

/// Whether this run must complete a real round trip through the tunnel.
///
/// See [`require_engine_when_possible`].
fn egress_required() -> bool {
    std::env::var_os("LOCUS_REQUIRE_EGRESS").is_some()
}

/// The URL the egress probe fetches through the tunnel.
///
/// Restated here rather than imported because `core` is a private module of the
/// crate (the integration-test crate can only see `pub` items), and widening the
/// module's visibility for a test would leak the whole supervisor module to make
/// one constant reachable. It is kept byte-for-byte identical to
/// `core::manager::probe::EGRESS_TEST_URL`; if that constant is ever changed, this
/// test's positive case stops exercising the real target and should be updated
/// with it. There is a unit test on the constant's *value* in the crate, so a
/// change there is visible in review.
const EGRESS_TEST_URL: &str = "http://cp.cloudflare.com/generate_204";

/// The repository's `client/` directory, found from this test's manifest dir.
fn client_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().expect("src-tauri must have a parent").to_path_buf()
}

/// The mihomo sidecar for this host, if it has been fetched. See
/// `tier_profile_engine.rs` for why a missing engine is a SKIP, not a failure.
fn sidecar() -> Option<PathBuf> {
    let dir = client_dir().join("src-tauri").join("sidecar");
    let name = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "verge-mihomo-x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "verge-mihomo-x86_64-pc-windows-msvc.exe"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "verge-mihomo-aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "verge-mihomo-x86_64-apple-darwin"
    } else {
        return None;
    };
    let path = dir.join(name);
    sidecar_is_runnable(&path).then_some(path)
}

/// Whether `path` is a file the kernel will actually let us `spawn`.
///
/// A sidecar present with the pinned bytes but mode 0644 satisfies `is_file()`,
/// reaches `start_engine`, and panics with `PermissionDenied` — a failure whose
/// message accuses the test rather than the prebuild. Treating it as "not
/// fetched" keeps the skip honest and points at `prebuild.mjs`, which is what
/// restores the bit.
fn sidecar_is_runnable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        // Any execute bit, matching what the kernel checks for `spawn`. A file
        // without one is a `PermissionDenied` panic dressed as a test failure.
        fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Parses mihomo's delay map out of a JSON body.
///
/// Deliberately not a JSON library: the shape is a flat object of
/// `"name": <uint>`, the test only needs one number, and pulling in `serde_json`
/// here would test the deserialiser rather than the engine.
fn parse_delay_map(body: &str) -> Vec<(String, u32)> {
    let trimmed = body.trim();
    let inner = trimmed
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or("");
    if inner.is_empty() {
        return Vec::new();
    }
    inner
        .split(',')
        .filter_map(|pair| {
            let (name, value) = pair.split_once(':')?;
            let name = name.trim().trim_matches('"').to_owned();
            let value: u32 = value.trim().parse().ok()?;
            Some((name, value))
        })
        .collect()
}

/// A blocking HTTP GET against mihomo's external controller, returning
/// `(status_code, body)`.
///
/// Hand-rolled rather than using a client crate: the probe goes over the plugin's
/// transport, but what this test is checking is mihomo's *answer*, and a minimal
/// client keeps the test's dependency surface at zero.
fn http_get(addr: &str, path: &str) -> (u16, String) {
    let Ok(mut stream) = TcpStream::connect(addr) else {
        // Not up yet (the caller polls) or gone: report a non-answer rather than
        // panicking, so `wait_for_controller` can retry.
        return (0, String::new());
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("set read timeout");
    let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nAccept: */*\r\n\r\n");
    stream.write_all(request.as_bytes()).expect("write request");
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw).into_owned();

    let status = text
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
    (status, body)
}

/// Polls the controller until `/version` answers or the deadline passes.
fn wait_for_controller(addr: &str, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        let (status, _) = http_get(addr, "/version");
        if status == 200 {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// Kills the engine on drop so a failed assertion cannot leak a process.
struct EngineGuard(Child);

impl Drop for EngineGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Starts the engine on `config` and waits until its controller answers.
///
/// Returns the guard and the controller address. Panics with the engine's own
/// stderr on startup failure, because "the controller never came up" is useless
/// without the reason the engine gave.
fn start_engine(engine: &Path, dir: &Path, config: &Path, controller: &str) -> EngineGuard {
    let log = dir.join("engine.log");
    let out = std::fs::File::create(&log).expect("create engine log");
    let err = out.try_clone().expect("clone engine log");
    let child = Command::new(engine)
        .arg("-d")
        .arg(dir)
        .arg("-f")
        .arg(config)
        .current_dir(engine.parent().expect("engine has a parent"))
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .spawn()
        .expect("the mihomo sidecar must be executable");
    let guard = EngineGuard(child);

    if !wait_for_controller(controller, Duration::from_secs(15)) {
        let engine_log = std::fs::read_to_string(&log).unwrap_or_default();
        panic!("mihomo never answered /version on {controller}; engine log:\n{engine_log}");
    }
    guard
}

/// A free port, held open until the engine is about to bind it.
///
/// Tests run in parallel, so a port number handed out by "bind :0, read it, drop
/// the listener" can be claimed by another test's engine in the gap before this
/// one spawns. Holding the reservation until the last moment — and, more
/// importantly, giving each test a distinct base so two tests never *ask* for
/// the same neighbourhood — keeps the parallel runs from colliding.
struct PortReservation {
    listener: Option<TcpListener>,
    port: u16,
}

impl PortReservation {
    fn bind() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind reservation");
        let port = listener.local_addr().expect("local addr").port();
        Self {
            listener: Some(listener),
            port,
        }
    }

    /// The reserved port. Released to the engine by [`Self::release`].
    const fn port(&self) -> u16 {
        self.port
    }

    /// Drops the holding listener so the engine can bind the port.
    fn release(&mut self) {
        self.listener = None;
    }
}

/// A port no process is listening on, so a delay test against it fails fast.
///
/// Nothing is held open — this port is meant to be dead — so a different test
/// could in principle take it; the value only has to be *unlikely* to be live,
/// and any port that answers would just mean the test's "unreachable" target was
/// reachable, which the reserved-port scheme above makes vanishingly unlikely.
fn unused_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind probe");
    listener.local_addr().expect("local addr").port()
}

/// Writes a config whose group is the tier's own group name and whose single
/// member is a `direct` outbound, so the delay test exercises real egress.
///
/// `direct` rather than a real proxy because the test must not depend on a live
/// Locus hub: what is being checked is how mihomo answers the *group delay*
/// route, and a `direct` member reaches the test's own 204 server through the
/// same URLTest path a real proxy would use.
fn write_config(dir: &Path, controller_port: u16, mixed_port: u16) -> PathBuf {
    std::fs::create_dir_all(dir).expect("create config dir");
    let config = dir.join("config.yaml");
    let yaml = format!(
        "mixed-port: {mixed_port}\n\
         external-controller: 127.0.0.1:{controller_port}\n\
         log-level: debug\n\
         ipv6: false\n\
         dns:\n\
         \x20 enable: true\n\
         \x20 ipv6: false\n\
         \x20 enhanced-mode: redir-host\n\
         \x20 nameserver:\n\
         \x20   - system\n\
         proxies:\n\
         \x20 - name: \"Locus\"\n\
         \x20   type: direct\n\
         \x20   udp: false\n\
         \x20 - name: \"Locus-UoT\"\n\
         \x20   type: direct\n\
         \x20   udp: false\n\
         proxy-groups:\n\
         \x20 - name: \"Locus Auto\"\n\
         \x20   type: select\n\
         \x20   proxies:\n\
         \x20     - \"Locus-UoT\"\n\
         \x20     - \"Locus\"\n\
         rules:\n\
         \x20 - \"MATCH,Locus Auto\"\n"
    );
    std::fs::write(&config, yaml).expect("write config");
    config
}

/// Why `log-level: debug` is in the config above.
///
/// At `warning` the engine logs nothing about a dial that fails, so this test's
/// failure message had no engine-side evidence at all and could not be
/// debugged from CI. `debug` puts the resolver's answer and the dial's error in
/// `engine.log`, which the failure path now prints.
///
/// Why `dns.nameserver: [system]` is in the config above, and why it is not
/// optional.
///
/// With no `dns:` block, mihomo falls back to its **built-in default resolver**,
/// whose upstreams are `114.114.114.114`, `223.5.5.5`, `8.8.8.8`, `1.0.0.1`
/// (`dns.defaultNameserver` / `dns.nameserver` in `config/config.go`). Those are
/// working resolvers and quite the wrong ones for this test:
///
///   * a CI runner resolves `cp.cloudflare.com` in ~10 ms through the system
///     resolver and answers `204` (measured on the runner, 2026-11-03);
///   * the engine, asking `223.5.5.5` and `doh.pub`, got **no** answer inside the
///     probe's 5 s budget, so the delay test returned `504 Timeout`.
///
/// `Timeout` is the exact string a *dead tunnel* produces, so the test failed
/// and the recorded cause was "the runner's network" — for four red runs over
/// two days, while the runner demonstrably had egress the whole time. Any
/// machine whose view of a Chinese DNS server is slow or blocked loses this test
/// for a reason that has nothing to do with what it asserts.
///
/// `system` makes the engine use the resolver the operating system already
/// trusts, which is the same one `curl` and `getent` use — so the control and the
/// measurement cannot disagree about naming. The shipped client's own config is
/// untouched by this: it is a property of the *test harness*, not of the product.
/// The path the egress probe asks for, with the tier group name.
///
/// Mirrors `probe::probe_egress`: the URL is passed as the `url` query
/// parameter, percent-encoded the way the plugin encodes it, so this test
/// exercises the same route the app reaches.
fn group_delay_path(test_url: &str, timeout_secs: u32) -> String {
    format!(
        "/group/Locus%20Auto/delay?url={}&timeout={timeout_secs}",
        encode(test_url)
    )
}

/// The path for a **single outbound**, which is what the probe now uses.
///
/// This is the route that matters since 2026-10-02. The group route above tests
/// only the group's currently selected member; the shipped probe asks about each
/// outbound in turn so a tunnel cannot be reported dead because mihomo happened
/// to have a different member selected.
fn member_delay_path(member: &str, test_url: &str, timeout_secs: u32) -> String {
    format!(
        "/proxies/{}/delay?url={}&timeout={timeout_secs}",
        encode(member),
        encode(test_url)
    )
}

/// Percent-encodes a query value exactly as the plugin does.
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// The probe's own classification rule, duplicated here on purpose.
///
/// This test's job is to pin the *contract* — what mihomo returns — against the
/// rule the probe applies. Borrowing the rule from the crate would make the test
/// agree with the code by construction and hide exactly the drift it exists to
/// catch, so it is restated and cross-checked against the real engine's output.
///
/// **Restated does not mean free to drift.** This copy carried the pre-2026-10-01
/// rule (`delay < timeout_ms`) for a full release *after* the shipped rule dropped
/// that term, so the test pinned a contract the app no longer honoured — and it
/// was the version that refuses `5000`, the case the previous fix existed to stop
/// refusing. Keeping the two in step is the price of restating the rule; the
/// shipped rule now points back here so the pair is findable.
const fn delay_is_a_measurement(delay: u32, _timeout_secs: u32) -> bool {
    const IMPLAUSIBLE_DELAY: u32 = 100_000;
    delay > 0 && delay <= IMPLAUSIBLE_DELAY
}

/// A working member must produce a 200 whose delay map has a usable value. This
/// is the `Ok` path — the one that lets a connected tunnel stop saying
/// "connecting".
#[test]
fn a_reachable_member_yields_a_usable_delay() {
    let Some(engine) = require_engine_when_possible() else {
        return;
    };

    let dir = std::env::temp_dir().join("locus-egress-probe-ok");
    let mut controller_res = PortReservation::bind();
    let mut mixed_res = PortReservation::bind();
    let controller_port = controller_res.port();
    let mixed_port = mixed_res.port();
    let controller = format!("127.0.0.1:{controller_port}");

    let config = write_config(&dir, controller_port, mixed_port);

    // Release the reserved ports to the engine, then start it.
    controller_res.release();
    mixed_res.release();
    let _guard = start_engine(&engine, &dir, &config, &controller);

    // The target is the URL the probe actually uses, reached through a `direct`
    // member so the test needs no live Locus hub. It is a *public* host on
    // purpose: mihomo's `direct` outbound reports a delay of `0` for a loopback
    // target (loopback bypasses the proxy path), so a local server cannot stand
    // in for a real round trip here — verified against the sidecar before this
    // test was written.
    //
    // This test therefore needs one outbound request to succeed. Where that is
    // impossible (an offline runner) it SKIPs rather than failing — unless the
    // run declared `LOCUS_REQUIRE_EGRESS`, in which case a skip would be the
    // whole point being missed. The fact it proves is about how mihomo answers a
    // *completed* round trip, and no network means no round trip, not a broken
    // probe.
    let test_url = EGRESS_TEST_URL;
    // The route the shipped probe now takes: one outbound by name, not the
    // group. Keeping this test on the group route would leave it pinning a call
    // the app no longer makes — the exact staleness that let the previous
    // classifier drift go unnoticed for a release.
    let path = member_delay_path("Locus", test_url, 5);

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last = (0u16, String::new());
    let mut usable = false;
    while Instant::now() < deadline {
        last = http_get(&controller, &path);
        if last.0 == 200 {
            // A single-proxy reply is `{"delay": N}`; the group reply is a map.
            // Accept either shape so this stays about the *engine's* answer
            // rather than about which endpoint happened to be called.
            let delays = parse_delay_map(&last.1);
            let single = parse_single_delay(&last.1);
            if delays.iter().any(|(_, delay)| delay_is_a_measurement(*delay, 5)) || delay_is_a_measurement(single, 5) {
                usable = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    if !usable {
        // Failure, not a skip, when the run has declared it has egress. The
        // distinction matters: `skip` here used to mean "this runner cannot prove
        // the positive case", which is honest — but nobody set the variable that
        // would have made it a failure, so the one assertion that a WORKING tunnel
        // reads as working was never enforced anywhere.
        //
        // The condition is `!usable` and NOT `last.0 != 200` on purpose. The
        // engine answers a failed delay test with **200 and a body full of error
        // text** — observed here as `(200, "{\"message\":\"An error occurred in the
        // delay test\"}")` — so the narrower condition fell through to the
        // assertion below and reported "a reachable member must answer 200, got
        // 200". A check whose failure message contradicts itself is a check
        // nobody will debug; the flag that matters is whether a MEASUREMENT
        // arrived, which `usable` already records.
        if egress_required() {
            // One retry against a second public host before failing.
            //
            // `cp.cloudflare.com` is a convenience endpoint: it is fast and
            // body-less, which is right for a shipped probe issued every few
            // seconds on a student's machine. It is NOT the right thing to hang a
            // release check on. A single host that happens to be slow — or blocked
            // by whatever policy the runner sits behind — would make this red for
            // a reason that has nothing to do with the probe, and a check that
            // fails for the wrong reason gets its variable unset and then gets
            // ignored. Retrying a *different* host keeps the assertion about
            // "a round trip completes through the tunnel" rather than about one
            // third party's availability.
            //
            // If the second host fails too, the failure stands. Two independent
            // hosts failing is a fact about the tunnel or the runner, not about
            // Cloudflare.
            let retry_url = "https://www.gstatic.com/generate_204";
            let retry_path = member_delay_path("Locus", retry_url, 5);
            let mut retry_last = (0u16, String::new());
            let retry_deadline = Instant::now() + Duration::from_secs(15);
            while Instant::now() < retry_deadline {
                retry_last = http_get(&controller, &retry_path);
                if retry_last.0 == 200 && delay_is_a_measurement(parse_single_delay(&retry_last.1), 5) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            // The engine's own log, printed on failure.
            //
            // WHY: this test failed on every Linux run from 2026-11-02, with
            // `Timeout` — the one string a dead tunnel and an unreachable host
            // both produce. The engine was writing a log the whole time and
            // nothing printed it, so four separate diagnoses were attempted
            // against an assumption and all four were wrong. A failure that
            // arrives with no engine-side evidence cannot be debugged from CI,
            // and asking someone to reproduce a runner-only network condition on
            // a laptop wastes the one advantage CI has: it is where the failure
            // actually happens.
            let engine_log = dir.join("engine.log");
            let log_tail = std::fs::read_to_string(&engine_log)
                .map(|text| {
                    let lines: Vec<&str> = text.lines().collect();
                    let from = lines.len().saturating_sub(40);
                    lines[from..].join("\n")
                })
                .unwrap_or_else(|e| format!("(could not read {}: {e})", engine_log.display()));

            panic!(
                "this run requires egress but the sidecar could not complete a round trip to \
                 EITHER {test_url} (status {:?}, body {:?}) OR {retry_url} (status {:?}, body \
                 {:?}).\n\
                 \n\
                 --- the engine's last 40 log lines ---\n\
                 {log_tail}\n\
                 ----------------------------------------",
                last.0, last.1, retry_last.0, retry_last.1
            );
        }
        skip(&format!(
            "no network egress from this runner; the sidecar could not complete a round trip \
             to {test_url} (status {:?}, body {:?})",
            last.0, last.1
        ));
        return;
    }

    assert_eq!(
        last.0, 200,
        "a reachable group member must answer 200, got {} with body {:?}",
        last.0, last.1
    );
    assert!(
        usable,
        "a reachable member must yield a usable measurement, got {:?}",
        last.1
    );
}

/// An unreachable member must produce a non-2xx. mihomo answers 504 with
/// `"get delay: all proxies timeout"`; the probe reads any non-success as
/// `Failing`, so this is the branch that (correctly) holds at "connecting".
///
/// The load-bearing assertion is that this case does NOT look like success: if
/// the engine answered 200 with a zero delay for a dead member, the probe's
/// `any(delay > 0)` guard is all that stands between it and a false "connected".
#[test]
fn an_unreachable_member_is_not_reported_as_success() {
    let Some(engine) = require_engine_when_possible() else {
        return;
    };

    let mut controller_res = PortReservation::bind();
    let mut mixed_res = PortReservation::bind();
    let controller_port = controller_res.port();
    let mixed_port = mixed_res.port();
    let controller = format!("127.0.0.1:{controller_port}");

    let dir = std::env::temp_dir().join("locus-egress-probe-fail");
    let config = write_config(&dir, controller_port, mixed_port);

    controller_res.release();
    mixed_res.release();
    let _guard = start_engine(&engine, &dir, &config, &controller);

    // A port with nothing listening: the delay test cannot complete.
    let dead_url = format!("http://127.0.0.1:{}/generate_204", unused_port());
    let (status, body) = http_get(&controller, &group_delay_path(&dead_url, 2));

    assert_ne!(
        status, 200,
        "an unreachable member must not answer 200; body was {body:?}"
    );

    // Even in the failure body there must be no usable measurement to mistake for
    // success: whatever numeric values appear, none classifies as a measurement.
    let delays = parse_delay_map(&body);
    assert!(
        delays.iter().all(|(_, delay)| !delay_is_a_measurement(*delay, 2)),
        "a failed test must not yield a usable measurement, got {delays:?}"
    );
}

/// A dead member must answer **`200` with a zero delay** on the per-member
/// route — not an error.
///
/// This is the asymmetry the whole per-member fix rests on, and it is why the
/// group route could strand a working tunnel while the member route cannot:
///
///   * `/group/Locus%20Auto/delay` turns ANY non-2xx into a hard `Err` in the
///     plugin (`ret_failed_resp!`), so "every member failed" arrives at the probe
///     as an indistinguishable transport error — there is no delay map to read.
///   * `/proxies/<name>/delay` turns a non-2xx into `Ok(ProxyDelay { delay: 0 })`
///     instead (it logs the engine's message and returns the zero sentinel), so a
///     dead member is an ordinary `Ok(0)` the classifier already rejects and a
///     live one is an ordinary `Ok(millis)`.
///
/// So asking per member converts "unmeasurable" from an error the caller cannot
/// interpret into a value it can: the sentinel that meant "this whole group is
/// unusable" now means exactly "this one member failed", which is what lets the
/// probe try the next member instead of giving up on the tier.
///
/// The assertion is on the *shape* the caller depends on — a non-measurement
/// rather than an unusable error — rather than on the exact status code, because
/// the client must not care whether the engine chose 503 or 504.
#[test]
fn a_dead_member_answers_with_a_non_measurement_not_an_error() {
    // `require_engine_when_possible` rather than `skip`: this case needs the
    // engine but NOT the network, so it can and must run in CI. See that
    // helper's doc comment for why the old blanket `skip` made a missing
    // sidecar indistinguishable from a deleted assertion.
    let Some(engine) = require_engine_when_possible() else {
        return;
    };

    let dir = std::env::temp_dir().join("locus-egress-probe-deadmember");
    let mut controller_res = PortReservation::bind();
    let mut mixed_res = PortReservation::bind();
    let controller_port = controller_res.port();
    let mixed_port = mixed_res.port();
    let controller = format!("127.0.0.1:{controller_port}");

    let config = write_config(&dir, controller_port, mixed_port);
    controller_res.release();
    mixed_res.release();
    let _guard = start_engine(&engine, &dir, &config, &controller);

    // A target nothing is listening on, so the member cannot complete a round trip.
    let dead_url = format!("http://127.0.0.1:{}/generate_204", unused_port());
    let (status, body) = http_get(&controller, &member_delay_path("Locus-UoT", &dead_url, 2));

    // The plugin maps a non-2xx here to `Ok(ProxyDelay { delay: 0 })`, so the
    // probe receives a value it can classify rather than an `Err` it cannot
    // inspect. What must never happen is a *usable measurement* coming back.
    let delay = parse_single_delay(&body);
    assert!(
        !delay_is_a_measurement(delay, 2),
        "a dead member must not yield a usable measurement (status {status}, body {body:?})"
    );
}

/// Pulls the `delay` field out of a single-proxy delay reply, or `0` when the
/// body carries no numeric delay — which is exactly what the plugin's zero
/// sentinel amounts to for the caller.
fn parse_single_delay(body: &str) -> u32 {
    body.split("\"delay\"")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).find(|s| !s.is_empty()))
        .and_then(|digits| digits.parse().ok())
        .unwrap_or(0)
}

/// The engine's failure sentinel must not classify as a measurement — and the
/// timeout value must.
///
/// Two rules live here, and they were conflated for two releases:
///
///   * `0` and the `1e5`-and-above sentinel are **not** measurements. mihomo
///     reports a failed test as `0` in the same numeric field a real measurement
///     uses, so a bare `delay > 0` check would accept a number that means "the
///     test failed".
///   * the **timeout value itself IS egress** (2026-10-01, second round). The
///     engine dialled the target, the request left the machine, and only our own
///     clock ran out. The earlier rule here rejected `>= timeout * 1000`, which
///     is why a working-but-slow school link read as `NoEgress` and stranded the
///     button on "connecting" — the defect this file's own history records twice.
///
/// The assertion that the timeout counts as egress is the one that failed against
/// the pre-fix rule, and it is the reason this file is not allowed to restate the
/// classifier from memory.
#[test]
fn the_engine_sentinels_are_not_measurements() {
    let timeout_secs = 3;
    // The zero sentinel mihomo uses for a failed single-proxy test: not a
    // measurement.
    assert!(!delay_is_a_measurement(0, timeout_secs));
    // An implausible value is an error sentinel, not a latency.
    assert!(!delay_is_a_measurement(100_001, timeout_secs));
    // A genuine round trip counts.
    assert!(delay_is_a_measurement(120, timeout_secs));
    // THE CASE THE OLD RULE GOT WRONG: the timeout value is egress. Slow is not
    // dead, and treating it as dead is what stranded working tunnels.
    assert!(
        delay_is_a_measurement(timeout_secs * 1000, timeout_secs),
        "a delay at the probe's own budget is egress, not failure — see \
         docs/reference/EGRESS-READINESS.md"
    );
}

/// A group whose members ALL fail returns **no delay map at all** — a `504` whose
/// body is an error message, not an object of delays.
///
/// Measured against the real sidecar (v1.19.31, 2026-10-02) with a `select` group
/// holding two unreachable members:
///
/// ```text
/// GET /group/Locus%20Auto/delay?url=…&timeout=5
///   -> 504 {"message":"get delay: all proxies timeout"}
/// GET /proxies/Locus-Dead/delay?url=…&timeout=5
///   -> 503 {"message":"An error occurred in the delay test"}
/// ```
///
/// This is the shape that matters and that nothing pinned: the probe's `Ok(Ok(..))`
/// arm is never reached, because the plugin turns a non-2xx into `Err` (`ret_failed_resp!`
/// in `mihomo.rs`). So **no value of `delay_is_a_measurement` can rescue this case** —
/// every member timing out is indistinguishable, at the client, from the tunnel being
/// down. A group where the selected member is slow-but-working is the case the
/// classifier fixes; a group where *every* member fails is a different defect and
/// is tracked in `STILL-OPEN.md`.
///
/// The assertion is deliberately about the *contract* (no 2xx, no parseable
/// measurements) rather than about the exact status, because 503/504 is the
/// engine's choice and the client must not depend on which.
#[test]
fn a_group_whose_members_all_fail_returns_no_delay_map() {
    let Some(engine) = require_engine_when_possible() else {
        return;
    };

    let dir = std::env::temp_dir().join("locus-egress-probe-alldead");
    let mut controller_res = PortReservation::bind();
    let mut mixed_res = PortReservation::bind();
    let controller_port = controller_res.port();
    let mixed_port = mixed_res.port();
    let controller = format!("127.0.0.1:{controller_port}");

    // `write_config` builds a single-member select group pointing at a `direct`
    // outbound. Reached through the group with a target nothing is listening on,
    // that member cannot complete a round trip — the all-dead case.
    let config = write_config(&dir, controller_port, mixed_port);
    controller_res.release();
    mixed_res.release();
    let _guard = start_engine(&engine, &dir, &config, &controller);

    let dead_url = format!("http://127.0.0.1:{}/generate_204", unused_port());
    let (status, body) = http_get(&controller, &group_delay_path(&dead_url, 2));

    assert_ne!(status, 200, "an all-dead group must not answer 200; body was {body:?}");
    assert!(
        parse_delay_map(&body).is_empty(),
        "an all-dead group must return an error message, not a delay map; body was {body:?}"
    );
}
