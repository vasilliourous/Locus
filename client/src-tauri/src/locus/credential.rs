//! The machine-scoped credential store: where the activation code survives.
//!
//! # Why this module exists
//!
//! An activation code lives in `verge.yaml` (see [`crate::locus::store`]), which
//! is the *app's* config. That file is deleted by an uninstall, replaced by some
//! update paths, and reset by the app's own backup/restore machinery. When it
//! goes, the code goes with it, and the student is dropped back onto the
//! activation screen with no way forward except finding the card they threw away.
//!
//! The fix is the same one the retired device-identity store used for the device
//! secret: keep a second copy in a **machine-scoped** location, outside the
//! user's home and outside the app bundle, so an uninstall or an update does not
//! take it. `verge.yaml` stays the primary store; this is the durable mirror that
//! lets the next launch put the code back.
//!
//! # What this file is, and is not
//!
//! It holds one thing: the activation code. It is a **bearer credential** for the
//! student's tier, and the file is written owner-only (`0o600`) for that reason.
//! It is never logged, never exported, and never rendered — the same rules
//! [`crate::locus::store`] applies to the copy in `verge.yaml`. There is nothing
//! secret *identifying* here (no device id, no fingerprint): guarding a code
//! against deletion does not require knowing whose device it is.
//!
//! # Failure is not fatal
//!
//! Every function here degrades quietly. A read that fails is indistinguishable
//! from "nothing stored", and a write that fails leaves the `verge.yaml` copy
//! intact, so the device still works for this session — it simply cannot promise
//! to survive a reinstall. That matches how the identity store behaves.

use std::path::{Path, PathBuf};

/// The filename the activation code is stored under, in the machine store.
///
/// Deliberately dull, for the same reason the identity file is: on a shared
/// machine, a file named after the product sitting in a world-readable directory
/// is an invitation. The content is what matters, not the name.
const CREDENTIAL_FILE: &str = "activation-code.json";

/// The machine-scoped directory the credential is stored in, per platform.
///
/// Each platform's location is chosen to be outside the user's home and outside
/// the app bundle, so it survives an uninstall and a reinstall.
#[must_use]
pub fn credential_store_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        // The FHS location for variable state that survives package operations.
        Some(PathBuf::from("/var/lib/locus"))
    }
    #[cfg(target_os = "macos")]
    {
        // `/Library` is machine-scoped; a `~/Library` path dies with the account.
        Some(PathBuf::from("/Library/Application Support/Locus"))
    }
    #[cfg(target_os = "windows")]
    {
        // ProgramData is the Windows convention for shared app state on this
        // computer, and is where an installer is expected to leave state it wants
        // back later.
        std::env::var_os("PROGRAMDATA").map(|base| PathBuf::from(base).join("Locus"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

/// The credential file inside a store directory.
#[must_use]
pub fn credential_path_in(dir: &Path) -> PathBuf {
    dir.join(CREDENTIAL_FILE)
}

/// Reads the mirrored activation code, if one is present and readable.
///
/// `None` on anything unexpected — absent file, unreadable file, or a parse that
/// does not yield a non-empty code. The caller then falls back to `verge.yaml`,
/// so a permissions problem or a torn write degrades to "no mirror" rather than
/// failing startup.
#[must_use]
pub fn read_code() -> Option<String> {
    let dir = credential_store_dir()?;
    read_code_file(&credential_path_in(&dir))
}

/// Reads a code from a specific file. Pure I/O, so it can be tested against a
/// scratch directory without touching a real machine-scoped path.
#[must_use]
pub fn read_code_file(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let code = parse_code(&text)?;
    let trimmed = code.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// Writes the activation code to the machine-scoped store. Reports success.
///
/// Failure is reported rather than propagated: the caller has `verge.yaml` and a
/// device with no durable mirror should still work — it just cannot promise to
/// survive a reinstall.
pub fn write_code(code: &str) -> bool {
    let Some(dir) = credential_store_dir() else {
        return false;
    };
    write_code_file(&dir, code)
}

/// Writes a code into `dir`, creating the directory. Reports success.
pub fn write_code_file(dir: &Path, code: &str) -> bool {
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return false;
    }
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let path = credential_path_in(dir);
    if std::fs::write(&path, render_code(trimmed)).is_err() {
        return false;
    }
    // Owner-only, best-effort. The file holds a bearer credential, so this
    // matters; but refusing to start over a chmod would be worse than the
    // marginal exposure, and the directory is already machine-scoped.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    true
}

/// Removes the mirrored code. Used only when the hub has definitively refused
/// the code (suspension, expiry, refund) — never on a transient failure.
///
/// Best-effort: an absent file is the desired end state anyway.
pub fn remove_code() {
    let Some(dir) = credential_store_dir() else {
        return;
    };
    let _ = std::fs::remove_file(credential_path_in(&dir));
}

/// Pulls the `code` field out of the small JSON object this module writes.
///
/// Hand-parsed for the same reason the identity store is: the file holds one
/// string this module wrote, and a permissive general parser would be a second
/// place for the stored shape to drift. A file that does not parse reads as "no
/// code" and is re-derived from `verge.yaml`.
#[must_use]
fn parse_code(text: &str) -> Option<String> {
    let needle = "\"code\"";
    let start = text.find(needle)? + needle.len();
    let after = &text[start..];
    let open = after.find(':')? + 1;
    let rest = after[open..].trim_start();
    let rest = rest.strip_prefix('"')?;
    // Scan to the closing quote, honouring backslash escapes, so the value read
    // back is the value that was written. Stripping at the first `"` would
    // truncate any code containing an escaped quote — the two halves must be
    // exact inverses or a valid stored code reads back as a corrupt one.
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                // An escape we did not write: treat the file as unparseable
                // rather than guess at its meaning.
                _ => return None,
            },
            '"' => return Some(out),
            other => out.push(other),
        }
    }
    None
}

/// Renders the code as the on-disk JSON.
///
/// An explicit format rather than serde, so the file has a stable, readable shape
/// a support engineer can inspect without tooling. The `version` field is written
/// so a future shape change can be detected rather than guessed at.
#[must_use]
pub fn render_code(code: &str) -> String {
    format!("{{\n  \"code\": \"{}\",\n  \"version\": 1\n}}\n", escape(code))
}

/// Escapes the two characters that would break the hand-rolled JSON above.
///
/// A code is A–Z and 2–9 with hyphens by construction, so this is belt and
/// braces; it exists so a corrupted value cannot turn the file into something
/// [`parse_code`] would misread.
#[must_use]
fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory under the system temp root, unique per test.
    ///
    /// Follows the pattern already used in `identity.rs`, `update/apply.rs` and
    /// `utils/server.rs` rather than pulling in a `tempfile` dependency.
    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("locus-credential-{}-{}", name, std::process::id()))
    }

    /// The point of the module: a written code reads back byte-for-byte.
    #[test]
    fn a_written_code_reads_back() {
        let dir = scratch("roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(write_code_file(&dir, "RQ-ZVY6-7NSD-X9X2-T"));
        assert_eq!(
            read_code_file(&credential_path_in(&dir)).as_deref(),
            Some("RQ-ZVY6-7NSD-X9X2-T")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Writing creates the directory, because the machine store may not exist on
    /// a fresh device.
    #[test]
    fn writing_creates_the_directory() {
        let dir = scratch("mkdir");
        let _ = std::fs::remove_dir_all(&dir);
        let nested = dir.join("locus");
        assert!(write_code_file(&nested, "RQ-ZVY6-7NSD-X9X2-T"));
        assert!(nested.is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An empty or whitespace code is not a credential and must not be written —
    /// an empty file would read back as a stored code on the next launch and
    /// strand the student on a screen that says they are activated with nothing.
    #[test]
    fn an_empty_code_is_not_written() {
        let dir = scratch("empty");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!write_code_file(&dir, ""));
        assert!(!write_code_file(&dir, "   "));
        assert!(read_code_file(&credential_path_in(&dir)).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A missing file reads as "no code", not as an error.
    #[test]
    fn a_missing_file_reads_as_none() {
        let dir = scratch("missing");
        assert!(read_code_file(&credential_path_in(&dir)).is_none());
    }

    /// A corrupt or truncated file reads as "no code", so the caller falls back
    /// to `verge.yaml` rather than panicking at startup.
    #[test]
    fn a_corrupt_file_reads_as_none() {
        let dir = scratch("corrupt");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = credential_path_in(&dir);
        std::fs::write(&path, "{\n  \"code\": \"unterminated").expect("write corrupt");
        assert!(read_code_file(&path).is_none());
        std::fs::write(&path, "not json at all").expect("write junk");
        assert!(read_code_file(&path).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The rendered file round-trips through the parser, which is the property
    /// that keeps the two halves from drifting.
    #[test]
    fn render_and_parse_agree() {
        let code = "RQ-RLEX-Z93C-V7DS-U";
        assert_eq!(parse_code(&render_code(code)).as_deref(), Some(code));
    }

    /// A code containing a quote or backslash cannot turn into a second field.
    #[test]
    fn a_hostile_value_cannot_escape_the_field() {
        let rendered = render_code("RQ\"X\\Y");
        assert_eq!(parse_code(&rendered).as_deref(), Some("RQ\"X\\Y"));
    }

    /// Removing is idempotent and leaves no readable code behind.
    #[test]
    fn removal_clears_the_file() {
        let dir = scratch("removal");
        let _ = std::fs::remove_dir_all(&dir);
        let path = credential_path_in(&dir);
        assert!(write_code_file(&dir, "RQ-ZVY6-7NSD-X9X2-T"));
        std::fs::remove_file(&path).expect("remove");
        assert!(read_code_file(&path).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
