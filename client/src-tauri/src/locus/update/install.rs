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

use anyhow::{Context as _, Result};
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
    pub fn install(self) -> Result<()> {
        let bytes = self
            .downloaded
            .read()
            .context("could not read the verified update for installation")?;
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
}
