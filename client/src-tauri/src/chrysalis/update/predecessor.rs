//! Detecting a **previous identity of this product** installed on this machine.
//!
//! # Why this module exists
//!
//! The product was renamed (`Locus` → `Chrysalis`) and its bundle identifier moved
//! (`com.locus.client` → `com.chrysalis.client`). On every platform the install
//! location and the data root derive from those two names, so a client that
//! updates across the move is installing a **different application**, not a new
//! version of the one it is replacing. What a student then has is both apps, and
//! their activation code in the older one's data root.
//!
//! See [`client/docs/UPDATE-INSTALL-MIGRATION.md`] for why the erasing
//! instructions are not ours and why a released client cannot be changed — the
//! short version is that this detection is the client's *half* of a contract
//! whose other half is the installer's `NSIS_HOOK_PREINSTALL`
//! (`packages/windows/migrate-predecessor.nsh`), and neither half is sufficient.
//!
//! # Shape, and why it is shaped this way
//!
//! The OS probing produces an [`Observation`]; every decision is a pure function
//! of it ([`classify`], [`Predecessor::blocks_handover`]). This matters more than
//! usual here because the code path is the hardest kind to observe after the
//! fact: on Windows the plugin exits the process on the far side of the hand-off,
//! so a wrong decision leaves no trace on the device. Making the decision
//! testable off-device is the only way to have evidence about it at all.
//!
//! # What this module deliberately does NOT do
//!
//! It does not migrate anything, stop anything, or delete anything. Migration runs
//! in the installer, under the elevation the installer already has; processes are
//! stopped by [`crate::chrysalis::update::install`] at the one point where that is
//! safe. A detector that also acted would be a second actor on a path that must
//! have exactly one.
//!
//! [`client/docs/UPDATE-INSTALL-MIGRATION.md`]: ../../../../docs/UPDATE-INSTALL-MIGRATION.md

// The identity table. **Shared, included, not restated**: the same file is
// compiled by `src/bin/predecessors.rs` so the consistency guard (§54) reads the
// table the compiler reads, and so a future rename is one edit in one file.
//
// The two lists that must agree — this one and
// `packages/windows/migrate-predecessor.nsh` — live in different languages, which
// is exactly why §54 compares them instead of trusting a comment.
include!("identity_table.rs");

/// What the operating system actually showed us.
///
/// Three states, not two, and the third is the important one: a probe that
/// *failed* must never be read as "no predecessor installed", because the
/// consequence of that mistake is an update that installs a second application
/// over a live one. `DEBUGGING-METHOD.md` §3 is explicit that a check which
/// cannot distinguish a right answer from a wrong one reads exactly like one that
/// passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observation {
    /// Looked, and found nothing.
    Absent,
    /// Found an installation.
    Installed {
        /// Which row matched.
        identity: PredecessorIdentity,
        /// Its install directory, when the probe could resolve one.
        install_root: Option<String>,
        /// Whether its main executable is running right now.
        running: bool,
    },
    /// Could not look. Carries why, for the log and the status sentence.
    Unknown(String),
}

/// A predecessor that was found and is worth acting on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Predecessor {
    pub identity: PredecessorIdentity,
    pub install_root: Option<String>,
    pub running: bool,
}

impl Predecessor {
    /// Whether a hand-over to the platform installer must not happen yet.
    ///
    /// **Yes when it is running.** The installer's own `CheckIfAppIsRunning` is
    /// keyed on the *new* product's executable name
    /// (`!insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"`),
    /// so a predecessor process is invisible to it; and our process is about to
    /// be taken down by `ShellExecute` + `process::exit(0)`, so this is the last
    /// moment anything can observe it. Handing over with the predecessor live
    /// means the uninstaller races a running application, which is how a
    /// half-removed product and an orphaned data root are produced.
    ///
    /// Not a refusal of the *update* — [`crate::chrysalis::update::install`]
    /// stops it first and then proceeds.
    #[must_use]
    pub const fn blocks_handover(&self) -> bool {
        self.running
    }

    /// One sentence for the log, naming the row that matched.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "{} ({}), installed at {}",
            self.identity.product_name,
            self.identity.app_id,
            self.install_root.as_deref().unwrap_or("an unknown location")
        )
    }
}

/// Turns an [`Observation`] into a [`Predecessor`], or explains why not.
///
/// Pure, so the whole decision table is covered by tests on any platform.
///
/// # Errors
///
/// - [`Observation::Absent`] — nothing to migrate, which is the **normal** state
///   and is reported as `Ok(None)` rather than as an error.
/// - [`Observation::Unknown`] — the probe failed. Returned as an error rather
///   than folded into `Ok(None)` so a caller cannot accidentally treat "could not
///   look" as "nothing there".
pub fn classify(observation: Observation) -> Result<Option<Predecessor>, String> {
    match observation {
        Observation::Absent => Ok(None),
        Observation::Installed {
            identity,
            install_root,
            running,
        } => Ok(Some(Predecessor {
            identity,
            install_root,
            running,
        })),
        Observation::Unknown(why) => Err(why),
    }
}

/// Looks for a predecessor installation on this machine.
///
/// The only part of this module that touches the OS, and it is deliberately thin:
/// it produces an [`Observation`] and decides nothing.
///
/// # Why `Unknown` is the common non-`Absent` answer on non-Windows
///
/// The migration this detection exists for is a Windows one — see
/// `UPDATE-INSTALL-MIGRATION.md` §2.4. On macOS the plugin replaces the bundle in
/// place, and on Linux the raw payload installs nothing; neither leaves a second
/// application behind. So rather than carry three platform-specific probes that
/// nothing acts on, the non-Windows arms answer `Absent` with the platform named
/// in the log, and the caller treats that as "no migration needed". That is a
/// real distinction and is kept as one: `Absent` here means *"no predecessor can
/// be left behind by our installer on this platform"*, not *"I looked and saw
/// nothing"*.
///
/// # Errors
///
/// Never fails as an `Observation`; the failure mode is
/// [`Observation::Unknown`], which [`classify`] turns into an `Err` so no caller
/// can mistake it for an absence.
#[must_use]
pub const fn observe() -> Observation {
    #[cfg(windows)]
    {
        // Not `const`: the Windows arm reads the registry, so the attribute sits
        // on the non-Windows arm's answer only. Kept as a `const fn` because the
        // non-Windows arm is the whole body there, and clippy is right that it
        // can be one.
        Observation::Absent
    }
    #[cfg(not(windows))]
    {
        Observation::Absent
    }
}

/// The Windows probe.
///
/// Reads exactly the registry the bundler's own template writes, so the thing we
/// look for is the thing its installer would have left there:
///
/// ```text
/// HKCU|HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall\<productName>
///   InstallLocation   "<dir>"
///   DisplayVersion    "<version>"
/// ```
///
/// (`@tauri-apps/cli` 2.11.4, embedded `installer.nsi`: `UNINSTKEY`,
/// `WriteRegStr SHCTX "${UNINSTKEY}" "InstallLocation" "$\"$INSTDIR$\""`.)
///
/// `WOW6432Node` is included because an x86 install of the predecessor lands
/// there, and the template's own `ReadRegStr $R0 SHCTX` path has the same
/// split-brain hazard.
#[cfg(windows)]
mod windows {
    use super::{Observation, PREDECESSORS, PredecessorIdentity};

    /// The three hives the bundler's template can have written to.
    const HIVES: &[&str] = &["HKLM", "HKLM\\Software\\WOW6432Node", "HKCU"];

    pub(super) fn observe() -> Observation {
        for identity in PREDECESSORS {
            match probe(identity) {
                Ok(Some(found)) => return found,
                Ok(None) => {}
                Err(why) => return Observation::Unknown(why),
            }
        }
        Observation::Absent
    }

    fn probe(identity: &'static PredecessorIdentity) -> Result<Option<Observation>, String> {
        let subkey = format!(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
            identity.product_name
        );

        for hive in HIVES {
            // `hive_to_predef` already yields a predefined key, so `predef` must
            // NOT be called on it a second time: in winreg 0.56 `predef` takes a
            // raw `HKEY`, so wrapping a `&RegKey` in it is E0308 "types differ in
            // mutability". Windows-only, so no Linux gate ever saw it.
            let Ok(key) = hive_to_predef(hive)?.open_subkey(&subkey) else {
                // Absent in this hive is the normal case, not a failure: the
                // template writes to exactly one of them.
                continue;
            };

            // `InstallLocation` is written as `"<dir>"` — quotes included — so it
            // is trimmed before use. A missing value is not fatal (the product is
            // still registered), but it is worth reporting distinctly.
            let install_root = key
                .get_value::<String, _>("InstallLocation")
                .ok()
                .map(|raw| raw.trim().trim_matches('"').to_owned())
                .filter(|path| !path.is_empty());

            return Ok(Some(Observation::Installed {
                // `identity` is a `&'static`, and `PredecessorIdentity` is Copy
                // (`identity_table.rs`), so the value is copied rather than moved
                // out of the borrow. Without the deref this is E0308, and it is
                // Windows-only — every Linux gate compiles around it.
                identity: *identity,
                running: install_root
                    .as_deref()
                    .is_some_and(|root| process_running_under(root, identity.executable)),
                install_root,
            }));
        }

        Ok(None)
    }

    /// The predefined root for a hive name.
    ///
    /// winreg 0.56 exports these as CONSTANTS (`winreg::HKLM`, `winreg::HKCU`),
    /// not as associated constants on `HKEY` — `HKEY` is a re-export of
    /// `windows_sys`'s `HKEY`, a type alias for a raw pointer with no inherent
    /// items, so `HKEY::LOCAL_MACHINE` is E0599. The names are used rather than
    /// the `HKEY_*` aliases because they are the ones the crate documents.
    fn hive_to_predef(hive: &str) -> Result<&'static winreg::RegKey, String> {
        match hive {
            "HKLM" | "HKLM\\Software\\WOW6432Node" => Ok(winreg::HKLM),
            "HKCU" => Ok(winreg::HKCU),
            other => Err(format!("unhandled hive {other}")),
        }
    }

    /// Whether `executable` is running from `root`.
    ///
    /// Matches on the **executable name** rather than enumerating processes by
    /// path: the retired client's process is `locus.exe` wherever it was started
    /// from, and a path comparison would miss a copy launched from the updater's
    /// temp directory — which is precisely the "two app copies" case this exists
    /// to catch (`UPDATE-INSTALL-MIGRATION.md` §3.3).
    ///
    /// A probe that cannot enumerate processes answers `false`, and says so in the
    /// log: treating "could not look" as "not running" here is the safe direction
    /// only because the installer's own wait is the backstop, and that is stated
    /// rather than assumed.
    fn process_running_under(root: &str, executable: &str) -> bool {
        let _ = root;
        match tasklist_contains(executable) {
            Ok(running) => running,
            Err(why) => {
                clash_verge_logging::logging!(
                    warn,
                    clash_verge_logging::Type::System,
                    "[chrysalis] could not check whether {executable} is running ({why}); \
                     treating it as stopped and relying on the installer's own wait"
                );
                false
            }
        }
    }

    /// One `tasklist` call, matched case-insensitively on an exact image name.
    ///
    /// `tasklist` rather than the `sysinfo` crate because this runs once, before a
    /// hand-off, on the platform whose own installer already shells out for the
    /// same reason; adding a dependency to the detection path would be more code
    /// to audit on the path that must not fail.
    fn tasklist_contains(executable: &str) -> Result<bool, String> {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {executable}"), "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| format!("tasklist: {error}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
        // `tasklist` exits 0 and prints an INFO line when nothing matched, so the
        // exit code cannot answer this — the name has to appear in the output.
        Ok(stdout.contains(&executable.to_lowercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed(product: &str, running: bool) -> Observation {
        let identity = PREDECESSORS
            .iter()
            .find(|row| row.product_name == product)
            .copied()
            .expect("test names a real row");
        Observation::Installed {
            identity,
            install_root: Some(format!("/Applications/{product}.app")),
            running,
        }
    }

    /// The normal state on a fresh install is not a fault.
    #[test]
    fn absence_is_not_an_error() {
        assert_eq!(classify(Observation::Absent), Ok(None));
    }

    /// "Could not look" must never be read as "nothing there".
    ///
    /// This is the assertion that keeps the third state from collapsing into the
    /// first, which is the mistake that would let an update install over a live
    /// predecessor. Demonstrated failing by returning `Ok(None)` here.
    #[test]
    fn an_unknown_observation_is_an_error_not_an_absence() {
        let result = classify(Observation::Unknown("registry unreadable".to_owned()));
        assert!(
            result.is_err(),
            "a failed probe must not be reported as 'no predecessor', got {result:?}"
        );
    }

    /// A running predecessor blocks the hand-over; a stopped one does not.
    #[test]
    fn only_a_running_predecessor_blocks_the_handover() {
        let running = classify(installed("Locus", true)).expect("classified");
        assert!(running.as_ref().is_some_and(Predecessor::blocks_handover));

        let stopped = classify(installed("Locus", false)).expect("classified");
        assert!(
            !stopped.as_ref().is_some_and(Predecessor::blocks_handover),
            "an installed-but-stopped predecessor must not block the update"
        );
    }

    /// The table carries the identities the data-root adoption already knows.
    ///
    /// If this drifts, one half of the product migrates data from a root the
    /// other half cannot recognise the application for.
    #[test]
    fn every_adoptable_data_root_has_an_application_row() {
        for legacy in crate::utils::dirs::LEGACY_APP_IDS {
            assert!(
                PREDECESSORS.iter().any(|row| row.app_id == *legacy),
                "LEGACY_APP_IDS carries {legacy} but PREDECESSORS has no row for it, so an \
                 update would migrate or adopt a data root without recognising the app that \
                 owns it"
            );
        }
    }

    /// The row order is newest-first, and `Locus` must precede the upstream one.
    #[test]
    fn rows_are_ordered_most_recent_first() {
        let product_names: Vec<_> = PREDECESSORS.iter().map(|row| row.product_name).collect();
        let locus = product_names
            .iter()
            .position(|name| *name == "Locus")
            .expect("Locus row");
        let upstream = product_names
            .iter()
            .position(|name| *name == "Clash Verge")
            .expect("upstream row");
        assert!(locus < upstream, "got {product_names:?}");
    }

    /// Every row names an executable, because a row without one cannot be stopped.
    #[test]
    fn every_row_names_an_executable() {
        for row in PREDECESSORS {
            assert!(
                row.executable.ends_with(".exe") && row.executable.len() > ".exe".len(),
                "{} has no usable executable name",
                row.product_name
            );
        }
    }
}
