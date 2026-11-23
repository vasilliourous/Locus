//! The hub's `signature_<platform>` field and the app's compiled-in `pubkey`
//! must match what `tauri-plugin-updater` actually decodes, or the install fails
//! on the student's machine with an error nobody can act on.
//!
//! # Why this test exists
//!
//! On 2026-09-29 the first real update attempt after the updater was wired
//! failed at verification:
//!
//! ```text
//! the update could not be installed: The signature
//! RWT9eeTiJVDWZDZ8hFQ7TOE6BW7OEwARW+kVEU7m2jIlxcjfiGR9InKW could not be
//! decoded, please check if it is a valid base64 string.
//! ```
//!
//! (That key is the one retired on 2026-10-16 — the literal is kept here because
//! it is a transcript of what was actually printed, not a live value. The key in
//! `tauri.conf.json` today is the rotated one; see `client/docs/SIGNING.md` §3.)
//!
//! Two things were wrong, and neither was visible from inside either language:
//!
//! 1. `tauri.conf.json`'s `pubkey` held the **bare** minisign key (the second
//!    line of the `.pub` file). The plugin calls `base64_to_string()` on it
//!    BEFORE `PublicKey::decode()`, and requires the decoded bytes to be UTF-8
//!    *text* — so a bare key fails at the first step, with the message above.
//!    The documented format is the base64 of the whole `.pub` file.
//! 2. The hub only ever checked that `signature_<platform>` was **present**, not
//!    that it had the right **shape** — so a malformed value was advertised,
//!    downloaded, and rejected only at the very end.
//!
//! # Why it cannot be caught by a unit test on either side
//!
//! The contract spans languages and layers: a Rust config field, a Python
//! publish script, a JS hub field, and a third-party plugin's decode chain. Each
//! half looked correct alone. The only place the mismatch is visible is where
//! they meet — which is what this test does, by reproducing the plugin's decode
//! path exactly and running real key material through it.
//!
//! Same discipline as `tier_profile_engine.rs`, which treats mihomo as the
//! authority rather than trusting that generated YAML "looks right".

// Integration tests are their own crate, so the workspace lint scope in `lib.rs`
// does not reach here. `panic!`/`expect()` are the correct idioms in a test.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cognitive_complexity
)]

use std::path::PathBuf;

/// The repository's `client/` directory, found from this test's manifest dir.
fn client_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a parent")
        .to_path_buf()
}

/// The pubkey compiled into the app, read from `tauri.conf.json`.
///
/// Read from the real config rather than duplicated as a literal: a literal
/// would keep passing after a key rotation, which is the one change where this
/// test must fail.
fn configured_pubkey() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    let json: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()));
    json["plugins"]["updater"]["pubkey"]
        .as_str()
        .expect("plugins.updater.pubkey must be a string")
        .to_owned()
}

/// The decode the plugin applies to BOTH the pubkey and the signature.
///
/// Deliberately the **product's** function (`app_lib::chrysalis::update::verify`)
/// rather than a copy in this file. It used to be a copy here, and a copy is
/// exactly the wrong shape for this test: the defect it exists to catch is the
/// decode chain disagreeing with the plugin, and a reimplementation can agree
/// with itself while disagreeing with both. Now the only way for this test to
/// pass is for the code that runs on a student's machine to decode the way the
/// plugin does.
///
/// The property being pinned: the plugin requires the decoded bytes to be
/// valid UTF-8 *text*, not key or signature bytes. That requirement is what the
/// bare-key pubkey violated.
fn base64_to_string(s: &str) -> Result<String, String> {
    use base64::Engine as _;
    app_lib::chrysalis::update::verify::base64_to_string(s).map_err(|e| {
        // Keep the two failure modes distinguishable in a failure message.
        if base64::engine::general_purpose::STANDARD.decode(s).is_err() {
            format!("base64 decode failed: {e}")
        } else {
            format!("decoded bytes are not UTF-8 text: {e}")
        }
    })
}

/// The `.pub` file in `.chrysalis-keys/`, when this checkout has one.
fn local_pub_file() -> Option<String> {
    std::fs::read_to_string(
        client_dir()
            .join("..")
            .join(".chrysalis-keys")
            .join("chrysalis_update.pub"),
    )
    .ok()
}

/// The configured pubkey must be base64 of the whole `.pub` file's text.
///
/// This is the assertion that fails on the value that shipped. The negative case
/// (the bare key) is asserted explicitly below, so the test documents the trap
/// rather than only the fix.
#[test]
fn the_configured_pubkey_is_base64_of_the_public_key_file() {
    let pubkey = configured_pubkey();

    let decoded = base64_to_string(&pubkey).expect(
        "the pubkey must base64-decode to UTF-8 TEXT — the plugin's \
         base64_to_string() runs before PublicKey::decode(), so a bare key \
         (binary) fails here and every update is refused with \
         'could not be decoded, please check if it is a valid base64 string'",
    );

    let mut lines = decoded.lines().filter(|l| !l.trim().is_empty());
    let comment = lines.next().expect("pubkey text must have a comment line");
    let key_line = lines.next().expect("pubkey text must have a key line");
    assert!(
        comment.starts_with("untrusted comment: "),
        "minisign pubkey text starts with an untrusted comment, got {comment:?}"
    );

    // And it must parse as a real minisign public key, with the key material
    // intact — not merely be the right shape.
    let parsed = minisign_verify::PublicKey::decode(&decoded)
        .unwrap_or_else(|e| panic!("PublicKey::decode rejected the configured pubkey: {e:?}"));

    // When the local `.pub` file exists, it is the source of truth for this key.
    if let Some(file_text) = local_pub_file() {
        let expected_line = file_text
            .lines()
            .find(|l| !l.trim().is_empty() && !l.starts_with("untrusted comment:"))
            .expect("the .pub file must contain a key line");
        assert_eq!(
            key_line.trim(),
            expected_line.trim(),
            "the configured pubkey does not match .chrysalis-keys/chrysalis_update.pub — \
             the running app would reject every update signed by the CI key \
             ('created with a different key')"
        );
        let _ = parsed;
    }
}

/// The exact value that caused the failure must be rejected.
///
/// A regression test that cannot fail on the bug that motivated it is not a
/// regression test. The bare key is a *plausible* value — it is what
/// `minisign -G` prints, and it is what an operator would copy out of the `.pub`
/// file by hand — so the trap is worth pinning.
///
/// The literal is the bare key line of the CURRENT keypair (rotated
/// 2026-10-16, key ID `4DA9B48F39251CEF`). It is retargeted rather than derived
/// from `.chrysalis-keys/chrysalis_update.pub` on purpose: the point of this test is to
/// pin the *shape* that fails, and a test that reads the value it is meant to
/// reject would pass on any key, including a malformed one. When the key is
/// rotated again, update this literal and the one in `SIGNING.md` §3 together.
#[test]
fn the_bare_public_key_is_not_a_valid_pubkey() {
    let bare = "RWTvHCU5j7SpTcZwVzJn852JUdL4vuLPBVD/uEpcMMbnlzulYxCJf6hF";
    let result = base64_to_string(bare);
    assert!(
        result.is_err(),
        "the bare (unwrapped) public key must FAIL the plugin's decode — if this \
         ever passes, the format assumption behind this test is wrong"
    );
    let message = result.unwrap_err();
    assert!(
        message.contains("UTF-8"),
        "the bare key fails because its 42 bytes are not text; got {message:?}"
    );
}

/// The `signature_<platform>` field must be base64 of the ENTIRE `.sig` file,
/// exactly once.
///
/// The neighbouring encodings are all wrong, and all were tested against a real
/// signature from the real key on 2026-09-29:
///
/// | value stored                        | result                       |
/// |-------------------------------------|------------------------------|
/// | base64(whole `.sig` text)           | **verifies**                 |
/// | the `.sig` text verbatim            | base64 decode fails (spaces) |
/// | the signature line only             | not UTF-8 (binary bytes)     |
/// | base64(signature line only)         | `Signature::decode` fails    |
#[test]
fn a_signature_field_is_base64_of_the_whole_sig_file() {
    use base64::Engine as _;

    let sig_text = "untrusted comment: signature from minisign secret key\n\
                    RUT9eeTiJVDWZMBlzoYnS0T3fWk2qM1kAAAA=\n\
                    trusted comment: Chrysalis linux\n\
                    AHhE2ZGRWCrFe02+ZLxfaDZmV6/ABBBBBBBBBBBBBBBBBBBBBBB=\n";

    // What publish-release.sh stores: the whole file, base64'd once.
    let wire = base64::engine::general_purpose::STANDARD.encode(sig_text);
    let decoded = base64_to_string(&wire).expect("the stored value must decode");
    assert_eq!(
        decoded, sig_text,
        "one base64 layer must yield the .sig file's text exactly"
    );

    // Verbatim text is NOT valid base64 input — a plausible wrong fix.
    assert!(
        base64_to_string(sig_text).is_err(),
        "the raw .sig text must not be accepted as a base64 field"
    );

    // The signature line alone is binary once decoded, so it cannot be the field.
    let line2 = sig_text.lines().nth(1).unwrap();
    assert!(
        base64_to_string(line2).is_err(),
        "the signature line alone decodes to binary signature bytes, not text"
    );

    // And base64(that line) loses the trusted comment, so a 4-line parse fails.
    let line2_wrapped = base64::engine::general_purpose::STANDARD.encode(line2);
    let as_text = base64_to_string(&line2_wrapped).expect("one layer decodes");
    let line_count = as_text.lines().filter(|l| !l.trim().is_empty()).count();
    assert!(
        line_count < 4,
        "base64(signature line only) cannot satisfy Signature::decode's four-line \
         requirement; the guard in publish-release.sh catches this"
    );
}

/// The publish script must refuse a `.sig` that is not minisign's four-line
/// shape, because the hub only checks that a signature is non-empty.
///
/// This asserts the guard exists in the script text. It is a blunt check, and
/// deliberately so: the script is Python embedded in a bash heredoc, so it
/// cannot be imported and called. The value is in failing when someone removes
/// the guard — the absence of exactly this check is what let the bad value ship.
#[test]
fn the_publish_script_guards_the_signature_shape() {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("server")
        .join("scripts")
        .join("publish-release.sh");
    let text = std::fs::read_to_string(&script).unwrap_or_else(|e| panic!("could not read {}: {e}", script.display()));

    assert!(
        text.contains(r"base64.b64encode(sig_bytes)"),
        "publish-release.sh must store base64 of the whole .sig file — \
         see a_signature_field_is_base64_of_the_whole_sig_file for the contract"
    );
    assert!(
        text.contains("is not a minisign signature"),
        "publish-release.sh must refuse a .sig that is not minisign's shape; \
         the hub only verifies presence, never form"
    );
}

/// A real signature from the real key must verify through the SAME function the
/// product calls before it hands bytes to the installer.
///
/// # Why this test is the one that closes a reported gap
///
/// `ReadyInstall::install` verifies the artifact *before* `install()`, because
/// inside `install()` a signature refusal and a successful launch are both
/// "the process exited", and on Windows that exit is `process::exit(0)` with no
/// log line. If this function is wrong, that check either refuses every update
/// (fleet stranded) or accepts everything (worse). So it is exercised against
/// real key material from `.chrysalis-keys/`, not a fixture.
///
/// Signing a 60 MB installer in a unit test is not reasonable, so the "artifact"
/// is a few bytes; what is being tested is the decode and verification chain,
/// which does not care about the payload's size or format.
///
/// # Skips when the key is absent
///
/// `.chrysalis-keys/` is a cache, empty after a clone (its README is explicit).
/// A test that failed on a clean clone would be a test everybody deletes. When
/// the secret key is present, this runs for real.
#[test]
fn a_real_signature_verifies_through_the_products_check() {
    use app_lib::chrysalis::update::verify::{SignatureVerdict, verify_file};
    use base64::Engine as _;
    use std::io::Write as _;

    let keys = client_dir().join("..").join(".chrysalis-keys");
    let Some(pub_text) = local_pub_file() else {
        eprintln!("skipping: .chrysalis-keys/chrysalis_update.pub is absent (clean clone)");
        return;
    };
    let secret_path = keys.join("chrysalis_update.key");
    if !secret_path.exists() {
        eprintln!("skipping: the secret key is not in the cache; rehydrate it to run this");
        return;
    }

    // Build a stand-in artifact, sign it with the REAL key, and verify it with
    // the product's function.
    let dir = std::env::temp_dir().join(format!("chrysalis-verify-contract-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let artifact = dir.join("artifact.exe");
    let mut file = std::fs::File::create(&artifact).expect("create artifact");
    file.write_all(b"MZ this stands in for an installer")
        .expect("write artifact");
    drop(file);

    let sig_text = std::process::Command::new("minisign")
        .args(["-S", "-s"])
        .arg(&secret_path)
        .args(["-m"])
        .arg(&artifact)
        .args(["-x"])
        .arg(dir.join("artifact.sig"))
        .output();
    let Ok(output) = sig_text else {
        eprintln!("skipping: `minisign` is not installed on this machine");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    assert!(
        output.status.success(),
        "minisign failed to sign the stand-in artifact: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let sig_file = std::fs::read_to_string(dir.join("artifact.sig")).expect("read .sig");
    // The wire form the hub stores: base64 of the whole `.sig` file.
    let wire = base64::engine::general_purpose::STANDARD.encode(sig_file.as_bytes());
    // The compiled-in pubkey's format: base64 of the whole `.pub` file.
    let pubkey_wire = base64::engine::general_purpose::STANDARD.encode(pub_text.as_bytes());

    let verdict = verify_file(&artifact, &wire, &pubkey_wire).expect("the file is readable");
    assert!(
        matches!(verdict, SignatureVerdict::Valid),
        "a real signature from the compiled-in key must verify, got {verdict:?}"
    );

    // Tampering must be caught. This is the half that proves the test above is
    // not simply accepting everything.
    std::fs::write(&artifact, b"MZ tampered with after signing").expect("tamper");
    let verdict = verify_file(&artifact, &wire, &pubkey_wire).expect("the file is readable");
    assert!(
        matches!(verdict, SignatureVerdict::Rejected(_)),
        "a tampered artifact must be REJECTED, got {verdict:?} — if this passes as \
         Valid the check is not checking"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The verification must be driven by the SAME pubkey the plugin uses.
///
/// A mismatch here is the failure mode that shipped once already: the app
/// carries key A, the hub signs with key B, and every update is refused at the
/// very end with an error nobody can act on. The check in `install.rs` reads
/// `plugins.updater.pubkey` from the resolved config; this asserts that config
/// value is the key `.chrysalis-keys/` holds, so a rotation cannot update one
/// without the other.
#[test]
fn the_pubkey_used_for_verification_is_the_one_the_client_trusts() {
    let pubkey = configured_pubkey();
    let decoded = base64_to_string(&pubkey).expect("the configured pubkey must decode to text");
    assert!(
        decoded.contains("untrusted comment: "),
        "the pubkey the product verifies against must be base64 of the whole `.pub` file"
    );

    if let Some(file_text) = local_pub_file() {
        let file_key = file_text
            .lines()
            .find(|line| !line.trim().is_empty() && !line.starts_with("untrusted comment:"))
            .expect("the .pub file must contain a key line")
            .trim();
        let configured_key = decoded
            .lines()
            .find(|line| !line.trim().is_empty() && !line.starts_with("untrusted comment:"))
            .expect("the configured pubkey must contain a key line")
            .trim();
        assert_eq!(
            configured_key, file_key,
            "the key `update::verify` will check against is not the key in \
             .chrysalis-keys/chrysalis_update.pub"
        );
    }
}
