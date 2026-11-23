//! Did the update actually replace the application?
//!
//! # The report this exists for
//!
//! "Updates install, but the taskbar icon, the window icon and the tray icon never
//! change." That sentence has two possible causes and the product could not tell
//! them apart, because **nothing in it ever compared the binary before an install
//! with the binary after one**:
//!
//!   * **The installer worked and the icon is fine** — a stale window, or the icon
//!     cache the OS keeps per user/per program (Explorer caches taskbar and window
//!     icons; a bitmap can outlive the file for a while).
//!   * **The installer did not replace the binary.** On Windows the plugin
//!     `ShellExecuteW`s the NSIS setup and then calls `std::process::exit(0)`
//!     (`tauri-plugin-updater` 2.11.0, `updater.rs:876`) **regardless of whether
//!     the setup succeeded**. A declined elevation, a locked image, or an NSIS
//!     `/UPDATE` that decides not to overwrite leaves the old executable in place
//!     and exits the process exactly as successfully as a real install would.
//!     `install()` never returns, so no `Err`, no log line and no UI state
//!     records it. On the next launch the app is the old build — with the old
//!     icons, because the icon is compiled into that binary.
//!
//! The second is not cosmetic. A client that believes it updated stops trying:
//! `chrysalis_update_check_reason` reports "not newer" and the prompt never
//! appears again, so one silent failed install strands a device permanently. That
//! is the defect worth fixing, and the icon is the symptom that made it visible.
//!
//! # The mechanism, and why it is shaped this way
//!
//! The attempt cannot report its own outcome: on Windows the process is gone
//! before anything after `install()` runs, and on macOS/Linux the old image keeps
//! running. So the record is written **before** the hand-off, and read by the
//! **next** launch. That launch is the first moment anything can see the result.
//!
//! What is recorded is the running version and a SHA-256 of the executable's own
//! bytes. Hashing rather than trusting the version string is deliberate: a
//! `perMachine` NSIS install can rewrite the version in metadata while failing to
//! replace the payload, and a version comparison would call that a success.
//!
//! # What this deliberately does NOT claim
//!
//! It does not prove *why* an install failed, and it cannot: the installer's own
//! log is not ours. It turns an invisible outcome into a recorded one, which is
//! the same move `InstallStage` made for the reported crash (`FIXES.md`,
//! 2026-11-23) and for the same reason — a path whose every branch ends in the
//! process dying has to write its record *before* the branch, not after it.

use anyhow::{Context as _, Result};
use clash_verge_logging::{Type, logging};
use sha2::{Digest as _, Sha256};

use super::install::{InstallStage, PreExitSnapshot};

/// Separates the two halves of the stored snapshot.
///
/// A character that cannot appear in a version string (`update::version` rejects
/// whitespace, slashes and tabs) and is not a hex digit. Chosen once, here, and
/// used by both the writer and the reader so they cannot disagree — the mistake
/// that made `download_*`/`update_*` silently wrong elsewhere in this tree.
const SEPARATOR: char = '|';

/// SHA-256 of the executable at `path`, hex-encoded.
///
/// Streamed rather than read into a `Vec`: this runs on every update attempt on a
/// machine that may be a 4 GB school laptop with a VPN core running, and the
/// binary is tens of megabytes. The same reasoning as `apply::sha256_file`, and
/// the same 256 KiB buffer.
///
/// # Errors
///
/// Returns an error only if the file cannot be opened or read. A caller that gets
/// `Err` must **not** record a snapshot: an unknown fingerprint would compare
/// unequal against everything and report every install as a replacement.
pub fn executable_digest(path: &std::path::Path) -> Result<String> {
    use std::io::Read as _;

    let mut file = std::fs::File::open(path)
        .with_context(|| format!("could not open {} to fingerprint the running app", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 256 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("could not read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Records what the application looks like, immediately before the installer is
/// handed the bytes.
///
/// Called as the last step of [`super::install::ReadyInstall::install`], next to
/// the `Launched` stage and for the same reason: it is the last moment this
/// process is guaranteed to be alive on Windows.
///
/// # Errors
///
/// Best-effort by contract — see the call site. This returns a `Result` so the
/// caller can log *why* nothing was recorded, which is the difference between
/// "the installer failed" and "we have no idea what happened" on the next launch.
pub async fn record_pre_exit(version: &str, executable: &std::path::Path) -> Result<PreExitSnapshot> {
    let exe_sha256 = executable_digest(executable)?;
    let snapshot = PreExitSnapshot {
        version: version.to_owned(),
        exe_sha256,
    };
    crate::chrysalis::store::record_update_pre_exit(&encode(&snapshot)).await?;
    Ok(snapshot)
}

/// Encodes a snapshot for storage.
#[must_use]
pub fn encode(snapshot: &PreExitSnapshot) -> String {
    format!("{}{SEPARATOR}{}", snapshot.version, snapshot.exe_sha256)
}

/// Decodes a stored snapshot.
///
/// `None` for anything that is not exactly two non-empty fields, including a
/// value written by a future build with a different shape. An unparseable record
/// must read as "nothing recorded", never as a match — the fail-closed direction,
/// because the alternative reports a successful install for a device that never
/// installed anything.
#[must_use]
pub fn decode(raw: &str) -> Option<PreExitSnapshot> {
    let (version, digest) = raw.split_once(SEPARATOR)?;
    if version.is_empty() || digest.is_empty() {
        return None;
    }
    Some(PreExitSnapshot {
        version: version.to_owned(),
        exe_sha256: digest.to_owned(),
    })
}

/// What the next launch concluded about the last attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The executable changed, so the installer did its job. The icon changing or
    /// not is now the OS's business (a cache), not ours.
    Replaced {
        /// The version this launch is running.
        now: String,
        /// The version the installation started from.
        was: String,
    },
    /// The executable is byte-identical to what it was before the hand-off. The
    /// installer completed and replaced nothing.
    NotReplaced { was: String, expected: String },
    /// Nothing to compare — no snapshot was recorded (an older build, or the
    /// fingerprint failed). **Not** a clean bill of health: it is the absence of
    /// evidence, which is why it has its own variant rather than being folded
    /// into `Replaced`.
    Unknown,
}

/// Decides the outcome of the previous attempt, given what is running now.
///
/// Pure, so it can be tested without a process, a file or a Tauri handle — the
/// property that lets the three cases be exercised at all, since reproducing a
/// declined Windows elevation in CI is not possible.
///
/// `running_digest` is `Option` because hashing the current executable can fail
/// (`Err` on a read) and because a test has no executable to hash. `None` yields
/// [`Outcome::Unknown`], never `Replaced`: a comparison that could not be made is
/// not a match.
#[must_use]
pub fn classify(stored: Option<&str>, running_version: &str, running_digest: Option<&str>) -> Outcome {
    let Some(snapshot) = stored.and_then(decode) else {
        return Outcome::Unknown;
    };
    let Some(digest) = running_digest else {
        return Outcome::Unknown;
    };

    if digest != snapshot.exe_sha256 {
        return Outcome::Replaced {
            now: running_version.to_owned(),
            was: snapshot.version,
        };
    }

    // Byte-identical. Two readings, and only one of them is a fault:
    //
    //  * a *failed or skipped* install — the installer was handed the bytes and
    //    nothing changed, so this device is stranded on the old build;
    //  * a *re-run of the same version* — the student installed the same release
    //    again, or the record was written twice by a retry, in which case
    //    "unchanged" is the correct and unremarkable answer.
    //
    // They are not distinguishable from the snapshot alone, and inventing a
    // distinction would be a claim the data does not support. So the version
    // comparison decides the display: when the running version is newer than the
    // one the install started from, the install plainly worked even though this
    // binary happens to hash the same — which is only possible for a build that
    // did not differ in bytes, and is reported as replaced rather than as a fault.
    if crate::chrysalis::update::is_newer(running_version, &snapshot.version) {
        return Outcome::Replaced {
            now: running_version.to_owned(),
            was: snapshot.version,
        };
    }

    Outcome::NotReplaced {
        was: running_version.to_owned(),
        expected: snapshot.version,
    }
}

/// Runs the comparison once, at start-up, and records the verdict.
///
/// Called from the setup sequence next to the other one-shot update work. Three
/// things happen, in this order, and the order is the point:
///
/// 1. read and **clear** the snapshot, so a crash after this line cannot make the
///    verdict be recorded twice;
/// 2. compare against the running executable;
/// 3. store the resulting stage, which is what the Account page renders.
///
/// Clearing first means a process that dies between 1 and 3 loses the verdict
/// rather than repeating it — the right way round, because a repeated
/// "this device did not update" on every launch is the kind of permanent warning
/// students learn to ignore (`COLOURS.md`'s rule about a claim that cannot change).
pub async fn classify_on_launch(running_version: &str) {
    let stored = match crate::chrysalis::store::take_update_pre_exit().await {
        Ok(Some(raw)) => raw,
        Ok(None) => {
            // The ordinary case on every launch that was not preceded by an
            // install attempt. Debug rather than info: it would otherwise be the
            // most frequent line in the log.
            logging!(
                debug,
                Type::System,
                "[chrysalis] no update attempt is pending; nothing to compare"
            );
            return;
        }
        Err(error) => {
            logging!(
                warn,
                Type::System,
                "[chrysalis] could not read the pending-update record: {error:#}"
            );
            return;
        }
    };

    let digest = std::env::current_exe()
        .map_err(anyhow::Error::from)
        .and_then(|path| executable_digest(&path));

    let digest = match digest {
        Ok(digest) => Some(digest),
        Err(error) => {
            // Missing evidence, not a failed update. Recorded as `Unknown` by
            // leaving the stage untouched, and logged with the reason so the next
            // report can say why nothing was concluded.
            logging!(
                warn,
                Type::System,
                "[chrysalis] could not fingerprint the running app after an update attempt: {error:#}"
            );
            None
        }
    };

    match classify(Some(&stored), running_version, digest.as_deref()) {
        Outcome::Replaced { now, was } => {
            logging!(
                info,
                Type::System,
                "[chrysalis] update applied: the running app is {now}, installed from {was}"
            );
            InstallStage::Replaced.record().await;
        }
        Outcome::NotReplaced { was, expected } => {
            // The one case worth an `error`: this device was told it updated and
            // did not, and nothing else in the product will ever mention it again.
            let decoded = decode(&stored).map_or_else(|| expected.clone(), |s| s.version);
            logging!(
                error,
                Type::System,
                "[chrysalis] update to {decoded} did NOT replace the application; still running {was} \
                 (the executable is unchanged). The icons cannot change either, because they are \
                 compiled into the binary that was never replaced."
            );
            InstallStage::NotReplaced.record().await;
        }
        Outcome::Unknown => {
            logging!(
                warn,
                Type::System,
                "[chrysalis] an update was attempted but its outcome cannot be determined \
                 (no record, or the running executable could not be read)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(version: &str) -> String {
        encode(&PreExitSnapshot {
            version: version.to_owned(),
            exe_sha256: "a".repeat(64),
        })
    }

    /// The reported case, as a property: a new executable is a replacement.
    ///
    /// Demonstrated failing by returning `NotReplaced` from this branch.
    #[test]
    fn a_changed_executable_means_the_update_applied() {
        let outcome = classify(Some(&snapshot("3.4.4")), "3.4.5", Some(&"b".repeat(64)));
        assert_eq!(
            outcome,
            Outcome::Replaced {
                now: "3.4.5".to_owned(),
                was: "3.4.4".to_owned()
            }
        );
    }

    /// The reported defect, as a property: the *same* executable after an attempt
    /// is not a success, however the version string reads.
    ///
    /// Demonstrated failing by making the identical-digest branch return
    /// `Replaced` — which is the bug that shipped, because nothing compared the
    /// binary at all.
    #[test]
    fn an_unchanged_executable_is_not_replaced() {
        let digest = "a".repeat(64);
        let outcome = classify(Some(&snapshot("3.4.4")), "3.4.4", Some(&digest));
        assert_eq!(
            outcome,
            Outcome::NotReplaced {
                was: "3.4.4".to_owned(),
                expected: "3.4.4".to_owned()
            }
        );
    }

    /// A newer version with an identical digest is a re-install of a build that did
    /// not change bytes, not a failure. Reported as replaced, and the test names
    /// why so the branch is not "fixed" into a false alarm later.
    #[test]
    fn a_newer_version_with_an_identical_digest_is_not_a_fault() {
        let digest = "a".repeat(64);
        assert_eq!(
            classify(Some(&snapshot("3.4.4")), "3.4.5", Some(&digest)),
            Outcome::Replaced {
                now: "3.4.5".to_owned(),
                was: "3.4.4".to_owned()
            }
        );
    }

    /// Missing evidence is `Unknown`, never `Replaced`. The direction matters: a
    /// comparison that could not be made must not report a clean bill of health.
    #[test]
    fn absent_evidence_is_unknown_and_not_success() {
        assert_eq!(classify(None, "3.4.5", Some(&"b".repeat(64))), Outcome::Unknown);
        assert_eq!(classify(Some(&snapshot("3.4.4")), "3.4.5", None), Outcome::Unknown);
    }

    /// A record the decoder cannot read is treated as no record, and must not
    /// match anything. Truncation is the realistic case (a partial write).
    #[test]
    fn a_malformed_record_cannot_be_read_as_a_match() {
        for bad in ["", "3.4.4", "3.4.4|", "|abc", "no-separator-at-all"] {
            assert_eq!(decode(bad), None, "{bad:?} decoded, and must not");
            assert_eq!(classify(Some(bad), "3.4.5", Some("x")), Outcome::Unknown);
        }
    }

    /// The encoder and decoder agree, including for a version that contains the
    /// characters the rest of the update path permits.
    #[test]
    fn the_round_trip_preserves_both_fields() {
        for version in ["3.4.4", "3.4.4-rc1", "3.4.4+build7", "v3.4.4"] {
            let encoded = snapshot(version);
            let decoded = decode(&encoded).expect("its own encoding must decode");
            assert_eq!(decoded.version, version);
            assert_eq!(decoded.exe_sha256, "a".repeat(64));
        }
    }

    /// A real file fingerprints deterministically, and differently from another
    /// file. This is the property the whole module rests on; without it a
    /// fingerprint that returned a constant would report every install as a
    /// replacement.
    ///
    /// Demonstrated failing by making `executable_digest` return `Ok(String::new())`.
    #[test]
    fn a_fingerprint_is_real_and_content_addressed() {
        let dir = std::env::temp_dir().join(format!("chrysalis-relabel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let first = dir.join("a.bin");
        let second = dir.join("b.bin");
        std::fs::write(&first, b"one").expect("write");
        std::fs::write(&second, b"two").expect("write");

        let a = executable_digest(&first).expect("hash");
        let b = executable_digest(&second).expect("hash");
        assert_eq!(a.len(), 64, "sha256 is 64 hex characters");
        assert_eq!(a, executable_digest(&first).expect("hash again"), "must be stable");
        assert_ne!(a, b, "different bytes must hash differently");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An unreadable path is an error rather than a fingerprint, so the caller
    /// records nothing instead of recording a value that matches everything.
    #[test]
    fn a_missing_file_errors_rather_than_hashing_nothing() {
        let missing = std::env::temp_dir().join("chrysalis-relabel-does-not-exist");
        assert!(executable_digest(&missing).is_err());
    }
}
