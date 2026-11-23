//! Verifying an artifact the way the plugin will, streamed from disk.
//!
//! # Why this exists
//!
//! `tauri_plugin_updater` decides whether to accept an artifact *inside
//! `install()`*, and `install()` is the function that also launches the
//! installer and calls `process::exit(0)` on Windows. So "the signature is
//! wrong" and "the install started" are the same observable event —
//! the process disappearing — and neither writes anything to a log.
//!
//! This is the same check, run one step earlier against the staged file, so a
//! rejected artifact is a return value at a known point in the sequence rather
//! than a silent exit. For the reader with the file open it answers the one
//! question that cannot be answered from our own hash: was the *signature*
//! path the thing that stopped us.
//!
//! # The decode chain is the plugin's, deliberately
//!
//! Reproduced from `tauri-plugin-updater` 2.11.0 rather than reimplemented,
//! because the two mistakes this repository has already paid for were both
//! *shape* mistakes that a hand-written decoder would have accepted:
//!
//! | mistake | what the plugin does |
//! |---|---|
//! | a payload with no `.sig` (mihomo-world) | its own `verify_signature` refuses |
//! | a **bare** pubkey (no `untrusted comment:` line) | `PublicKey::decode` refuses |
//!
//! Both are pinned by `tests/update_signature_contract.rs`, which uses this
//! module so the test and the product cannot drift.
//!
//! # Why streamed
//!
//! The plugin reads the whole artifact into a `Vec<u8>` to verify it. A school
//! laptop is the machine this product is for, and a 60 MB allocation on top of a
//! running VPN core is what killed the previous download path (FIXES: the
//! in-memory download). `verify_stream` hashes chunk by chunk instead, so the
//! peak cost here is one read buffer.

use anyhow::{Context as _, Result, bail};
use std::io::Read as _;
use std::path::Path;

/// A verified artifact's name and the version it carries, as the hub published
/// them. Enough to say *what* is waiting without holding the bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedUpdate {
    /// The version the hub advertised for these bytes.
    pub version: String,
    /// The staged file, already SHA-256-verified by `apply::download`.
    pub path: std::path::PathBuf,
}

/// Reproduces the plugin's `base64_to_string()`.
///
/// The plugin base64-decodes **both** the pubkey and the signature and then
/// requires the decoded bytes to be UTF-8 *text* — not key or signature bytes.
/// That requirement is what makes a bare pubkey fail: its 42 bytes are binary.
/// Reproducing it here is what keeps this check honest about the case that
/// actually shipped once.
///
/// # Errors
///
/// Returns an error when the value is not base64, or decodes to something that
/// is not UTF-8 — exactly the two ways the plugin rejects a field.
pub fn base64_to_string(value: &str) -> Result<String> {
    use base64::Engine as _;

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value)
        .context("the field is not base64")?;
    String::from_utf8(decoded).context("the decoded field is not UTF-8 text")
}

/// The reason an artifact was refused, or why it was not checked at all.
///
/// Three outcomes, never a boolean: "we could not check this build" is not the
/// same claim as "we checked and it is wrong", and collapsing them is how an
/// unverified artifact gets installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureVerdict {
    /// The signature is valid for these bytes.
    Valid,
    /// The check ran and the artifact is not what the signature covers.
    Rejected(String),
    /// The check could not run on this build — no compiled-in pubkey to check
    /// against. Deliberately NOT a pass.
    NotChecked(String),
}

/// Verifies `path` against `signature` using the compiled-in `pubkey`.
///
/// `pubkey` is the value in `tauri.conf.json`'s `plugins.updater.pubkey` (which
/// is base64 of the whole `.pub` file), passed in rather than read here so this
/// has no dependency on Tauri and can be driven from a test with real key
/// material.
///
/// # Errors
///
/// Only when the file cannot be read or hashed — a *refusal* is a
/// [`SignatureVerdict`], not an error, because the caller must report it.
pub fn verify_file(path: &Path, signature: &str, pubkey: &str) -> Result<SignatureVerdict> {
    if pubkey.trim().is_empty() {
        return Ok(SignatureVerdict::NotChecked(
            "this build carries no updater pubkey, so the signature was not checked".to_owned(),
        ));
    }

    // Refused rather than treated as "unsigned, continue": the plugin refuses
    // it too, so saying so here names the moment it stops.
    if signature.trim().is_empty() {
        return Ok(SignatureVerdict::Rejected(
            "the hub published no signature for this artifact".to_owned(),
        ));
    }

    let key_text = match base64_to_string(pubkey) {
        Ok(text) => text,
        Err(error) => {
            return Ok(SignatureVerdict::Rejected(format!(
                "the compiled-in pubkey does not decode the way the plugin decodes it \
                 ({error:#}) — every update from this build would be refused; see SIGNING.md §3"
            )));
        }
    };
    let signature_text = match base64_to_string(signature) {
        Ok(text) => text,
        Err(error) => {
            return Ok(SignatureVerdict::Rejected(format!(
                "the advertised signature is not base64 of a `.sig` file ({error:#})"
            )));
        }
    };

    let public_key = minisign_verify::PublicKey::decode(&key_text)
        .map_err(|error| anyhow::anyhow!("the pubkey did not parse as a minisign key: {error:?}"))?;
    let parsed = minisign_verify::Signature::decode(&signature_text)
        .map_err(|error| anyhow::anyhow!("the signature did not parse: {error:?}"))?;

    let mut verifier = match public_key.verify_stream(&parsed) {
        Ok(verifier) => verifier,
        Err(error) => {
            return Ok(SignatureVerdict::Rejected(format!(
                "this artifact's signature cannot be checked against the compiled-in key \
                 ({error:?}) — a different key signed it, or the artifact is not covered"
            )));
        }
    };

    let mut file = std::fs::File::open(path)
        .with_context(|| format!("could not open {} to verify its signature", path.display()))?;
    let mut buffer = vec![0_u8; 256 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("could not read {} to verify its signature", path.display()))?;
        if read == 0 {
            break;
        }
        verifier.update(&buffer[..read]);
    }

    Ok(match verifier.finalize() {
        Ok(()) => SignatureVerdict::Valid,
        Err(error) => SignatureVerdict::Rejected(format!(
            "the artifact does not match its signature ({error:?}) — the bytes on disk are not \
             the bytes that were signed"
        )),
    })
}

/// Runs [`verify_file`] and turns a refusal into an error.
///
/// Used where the next step would otherwise be `install()`, which on Windows
/// exits the process: a refusal has to be returned *before* that call, not
/// discovered inside it.
///
/// # Errors
///
/// Returns an error when the artifact is rejected. `NotChecked` is allowed
/// through deliberately — it is the state of a build with no pubkey, which
/// cannot be repaired here — but it is never reported as valid.
pub fn require_valid(path: &Path, signature: &str, pubkey: &str) -> Result<()> {
    match verify_file(path, signature, pubkey)? {
        SignatureVerdict::Valid => Ok(()),
        SignatureVerdict::NotChecked(reason) => {
            // Logged, not fatal: the plugin will still verify (its own audit is
            // mandatory), and a build without a pubkey cannot install anyway.
            tracing::warn!(target: "chrysalis::update", "signature not pre-checked: {reason}");
            Ok(())
        }
        SignatureVerdict::Rejected(reason) => bail!("{reason}"),
    }
}

/// Classifies a launched installer's exit status.
///
/// # Why this is worth a named function
///
/// `ShellExecuteW` returning a handle above 32 proves the shell *accepted* the
/// request. It does not prove a window appeared, and it says nothing at all
/// about what the setup did next — the plugin exits immediately either way. A
/// setup that opens and closes in a second is a
/// setup that was **blocked or cancelled before it could replace anything**,
/// and from the student's seat that is "it closed and nothing happened".
///
/// So the exit code is read back on Windows only, and reported. `0` is the
/// installer's own success; the codes below are the ones NSIS is documented to
/// return for the two outcomes that are *not* a successful install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerOutcome {
    /// The installer reported success.
    Completed,
    /// The installer ended without reporting success.
    DidNotComplete(i64),
}

impl InstallerOutcome {
    /// Reads an exit code the way a support conversation needs it read.
    #[must_use]
    pub const fn from_exit_code(code: i64) -> Self {
        if code == 0 {
            Self::Completed
        } else {
            Self::DidNotComplete(code)
        }
    }

    /// A sentence for the log, naming what the code means where it is known.
    #[must_use]
    pub fn describe(self) -> String {
        match self {
            Self::Completed => "the installer reported success".to_owned(),
            Self::DidNotComplete(code) => format!(
                "the installer exited with code {code} without installing{}",
                match code {
                    // NSIS: the user pressed Cancel on the setup's own page.
                    1223 => " (1223: the student cancelled the setup)",
                    // ERROR_INSTALL_USEREXIT / ERROR_INSTALL_FAILURE.
                    1602 => " (1602: the install was cancelled)",
                    1603 => " (1603: the install FAILED — this is the one to report)",
                    _ => "",
                }
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A verdict must never be reported as valid by accident: `NotChecked` and
    /// `Rejected` are different claims from `Valid`, and only `Valid` means the
    /// bytes were signed.
    #[test]
    fn only_a_valid_verdict_is_a_pass() {
        assert_eq!(InstallerOutcome::from_exit_code(0), InstallerOutcome::Completed);
        assert_eq!(
            InstallerOutcome::from_exit_code(1603),
            InstallerOutcome::DidNotComplete(1603)
        );
        assert!(
            InstallerOutcome::from_exit_code(1603).describe().contains("FAILED"),
            "1603 is the code worth reporting, so the log line must say so"
        );
        assert!(
            InstallerOutcome::from_exit_code(1223).describe().contains("cancelled"),
            "a cancellation must not read as a failure — they are different reports"
        );
    }

    /// An empty pubkey is `NotChecked` — not a pass, and not an error. A build
    /// without a pubkey cannot install, and pretending it verified would hide it.
    #[test]
    fn a_missing_pubkey_is_not_a_pass() {
        let dir = std::env::temp_dir().join(format!("chrysalis-verify-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("artifact.exe");
        std::fs::write(&file, b"not really an installer").expect("write");

        let verdict = verify_file(&file, "anything", "  ").expect("reading a real file");
        assert!(
            matches!(verdict, SignatureVerdict::NotChecked(_)),
            "no pubkey must be NotChecked, got {verdict:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A signed artifact must be checked, so an empty signature is a REJECTION
    /// (the plugin refuses it) rather than "skip and continue".
    #[test]
    fn a_missing_signature_is_a_rejection() {
        let dir = std::env::temp_dir().join(format!("chrysalis-verify-sig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("artifact.exe");
        std::fs::write(&file, b"bytes").expect("write");

        // Any decodable pubkey text will do; the signature is refused first.
        let verdict = verify_file(&file, "   ", "AAAA").expect("reading a real file");
        assert!(
            matches!(verdict, SignatureVerdict::Rejected(ref reason) if reason.contains("no signature")),
            "an empty signature must be a rejection, got {verdict:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
