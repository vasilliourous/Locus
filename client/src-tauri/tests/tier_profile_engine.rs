//! The generated tier profile must be accepted by a real engine, not just by
//! unit tests.
//!
//! This is the check that was missing when the proxy-group loop shipped: a
//! document can be perfectly valid YAML, pass every assertion about its own
//! shape, and still be refused outright by mihomo. The failure surfaces on a
//! device as "the tunnel never starts", with an engine error the student cannot
//! act on.
//!
//! The engine is treated as the authority. Where this test and the engine
//! disagree, the engine is right — that is the whole point of running it.

// Integration tests are their own crates, so the `cfg_attr(test, ...)` lint
// scope in `lib.rs` does not reach here. `panic!`/`expect()` are the correct
// idioms in a test (see that comment for the reasoning).
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cognitive_complexity
)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository's `client/` directory, found from this test's manifest dir.
fn client_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().expect("src-tauri must have a parent").to_path_buf()
}

/// The mihomo sidecar for this host, if it has been fetched.
///
/// `scripts/prebuild.mjs` downloads it into the gitignored `src-tauri/sidecar/`,
/// so a clean checkout has none. A missing engine is reported as a SKIP rather
/// than a failure: the test must not fail CI on a platform whose sidecar has not
/// been fetched, but it must be loud about not having run.
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

/// Runs `verge-mihomo -t -f <config>` and returns its combined output.
///
/// `-t` is the config-test mode: the engine parses, validates and resolves the
/// document, then exits without touching the network or the TUN interface.
fn validate_with_engine(engine: &Path, config: &Path) -> (bool, String) {
    let output = Command::new(engine)
        .arg("-t")
        .arg("-f")
        .arg(config)
        .current_dir(engine.parent().expect("engine has a parent"))
        .output()
        .expect("the mihomo sidecar must be executable");

    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), combined)
}

/// A Strike tier: TCP on 8445, UDP-over-TCP on 8446.
fn strike_profile_yaml() -> String {
    // Built through the crate's own API rather than hand-written, so this test
    // exercises the code that actually runs. The values are arbitrary but
    // structurally identical to what the hub sends.
    //
    // The server host is deliberately a placeholder, NOT the live hub: this
    // fixture used to carry the real address, which then went stale three times
    // and finished by naming one that no longer existed. A test must not encode a
    // world claim — see docs/operate/CLAIMS.md §6. The domain is the only stable
    // identifier and even that does not belong in a unit test.
    app_lib::locus::tier::build_profile(
        &app_lib::locus::contract::TierConfig {
            server: "hub.example.invalid".into(),
            server_port: 8445,
            password: "MXBLrr0jTazDXjMo+33Ic8+IQKCWr6ueOogcykZGVYA=".into(),
            method: "2022-blake3-aes-256-gcm".into(),
            uot_port: 8446,
        },
        true,
    )
    .yaml
}

/// An Eco tier: plain TCP, no UoT endpoint.
fn eco_profile_yaml() -> String {
    app_lib::locus::tier::build_profile(
        &app_lib::locus::contract::TierConfig {
            server: "hub.example.invalid".into(),
            server_port: 8443,
            password: "MXBLrr0jTazDXjMo+33Ic8+IQKCWr6ueOogcykZGVYA=".into(),
            method: "2022-blake3-aes-256-gcm".into(),
            uot_port: 0,
        },
        false,
    )
    .yaml
}

#[test]
fn the_engine_accepts_a_strike_profile() {
    let Some(engine) = sidecar() else {
        eprintln!("SKIP: no mihomo sidecar fetched; run `node scripts/prebuild.mjs` first");
        return;
    };

    let dir = std::env::temp_dir().join("locus-tier-engine-test-strike");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let config = dir.join("strike.yaml");
    std::fs::write(&config, strike_profile_yaml()).expect("write profile");

    let (ok, output) = validate_with_engine(&engine, &config);
    assert!(
        ok,
        "the real engine refused the generated Strike profile. \
         A config can pass every unit test and still be rejected here — \
         this is how the proxy-group loop was found.\n\n{output}"
    );
}

#[test]
fn the_engine_accepts_an_eco_profile() {
    let Some(engine) = sidecar() else {
        eprintln!("SKIP: no mihomo sidecar fetched; run `node scripts/prebuild.mjs` first");
        return;
    };

    let dir = std::env::temp_dir().join("locus-tier-engine-test-eco");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let config = dir.join("eco.yaml");
    std::fs::write(&config, eco_profile_yaml()).expect("write profile");

    let (ok, output) = validate_with_engine(&engine, &config);
    assert!(ok, "the real engine refused the generated Eco profile.\n\n{output}");
}

/// The engine must be able to resolve `udp-over-tcp-version: 2` on the UoT
/// outbound.
///
/// This is the assertion that would catch a field name the engine does not
/// know: mihomo ignores unrecognised keys silently in some paths, so a typo
/// here would produce a config that loads and then never negotiates v2.
#[test]
fn the_engine_accepts_the_pinned_uot_protocol_version() {
    let Some(engine) = sidecar() else {
        eprintln!("SKIP: no mihomo sidecar fetched; run `node scripts/prebuild.mjs` first");
        return;
    };

    let yaml = strike_profile_yaml();
    assert!(
        yaml.contains("udp-over-tcp-version"),
        "the profile must pin the UoT protocol revision"
    );

    let dir = std::env::temp_dir().join("locus-tier-engine-test-uot");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let config = dir.join("uot.yaml");
    std::fs::write(&config, yaml).expect("write profile");

    let (ok, output) = validate_with_engine(&engine, &config);
    assert!(
        ok,
        "the engine refused a profile with an explicit udp-over-tcp-version.\n\n{output}"
    );
}
