//! Driving the plugin's installer from Locus update metadata.
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

        let Some(platform) = crate::locus::update::Platform::current() else {
            anyhow::bail!("this build target is not supported by the updater");
        };

        let endpoint = manifest_endpoint(hub_url, current_version, platform.key());
        let url = url::Url::parse(&endpoint)
            .with_context(|| format!("the update endpoint is not a valid URL: {endpoint}"))?;

        let updater = app
            .updater_builder()
            // Replace the configured endpoint list entirely: nothing should ever
            // consult a static manifest.
            .endpoints(vec![url])
            .context("the update endpoint was rejected")?
            .build()
            .context("could not build the updater")?;

        let Some(update) = updater.check().await.context("the update check failed")? else {
            return Ok(None);
        };

        // Second gate. The hub owns rollout and entitlement, but a version that
        // is not strictly newer must never be installed, whoever advertised it.
        if let Some(reason) = crate::locus::update::rejection_reason(&update.version, current_version) {
            logging_rejection(&reason);
            return Ok(None);
        }

        // The plugin will refuse to install without a signature anyway, so
        // catching it here turns an opaque installer error into a clear state.
        if update.signature.trim().is_empty() {
            logging_rejection("the hub offered an update with no signature");
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
    /// # Errors
    ///
    /// Returns an error if the download fails, if the SHA-256 does not match (or
    /// the hub published none), or if the artifact is implausibly small.
    pub async fn download_to_file(
        self,
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
        Ok(ReadyInstall {
            update: self.update,
            downloaded,
        })
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
/// inside a Locus window, with the drive letter the *build runner* used
/// (`D:\a\Locus\Locus` on GitHub Actions). Nothing named Locus, the install
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
        b"!<arch>\n", // `ar` archive: .deb
        b"\xed\xab\xee\xdb", // RPM
        b"\xfd7zXZ\x00", // xz: AppImage
        b"koly", // DMG trailer, at the END of the file
        b"\x1f\x8b", // gzip
        b"PK\x03\x04", // zip
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
    window
        .windows(needle.len())
        .any(|candidate| candidate == needle)
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
    /// # Errors
    ///
    /// Returns an error if the artifact cannot be read back, if the plugin's
    /// minisign verification rejects it, or if the installer refuses to run.
    /// # Errors
    ///
    /// Returns [`NotAnInstaller`] if the bytes are a bare executable rather
    /// than an installer. This runs **before** the plugin sees the bytes, so a
    /// payload it would happily execute as a program never reaches it.
    pub fn install(self) -> Result<()> {
        let bytes = self
            .downloaded
            .read()
            .context("could not read the verified update for installation")?;

        if !is_installer_payload(&bytes) {
            // Logged as well as returned: the UI has one line to show, and the
            // diagnosis needs the shape of the payload and where it came from.
            let detail = format!(
                "the update to {} is not an installer ({} bytes); refusing to execute it",
                self.downloaded.version,
                bytes.len()
            );
            logging!(error, Type::System, "[locus] {detail}");
            bail!(
                "{detail} — this build cannot install a raw executable, and neither can the platform installer"
            );
        }

        self.update
            .install(bytes)
            .context("the update could not be installed")
    }
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
        "[locus] ignoring the offered update: {reason}"
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
        assert_eq!(sha256_from_manifest(&serde_json::json!({ "sha256": { "h": "1" } })), None);
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
        // Byte-for-byte from `installer-Locus_3.2.18_x64-setup.exe` at the
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
        assert!(
            is_installer_payload(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1"),
            "an .msi"
        );
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
}
