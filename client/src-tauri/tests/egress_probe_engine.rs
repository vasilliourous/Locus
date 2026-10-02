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

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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
    path.is_file().then_some(path)
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
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nAccept: */*\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .expect("write request");
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
         log-level: warning\n\
         proxies:\n\
         \x20 - name: \"Locus\"\n\
         \x20   type: direct\n\
         \x20   udp: false\n\
         proxy-groups:\n\
         \x20 - name: \"Locus Auto\"\n\
         \x20   type: select\n\
         \x20   proxies:\n\
         \x20     - \"Locus\"\n\
         rules:\n\
         \x20 - \"MATCH,Locus Auto\"\n"
    );
    std::fs::write(&config, yaml).expect("write config");
    config
}

/// The path the egress probe asks for, with the tier group name.
///
/// Mirrors `probe::probe_egress`: the group name is `Locus Auto`, and the URL is
/// passed as the `url` query parameter. The name is percent-encoded the way the
/// plugin encodes it, so this test exercises the same route the app reaches.
fn group_delay_path(test_url: &str, timeout_secs: u32) -> String {
    let encoded: String = test_url
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    format!("/group/Locus%20Auto/delay?url={encoded}&timeout={timeout_secs}")
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
    let Some(engine) = sidecar() else {
        eprintln!("SKIP: no mihomo sidecar fetched; run `node scripts/prebuild.mjs` first");
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
    // impossible (an offline CI runner) it SKIPs rather than failing: the fact it
    // proves is about how mihomo answers a *completed* round trip, and no network
    // means no round trip, not a broken probe.
    let test_url = EGRESS_TEST_URL;
    let path = group_delay_path(test_url, 5);

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last = (0u16, String::new());
    let mut usable = false;
    while Instant::now() < deadline {
        last = http_get(&controller, &path);
        if last.0 == 200 {
            let delays = parse_delay_map(&last.1);
            if delays
                .iter()
                .any(|(_, delay)| delay_is_a_measurement(*delay, 5))
            {
                usable = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    if !usable && last.0 != 200 {
        eprintln!(
            "SKIP: no network egress from this runner; the sidecar could not complete a round \
             trip to {test_url} (last answer: {last:?})"
        );
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
    let Some(engine) = sidecar() else {
        eprintln!("SKIP: no mihomo sidecar fetched; run `node scripts/prebuild.mjs` first");
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
        delays
            .iter()
            .all(|(_, delay)| !delay_is_a_measurement(*delay, 2)),
        "a failed test must not yield a usable measurement, got {delays:?}"
    );
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
    let Some(engine) = sidecar() else {
        eprintln!("SKIP: no mihomo sidecar fetched; run `node scripts/prebuild.mjs` first");
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

    assert_ne!(
        status, 200,
        "an all-dead group must not answer 200; body was {body:?}"
    );
    assert!(
        parse_delay_map(&body).is_empty(),
        "an all-dead group must return an error message, not a delay map; body was {body:?}"
    );
}
