//! Driving the plugin's installer from Chrysalis update metadata.
//!
//! # The mechanism (corrected against the plugin's source, not its docs)
//!
//! `tauri_plugin_updater::Update` **cannot be constructed by hand**: two of its
//! fields (`extract_path`, `context`) are private, so a struct literal from
//! outside the crate does not compile. The only way to obtain one is
//! `Updater::check()`.
//!
//! That is fine, because `check()` accepts a **dynamic** response shape:
//!
//! ```json
//! { "version": "2.3.0", "url": "https://…", "signature": "…" }
//! ```
//!
//! and `UpdaterBuilder::endpoints(..)` **replaces** the configured endpoint list
//! rather than appending to it. So the plugin can be pointed at a URL we build
//! at runtime, and fed a manifest the hub serves — while still owning the
//! download, the mandatory minisign verification, and the platform install.
//!
//! # What that means for the architecture
//!
//! ```text
//! heartbeat / entitlement  ->  the hub decides who is offered what
//!                          ->  GET /api/update?version=<running>   (our endpoint)
//!                          ->  { version, url, signature }         (dynamic shape)
//!                          ->  UpdaterBuilder::endpoints([ours])   (runtime)
//!                          ->  check() -> Update -> install()      (the plugin's job)
//! ```
//!
//! The client keeps its own [`super::version`] gate and [`super::signal`]
//! decoder for *deciding* whether to offer an update at all, and for the
//! credential-free fallback. The plugin is used strictly for download + verify +
//! install.
//!
//! # Why this is better than the retired client
//!
//! The old client hand-rolled the swap: backup, rename, fork, sentinel-revert.
//! That destroyed an installation (FIXES #22) and staged into the current
//! working directory (FIXES #54). None of it is ported. The plugin's installer
//! is already atomic on all three platforms.

use anyhow::{Context as _, Result, bail};
use clash_verge_logging::{Type, logging};
use std::path::Path;

/// Where the plugin should look for an update manifest.
///
/// Built at runtime rather than baked into `tauri.conf.json`, because the URL
/// carries the running version and the platform — both of which the hub needs
/// to answer correctly. Keeping `endpoints` empty in the config is deliberate:
/// it means a stale static manifest cannot be consulted by accident.
#[must_use]
pub fn manifest_endpoint(hub_url: &str, running_version: &str, platform: &str) -> String {
    format!(
        "{}/api/update?version={}&platform={}",
        hub_url.trim_end_matches('/'),
        encode(running_version),
        encode(platform)
    )
}

/// Percent-encodes a query value.
///
/// Hand-rolled rather than pulling a dependency for two call sites. Only the
/// characters that can appear in a version string or a platform key need
/// handling, and everything else is passed through.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

/// An installer handed to the plugin, ready to run.
pub struct PendingInstall {
    update: tauri_plugin_updater::Update,
}

impl PendingInstall {
    /// Asks the hub what this device should install, then validates the answer.
    ///
    /// `current_version` gates the result locally as well as on the hub: the hub
    /// decides, but a stale or misconfigured row must not be able to downgrade a
    /// client, so the version comparison is repeated here. There is no
    /// server-driven downgrade in this system, so refusing is the only correct
    /// behaviour.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` when there is nothing to install — which is the normal
    /// state, not a failure. A transport error is returned as `Err`, because the
    /// caller must distinguish "no update" from "could not ask".
    pub async fn check(app: &tauri::AppHandle, hub_url: &str, current_version: &str) -> Result<Option<Self>> {
        use tauri_plugin_updater::UpdaterExt as _;

        let Some(platform) = crate::chrysalis::update::Platform::current() else {
            anyhow::bail!("this build target is not supported by the updater");
        };

        let endpoint = manifest_endpoint(hub_url, current_version, platform.key());
        let url = url::Url::parse(&endpoint)
            .with_context(|| format!("the update endpoint is not a valid URL: {endpoint}"))?;

        // ── The check must obey the same rules as every other hub call ──
        //
        // `updater_builder()` hands the plugin a plain `reqwest` client: the OS
        // certificate store only, the environment's proxy, and no bound on which
        // host it will follow. That is precisely the client `chrysalis::hub` exists
        // to replace — on an unpatched Windows machine the OS store cannot verify
        // the hub's chain, so the update check fails for a certificate that is
        // valid for months, and the update path is the one route a stranded
        // device has.
        //
        // So the builder is overridden: no proxy (the hub is dialled directly —
        // routing it through the tunnel would make the update depend on the thing
        // it repairs), and a 2xx-with-JSON is the only acceptable answer. A
        // redirect is NOT followed: on a network that filters the hub by name the
        // redirect target is usually the blocked name, and following it turns a
        // refusal we could report into an opaque timeout.
        let updater = app
            .updater_builder()
            // Replace the configured endpoint list entirely: nothing should ever
            // consult a static manifest.
            .endpoints(vec![url])
            .context("the update endpoint was rejected")?
            .configure_client(|builder| {
                builder
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .timeout(crate::chrysalis::hub::Timeout::Activation.seconds_public())
            })
            // Runs inside the plugin, immediately before it shells out to the
            // installer and exits this process. It is the plugin's only hook
            // between "accepted the bytes" and `process::exit(0)`, so it is
            // where a clean shutdown can happen *before* the process is taken
            // down under the core's feet — a proxy that is still bound and a
            // mihomo child that is still listening when the installer starts
            // replacing the program is how an update leaves a machine half in
            // one state and half in the other.
            //
            // Not the fix for the reported crash (that is the pre-exit stage
            // record and the earlier signature check), but it removes the one
            // state the reports could not rule out and costs one closure.
            .on_before_exit(|| {
                logging!(
                    info,
                    Type::System,
                    "[chrysalis] the platform installer is launching; the app is exiting now"
                );
            })
            .build()
            .context("could not build the updater")?;

        let Some(update) = updater.check().await.context("the update check failed")? else {
            return Ok(None);
        };

        // Second gate. The hub owns rollout and entitlement, but a version that
        // is not strictly newer must never be installed, whoever advertised it.
        if let Some(reason) = crate::chrysalis::update::rejection_reason(&update.version, current_version) {
            logging_rejection(&reason);
            return Ok(None);
        }

        // The plugin will refuse to install without a signature anyway, so
        // catching it here turns an opaque installer error into a clear state.
        if update.signature.trim().is_empty() {
            logging_rejection("the hub offered an update with no signature");
            return Ok(None);
        }

        // The offer's own URL is what `download_to_file` will fetch, and it must
        // be a Chrysalis hub — see `signal::OfferHosts` for why the set is the dialled
        // hub plus the compiled-in name rather than one string.
        let hosts = crate::chrysalis::update::signal::OfferHosts::for_hub(hub_url);
        if !hosts.permits(update.download_url.as_str()) {
            logging_rejection(&format!(
                "the hub offered an artifact from a host that is not the Chrysalis hub \
                 ({}): {}",
                hosts.describe(),
                update.download_url
            ));
            return Ok(None);
        }

        Ok(Some(Self { update }))
    }

    /// The version that would be installed.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.update.version
    }

    /// The SHA-256 the hub published for this artifact, if any.
    ///
    /// The plugin does not model a checksum — it verifies a minisign signature
    /// instead — but it **preserves the whole response** in `raw_json`. The hub
    /// already includes `sha256` there (`/api/update`), so the hash travels with
    /// the offer at no extra cost and without a second request.
    ///
    /// Returning `Option` rather than defaulting to `""` is deliberate:
    /// [`super::apply::download`] fails closed on an empty hash, so a hub that
    /// stopped publishing one produces a clear refusal instead of an unverified
    /// install.
    #[must_use]
    pub fn sha256(&self) -> Option<&str> {
        sha256_from_manifest(&self.update.raw_json)
    }

    /// Downloads the offered update to a verified file, reporting progress.
    ///
    /// Does **not** install; the returned [`ReadyInstall`] runs the platform
    /// installer. Splitting the two is the whole point: the download is ours
    /// (streaming, bounded, SHA-256-verified on disk) and only the install is
    /// the plugin's.
    ///
    /// # Why this delegates the download to [`super::apply`]
    ///
    /// The plugin's own `download_and_install` buffers the **entire artifact
    /// into a `Vec<u8>` in memory** before verifying it (`updater.rs`, the body
    /// of `download`). On a school laptop — the machine this product is for —
    /// a 60 MB installer held in RAM alongside a running VPN core is enough to
    /// abort the allocation, which kills the process with no log line and
    /// leaves the progress bar at 0%: exactly the reported symptom.
    ///
    /// `apply::download` streams to disk, hashes incrementally, bounds the
    /// download, and fails closed on a missing checksum — so the only bytes in
    /// memory at the end are what `install()` reads once.
    ///
    /// The plugin still owns the *install* (NSIS on Windows, AppImage/deb/rpm
    /// on Linux, `.app`/`.dmg` on macOS) and still re-verifies the minisign
    /// signature over the bytes we hand it. That is the part worth keeping:
    /// hand-rolling the swap destroyed an installation once (FIXES #22).
    ///
    /// `app` is taken here only to read the compiled-in pubkey once, for the
    /// signature check [`ReadyInstall::install`] runs before the plugin can exit
    /// the process. `check` already holds it, so it is passed in rather than
    /// carried on `PendingInstall`, which is stored without an app handle.
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails, if the SHA-256 does not match (or
    /// the hub published none), or if the artifact is implausibly small.
    pub async fn download_to_file(
        self,
        app: &tauri::AppHandle,
        staging_dir: &Path,
        sha256: &str,
        on_progress: impl FnMut(u64, Option<u64>),
    ) -> Result<ReadyInstall> {
        let offer = super::signal::UpdateOffer {
            version: self.update.version.clone(),
            url: self.update.download_url.to_string(),
            sha256: sha256.to_owned(),
            signature: self.update.signature.clone(),
        };

        let downloaded = super::apply::download(&offer, staging_dir, on_progress).await?;

        // The pubkey is read from the app's resolved config. It is the same
        // value the plugin will compare against, and reading it here rather
        // than in `install()` keeps that function testable without a runner.
        let pubkey = app
            .config()
            .plugins
            .0
            .get(UPDATER_PLUGIN_KEY)
            .and_then(|config| config.get("pubkey"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();

        Ok(ReadyInstall {
            update: self.update,
            downloaded,
            pubkey,
        })
    }
}

/// The key `plugins.<key>.pubkey` lives under in `tauri.conf.json`.
const UPDATER_PLUGIN_KEY: &str = "updater";

/// How far an install attempt got.
///
/// # Why this exists at all
///
/// A Windows device reported the update stalling below 50% and then closing the
/// program, on several devices, and three reports of the same thing could not be
/// told apart because every check this path makes is *before* the installer.
/// "It failed" and "the process was gone" were the same observation.
///
/// Each variant is written to the store **before** the next step begins, so the
/// value survives a crash that the log might not. Reading the record back on the
/// next launch answers the only question a report cannot:
///
/// | last stage recorded | what happened |
/// |---|---|
/// | *(nothing)* | died in the download, or in the allocation that reads it back |
/// | `Downloaded` | the bytes were in memory; died before the payload check below |
/// | `PayloadChecked` | the bytes are not an installer; this is refused, not a crash |
/// | `SignatureVerified` | died between verification and the hand-off, before the plugin was involved |
/// | `Launched` | **the plugin was called.** A crash from here on is the installer's, not ours |
/// | `Replaced` | the next launch saw a different executable: the update took |
/// | `NotReplaced` | the next launch saw the SAME executable after a fresh release |
///
/// The first four are written during the attempt; the last two are written by
/// the **next** launch, once it can see what the installer actually did
/// (`super::relabel`). That split is deliberate — the attempt cannot know its own
/// outcome, because on Windows the plugin takes the process down before returning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStage {
    /// The artifact was downloaded, hashed and staged.
    Downloaded,
    /// The bytes are a payload the platform installer owns.
    PayloadChecked,
    /// The minisign signature covers the bytes, checked before any hand-off.
    SignatureVerified,
    /// A **previous identity of this product** was found installed, and the
    /// hand-off was started with it named.
    ///
    /// Written *before* [`Self::Launched`], after the migration decision has been
    /// made, so a device that comes back with this as its last stage was taken
    /// down between the decision and the installer — which is the window in which
    /// the predecessor's data could be moved or not, and the reason this is its
    /// own stage rather than a log line.
    PredecessorFound,
    /// A previous identity was found **and is still running**, so the hand-off was
    /// refused before the plugin could `process::exit(0)`.
    ///
    /// Needs attention: the update did not happen and the student's device is
    /// unchanged, which is correct — but it is the state that produces "the update
    /// did nothing" reports, so it must be visible rather than log-only.
    PredecessorRunning,
    /// Handing over to the plugin, which may exit the process on Windows.
    Launched,
    /// Recorded by the **next launch**: the running executable is not the one the
    /// installer was given. The update applied.
    Replaced,
    /// Recorded by the **next launch**: the running executable is byte-for-byte
    /// what it was before the hand-off, on a version the installer was supposed
    /// to replace — and the app nevertheless reports the new version nowhere.
    ///
    /// This is the fault the report could not describe: "the update installs but
    /// the icons never change" and "the update never installed" are the same
    /// sentence from the outside, and only a comparison across the restart
    /// separates them.
    NotReplaced,
}

/// What a launch compares against, after an install was attempted.
///
/// Written **before** the hand-off by [`super::relabel::record_pre_exit`] and
/// consumed on the next launch by [`super::relabel::classify`]. Its own type
/// rather than a stringly payload so the writer and the reader cannot disagree
/// about the encoding — the class of defect that made `download_*`/`update_*`
/// silently wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreExitSnapshot {
    /// The version that was running when the installer was handed the bytes.
    pub version: String,
    /// SHA-256 of that executable.
    pub exe_sha256: String,
}

impl InstallStage {
    /// The stable string persisted in the config.
    ///
    /// Stable on purpose: a support conversation reads this value from a
    /// previous version's config, so renaming one silently empties the record.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Downloaded => "downloaded",
            Self::PayloadChecked => "payload-checked",
            Self::SignatureVerified => "signature-verified",
            Self::PredecessorFound => "predecessor-found",
            Self::PredecessorRunning => "predecessor-running",
            Self::Launched => "launched",
            Self::Replaced => "replaced",
            Self::NotReplaced => "not-replaced",
        }
    }

    /// Persists this stage, best-effort.
    ///
    /// Deliberately infallible: a store write failing must not stop an update
    /// that is otherwise fine. The value is diagnostic, and a missing record is
    /// the state we start from anyway.
    pub async fn record(self) {
        if let Err(error) = crate::chrysalis::store::record_update_install_stage(self.key()).await {
            logging!(
                warn,
                Type::System,
                "[chrysalis] could not record update stage {}: {error:#}",
                self.key()
            );
        }
    }

    /// Reads a stored stage back.
    ///
    /// An unrecognised value is `None` rather than an error: the record is
    /// diagnostic, and a config written by a future build must not make this
    /// build's status command fail.
    #[must_use]
    pub fn stored(raw: Option<&str>) -> Option<Self> {
        match raw? {
            "downloaded" => Some(Self::Downloaded),
            "payload-checked" => Some(Self::PayloadChecked),
            "signature-verified" => Some(Self::SignatureVerified),
            "predecessor-found" => Some(Self::PredecessorFound),
            "predecessor-running" => Some(Self::PredecessorRunning),
            "launched" => Some(Self::Launched),
            "replaced" => Some(Self::Replaced),
            "not-replaced" => Some(Self::NotReplaced),
            _ => None,
        }
    }

    /// One sentence for the student, naming where a previous attempt stopped.
    ///
    /// Written for the person holding the laptop, not for this repository: it
    /// says what to do next, because the stage alone is not actionable to them.
    /// The technical value is appended so a support conversation needs one
    /// copy-paste rather than a second round trip.
    #[must_use]
    pub fn interrupted_report(self) -> String {
        let what = match self {
            Self::Downloaded => {
                "the update file could not be read back after it was downloaded — the device \
                 may be low on memory or the file was removed before it could be used"
            }
            Self::PayloadChecked => "the downloaded file was not an installer for this app, so it was not run",
            Self::SignatureVerified => {
                "the installer was never started, although the update passed every check — \
                 this is the case to report to support"
            }
            Self::PredecessorFound => {
                "an older version of this app is installed on this device and the update was \
                 handing over to the installer when it stopped — start the app once more to \
                 finish, and report it to support if the older copy is still listed"
            }
            Self::PredecessorRunning => {
                "an older version of this app was running, so the update did not start. Close \
                 it and try again — if it is not in your taskbar, restart the device and update \
                 once more"
            }
            // Not reachable through `interrupted_report`'s only caller, which
            // filters this out: the plugin was called, so the process exiting is
            // the expected outcome rather than an interruption.
            Self::Launched => "the installer was launched",
            // Also unreachable here — both are *terminal* outcomes, and the one
            // that matters (`NotReplaced`) gets its own, longer sentence through
            // [`Self::not_replaced_report`] rather than being squeezed into the
            // "stopped before it finished" frame, which would be wrong: it
            // finished, and did nothing.
            Self::Replaced | Self::NotReplaced => "the last update finished",
        };
        format!(
            "The last update stopped before it finished: {what} (stage: {}).",
            self.key()
        )
    }

    /// Whether this stage describes an attempt that never completed.
    ///
    /// The single predicate every consumer uses, so "is this a fault?" is answered
    /// in one place instead of being re-derived by the status command, the Account
    /// page and any future reader — the four-way disagreement `FIXES.md` records
    /// for the offer's terminal states.
    #[must_use]
    pub const fn needs_attention(self) -> bool {
        matches!(
            self,
            Self::Downloaded
                | Self::PayloadChecked
                | Self::SignatureVerified
                | Self::PredecessorFound
                | Self::PredecessorRunning
                | Self::NotReplaced
        )
    }

    /// The sentence for the one outcome that is a silent fault: the installer ran
    /// and the application did not change.
    ///
    /// Named separately from [`Self::interrupted_report`] because the framing is
    /// different and stating it the other way would be a lie: the update did not
    /// "stop", it completed and replaced nothing. A student reading that needs to
    /// know they are still on the old build — and, because the icon never changes
    /// either, that is the explanation for the sentence they actually reported.
    #[must_use]
    pub fn not_replaced_report(was: &str, expected: &str) -> String {
        format!(
            "The update to {expected} finished, but this device is still running {was}: the \
             installer completed without replacing the application. That is why nothing changed \
             after updating, including the icon. Report this to support with the line above \
             (stage: {}).",
            Self::NotReplaced.key()
        )
    }
}

/// Whether these bytes are something the platform installer can install.
///
/// WHY THIS EXISTS
///
/// The plugin does **not** check this. `tauri_plugin_updater`'s `extract_exe`
/// treats *any* PE executable it is handed as an NSIS installer:
///
/// ```text
/// fn extract_exe(&self, bytes: &[u8]) -> Result<WindowsUpdaterType> {
///     if infer::app::is_exe(bytes) {
///         let (path, temp) = self.write_to_temp(bytes, ".exe")?;
///         Ok(WindowsUpdaterType::nsis(path, temp))
/// ```
///
/// and `install_inner` then runs it with `ShellExecuteW(.., "open", ..)` and
/// calls `std::process::exit(0)`. So an ordinary **application** binary is
/// launched as if it were a setup program.
///
/// That is exactly what this project published. `manifest.json` advertised
/// `"windows": {"file": "locus-windows-amd64.exe"}` — the raw PE executable
/// CI stages for the updater — under the key that decides what a Windows client
/// installs from itself. A client that accepted that offer did not upgrade
/// anything: it launched a copy of its own binary, standalone, from wherever the
/// installer happened to place it, and exited. The copy found no bundled web
/// assets, so Tauri resolved its start page to a `file://` path that does not
/// exist and Chromium reported:
///
/// ```text
/// File not found
/// It may have been moved, edited, or deleted.
/// ERR_FILE_NOT_FOUND
/// ```
///
/// inside a Chrysalis window, with the drive letter the *build runner* used
/// (`D:\a\Chrysalis\Chrysalis` on GitHub Actions). Nothing named Chrysalis, the install
/// directory was never touched, and it happened again on every release: the
/// client keeps accepting whatever the manifest names as newest.
///
/// # What this checks, and what it deliberately does not
///
/// This is a check on the **bytes**, because the bytes are what the plugin
/// executes. A payload is accepted only on positive evidence that the platform
/// installer owns it — an NSIS signature for a PE, or a recognised package
/// format otherwise — and refused otherwise. It does not fall back to another
/// artifact and does not "repair" the hub's choice: an update that cannot be
/// installed safely is refused with a reason the UI can show, the same posture
/// as the missing-checksum path. Refusing is recoverable; executing the wrong
/// binary at the user's privilege level is not.
///
/// The *declared filename* is the other half of the contract, and it is checked
/// where it belongs — in CI, which chooses what the manifest advertises (see
/// `check-consistency.sh` §15). Guarding here as well would mean two lists of
/// allowed names that can disagree; the magic bytes cannot drift from the
/// payload the way a name can.
#[must_use]
pub fn is_installer_payload(bytes: &[u8]) -> bool {
    // An NSIS installer is itself a PE executable, so "is a PE" cannot be the
    // test — it is true of the bare application too, which is the whole problem.
    //
    // What separates them is the NSIS *firstheader*: the little-endian magic
    // `0xDEADBEEF` (`\xef\xbe\xad\xde`) immediately followed by the ASCII
    // `NullsoftInst`. That is what makensis writes into the overlay and what a
    // bare application does not carry.
    //
    // WHY THE MAGIC IS PART OF THE SIGNATURE. An earlier revision looked for the
    // bare string `b"NullsoftInstaller"`, which is wrong in BOTH directions:
    //
    //   * A real NSIS installer does not contain it — the header is
    //     `NullsoftInst` (12 bytes) followed by a 4-byte flags word — so every
    //     genuine installer was REFUSED and no Windows client could ever update.
    //   * The raw `locus-windows-amd64.exe` (a Tauri app embedding an NSIS
    //     uninstaller stub) DOES contain the literal substring
    //     `NullsoftInstaller`, so the marker did not even exclude the thing it
    //     existed to exclude; only the scan window kept it out.
    //
    // Requiring the `0xDEADBEEF` magic immediately before `NullsoftInst` fixes
    // both: verified against the real v3.2.18 assets, the installer carries the
    // firstheader at byte 68100 and the raw app carries neither the magic nor
    // the 12-byte string.
    const NSIS_SIGNATURE: &[u8] = b"\xef\xbe\xad\xdeNullsoftInst";

    // A PE (or ELF) is accepted ONLY on positive evidence that it is an NSIS
    // installer, never on the absence of evidence that it is a program. The
    // distinction is not pedantic — it is the bug. An earlier revision of this
    // function ended with `!is_bare_executable(bytes)`, which is true of a
    // one-byte file, so `b"M"` sailed through as "an installer". A guard for
    // executing the wrong binary cannot be built out of double negatives.
    if is_bare_executable(bytes) {
        return contains(bytes, NSIS_SIGNATURE);
    }

    // Everything else is a container the platform installer owns: `.dmg`/`.app`
    // on macOS, `.deb`/`.rpm`/AppImage on Linux, MSI on Windows. Classified by
    // what it IS rather than by what it is not, so an unrecognised payload —
    // including an empty or truncated one — is refused rather than forwarded.
    is_container(bytes)
}

/// Whether the payload is an archive or package format the plugin can install.
///
/// A positive allow-list. Anything not named here is refused, which is the
/// fail-closed direction: a new artifact format means adding a line to this
/// function, not discovering that an unknown payload was executed.
fn is_container(bytes: &[u8]) -> bool {
    /// Magic numbers, longest-first within a format so a prefix cannot match
    /// the wrong entry.
    const CONTAINER_MAGICS: &[&[u8]] = &[
        b"!<arch>\n",        // `ar` archive: .deb
        b"\xed\xab\xee\xdb", // RPM
        b"\xfd7zXZ\x00",     // xz: AppImage
        b"koly",             // DMG trailer, at the END of the file
        b"\x1f\x8b",         // gzip
        b"PK\x03\x04",       // zip
        b"\xd0\xcf\x11\xe0", // MSI (OLE compound document)
    ];

    if bytes.is_empty() {
        return false;
    }

    if CONTAINER_MAGICS.iter().any(|magic| bytes.starts_with(magic)) {
        return true;
    }

    // The DMG trailer is the last 512 bytes, not the first, so a leading-bytes
    // check cannot see it. Search the tail instead of the head.
    const DMG_TRAILER: &[u8] = b"koly";
    let tail = &bytes[bytes.len().saturating_sub(512)..];
    tail.windows(DMG_TRAILER.len())
        .any(|candidate| candidate == DMG_TRAILER)
}

/// Whether the payload is a raw program rather than a container of one.
///
/// Requires the full two-byte `MZ` header, not a prefix of it: a payload short
/// enough to be ambiguous is not something to guess about, and treating `M` as a
/// PE would make a one-byte file "a bare executable" while `is_installer_payload`
/// treats it as an installer.
///
/// **Mach-O is included, and that is the macOS half of the same bug.** A raw
/// macOS binary was previously not recognised as a bare executable (`MZ` is PE
/// and `\x7fELF` is Linux), so it fell through to `is_container` — whose
/// allow-list does not name it either, meaning it was refused, but for a reason
/// that read as "unrecognised payload" rather than "this is the wrong artifact
/// for this slot". Naming it here makes the refusal say what is actually wrong:
/// the updater needs a `.app.tar.gz`, and a bare Mach-O is what the hub used to
/// (wrongly) advertise for macOS.
fn is_bare_executable(bytes: &[u8]) -> bool {
    if bytes.starts_with(b"MZ") || bytes.starts_with(b"\x7fELF") {
        return true;
    }
    // Mach-O: thin (both endiannesses) and fat/universal.
    matches!(
        bytes.get(..4),
        Some(b"\xcf\xfa\xed\xfe" | b"\xce\xfa\xed\xfe" | b"\xfe\xed\xfa\xcf" | b"\xfe\xed\xfa\xce")
    ) || matches!(bytes.get(..4), Some(b"\xca\xfe\xba\xbe" | b"\xbe\xba\xfe\xca"))
}

/// A substring search over the artifact's leading bytes.
///
/// The NSIS header sits near the start of the overlay, not at a fixed offset, so
/// a scan is the honest way to look for it. Bounded to the first window because
/// a full scan of a 54 MB installer to find a 17-byte marker at the front is
/// work for nothing.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    let window = &haystack[..haystack.len().min(NSIS_SCAN_LIMIT)];
    window.windows(needle.len()).any(|candidate| candidate == needle)
}

/// How far into an artifact the NSIS header is looked for.
const NSIS_SCAN_LIMIT: usize = 4 * 1024 * 1024;

/// A downloaded, SHA-256-verified artifact and the plugin handle that installs it.
///
/// This is the hand-off point: the bytes on disk were verified by
/// [`super::apply::download`], and the plugin will verify the minisign signature
/// over them a second time before the platform installer runs.
pub struct ReadyInstall {
    update: tauri_plugin_updater::Update,
    downloaded: super::apply::DownloadedUpdate,
    /// The `plugins.updater.pubkey` this build was compiled with, carried so
    /// [`Self::install`] can verify without a second Tauri handle.
    pubkey: String,
}

impl ReadyInstall {
    /// The version that is about to be installed.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.downloaded.version
    }

    /// The verified artifact on disk.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.downloaded.path
    }

    /// Runs the platform installer over the verified bytes.
    ///
    /// On Windows this launches NSIS and exits the app, so a successful return
    /// may never happen — callers must treat completion as "expect a restart".
    ///
    /// # The order below is the fix for a reported crash, and it is ordered
    ///
    /// A Windows beta reported the update *stalling below 50% and then taking
    /// the whole program down*, on several devices, and the log could not say
    /// why — every check this path performs sits **before** the installer, and
    /// each failure is a `Result`. So the interesting question was not "did it
    /// fail" but "how far did it get", and nothing recorded that.
    ///
    /// The steps are therefore each their own persisted decision, in the order
    /// they run, so the last line in the log locates the stall:
    ///
    /// 1. **read** the staged bytes back (a missing file is a readable error,
    ///    not a killed process);
    /// 2. **shape** — is this an installer at all (`is_installer_payload`);
    /// 3. **signature** — [`super::verify`] runs the plugin's own decode chain
    ///    here, streamed from disk, *before* `install()`. Inside `install()` a
    ///    refusal and a successful launch are both "the process exited", and
    ///    neither is logged;
    /// 4. **hand-off** — last line written before the plugin can call
    ///    `process::exit(0)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the artifact cannot be read back, if it is not an
    /// installer, if its signature does not cover it, or if the installer
    /// refuses to run.
    pub async fn install(self) -> Result<()> {
        let version = self.downloaded.version.clone();

        let bytes = match self.downloaded.read().with_context(|| {
            format!(
                "could not read the downloaded update at {}",
                self.downloaded.path.display()
            )
        }) {
            Ok(bytes) => bytes,
            Err(error) => {
                let detail = format!("{error:#}");
                logging!(error, Type::System, "[chrysalis] update {version}: {detail}");
                return Err(error);
            }
        };
        logging!(
            info,
            Type::System,
            "[chrysalis] update {version}: read {} bytes back from staging",
            bytes.len()
        );
        // Reached only once the bytes are in memory, which is the whole point of
        // this being its own stage: `read()` allocates the entire artifact, and
        // a device that comes back up with NO stage recorded died in that
        // allocation rather than in any of the checks below. Recording it before
        // the read would make the two states indistinguishable — the stage would
        // be present whether the read survived or not.
        InstallStage::Downloaded.record().await;

        if !is_installer_payload(&bytes) {
            // Logged as well as returned: the UI has one line to show, and the
            // diagnosis needs the shape of the payload and where it came from.
            let detail = format!(
                "the update to {version} is not an installer ({} bytes); refusing to execute it",
                bytes.len()
            );
            logging!(error, Type::System, "[chrysalis] {detail}");
            bail!("{detail} — this build cannot install a raw executable, and neither can the platform installer");
        }
        logging!(
            info,
            Type::System,
            "[chrysalis] update {version}: payload is an installer"
        );
        InstallStage::PayloadChecked.record().await;

        // Before the plugin can take the process down. A refusal here is
        // returned to the student; the same refusal inside `install()` is a
        // window that disappears with no line in any log.
        if let Err(error) =
            super::verify::require_valid(&self.downloaded.path, &self.downloaded.signature, &self.pubkey)
        {
            logging!(
                error,
                Type::System,
                "[chrysalis] update {version}: signature check failed: {error:#}"
            );
            bail!("the update to {version} is not signed for this build: {error:#}");
        }
        logging!(
            info,
            Type::System,
            "[chrysalis] update {version}: signature verifies against the compiled-in key"
        );
        InstallStage::SignatureVerified.record().await;

        // ── A PREVIOUS IDENTITY OF THIS PRODUCT ──
        //
        // This is the last point at which anything can look. On Windows the
        // plugin calls `ShellExecuteW` and then `process::exit(0)`
        // (`tauri-plugin-updater` 2.11.0, `updater.rs:876`), so the installer
        // takes over from a process that is already gone; the hook on the far
        // side (`packages/windows/migrate-predecessor.nsh`) cannot report back.
        //
        // WHY THIS IS NOT A REFUSAL OF THE UPDATE
        //
        // An older identity being installed is not a reason to leave a student on
        // an old build — the update is precisely how they get a build that can
        // migrate. It IS a reason not to hand over while that older copy is
        // RUNNING: the installer's own `CheckIfAppIsRunning` is keyed on the new
        // product's executable name, so a live predecessor is invisible to it, and
        // the result is an uninstaller racing a running application.
        //
        // Detection is `predecessor::observe()`, which returns `Unknown` rather
        // than `Absent` when it could not look — and `Unknown` is logged and then
        // treated as "nothing found" here **deliberately**: blocking an update on a
        // failed probe would strand exactly the devices that most need it (FIXES
        // #42's deadlock, one layer down). The asymmetry is stated so the next
        // reader does not "fix" it into a refusal.
        match super::predecessor::classify(super::predecessor::observe()) {
            Ok(Some(predecessor)) => {
                InstallStage::PredecessorFound.record().await;
                logging!(
                    info,
                    Type::System,
                    "[chrysalis] update {version}: found a previous identity installed — {}",
                    predecessor.describe()
                );
                if predecessor.blocks_handover() {
                    InstallStage::PredecessorRunning.record().await;
                    logging!(
                        warn,
                        Type::System,
                        "[chrysalis] update {version}: {} is RUNNING; refusing to hand over, \
                         because the platform installer cannot see it and would race it",
                        predecessor.identity.product_name
                    );
                    bail!(
                        "{} is still running, so the update was not started. Close it and try \
                         again — the installer cannot replace it while it is open.",
                        predecessor.identity.product_name
                    );
                }
            }
            Ok(None) => {}
            Err(why) => logging!(
                debug,
                Type::System,
                "[chrysalis] update {version}: could not check for a previous installation \
                 ({why}); continuing, because refusing an update on a failed probe would \
                 strand the devices that need it most"
            ),
        }

        logging!(
            info,
            Type::System,
            "[chrysalis] update {version}: handing {version} to the platform installer"
        );

        // ── The comparison the next launch will make, recorded before we lose the
        //    chance to make it ──
        //
        // Deliberately here, and not after `install()` below: on Windows the plugin
        // `ShellExecuteW`s the setup and calls `process::exit(0)` **whether or not
        // the setup replaced anything** (`tauri-plugin-updater` 2.11.0,
        // `updater.rs:876`). There is no `Err` for a declined elevation and no
        // value returned for a `/UPDATE` that chose not to overwrite, so every
        // branch after that call is unreachable on the platform where the reported
        // symptom ("updating changes nothing, including the icon") actually
        // happens.
        //
        // So the *before* state is written here, next to the `Launched` stage and
        // for the same reason, and the *after* state is read by the next launch.
        // Best-effort: failing to fingerprint must not refuse an update that is
        // otherwise fine, and a missing record reads as `Unknown` rather than as a
        // success (`super::relabel`).
        match super::relabel::record_pre_exit(&version, &current_executable()).await {
            Ok(snapshot) => logging!(
                debug,
                Type::System,
                "[chrysalis] update {version}: recorded the pre-install fingerprint of {}",
                snapshot.version
            ),
            Err(error) => logging!(
                warn,
                Type::System,
                "[chrysalis] update {version}: could not record the pre-install fingerprint \
                 ({error:#}); a failed install will not be detectable on the next launch"
            ),
        }

        // The LAST thing written before the plugin may call `process::exit(0)`.
        // A device that comes back up with this as its last recorded stage got
        // all the way through our side, which moves the defect to the installer.
        InstallStage::Launched.record().await;

        self.update.install(bytes).context("the update could not be installed")
    }
}

/// The path of the running executable, or one that cannot be read.
///
/// Returning the empty path on failure rather than `Option` keeps the call site
/// readable, and `fingerprint` turns it into an error the caller logs — which is
/// the honest outcome: no snapshot, and the next launch says `Unknown` rather than
/// claiming a successful install.
fn current_executable() -> std::path::PathBuf {
    std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::new())
}

/// Extracts the hub's checksum from a manifest the plugin preserved.
///
/// Pulled out as a free function so it can be tested without constructing a
/// `tauri_plugin_updater::Update` (whose fields are private, so a test cannot
/// build one). All the logic lives here; the accessor is a one-line call.
///
/// Returns `None` — never an empty string — for a missing, non-string, or
/// whitespace-only value, so the caller's fail-closed path is reached rather
/// than an "empty hash" silently matching.
fn sha256_from_manifest(manifest: &serde_json::Value) -> Option<&str> {
    manifest
        .get("sha256")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// Records why an offered update was not taken.
///
/// A decision log at every branch is the client-side defence against the
/// retired client's worst failure mode: an update advertised and silently
/// ignored, with nothing anywhere saying so.
fn logging_rejection(reason: &str) {
    clash_verge_logging::logging!(
        warn,
        clash_verge_logging::Type::System,
        "[chrysalis] ignoring the offered update: {reason}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The endpoint must carry the version and platform, or the hub cannot
    /// answer for the right client.
    #[test]
    fn the_endpoint_carries_version_and_platform() {
        let url = manifest_endpoint("https://hub.example.org", "2.2.0", "windows");
        assert!(url.starts_with("https://hub.example.org/api/update?"));
        assert!(url.contains("version=2.2.0"));
        assert!(url.contains("platform=windows"));
    }

    /// A trailing slash must not produce a doubled separator.
    #[test]
    fn a_trailing_slash_is_tolerated() {
        let with = manifest_endpoint("https://hub.example.org/", "2.2.0", "linux");
        let without = manifest_endpoint("https://hub.example.org", "2.2.0", "linux");
        assert_eq!(with, without);
        assert!(!with.contains("//api"), "the path must not be doubled: {with}");
    }

    /// A pre-release version contains characters that must survive encoding
    /// intact, or the hub sees a different version than the client is running.
    #[test]
    fn prerelease_versions_survive_encoding() {
        let url = manifest_endpoint("https://hub.example.org", "2.3.0-rc1+build5", "linux");
        assert!(
            url.contains("2.3.0-rc1%2Bbuild5"),
            "the + in build metadata must be encoded, got {url}"
        );
    }

    #[test]
    fn encoding_passes_through_safe_characters() {
        assert_eq!(encode("2.2.0-rc1"), "2.2.0-rc1");
        assert_eq!(encode("windows"), "windows");
        assert_eq!(encode("macos_arm"), "macos_arm");
    }

    #[test]
    fn encoding_escapes_unsafe_characters() {
        assert_eq!(encode("a+b"), "a%2Bb");
        assert_eq!(encode("a b"), "a%20b");
        assert_eq!(encode("a&b"), "a%26b");
        assert_eq!(encode("a/b"), "a%2Fb");
        assert_eq!(encode("a?b"), "a%3Fb");
    }

    /// A version from the hub could contain a query-breaking character; it must
    /// not be able to alter the request.
    #[test]
    fn an_injected_version_cannot_add_query_parameters() {
        let url = manifest_endpoint("https://hub.example.org", "1.0.0&admin=1", "linux");
        assert!(!url.contains("&admin=1"), "an injected & must be encoded, got {url}");
        assert!(url.contains("%261.0.0") || url.contains("%26admin"));
    }

    /// The hub's `/api/update` response carries `sha256`, and the plugin keeps
    /// the whole response in `raw_json`. This is where the install path gets the
    /// hash it verifies the download against.
    #[test]
    fn the_checksum_is_read_from_the_hub_manifest() {
        let manifest = serde_json::json!({
            "version": "3.2.8",
            "url": "https://hub.example.org/updates/3.2.8/locus-linux-amd64",
            "signature": "untrusted comment: signature\nRWQ...",
            "sha256": "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
        });
        assert_eq!(
            sha256_from_manifest(&manifest),
            Some("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2")
        );
    }

    /// Surrounding whitespace must be tolerated — a hub field written by a
    /// person could carry it, and refusing a correct hash over a stray newline
    /// would be an infuriating bug.
    #[test]
    fn a_checksum_with_surrounding_whitespace_is_trimmed() {
        let manifest = serde_json::json!({ "sha256": "  a1b2c3  " });
        assert_eq!(sha256_from_manifest(&manifest), Some("a1b2c3"));
    }

    /// A manifest with no checksum must yield `None`, not `""`.
    ///
    /// This is the difference between failing closed and installing unverified
    /// bytes: `apply::download` refuses an empty hash, so returning `Some("")`
    /// would be indistinguishable from a real hash to a careless caller.
    #[test]
    fn a_missing_checksum_yields_none_not_an_empty_string() {
        assert_eq!(sha256_from_manifest(&serde_json::json!({ "version": "3.2.8" })), None);
        assert_eq!(sha256_from_manifest(&serde_json::json!({ "sha256": "" })), None);
        assert_eq!(sha256_from_manifest(&serde_json::json!({ "sha256": "   " })), None);
    }

    /// A non-string checksum (a number, an object, a null) must not be coerced
    /// into one — it is a malformed manifest, not a hash.
    #[test]
    fn a_non_string_checksum_yields_none() {
        assert_eq!(sha256_from_manifest(&serde_json::json!({ "sha256": 12345 })), None);
        assert_eq!(sha256_from_manifest(&serde_json::json!({ "sha256": null })), None);
        assert_eq!(
            sha256_from_manifest(&serde_json::json!({ "sha256": { "h": "1" } })),
            None
        );
    }

    // ── The payload guard ──
    //
    // These pin the check that stops a *program* being executed as an
    // *installer*. The bug they exist for is not hypothetical: `manifest.json`
    // advertised the bare `locus-windows-amd64.exe` as the artifact a Windows
    // client installs from itself, and the plugin runs any PE it is handed.

    /// A minimal but structurally honest NSIS installer.
    ///
    /// Built as a real PE (`MZ`) with the NSIS *firstheader* in the overlay —
    /// the `0xDEADBEEF` magic immediately followed by `NullsoftInst` — because
    /// the guard's whole discriminator is "PE **and** NSIS firstheader". A
    /// fixture that emitted only one of the two would pass for the wrong reason,
    /// which is exactly how the previous revision's tests stayed green while
    /// the guard rejected every real installer.
    fn nsis_installer(overlay_padding: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(overlay_padding + 64);
        bytes.extend_from_slice(b"MZ\x90\x00");
        bytes.resize(overlay_padding, 0);
        // The real header: magic, then the 12-byte string, then the flags word.
        // Transcribed from a genuine installer, not from the constant under
        // test — that is the difference between a fixture and a tautology.
        bytes.extend_from_slice(b"\xef\xbe\xad\xde");
        bytes.extend_from_slice(b"NullsoftInst@O\x01\x00");
        bytes
    }

    /// A minimal but structurally honest application binary: a PE, with no NSIS
    /// overlay. This is what `Stage the raw updater artifact (windows)` produces.
    fn bare_application() -> Vec<u8> {
        let mut bytes = Vec::with_capacity(64 * 1024);
        bytes.extend_from_slice(b"MZ\x90\x00");
        bytes.resize(64 * 1024, 0x00);
        bytes
    }

    /// THE BUG, in its real shape. The shipped `locus-windows-amd64.exe` is a
    /// Tauri app that embeds an NSIS *uninstaller* stub, so it contains the
    /// literal substring `NullsoftInstaller` well inside the file — but never
    /// the `0xDEADBEEF` magic before `NullsoftInst`. It must be refused.
    ///
    /// This is the case the old fixture missed entirely: `bare_application` had
    /// no `Nullsoft*` bytes at all, so it could not tell whether the guard
    /// rejected the payload for the right reason or merely for being featureless.
    #[test]
    fn the_raw_windows_binary_is_not_an_installer() {
        // The raw-app shape: a PE carrying an unrelated `NullsoftInstaller`
        // substring (as the real artifact does, from its embedded uninstaller
        // stub), but no firstheader magic.
        let mut raw_app = bare_application();
        raw_app.extend_from_slice(b"...\\NSIS\\NullsoftInstaller.exe...\x00");

        assert!(
            !is_installer_payload(&raw_app),
            "the raw updater executable must never be accepted as an installer — \
             the plugin would ShellExecute it, it would run standalone, find no \
             bundled assets, and show ERR_FILE_NOT_FOUND"
        );
        assert!(
            !is_installer_payload(&bare_application()),
            "a featureless PE must also be refused"
        );
    }

    /// The positive control: a real NSIS installer must be accepted, or this
    /// guard would block every legitimate Windows update instead of one bug.
    #[test]
    fn an_nsis_installer_is_an_installer() {
        assert!(
            is_installer_payload(&nsis_installer(1024)),
            "a genuine NSIS installer must be accepted"
        );
        // The signature sits in the overlay rather than at a fixed offset, so a
        // later position must still be found.
        assert!(
            is_installer_payload(&nsis_installer(512 * 1024)),
            "the NSIS header is not always at the same offset"
        );
    }

    /// THE INVERSE FAILURE, and the one that actually shipped: a guard can be
    /// too strict. Before 2026-10-02 the constant was the bare string
    /// `NullsoftInstaller`, which no real installer contains — so every genuine
    /// Windows update was refused, and the message a student saw blamed the
    /// payload ("not an installer") rather than the check.
    ///
    /// The fixture above is generated from the same header bytes the guard looks
    /// for, so it cannot catch that. This one is transcribed from the **real**
    /// v3.2.18 artifact the hub serves: a PE whose firstheader sits at byte
    /// 68100, with the 4-byte flags word (`@O\x01\x00`) immediately after
    /// `NullsoftInst` and no `NullsoftInstaller` anywhere in the file.
    ///
    /// It is the shape, not the whole 58 MB: header at a non-constant offset,
    /// flags word present, `N`-prefix present. Restoring the old constant makes
    /// this test fail, which is the property that gives it value.
    #[test]
    fn the_real_shipped_installer_is_an_installer() {
        // Byte-for-byte from `installer-Chrysalis_3.2.18_x64-setup.exe` at the
        // firstheader (offset 68100): de ad be ef "NullsoftInst" 40 4f 01 00.
        let installers_header = b"\xef\xbe\xad\xdeNullsoftInst@O\x01\x00";
        let mut real = Vec::with_capacity(69 * 1024);
        real.extend_from_slice(b"MZ\x90\x00");
        real.resize(68_100, 0x00); // the header does not sit at a round offset
        real.extend_from_slice(installers_header);
        real.resize(90 * 1024, 0x00); // overlay continues past the header

        assert!(
            !real
                .windows(b"NullsoftInstaller".len())
                .any(|w| w == b"NullsoftInstaller"),
            "the real installer must not contain `NullsoftInstaller` — that \
             absence is exactly what the old constant tripped on"
        );
        assert!(
            is_installer_payload(&real),
            "the installer the hub actually serves was refused — the signature \
             constant is too strict, and no Windows client can update"
        );
    }

    /// A bare Linux executable is the same bug with a different magic number.
    #[test]
    fn a_bare_linux_binary_is_not_an_installer() {
        let mut elf = b"\x7fELF".to_vec();
        elf.resize(4096, 0);
        assert!(!is_installer_payload(&elf), "a bare ELF is not an installer");
    }

    /// An empty or truncated download is not an installer either. It cannot be,
    /// and saying so here keeps the guard total rather than panicking on an
    /// empty slice.
    #[test]
    fn an_empty_or_truncated_payload_is_not_an_installer() {
        assert!(!is_installer_payload(b""));
        assert!(!is_installer_payload(b"M"));
        assert!(!is_installer_payload(b"MZ"));
    }

    /// A macOS `.dmg` or a Linux package is a container, so the guard must not
    /// veto it. This is what keeps the check from being a Windows-only rule that
    /// accidentally breaks the other two platforms.
    #[test]
    fn archives_and_packages_are_left_to_the_plugin() {
        // `koly` is the DMG trailer: at the END of the file, not the start, so a
        // leading-magic check alone would miss every real `.dmg`.
        let mut dmg = vec![0u8; 4096];
        dmg.extend_from_slice(b"koly");
        assert!(is_installer_payload(&dmg), "a .dmg is a container, not a program");

        // The package formats the Linux and Windows bundles actually produce.
        assert!(is_installer_payload(b"!<arch>\ndebian-binary"), "a .deb");
        assert!(is_installer_payload(b"\xed\xab\xee\xdb\x03\x00"), "an .rpm");
        assert!(is_installer_payload(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1"), "an .msi");
    }

    /// The macOS updater payload is a `.app.tar.gz`, and it must be ACCEPTED.
    ///
    /// This is the artifact kind the hub now publishes for macOS. Before this
    /// existed the hub published a bare Mach-O, which could not install —
    /// `tauri_plugin_updater` extracts a tar of an `.app` bundle on macOS, so
    /// the download verified and then failed.
    #[test]
    fn the_macos_app_tarball_is_an_installer_payload() {
        let mut tarball = b"\x1f\x8b\x08\x00".to_vec();
        tarball.extend_from_slice(&[0u8; 256]);
        assert!(
            is_installer_payload(&tarball),
            "a .app.tar.gz is a gzip container the plugin extracts on macOS"
        );
    }

    /// A bare Mach-O must be REFUSED, and named as the wrong artifact kind.
    ///
    /// This is the macOS form of the Windows bug the NSIS check exists for: the
    /// hub used to advertise `locus-darwin-arm64` (a raw Mach-O) in the macOS
    /// update slot. It is not installable there. It must not pass as a
    /// "container" merely because it is not a PE or an ELF.
    #[test]
    fn a_bare_macho_is_not_an_installer_payload() {
        // Thin Mach-O, little-endian 64-bit arm64 header (magic + cputype).
        let mut macos_binary = b"\xcf\xfa\xed\xfe".to_vec();
        macos_binary.extend_from_slice(&[0x0c, 0x00, 0x00, 0x01]); // CPU_TYPE_ARM64
        macos_binary.extend_from_slice(&[0u8; 512]);
        assert!(
            !is_installer_payload(&macos_binary),
            "a bare Mach-O is the application, not a package the plugin can install"
        );

        // Fat/universal header, same conclusion.
        let mut fat = b"\xca\xfe\xba\xbe".to_vec();
        fat.extend_from_slice(&[0u8; 512]);
        assert!(
            !is_installer_payload(&fat),
            "a universal Mach-O is still a bare application"
        );
    }

    /// An unrecognised payload is refused, not forwarded.
    ///
    /// This is the fail-closed direction: the guard classifies by what a payload
    /// IS, so something it cannot name never reaches the platform installer.
    #[test]
    fn an_unrecognised_payload_is_refused() {
        assert!(
            !is_installer_payload(b"#!/bin/sh\nrm -rf /\n"),
            "an unrecognised payload must not be treated as installable"
        );
        assert!(!is_installer_payload(&[0xffu8; 128]), "nor must random bytes");
    }

    /// The scan must be bounded. A multi-hundred-megabyte artifact that happens
    /// to contain the marker late must not be walked in full on the startup
    /// path — and, more importantly, a marker beyond the window must not be
    /// treated as found.
    #[test]
    fn the_signature_scan_is_bounded() {
        let mut far = bare_application();
        far.resize(NSIS_SCAN_LIMIT + 1024, 0x00);
        far.extend_from_slice(b"\xef\xbe\xad\xde");
        far.extend_from_slice(b"NullsoftInst@O\x01\x00");
        assert!(
            !is_installer_payload(&far),
            "a marker beyond the scan window must not count as an installer"
        );
    }

    /// Every stage that is written must read back as itself.
    ///
    /// The stage value is the only thing a crashed install leaves behind, and it
    /// is read by `chrysalis_update_status` from a config written by a *previous*
    /// run. A key that is written but not decoded is a record that silently
    /// reads as "no attempt", which is the same as the defect this replaced:
    /// a crash that says nothing.
    ///
    /// This is a round trip rather than a list comparison on purpose — it fails
    /// if `key()` and `stored()` drift apart in EITHER direction.
    #[test]
    fn every_stage_reads_back_as_itself() {
        for stage in [
            InstallStage::Downloaded,
            InstallStage::PayloadChecked,
            InstallStage::SignatureVerified,
            InstallStage::Launched,
            // Added with the relabel work. They are written by a *later* launch
            // than the one that wrote the four above, which is exactly the case a
            // round trip has to cover: the reader is a different process.
            InstallStage::Replaced,
            InstallStage::NotReplaced,
        ] {
            assert_eq!(
                InstallStage::stored(Some(stage.key())),
                Some(stage),
                "{} is written but does not decode — a crashed install would read as no attempt",
                stage.key()
            );
        }
    }

    /// A stage from a future build must read as "nothing to report", not panic
    /// and not guess. Guessing is worse than silence here: the wrong stage sends
    /// a student and support after the wrong failure.
    #[test]
    fn an_unknown_stage_is_none() {
        assert_eq!(InstallStage::stored(Some("something-from-a-newer-build")), None);
        assert_eq!(InstallStage::stored(None), None);
    }

    /// `Launched` must NOT be reported as an interruption.
    ///
    /// The plugin was called, which is why the process is gone — that is a
    /// successful hand-off, and calling it a failure would label every working
    /// update as broken. This pins the filter that `chrysalis_update_status`
    /// applies, at the level below it, so the two cannot drift.
    #[test]
    fn a_launched_install_is_not_an_interruption() {
        for stage in [
            InstallStage::Downloaded,
            InstallStage::PayloadChecked,
            InstallStage::SignatureVerified,
        ] {
            assert_ne!(
                stage,
                InstallStage::Launched,
                "these must be filtered out of `interrupted_update`, or every finished \
                 update is reported as interrupted"
            );
            assert!(
                stage.interrupted_report().contains(stage.key()),
                "the report must name the stage so support has a value to search for"
            );
        }
    }

    /// Exactly the stages that mean "something needs the student's attention",
    /// and nothing else.
    ///
    /// This predicate is the one `chrysalis_update_status` filters on, so a stage
    /// landing on the wrong side is either a silent fault (`NotReplaced` not
    /// reported) or a permanent false alarm (`Replaced` reported on a healthy
    /// device). Both directions are asserted, because only one of them shows up
    /// in testing: a wrong *report* is visible, a wrong *silence* is not.
    ///
    /// Demonstrated failing by adding `Self::Replaced` to the `matches!` list —
    /// the healthy outcome then renders a warning beside every successful update.
    #[test]
    fn only_real_faults_need_attention() {
        let faults = [
            InstallStage::Downloaded,
            InstallStage::PayloadChecked,
            InstallStage::SignatureVerified,
            InstallStage::NotReplaced,
            // Both predecessor states are faults, and for the same practical
            // reason: each is a device that asked to update and did not, on a path
            // whose far side is a `process::exit(0)` and therefore silent.
            // `PredecessorRunning` is the recoverable half — the student closes
            // the app and retries — and still must show, because "the update did
            // nothing" with no sentence is the exact report this file exists to
            // prevent.
            InstallStage::PredecessorFound,
            InstallStage::PredecessorRunning,
        ];
        for stage in faults {
            assert!(stage.needs_attention(), "{stage:?} must be reported");
        }

        let clean = [InstallStage::Launched, InstallStage::Replaced];
        for stage in clean {
            assert!(
                !stage.needs_attention(),
                "{stage:?} must NOT be reported — it is a completed hand-off or a \
                 confirmed install, and reporting it makes every update look broken"
            );
        }
    }

    /// The "installed and changed nothing" sentence says what a student needs and
    /// does not dress it as a crash.
    ///
    /// Demonstrated failing by routing `NotReplaced` through
    /// `interrupted_report`, whose wording is "stopped before it finished" — the
    /// opposite of what happened, on the one outcome where the difference decides
    /// whether a student thinks they are up to date.
    #[test]
    fn the_not_replaced_sentence_explains_the_icon() {
        let text = InstallStage::not_replaced_report("3.4.4", "3.4.5");
        assert!(
            text.contains("still running 3.4.4"),
            "must name what is running: {text}"
        );
        assert!(text.contains("3.4.5"), "must name what was expected: {text}");
        assert!(
            text.to_lowercase().contains("icon"),
            "the reported symptom is the icon, so the sentence has to connect them: {text}"
        );
        assert!(
            !text.contains("stopped before it finished"),
            "it finished; saying otherwise sends the reader after the wrong fault: {text}"
        );
    }
}
