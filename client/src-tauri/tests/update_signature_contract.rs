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

/// Mirrors `tauri-plugin-updater`'s `base64_to_string()`, verbatim.
///
/// This is the actual decode the plugin applies to BOTH the pubkey and the
/// signature. Reproducing it here — rather than decoding to bytes — is the
/// point: the plugin requires the decoded bytes to be valid UTF-8 *text*, and
/// fails with `SignatureUtf8` otherwise. That requirement is what the bare-key
/// pubkey violated.
fn base64_to_string(s: &str) -> Result<String, String> {
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("base64 decode failed: {e}"))?;
    String::from_utf8(decoded).map_err(|e| format!("decoded bytes are not UTF-8 text: {e}"))
}

/// The `.pub` file in `.locus-keys/`, when this checkout has one.
fn local_pub_file() -> Option<String> {
    std::fs::read_to_string(client_dir().join("..").join(".locus-keys").join("locus_update.pub")).ok()
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
            "the configured pubkey does not match .locus-keys/locus_update.pub — \
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
#[test]
fn the_bare_public_key_is_not_a_valid_pubkey() {
    let bare = "RWT9eeTiJVDWZDZ8hFQ7TOE6BW7OEwARW+kVEU7m2jIlxcjfiGR9InKW";
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
                    trusted comment: Locus linux\n\
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
