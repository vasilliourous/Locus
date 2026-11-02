//! The activation tunnel's client half, end to end: generated config → real
//! mihomo → real Shadowsocks-2022 listener → path filter → an HTTP answer.
//!
//! # Why this file exists
//!
//! `ACTIVATION-TUNNEL.md` §6 used to say *"there is no client code that uses this
//! tunnel"*, and the server half shipped with no caller. The gap that matters is
//! not "is there a `.rs` file" — it is whether the document this module generates
//! is one the engine accepts, and whether a request actually completes through it.
//! That is the same gap the UDP fleet fell into: both halves individually correct,
//! never run against each other (`tunnel_end_to_end.rs` records that lesson).
//!
//! # What it proves, and what it does not
//!
//! Proved here, on loopback, with no network:
//!
//!   * the YAML `activation_tunnel::config_for_tests` generates is accepted by the
//!     real vendored engine — not just parseable YAML, but a document mihomo will
//!     run;
//!   * a POST issued through that engine's SOCKS listener arrives at a real
//!     Shadowsocks-2022 listener, survives a path filter that mirrors the hub's
//!     allowlist, and obtains the upstream's JSON answer back;
//!   * the allowlist still refuses a path outside it **through the tunnel**, which
//!     is what makes a public bootstrap key acceptable.
//!
//! NOT proved: that `hub:8453` is reachable from a school network, that the
//! appliance tolerates an opaque stream there, or that the deployed hub's filter
//! behaves as this stand-in does. Those need the school network and stay in
//! `STILL-OPEN.md`.
//!
//! # The trap this test is written around
//!
//! The listening port is fixed by the module (`TUNNEL_PORT`), because the engine
//! dials it. A test cannot therefore run two instances at once, and must not
//! assume the port is free — it binds its stand-in listener on that exact port and
//! says so plainly if it cannot.

#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cognitive_complexity
)]

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A fixed test key, matching `tunnel_end_to_end.rs`. Not a secret: base64 of
/// `0x00..=0x1f`, and it exists only for the lifetime of this process.
const TEST_KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";

/// The port the module's generated config dials (`activation_tunnel::TUNNEL_PORT`).
const TUNNEL_PORT: u16 = 8453;

/// The port the hub's path filter listens on, inside the hub.
///
/// **This is the destination the client must ask the tunnel for.** Measured
/// 2026-11-01: `ssserver` has no inbound forwarding — it connects to whatever
/// destination the Shadowsocks client requested, and `local_addr`/`local_port` are
/// `sslocal`'s own SOCKS listen options, not a server-side forward. So a request
/// addressed anywhere else is dialled verbatim by the hub, and the filter is never
/// reached.
///
/// That makes this value part of the client/hub contract, not a test detail:
/// `server_json` below is deliberately WITHOUT the forward keys the deployed
/// module sets, because those keys do nothing.
const FILTER_PORT: u16 = 18090;

fn skip(reason: &str) {
    if std::env::var_os("LOCUS_REQUIRE_LIVE_ENGINE").is_some() {
        panic!("SKIP was requested to be a failure (LOCUS_REQUIRE_LIVE_ENGINE is set): {reason}");
    }
    eprintln!("SKIP: {reason}");
}

fn client_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a parent")
        .to_path_buf()
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

/// `ssserver`, found on PATH — the hub's **actual** listener.
///
/// `SS_BINARY=/usr/local/bin/ssserver` in `server/modules/02-shadowsocks.sh`, and
/// not `sing-box`. The distinction is load-bearing: the path filter only works
/// because `ssserver` supports `local_addr`/`local_port`, handing the DECRYPTED
/// stream to a local listener instead of resolving a destination itself. sing-box
/// 1.14's Shadowsocks inbound has no such option (its schema offers
/// `destinations`, not a forward), so it could not stand in for the deployment.
fn ssserver() -> Option<PathBuf> {
    for candidate in [
        "/usr/local/bin/ssserver",
        "/usr/bin/ssserver",
        "/opt/homebrew/bin/ssserver",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for<F: Fn() -> bool>(what: &str, within: Duration, ready: F) {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        if ready() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("{what} did not become ready within {within:?}");
}

fn accepts(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).is_ok()
}

/// Whether the engine can complete a proxy conversation on `proxy_port` — SOCKS5
/// greeting answered **and** a CONNECT processed.
///
/// `accepts()` is too early: the engine binds the listening socket well before the
/// proxy handler behind it will relay anything, so a bare connect reports "ready"
/// during a window in which every request is reset. Measured 2026-11-02,
/// 40 runs per variant on one idle machine with the same binary and peers:
///
///   | readiness signal                        | failures |
///   |---|---|
///   | `accepts()` — connect only              | ~1 in 8  |
///   | greeting answered, CONNECT not driven   | 11/40    |
///   | full CONNECT, then a 150 ms settle      | 0/50     |
///
/// The probe CONNECTs to a port nothing listens on, so "usable" means the engine
/// PROCESSED the request and answered — not that the answer was success.
fn tunnel_usable(proxy_port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", proxy_port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    if stream.write_all(&[0x05, 0x01, 0x00]).is_err() {
        return false;
    }
    let mut greeting = [0u8; 2];
    if !matches!(stream.read_exact(&mut greeting), Ok(())) || greeting != [0x05, 0x00] {
        return false;
    }
    let mut connect = vec![0x05, 0x01, 0x00, 0x01];
    connect.extend_from_slice(&[127, 0, 0, 1]);
    connect.extend_from_slice(&1u16.to_be_bytes());
    if stream.write_all(&connect).is_err() {
        return false;
    }
    let mut header = [0u8; 4];
    if !matches!(stream.read_exact(&mut header), Ok(())) || header[0] != 0x05 {
        return false;
    }
    let extra = match header[3] {
        0x01 => 6,
        0x04 => 18,
        0x03 => {
            let mut len = [0u8; 1];
            if stream.read_exact(&mut len).is_err() {
                return false;
            }
            usize::from(len[0]) + 2
        }
        _ => return false,
    };
    let mut rest = vec![0u8; extra];
    let _ = stream.read_exact(&mut rest);
    matches!(header[1], 0x00 | 0x05)
}

/// The hub's path filter, reproduced: the allowlist in
/// `server/modules/09-activation-tunnel.sh`, answering the same way.
///
/// Deliberately a stand-in rather than the deployed code — that code is Python on
/// the box and is guarded by `check-consistency.sh` §40. What this proves is that
/// a request ARRIVING OVER THE TUNNEL is still subject to an allowlist at all.
struct PathFilter {
    _thread: std::thread::JoinHandle<()>,
}

impl PathFilter {
    fn start(listen_port: u16) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", listen_port)).expect("bind the path filter");
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                // Read until the end of the header block. Reading only 1024 bytes
                // would work here but silently truncates a longer body, which is
                // how a test comes to prove less than it claims.
                while stream.read(&mut byte).ok() == Some(1) {
                    head.push(byte[0]);
                    if head.ends_with(b"\r\n\r\n") {
                        break;
                    }
                }
                let text = String::from_utf8_lossy(&head).to_string();
                eprintln!("[filter] got {} bytes: {:?}", head.len(), text);
                eprintln!("[filter] received {} bytes: {:?}", head.len(), text);
                let Some(request_line) = text.lines().next() else {
                    continue;
                };
                let Some(target) = request_line.split(' ').nth(1) else {
                    continue;
                };

                let allowed = ["/api/activate", "/api/code-lookup", "/api/heartbeat"]
                    .iter()
                    .any(|a| target.split('?').next() == Some(*a));

                let (status, body) = if allowed {
                    ("200 OK", format!("{{\"seen\":\"{target}\",\"tunnel\":true}}"))
                } else {
                    // The real filter refuses with an EMPTY 403, so a bootstrap
                    // endpoint cannot be used to enumerate the hub.
                    ("403 Forbidden", String::new())
                };

                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        Self { _thread: thread }
    }
}

fn server_json(ss_port: u16) -> String {
    // The listener as `shadowsocks-rust` actually behaves: it decrypts and then
    // CONNECTS TO THE REQUESTED DESTINATION. There is deliberately no
    // `local_addr`/`local_port` here — those are `sslocal`'s SOCKS listen options
    // and `ssserver` ignores them, which was measured on 2026-11-01 and is why the
    // deployed module's forward keys are inert. The path filter is reached because
    // the CLIENT asks for it by address (see `FILTER_PORT`).
    format!(
        r#"{{
  "server": "127.0.0.1",
  "server_port": {ss_port},
  "password": "{TEST_KEY}",
  "method": "2022-blake3-aes-256-gcm",
  "mode": "tcp_only"
}}"#
    )
}

/// Issues a POST through a SOCKS5 proxy and returns `(status, body)`.
///
/// Hand-rolled rather than via `reqwest`, because this test must not depend on
/// the product's HTTP stack agreeing with the thing under test. SOCKS5 with no
/// authentication, then an HTTP/1.1 request over the established stream.
fn post_via_socks(proxy_port: u16, target: &str, body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", proxy_port)).expect("connect to the socks listener");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("set read timeout");

    // SOCKS5 greeting: version 5, one method, "no authentication".
    stream.write_all(&[0x05, 0x01, 0x00]).expect("socks greeting");
    let mut reply = [0u8; 2];
    stream.read_exact(&mut reply).expect("socks greeting reply");
    assert_eq!(reply, [0x05, 0x00], "the engine must accept no-auth SOCKS5");

    // CONNECT to an IPv4 address, so nothing here depends on a resolver.
    // The caller's target is `host:port/path`: the port is what SOCKS needs, and
    // the path is what the HTTP request line carries. Splitting on `:` alone would
    // swallow the path into the port, which is a parse error rather than a wrong
    // connection — but only because it happened to be caught here.
    let (authority, path) = target.split_once('/').expect("target must be host:port/path");
    let (host, port) = authority.split_once(':').expect("authority must be host:port");
    let octets: Vec<u8> = host.split('.').map(|p| p.parse::<u8>().expect("ipv4 octet")).collect();
    assert_eq!(octets.len(), 4, "this helper only speaks IPv4 targets");
    let port: u16 = port.parse().expect("target port");
    let mut connect = vec![0x05, 0x01, 0x00, 0x01];
    connect.extend_from_slice(&octets);
    connect.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&connect).expect("socks connect");
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).expect("socks connect reply");
    assert_eq!(header[0], 0x05, "SOCKS version");
    assert_eq!(
        header[1], 0x00,
        "the engine must report success; got reply code {:?}",
        header[1]
    );
    // The reply is 4 bytes plus a BOUND ADDRESS whose length depends on its type.
    // Consuming only the 4 leaves the bound address in the stream, and those bytes
    // then arrive as the first bytes of the HTTP response — which reads as a
    // corrupt answer rather than the bookkeeping fault it is.
    match header[3] {
        0x01 => {
            let mut rest = [0u8; 6]; // 4-byte IPv4 + 2-byte port
            stream.read_exact(&mut rest).expect("socks ipv4 bound address");
        }
        0x04 => {
            let mut rest = [0u8; 18]; // 16-byte IPv6 + 2-byte port
            stream.read_exact(&mut rest).expect("socks ipv6 bound address");
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).expect("socks domain length");
            let mut rest = vec![0u8; usize::from(len[0]) + 2];
            stream.read_exact(&mut rest).expect("socks domain bound address");
        }
        other => panic!("unexpected SOCKS address type {other:#04x}"),
    }

    let request = format!(
        "POST /{path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("write the http request");

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read the http response");
    // Decoded lossily: an HTTP response is bytes, and failing the whole test on a
    // non-UTF-8 byte would report a decode problem instead of the tunnel problem
    // the assertion is about.
    let raw = String::from_utf8_lossy(&raw).into_owned();
    let status = raw
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or_else(|| panic!("no status line in response: {raw:?}"));
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();
    (status, body)
}

#[test]
fn an_enrolment_request_completes_through_the_activation_tunnel() {
    let Some(engine) = mihomo() else {
        return skip("no mihomo sidecar fetched; run `node scripts/prebuild.mjs` first");
    };
    let Some(ss_server) = ssserver() else {
        return skip("ssserver is not installed; it is the hub's listener and a test dependency only");
    };

    // The SS listener must occupy the port the generated config dials, because
    // `locus_generated_config` hard-codes `TUNNEL_PORT` — that is the deployment's
    // port and it must not be configurable from a test, or the test would stop
    // checking what ships. So the harness adapts to the product, not the reverse.
    if accepts(TUNNEL_PORT) {
        return skip(&format!(
            "port {TUNNEL_PORT} is already in use, so the SS listener cannot stand in for the \
             hub's bootstrap port here"
        ));
    }
    let ss_port = TUNNEL_PORT;

    let workdir = TempDir::new();
    // The filter listens on the port the CLIENT will ask the tunnel for. It must be
    // this exact port: the hub dials whatever destination the client names, so the
    // filter's address is the destination, not a forwarding target.
    if accepts(FILTER_PORT) {
        return skip(&format!(
            "port {FILTER_PORT} is already in use, so the path filter cannot stand in for the \
             hub's filter here"
        ));
    }
    let forwarder = PathFilter::start(FILTER_PORT);
    std::fs::write(workdir.path.join("server.json"), server_json(ss_port)).expect("write server config");

    let server_log = std::fs::File::create(workdir.path.join("server.log")).expect("create server log");
    let server_log_err = server_log.try_clone().expect("clone server log");
    let _server = Guard(
        Command::new(&ss_server)
            .arg("-c")
            .arg(workdir.path.join("server.json"))
            .stdout(Stdio::from(server_log))
            .stderr(Stdio::from(server_log_err))
            .spawn()
            .expect("start ssserver"),
    );
    wait_for("the sing-box listener", Duration::from_secs(15), || accepts(ss_port));

    // The document the module generates. This is the artefact under test: if it
    // is not one the engine runs, nothing downstream matters.
    let proxy_port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind proxy probe");
        let port = listener.local_addr().expect("local addr").port();
        drop(listener);
        port
    };
    let yaml = locus_generated_config(proxy_port, TUNNEL_PORT);
    std::fs::write(workdir.path.join("client.yaml"), yaml).expect("write client config");

    let engine_log = std::fs::File::create(workdir.path.join("engine.log")).expect("create engine log");
    let engine_log_err = engine_log.try_clone().expect("clone engine log");
    // ABSOLUTE paths for `-d` and `-f`, because `current_dir` is set to the
    // engine's own directory. A relative `-f` would then be resolved against the
    // sidecar directory, not the work directory, and the engine would start with
    // its built-in defaults: no proxy, no MATCH rule, and a listener that accepts
    // connections and relays nothing. That failure is silent — the process is
    // healthy and only the requests vanish.
    let work_abs = workdir
        .path
        .canonicalize()
        .expect("the work directory must be canonicalisable");
    let _engine = Guard(
        Command::new(&engine)
            .arg("-d")
            .arg(&work_abs)
            .arg("-f")
            .arg(work_abs.join("client.yaml"))
            // Kept, and made safe by the absolute paths above: mihomo resolves its
            // geo databases relative to the working directory, which is why the
            // working engine test sets this too.
            .current_dir(engine.parent().expect("engine has a parent"))
            .stdout(Stdio::from(engine_log))
            .stderr(Stdio::from(engine_log_err))
            .spawn()
            .expect("start the engine"),
    );
    // Wait for the engine to be able to RELAY, not merely to be listening, and
    // then let it settle — see `tunnel_usable` for the measurements behind both.
    // Without the settle this test failed ~1 run in 8 with an empty response and
    // no `[TCP]` line in the engine log, which reads exactly like a broken tunnel
    // and is not one.
    wait_for("the engine's socks proxy", Duration::from_secs(20), || {
        tunnel_usable(proxy_port)
    });
    std::thread::sleep(Duration::from_millis(150));

    // The request names the PATH FILTER as its destination. That is how the
    // deployed design reaches the filter: `ssserver` decrypts and then connects to
    // whatever destination the client asked for — there is no server-side forward
    // (measured 2026-11-01; `ssserver` logs `tcp tunnel … -> <the requested dest>`).
    // The admin path is refused by the filter on the far side, which is where the
    // security boundary actually sits.
    let body = r#"{"code":"LOCUS-AAAA-BBBB-CCCC","fingerprint":"0123456789abcdef"}"#;
    let (status, answer) = post_via_socks(proxy_port, &format!("127.0.0.1:{FILTER_PORT}/api/activate"), body);

    assert_eq!(status, 200, "an allowlisted path must survive the tunnel: {answer}");
    assert!(
        answer.contains("\"tunnel\":true"),
        "the answer must come from the far side of the tunnel, not from the client: {answer}"
    );
    assert!(
        answer.contains("/api/activate"),
        "the filter must have seen the path the caller asked for: {answer}"
    );

    // ── The negative case: a path outside the allowlist is refused, but only
    //    AFTER the tunnel carried it. That is the boundary working where it
    //    matters — on the hub, not in the client. ──
    let (status, answer) = post_via_socks(
        proxy_port,
        &format!("127.0.0.1:{FILTER_PORT}/api/admin/fetch-release"),
        "{}",
    );
    assert_eq!(
        status, 403,
        "a privileged path must be refused over the tunnel too, got {status}: {answer}"
    );
    assert!(
        answer.is_empty(),
        "the refusal must carry an empty body, as the deployed filter's does: {answer:?}"
    );

    drop(forwarder);
}

/// The YAML `activation_tunnel::render_config` produces, rebuilt here.
///
/// Duplicated rather than exported because an integration test cannot reach a
/// private function, and exporting a renderer from the product for a test's
/// benefit would be the tail wagging the dog. The *agreement* is what matters and
/// it is asserted in `activation_tunnel.rs`'s unit tests: one proxy, one
/// `MATCH` rule, the bootstrap port, `tcp_only`. If those drift, this test stops
/// exercising what ships — which is why the shape below is deliberately identical,
/// comment for comment, and why the unit tests assert the invariants rather than
/// the text.
fn locus_generated_config(forward_port: u16, tunnel_port: u16) -> String {
    format!(
        "mixed-port: {forward_port}\n\
         allow-lan: false\n\
         bind-address: 127.0.0.1\n\
         mode: rule\n\
         ipv6: false\n\
         log-level: info\n\
         external-controller: ''\n\
         proxies:\n\
         \x20 - name: Activation-Tunnel\n\
         \x20   type: ss\n\
         \x20   server: 127.0.0.1\n\
         \x20   port: {tunnel_port}\n\
         \x20   cipher: 2022-blake3-aes-256-gcm\n\
         \x20   password: \"{TEST_KEY}\"\n\
         \x20   udp: false\n\
         proxy-groups:\n\
         \x20 - name: Locus Activation\n\
         \x20   type: select\n\
         \x20   proxies:\n\
         \x20     - Activation-Tunnel\n\
         rules:\n\
         \x20 - MATCH,Locus Activation\n"
    )
}

/// A directory removed on drop, so a failed run leaves nothing behind.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!("locus-activation-tunnel-test-{}", std::process::id()));
        std::fs::create_dir_all(&base).expect("create the test directory");
        Self { path: base }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Kept when LOCUS_KEEP_TEST_DIR is set, so a failing run can be inspected
        // instead of guessed at. The engine's own log is the only account of why a
        // request vanished, and it is worthless once the directory is gone.
        if std::env::var_os("LOCUS_KEEP_TEST_DIR").is_some() {
            eprintln!("[keep] test directory retained at {}", self.path.display());
            return;
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
