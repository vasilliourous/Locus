//! The tunnel path end to end: mihomo client → real Shadowsocks-2022 listener →
//! the target, over both TCP and UDP-over-TCP.
//!
//! # Why this file exists
//!
//! Every other engine test in this repository proves something about mihomo
//! *alone*. `tier_profile_engine.rs` proves the generated document is accepted;
//! `egress_probe_engine.rs` proves the delay-test answers are classified
//! correctly. Neither runs a client and a server together, and the two shipped
//! mechanisms that failed in the field — UDP-over-TCP dead fleet-wide under a
//! field-name mismatch, and the per-member delay test answered by the wrong
//! outbound — both sat exactly in that gap. Each side was individually "correct"
//! and the pair were never run against each other.
//!
//! `STILL-OPEN.md` recorded this as unclosable: *"nothing has connected a real
//! machine and played"*. That is true of playing a game, and false of the
//! question underneath it, which is whether the negotiation works at all.
//!
//! # What it proves, and what it does not
//!
//! Proved here, on loopback, with no network:
//!
//!   * a real `sing-box` Shadowsocks-2022 listener accepts this engine's
//!     outbound, so the cipher, key format and wire framing agree;
//!   * **UDP-over-TCP v2 negotiates** — the server logs
//!     `to sp.v2.udp-over-tcp.arpa:0`, which is the magic domain for the
//!     revision `tier.rs` pins with `udp-over-tcp-version: 2`;
//!   * a datagram completes a round trip through it;
//!   * the per-member delay test the readiness probe depends on returns a real
//!     **measurement** through a real tunnel, not a failure sentinel.
//!
//! NOT proved: that the live hub's listener is reachable from a school network,
//! that 1 Mbps is playable, or anything about throughput. Those need hardware on
//! a real network and stay in `STILL-OPEN.md`.
//!
//! # The trap this file was written around
//!
//! The first version of this test pointed the delay probe at a public 204 host,
//! which is what `probe::EGRESS_TEST_URL` names. It reported `Timeout` through a
//! tunnel that was demonstrably working, because this runner has outbound HTTP
//! blocked by sandbox policy — **and the engine reports that as a timeout with no
//! egress**, which is indistinguishable from "the tunnel is dead". So the target
//! here is a local HTTP server. That was checked, not assumed: the same delay
//! probe against `127.0.0.1` returns `{"delay":3}`, and mihomo documents why — a
//! `direct` outbound reports `0` for loopback (loopback bypasses the proxy path),
//! but a *real* proxy outbound tunnels to loopback properly and times it. If a
//! future engine starts short-circuiting loopback for proxies too, the positive
//! case here fails loudly rather than passing for the wrong reason.

// Integration tests are their own crates, so the `cfg_attr(test, ...)` lint scope
// in `lib.rs` does not reach here. `panic!`/`expect()` are the correct idioms in a
// test: a `Result`-returning test reports "failed" without saying which invariant
// broke, and these assertions exist to name the defect.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cognitive_complexity,
    clippy::large_stack_frames
)]

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Shadowsocks-2022 key: 32 raw bytes, base64. Both ends must use the same.
///
/// A fixed test key, not a secret: it exists only for the lifetime of a test
/// process on loopback. Written as the base64 of `0x00..=0x1f` so a reader can
/// verify the length requirement (32 bytes for `2022-blake3-aes-256-gcm`) without
/// decoding anything.
const TEST_KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
const TEST_METHOD: &str = "2022-blake3-aes-256-gcm";

/// Report that this test could not run.
///
/// Unlike the other engine suites, a missing dependency here is legitimately a
/// SKIP rather than a failure, and the difference is worth stating because the
/// rest of this repository argues the opposite:
///
///   * `sing-box` is the HUB's listener, not something this project ships. It is
///     installed on the runner for this test alone, and a machine without it can
///     still say everything else about the client. Demanding it of every
///     contributor would make `cargo test` unrunnable after a fresh clone.
///   * the slicecar may be absent in the `verify` job, which fetches it with
///     `continue-on-error` so that a network failure reports through the §39
///     proof's own message rather than as a cryptic build break.
///
/// `LOCUS_REQUIRE_LIVE_ENGINE=1` turns every one of these into a failure, which is
/// the mode CI uses where both are installed deliberately. A skip that cannot be
/// made to fail is the thing `STILL-OPEN.md` spent two entries complaining about.
fn skip(reason: &str) {
    if std::env::var_os("LOCUS_REQUIRE_LIVE_ENGINE").is_some() {
        panic!("SKIP was requested to be a failure (LOCUS_REQUIRE_LIVE_ENGINE is set): {reason}");
    }
    eprintln!("SKIP: {reason}");
}

fn client_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().expect("src-tauri must have a parent").to_path_buf()
}

/// The mihomo sidecar, by the same rule the other engine tests use.
fn mihomo() -> Option<PathBuf> {
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

/// `sing-box`, found on PATH.
///
/// The hub runs sing-box as its Shadowsocks listener, so it is the only server
/// whose acceptance of this client's outbound means anything. A missing `sing-box`
/// is a SKIP: it is a test dependency, not a product dependency, and demanding it
/// of every contributor would make this suite unrunnable on a fresh laptop.
fn sing_box() -> Option<PathBuf> {
    for candidate in [
        "/usr/bin/sing-box",
        "/usr/local/bin/sing-box",
        "/opt/homebrew/bin/sing-box",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// A minimal 204 responder, so the delay test measures a specific port.
///
/// Binding to port 0 and reading the assigned port back means two runs, or two
/// concurrent tests, cannot collide.
struct Http204 {
    port: u16,
    _thread: std::thread::JoinHandle<()>,
}

impl Http204 {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the 204 responder");
        let port = listener.local_addr().expect("local addr").port();
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                // Read the request line and drain the headers, then answer. A
                // client that sends nothing gets nothing, which is what a timeout
                // looks like — the test would then fail with "no measurement"
                // rather than hanging.
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
                let _ = stream.flush();
            }
        });
        Self { port, _thread: thread }
    }
}

/// A UDP echo server. Replies `echo:` + the payload, so a datagram that arrives
/// is distinguishable from one that was silently dropped.
struct UdpEcho {
    port: u16,
    _thread: std::thread::JoinHandle<()>,
}

impl UdpEcho {
    fn start() -> Self {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("bind the udp echo");
        let port = socket.local_addr().expect("local addr").port();
        let thread = std::thread::spawn(move || {
            let mut buf = [0u8; 2048];
            while let Ok((n, peer)) = socket.recv_from(&mut buf) {
                let mut reply = b"echo:".to_vec();
                reply.extend_from_slice(&buf[..n]);
                let _ = socket.send_to(&reply, peer);
            }
        });
        Self { port, _thread: thread }
    }
}

/// Kills its child on drop, so a failed assertion does not leave an engine or a
/// server running on the developer's machine.
struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind probe")
        .local_addr()
        .expect("local addr")
        .port()
}

fn wait_for<F: Fn() -> bool>(what: &str, within: Duration, ready: F) {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if ready() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("{what} did not become ready within {within:?}");
}

/// The client config, in the shape `locus::tier::build_profile` produces for a
/// tier with UDP-over-TCP.
fn client_yaml(ss_port: u16, controller: u16, mixed: u16) -> String {
    format!(
        "mixed-port: {mixed}\n\
         external-controller: 127.0.0.1:{controller}\n\
         log-level: warning\n\
         proxies:\n\
         \x20 - name: \"Locus\"\n\
         \x20   type: ss\n\
         \x20   server: 127.0.0.1\n\
         \x20   port: {ss_port}\n\
         \x20   cipher: {TEST_METHOD}\n\
         \x20   password: \"{TEST_KEY}\"\n\
         \x20   udp: false\n\
         \x20 - name: \"Locus-UoT\"\n\
         \x20   type: ss\n\
         \x20   server: 127.0.0.1\n\
         \x20   port: {ss_port}\n\
         \x20   cipher: {TEST_METHOD}\n\
         \x20   password: \"{TEST_KEY}\"\n\
         \x20   udp: true\n\
         \x20   udp-over-tcp: true\n\
         \x20   udp-over-tcp-version: 2\n\
         proxy-groups:\n\
         \x20 - name: \"Locus Auto\"\n\
         \x20   type: select\n\
         \x20   proxies:\n\
         \x20     - \"Locus-UoT\"\n\
         \x20     - \"Locus\"\n\
         rules:\n\
         \x20 - \"MATCH,Locus Auto\"\n"
    )
}

fn server_json(ss_port: u16) -> String {
    format!(
        r#"{{
  "log": {{ "level": "info" }},
  "inbounds": [
    {{
      "type": "shadowsocks",
      "tag": "ss-in",
      "listen": "127.0.0.1",
      "listen_port": {ss_port},
      "method": "{TEST_METHOD}",
      "password": "{TEST_KEY}"
    }}
  ],
  "outbounds": [{{ "type": "direct", "tag": "direct" }}]
}}"#
    )
}

/// One `GET` against the controller, returning `(status, body)`.
fn http_get(addr: &str, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("controller must accept");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("set timeout");
    let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).expect("write request");

    let mut raw = String::new();
    let _ = stream.read_to_string(&mut raw);
    let status = raw.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
    (status, body)
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Pulls `delay` out of a single-proxy reply. `0` when absent — the engine's own
/// failure sentinel, which the probe's `delay_is_a_measurement` rejects.
fn delay_of(body: &str) -> u32 {
    body.split("\"delay\"")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).find(|s| !s.is_empty()))
        .and_then(|digits| digits.parse().ok())
        .unwrap_or(0)
}

/// Whether a delay is a usable measurement.
///
/// Deliberately a restatement rather than a call: `core` is private to the crate,
/// and `delay_is_a_measurement` lives behind it. The rules are `0` means "the
/// test failed" and `>= 1e5` is the unmeasurable sentinel; a real millisecond
/// count sits between them.
const fn usable(delay: u32) -> bool {
    delay > 0 && delay < 100_000
}

/// Everything one run needs up, in order, with teardown guaranteed.
struct Harness {
    _singbox: Guard,
    _mihomo: Guard,
    controller_addr: String,
    mixed_port: u16,
    server_log: PathBuf,
}

/// `label` names the scratch directory, and every test passes its own.
///
/// A shared directory looked fine until `cargo test` ran the file's three tests in
/// parallel: each `start()` deleted the directory the previous one had just
/// written its server config and log into, and two tests failed with "the sing-box
/// listener did not become ready". Running with `--test-threads=1` hid it
/// completely, which is exactly how a flake becomes a mystery on CI — the default
/// there is parallel. Distinct directories per test, and no cleanup of anybody
/// else's.
fn start(label: &str, engine: &Path, sing: &Path) -> Harness {
    let dir = std::env::temp_dir().join(format!("locus-e2e-tunnel-{label}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the harness directory");

    let ss_port = free_port();
    let controller_port = free_port();
    let mixed_port = free_port();

    let server_path = dir.join("server.json");
    std::fs::write(&server_path, server_json(ss_port)).expect("write server config");
    let client_path = dir.join("client.yaml");
    std::fs::write(&client_path, client_yaml(ss_port, controller_port, mixed_port)).expect("write client config");

    // sing-box first: its listener must exist before the client dials it.
    let server_log_path = dir.join("server.log");
    let server_log = std::fs::File::create(&server_log_path).expect("create server log");
    let server_log_err = server_log.try_clone().expect("clone server log");
    let _singbox = Guard(
        Command::new(sing)
            .args(["run", "-c"])
            .arg(&server_path)
            .stdout(Stdio::from(server_log))
            .stderr(Stdio::from(server_log_err))
            .spawn()
            .expect("sing-box must spawn"),
    );

    let listener_up = |port: u16| TcpStream::connect(("127.0.0.1", port)).is_ok();
    wait_for("the sing-box listener", Duration::from_secs(15), || {
        listener_up(ss_port)
    });

    let client_log = dir.join("client.log");
    let out = std::fs::File::create(&client_log).expect("create client log");
    let err = out.try_clone().expect("clone client log");
    let _mihomo = Guard(
        Command::new(engine)
            .arg("-d")
            .arg(&dir)
            .arg("-f")
            .arg(&client_path)
            .current_dir(engine.parent().expect("engine has a parent"))
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .expect("the mihomo sidecar must be executable"),
    );

    let controller_addr = format!("127.0.0.1:{controller_port}");
    wait_for("the mihomo controller", Duration::from_secs(15), || {
        TcpStream::connect(controller_addr.as_str()).is_ok()
    });

    Harness {
        _singbox,
        _mihomo,
        controller_addr,
        mixed_port,
        server_log: server_log_path,
    }
}

#[test]
fn tcp_crosses_a_real_shadowsocks_2022_listener() {
    let Some(engine) = mihomo() else {
        skip("no mihomo sidecar fetched; run `node scripts/prebuild.mjs` first");
        return;
    };
    let Some(sing) = sing_box() else {
        skip("sing-box is not installed; it is the hub's listener and the only meaningful peer");
        return;
    };

    let target = Http204::start();
    let h = start("tcp", &engine, &sing);
    let url = format!("http://127.0.0.1:{}/generate_204", target.port);

    // Through the mixed port, which routes by the group: exactly what a student's
    // browser does once the profile is applied.
    let proxy = format!("http://127.0.0.1:{}", h.mixed_port);
    let output = Command::new("curl")
        .args([
            "-sS",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "--max-time",
            "20",
            "-x",
            &proxy,
            &url,
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let code = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            assert_eq!(
                code,
                "204",
                "a request through the tunnel must reach the target; sing-box log:\n{}",
                std::fs::read_to_string(&h.server_log).unwrap_or_default()
            );
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // No `curl` on this machine is a SKIP; a curl that ran and failed is
            // a defect. Conflating them is how "could not test" reads as "passed".
            if stderr.contains("command not found") || out.status.code().is_none() {
                skip("curl is not available to drive the tunnel");
                return;
            }
            panic!(
                "curl through the tunnel failed: {stderr}\nsing-box log:\n{}",
                std::fs::read_to_string(&h.server_log).unwrap_or_default()
            );
        }
        Err(error) => {
            skip(&format!(
                "curl could not be started ({error}); it drives the TCP assertion"
            ));
        }
    }
}

#[test]
fn udp_over_tcp_v2_negotiates_and_carries_a_datagram() {
    let Some(engine) = mihomo() else {
        skip("no mihomo sidecar fetched; run `node scripts/prebuild.mjs` first");
        return;
    };
    let Some(sing) = sing_box() else {
        skip("sing-box is not installed; it is the hub's listener and the only meaningful peer");
        return;
    };

    let echo = UdpEcho::start();
    let h = start("udp", &engine, &sing);

    // SOCKS5 UDP ASSOCIATE through the mixed port. The rule pins UDP to
    // `Locus-UoT`, so this exercises the `udp-over-tcp` outbound rather than the
    // plain one — which is the whole point: the 3.3.x field defect was that
    // outbound never being constructed while both sides looked correct.
    let mut control = TcpStream::connect(("127.0.0.1", h.mixed_port)).expect("connect to the mixed port");
    control
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    control.write_all(&[0x05, 0x01, 0x00]).expect("greeting");
    let mut reply = [0u8; 2];
    control.read_exact(&mut reply).expect("greeting reply");
    assert_eq!(reply, [0x05, 0x00], "the mixed port must accept a SOCKS5 greeting");

    // UDP ASSOCIATE with address 0.0.0.0:0 — "tell me the relay address".
    let mut associate = vec![0x05, 0x03, 0x00, 0x01];
    associate.extend_from_slice(&[0, 0, 0, 0]);
    associate.extend_from_slice(&[0, 0]);
    control.write_all(&associate).expect("associate");
    let mut reply = [0u8; 10];
    control.read_exact(&mut reply).expect("associate reply");
    assert_eq!(reply[1], 0x00, "UDP ASSOCIATE must succeed, got reply {reply:?}");
    let relay_port = u16::from_be_bytes([reply[8], reply[9]]);
    let relay = format!("127.0.0.1:{relay_port}");

    // SOCKS5 UDP request header, then the payload.
    let mut datagram = vec![0x00, 0x00, 0x00, 0x01];
    datagram.extend_from_slice(&[127, 0, 0, 1]);
    datagram.extend_from_slice(&echo.port.to_be_bytes());
    datagram.extend_from_slice(b"hello-uot");

    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind udp probe");
    socket.set_read_timeout(Some(Duration::from_secs(10))).expect("timeout");
    socket.send_to(&datagram, &relay).expect("send the datagram");

    let mut buf = [0u8; 2048];
    let (n, _) = socket
        .recv_from(&mut buf)
        .expect("a datagram must come back through UDP-over-TCP — this is the path that was dead fleet-wide");
    // The reply carries the same 10-byte SOCKS5 header.
    let payload = &buf[10..n];
    assert_eq!(
        payload,
        b"echo:hello-uot",
        "the echo must survive the round trip intact; sing-box log:\n{}",
        std::fs::read_to_string(&h.server_log).unwrap_or_default()
    );

    // The negotiation itself, from the server's side. This is the assertion that
    // names the revision: `sp.v2.udp-over-tcp.arpa` is v2, and `tier.rs` pins v2.
    let log = std::fs::read_to_string(&h.server_log).unwrap_or_default();
    assert!(
        log.contains("sp.v2.udp-over-tcp.arpa"),
        "the server must have negotiated UDP-over-TCP **v2**; its log says:\n{log}"
    );
}

#[test]
fn the_probe_reads_a_measurement_through_a_real_tunnel() {
    let Some(engine) = mihomo() else {
        skip("no mihomo sidecar fetched; run `node scripts/prebuild.mjs` first");
        return;
    };
    let Some(sing) = sing_box() else {
        skip("sing-box is not installed; it is the hub's listener and the only meaningful peer");
        return;
    };

    let target = Http204::start();
    let h = start("probe", &engine, &sing);
    let url = percent_encode(&format!("http://127.0.0.1:{}/generate_204", target.port));

    // THE EXACT CALL THE READINESS PROBE MAKES: one outbound, by name.
    //
    // A local target, not `probe::EGRESS_TEST_URL`. The probe's public host is
    // unreachable from a sandboxed runner and the engine reports that as a
    // timeout — identical to a dead tunnel — so pointing this at the public host
    // would assert something about this machine's egress policy and call it a
    // statement about the probe. What is being checked here is that the probe's
    // call shape yields a *measurement* through a working tunnel, and loopback
    // serves that without asking anything of the network.
    //
    // This is only valid because a real proxy outbound tunnels to loopback
    // properly; a `direct` outbound reports 0 for loopback and is why the older
    // test needed a public host. Verified against the sidecar before writing it.
    let mut last = (0u16, String::new());
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        last = http_get(&h.controller_addr, &format!("/proxies/Locus/delay?url={url}&timeout=5"));
        if last.0 == 200 && usable(delay_of(&last.1)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    panic!(
        "the per-member delay test must yield a usable measurement through a real tunnel; \
         got status {} body {:?}. If this is a timeout, check sing-box's log first — a dead \
         upstream and a working tunnel look identical from here.",
        last.0, last.1
    );
}
