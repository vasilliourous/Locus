use crate::core::{CoreManager, handle, manager::RunningMode};
use anyhow::Result;
use async_trait::async_trait;
use clash_verge_logging::{Type, logging};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::Manager as _;

/// Chrysalis's application identity. This is the app-data directory name, and it
/// is also what the privileged Service uses to recognise this app: on Windows the
/// owner SID of this directory plus the token file inside it are the
/// credentials the Service checks. See `core::owner_identity`.
///
/// **MOVED FROM `com.locus.client` ON 2026-11-25, and the move is safe for one
/// specific reason: this value is *read*, never compiled into anything that
/// outlives the install.** The update path does not depend on it — the updater is
/// handed a URL by the hub and verifies the bytes against the compiled-in
/// minisign pubkey, neither of which names the identifier (`/api/update` emits no
/// identifier field at all). What it DOES decide is where the activation code,
/// device fingerprint and tier live, which is why the previous value had to stay
/// adoptable: [`LEGACY_APP_IDS`] carries it, and
/// [`migrate_legacy_app_data_dir`] renames the old root on first launch.
///
/// **Ordering is load-bearing.** The adoption runs in `lib::run` *before* the
/// Windows owner repair, the singleton lock and the logger, because all three
/// resolve paths through this root. See the comment there.
#[cfg(not(feature = "verge-dev"))]
pub static APP_ID: &str = "com.chrysalis.client";
/// **DEPRECATED ALIAS — `locus-backup` is retained on purpose, and it is NOT the
/// same kind of name as [`APP_ID`].**
///
/// [`APP_ID`] is a directory *this app* creates and reads on the local disk, so
/// [`migrate_legacy_app_data_dir`] renames it on first launch and the move is
/// invisible. This constant is not that: it is the remote collection name used in
/// WebDAV paths (`{BACKUP_DIR}/{file}` in `core::backup`), so it names a directory
/// on a **server this product does not control**.
///
/// Renaming it would not migrate anything — it would point at a collection that
/// does not exist, so `mkcol` creates an empty one and every backup a user has
/// already uploaded becomes invisible to the app while remaining in their
/// storage. That is indistinguishable from data loss, and there is no migration
/// path short of enumerating and moving every user's remote files.
///
/// It appears in no user-facing string — the product is `Chrysalis` everywhere a
/// person sees it — so a rename buys nothing but the risk above. Retires when a
/// deliberate backup-location migration ships, with the remote move as part of
/// it, never before.
#[cfg(not(feature = "verge-dev"))]
pub static BACKUP_DIR: &str = "locus-backup";

#[cfg(feature = "verge-dev")]
pub static APP_ID: &str = "com.chrysalis.client.dev";
/// The dev channel keeps its own remote collection so a dev build cannot write
/// into a release user's backup directory. Same retention reason as the release
/// value above.
#[cfg(feature = "verge-dev")]
pub static BACKUP_DIR: &str = "locus-backup-dev";

/// Application identities this product used BEFORE it was Chrysalis.
///
/// Kept so one launch can adopt an existing data directory rather than
/// abandoning it — see [`migrate_legacy_app_data_dir`]. These are historical
/// literals: nothing writes them any more, and they must not be "tidied" into
/// the current [`APP_ID`], because then the migration would have nothing to
/// look for.
///
/// **ORDER MATTERS, and the newest first is not a style choice.** The loop in
/// [`adopt_legacy_app_data_dir`] takes the FIRST id that exists, so
/// `com.locus.client` (the fleet's data root until 2026-11-25) must be checked
/// before the Clash Verge Rev one it was itself adopted from. Reversing these
/// would make a machine that never adopted the upstream directory — which is
/// most of them — adopt the wrong one if both are present.
#[cfg(not(feature = "verge-dev"))]
pub(crate) const LEGACY_APP_IDS: &[&str] = &[
    // The identity this product shipped under until the 2026-11-25 move.
    "com.locus.client",
    // The upstream Clash Verge Rev identity the fork was copied from.
    "io.github.clash-verge-rev.clash-verge-rev",
];

/// Development builds live beside the release ones rather than sharing a root.
#[cfg(feature = "verge-dev")]
pub(crate) const LEGACY_APP_IDS: &[&str] = &[
    "com.locus.client.dev",
    "com.locus.client",
    "io.github.clash-verge-rev.clash-verge-rev.dev",
    "io.github.clash-verge-rev.clash-verge-rev",
];

pub static CLASH_CONFIG: &str = "config.yaml";
pub static VERGE_CONFIG: &str = "verge.yaml";
pub static PROFILE_YAML: &str = "profiles.yaml";
/// Marks that the one-shot raise of too-short auto-update intervals has already run.
pub static UPDATE_INTERVAL_MIGRATED: &str = ".update-interval-migrated";

/// Uses the same platform data resolver as Tauri, including before its handle exists.
pub fn app_home_dir() -> Result<PathBuf> {
    ::dirs::data_dir()
        .map(|root| root.join(APP_ID))
        .ok_or_else(|| anyhow::anyhow!("Failed to get the app home directory"))
}

/// What the one-time adoption did, if anything.
///
/// A type rather than `Option<PathBuf>` because the two successes are **not**
/// the same event, and collapsing them is how a student ends up on an empty
/// profile set with nothing anywhere saying so:
///
/// - [`Self::Relocated`] — the old root became the current one. The activation
///   code, device fingerprint and tier moved with it.
/// - [`Self::AdoptedAside`] — a current root **already existed**, so the old one
///   was moved to a sibling name instead of being merged or discarded. The data
///   is safe and reachable, but the app is *not* running on it, which means the
///   code it holds may not be the one this device will use. That distinction is
///   the reason this enum exists: the previous `Option<PathBuf>` reported both
///   as "adopted", and only the first is a success the user need not see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataRootAdoption {
    /// The legacy root was renamed into the current one.
    Relocated {
        /// The path that moved.
        from: PathBuf,
    },
    /// The current root already existed; the legacy root was renamed aside.
    AdoptedAside {
        /// The path that moved.
        from: PathBuf,
        /// Where it now lives.
        to: PathBuf,
    },
}

impl DataRootAdoption {
    /// The path the adoption acted on, for a log line.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Relocated { from } => from.display().to_string(),
            Self::AdoptedAside { from, to } => format!("{} (moved aside to {})", from.display(), to.display()),
        }
    }

    /// Whether this outcome means the app is NOT running on the adopted data.
    #[must_use]
    pub const fn is_side_adoption(&self) -> bool {
        matches!(self, Self::AdoptedAside { .. })
    }
}

/// Adopts a pre-Chrysalis data directory, once, on the first launch under the
/// current identity.
///
/// WHY THIS EXISTS: `APP_ID` is not a cosmetic string. It is the app-data root,
/// and on Windows it is also how the privileged Service authenticates the app
/// (the root's owner SID plus the token file inside it). Renaming the product
/// therefore orphans a real directory of user state — activation code, device
/// fingerprint, tier, profiles — and, if it happened while the Service was
/// installed, leaves the Service's registration pointing at the old root.
///
/// The rule is deliberately conservative, because the cost of being wrong is a
/// half-copied config directory:
///
///   * Renames rather than copies. `rename` is atomic within a filesystem, so a
///     crash mid-migration cannot leave two half-populated roots. A rename
///     across filesystems fails, and that failure is reported rather than
///     silently degrading to a copy.
///   * Tries candidates newest-first and stops at the first that exists. Only
///     one legacy root has ever existed per build flavour, so this is a
///     single-step migration; the list exists so a future identity change can be
///     added without restructuring the call.
///   * **When the current root ALREADY exists, a non-empty legacy one is moved
///     aside — never merged, never silently ignored.** This used to be an
///     unconditional no-op, and that was a real hole rather than a tidiness
///     matter: an update installs the new build, the new build starts once
///     (creating an empty root), and from then on every launch finds a current
///     root and stops looking — leaving the activation code in a directory the
///     app no longer reads. Not a loss in the `fs` sense; one from the student's
///     seat, which is the seat that counts.
///
/// Returns what happened, so the caller can log it and surface the second case.
pub fn migrate_legacy_app_data_dir() -> Result<Option<DataRootAdoption>> {
    let Some(data_root) = ::dirs::data_dir() else {
        return Ok(None);
    };
    adopt_legacy_app_data_dir(&data_root, APP_ID, LEGACY_APP_IDS)
}

/// The body of [`migrate_legacy_app_data_dir`], parameterised by root so it can
/// be tested without reading or writing the real user data directory.
fn adopt_legacy_app_data_dir(data_root: &Path, app_id: &str, legacy_ids: &[&str]) -> Result<Option<DataRootAdoption>> {
    let current = data_root.join(app_id);

    for legacy_id in legacy_ids {
        // A build flavour must never adopt a root with its own name: that is a
        // self-rename, which `fs::rename` answers with success, so it would
        // report a migration that moved nothing.
        if *legacy_id == app_id {
            continue;
        }

        let legacy = data_root.join(legacy_id);
        if !legacy.exists() {
            continue;
        }

        // ── The current root already exists ──
        //
        // Only meaningful for a NON-EMPTY legacy root: an empty one carries
        // nothing worth keeping, and moving it aside would litter the data
        // directory for every user, on every launch, forever.
        if current.exists() {
            if is_effectively_empty(&legacy) {
                return Ok(None);
            }
            let aside = sibling_adoption_path(&current, legacy_id);
            return match fs::rename(&legacy, &aside) {
                Ok(()) => Ok(Some(DataRootAdoption::AdoptedAside {
                    from: legacy,
                    to: aside,
                })),
                Err(error) => Err(anyhow::anyhow!(
                    "found the pre-Chrysalis data directory {legacy:?} but could not move it aside as \
                     {aside:?}: {error}. Its contents (including the activation code) are still there; \
                     move it manually."
                )),
            };
        }

        match fs::rename(&legacy, &current) {
            Ok(()) => return Ok(Some(DataRootAdoption::Relocated { from: legacy })),
            Err(error) => {
                // Do NOT fall back to a recursive copy here. A partial copy is
                // worse than no migration: the app would start on a truncated
                // profile set and rewrite it, and the original would look like
                // the stale copy. Report it and let the caller decide.
                return Err(anyhow::anyhow!(
                    "could not adopt the pre-Chrysalis data directory {legacy:?} as {current:?}: {error}. \
                     Move it manually (the app will otherwise start with an empty profile set)."
                ));
            }
        }
    }

    Ok(None)
}

/// Whether a directory exists but holds nothing a person would miss.
///
/// Deliberately strict: only a directory with no entries at all counts. Anything
/// with a file or a subdirectory in it is treated as real data, even if a future
/// build would consider it disposable — the cost of moving an uninteresting
/// directory aside is one stray folder, and the cost of the opposite is a
/// student's paid code.
fn is_effectively_empty(dir: &Path) -> bool {
    match fs::read_dir(dir) {
        Ok(mut entries) => entries.next().is_none(),
        // Unreadable is NOT empty. An unreadable directory is exactly the case
        // where deciding on the student's behalf is least defensible.
        Err(_) => false,
    }
}

/// The sibling name an already-existing root gets moved to.
///
/// Deterministic rather than random, so a second launch finds the same name
/// instead of accumulating copies. The suffix is the *legacy* id, so the folder
/// names the product whose data it holds — the only thing a person looking at
/// their data directory can act on.
fn sibling_adoption_path(current: &Path, legacy_id: &str) -> PathBuf {
    let name = current.file_name().and_then(|name| name.to_str()).unwrap_or("data");
    current.with_file_name(format!("{name}.adopted-from-{legacy_id}"))
}

pub fn preinit_app_data_dir() -> Result<PathBuf> {
    app_home_dir()
}

pub fn app_resources_dir() -> Result<PathBuf> {
    let app_handle = handle::Handle::app_handle();

    match app_handle.path().resource_dir() {
        Ok(dir) => Ok(dir.join("resources")),
        Err(e) => {
            logging!(error, Type::File, "Failed to get the resource directory: {e}");
            Err(anyhow::anyhow!("Failed to get the resource directory"))
        }
    }
}

pub fn app_profiles_dir() -> Result<PathBuf> {
    Ok(app_home_dir()?.join("profiles"))
}

pub fn app_icons_dir() -> Result<PathBuf> {
    Ok(app_home_dir()?.join("icons"))
}

pub fn find_target_icons(target: &str) -> Result<Option<String>> {
    let icons_dir = app_icons_dir()?;
    let icon_path = fs::read_dir(&icons_dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .find(|path| {
            let prefix_matches = path
                .file_prefix()
                .and_then(|p| p.to_str())
                .is_some_and(|prefix| prefix.starts_with(target));
            let ext_matches = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ico") || ext.eq_ignore_ascii_case("png"));
            prefix_matches && ext_matches
        });

    icon_path.map(|path| path_to_str(&path).map(|s| s.into())).transpose()
}

pub fn app_logs_dir() -> Result<PathBuf> {
    Ok(app_home_dir()?.join("logs"))
}

#[cfg(target_os = "macos")]
pub fn service_logs_root_dir() -> Result<PathBuf> {
    Ok(app_home_dir()?.join("service-logs"))
}

#[cfg(not(target_os = "macos"))]
pub fn service_logs_root_dir() -> Result<PathBuf> {
    app_logs_dir()
}

pub fn app_latest_log() -> Result<PathBuf> {
    Ok(app_logs_dir()?.join("latest.log"))
}

pub fn local_backup_dir() -> Result<PathBuf> {
    let dir = app_home_dir()?.join(BACKUP_DIR);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn clash_path() -> Result<PathBuf> {
    Ok(app_home_dir()?.join(CLASH_CONFIG))
}

pub fn verge_path() -> Result<PathBuf> {
    Ok(app_home_dir()?.join(VERGE_CONFIG))
}

pub fn profiles_path() -> Result<PathBuf> {
    Ok(app_home_dir()?.join(PROFILE_YAML))
}

pub fn update_interval_migrated_path() -> Result<PathBuf> {
    Ok(app_home_dir()?.join(UPDATE_INTERVAL_MIGRATED))
}

#[cfg(target_os = "macos")]
pub fn service_path() -> Result<PathBuf> {
    let res_dir = app_resources_dir()?;
    Ok(res_dir.join("clash-verge-service"))
}

#[cfg(windows)]
pub fn service_path() -> Result<PathBuf> {
    let res_dir = app_resources_dir()?;
    Ok(res_dir.join("clash-verge-service.exe"))
}

pub fn sidecar_log_dir() -> Result<PathBuf> {
    let log_dir = app_logs_dir()?.join("sidecar");
    let _ = std::fs::create_dir_all(&log_dir);

    Ok(log_dir)
}

pub fn service_log_dir() -> Result<PathBuf> {
    let log_dir = service_logs_root_dir()?.join("service");
    let _ = std::fs::create_dir_all(&log_dir);

    Ok(log_dir)
}

pub fn clash_latest_log() -> Result<PathBuf> {
    match *CoreManager::global().get_running_mode() {
        RunningMode::Service => Ok(service_log_dir()?.join("service_latest.log")),
        RunningMode::NotRunning => Ok(sidecar_log_dir()?.join("sidecar_latest.log")),
    }
}

pub fn path_to_str(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow::anyhow!("failed to get path from {:?}", path))
}

pub fn get_encryption_key() -> Result<Vec<u8>> {
    let app_dir = app_home_dir()?;
    let key_path = app_dir.join(".encryption_key");

    if key_path.exists() {
        fs::read(&key_path).map_err(|e| anyhow::anyhow!("Failed to read encryption key: {}", e))
    } else {
        let mut key = vec![0u8; 32];
        getrandom::fill(&mut key)?;

        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent).map_err(|e| anyhow::anyhow!("Failed to create key directory: {}", e))?;
        }
        fs::write(&key_path, &key).map_err(|e| anyhow::anyhow!("Failed to save encryption key: {}", e))?;
        Ok(key)
    }
}

pub fn ipc_path() -> Result<PathBuf> {
    Ok(PathBuf::from(clash_verge_service_ipc::mihomo_ipc_path(
        &crate::core::owner_identity::current_owner_identity()?,
    )))
}

#[cfg(target_os = "macos")]
pub fn sidecar_ipc_path() -> Result<PathBuf> {
    sidecar_ipc_path_for(
        std::path::Path::new(""),
        &crate::core::owner_identity::current_owner_identity()?,
    )
}

#[cfg(not(target_os = "macos"))]
pub fn sidecar_ipc_path() -> Result<PathBuf> {
    Ok(sidecar_ipc_path_for(
        &preinit_app_data_dir()?,
        &crate::core::owner_identity::current_owner_identity()?,
    ))
}

#[cfg(target_os = "linux")]
fn sidecar_ipc_path_for(app_root: &std::path::Path, _identity: &clash_verge_service_ipc::OwnerIdentity) -> PathBuf {
    app_root.join("verge-mihomo.sock")
}

#[cfg(target_os = "macos")]
fn sidecar_ipc_path_for(
    _app_root: &std::path::Path,
    _identity: &clash_verge_service_ipc::OwnerIdentity,
) -> Result<PathBuf> {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _};

    // SAFETY: A null buffer with size zero asks confstr for the required buffer length.
    let required_len = unsafe { libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, std::ptr::null_mut(), 0) };
    if required_len == 0 {
        return Err(anyhow::anyhow!("macOS per-user temporary directory is unavailable"));
    }

    let mut buffer = vec![0_u8; required_len];
    // SAFETY: buffer is writable for buffer.len() bytes, as required by confstr.
    let written = unsafe { libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, buffer.as_mut_ptr().cast(), buffer.len()) };
    if written == 0 || written > buffer.len() {
        return Err(anyhow::anyhow!("failed to read macOS per-user temporary directory"));
    }

    let root = std::ffi::CStr::from_bytes_until_nul(&buffer)
        .map_err(|_| anyhow::anyhow!("macOS per-user temporary directory is not NUL-terminated"))?;
    #[cfg(feature = "verge-dev")]
    let filename = "verge-mihomo-dev.sock";
    #[cfg(not(feature = "verge-dev"))]
    let filename = "verge-mihomo.sock";
    let path = PathBuf::from(OsStr::from_bytes(root.to_bytes())).join(filename);

    let path_len = path.as_os_str().as_bytes().len();
    if path_len >= 104 {
        return Err(anyhow::anyhow!(
            "macOS Sidecar IPC path is {path_len} bytes, but sockaddr_un.sun_path requires fewer than 104 bytes: {:?}",
            path,
        ));
    }

    Ok(path)
}

#[cfg(windows)]
fn sidecar_ipc_path_for(_app_root: &std::path::Path, identity: &clash_verge_service_ipc::OwnerIdentity) -> PathBuf {
    PathBuf::from(sidecar_pipe_name(identity, cfg!(feature = "verge-dev")))
}

#[cfg(any(windows, test))]
fn sidecar_pipe_name(identity: &clash_verge_service_ipc::OwnerIdentity, is_dev: bool) -> String {
    let flavor = if is_dev { "dev" } else { "release" };
    format!(
        r"\\.\pipe\verge-mihomo-sidecar-{flavor}-{}",
        clash_verge_service_ipc::owner_key(identity)
    )
}

#[cfg(all(test, target_os = "linux"))]
mod ipc_tests {
    use super::sidecar_ipc_path_for;
    use clash_verge_service_ipc::OwnerIdentity;
    use std::path::Path;

    #[test]
    fn sidecar_ipc_stays_in_the_app_root() {
        let identity = OwnerIdentity::Unix { uid: 501, gid: 20 };
        let app_root = Path::new("/home/test/.local/share/io.github.clash-verge-rev.clash-verge-rev");
        let path = sidecar_ipc_path_for(app_root, &identity);

        assert_eq!(path, app_root.join("verge-mihomo.sock"));
        assert_ne!(
            path.to_string_lossy(),
            clash_verge_service_ipc::mihomo_ipc_path(&identity)
        );
    }
}

#[cfg(all(test, target_os = "macos"))]
mod ipc_tests {
    use super::sidecar_ipc_path_for;
    use clash_verge_service_ipc::OwnerIdentity;
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _, path::Path};

    #[test]
    fn sidecar_ipc_ignores_long_app_root_and_fits_sockaddr_un() -> anyhow::Result<()> {
        let identity = OwnerIdentity::Unix { uid: 501, gid: 20 };
        let app_root =
            Path::new("/Users/support/Library/Application Support/io.github.clash-verge-rev.clash-verge-rev.dev");
        let path = sidecar_ipc_path_for(app_root, &identity)?;

        assert!(!path.starts_with(app_root));
        assert!(path.as_os_str().as_bytes().len() < 104);
        #[cfg(feature = "verge-dev")]
        assert_eq!(path.file_name(), Some(OsStr::new("verge-mihomo-dev.sock")));
        #[cfg(not(feature = "verge-dev"))]
        assert_eq!(path.file_name(), Some(OsStr::new("verge-mihomo.sock")));
        assert_eq!(path, sidecar_ipc_path_for(Path::new("/different/root"), &identity)?);
        assert!(path.parent().is_some_and(Path::is_dir));
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod ipc_tests {
    use super::sidecar_ipc_path_for;
    use clash_verge_service_ipc::OwnerIdentity;
    use std::path::Path;

    #[test]
    fn sidecar_ipc_uses_the_current_owners_named_pipe() {
        let identity = OwnerIdentity::Windows {
            sid: "S-1-5-21-1000".to_owned(),
        };
        let path = sidecar_ipc_path_for(Path::new(r"C:\ignored"), &identity);

        assert_eq!(
            path,
            Path::new(&format!(
                r"\\.\pipe\verge-mihomo-sidecar-{}-{}",
                if cfg!(feature = "verge-dev") { "dev" } else { "release" },
                clash_verge_service_ipc::owner_key(&identity)
            ))
        );
    }
}

#[cfg(test)]
mod app_data_migration_tests {
    use super::{APP_ID, DataRootAdoption, LEGACY_APP_IDS, adopt_legacy_app_data_dir};
    use std::fs;

    /// A scratch data root, unique per test so tests can run concurrently.
    struct Scratch {
        root: std::path::PathBuf,
    }

    impl Scratch {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("chrysalis-migrate-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("create scratch root");
            Self { root }
        }

        fn dir(&self, name: &str) -> std::path::PathBuf {
            self.root.join(name)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    /// The migration must actually carry the contents across, not just the name —
    /// the whole point is that activation state survives the identity change.
    #[test]
    fn adopts_the_legacy_directory_and_its_contents() {
        let scratch = Scratch::new("adopts");
        let legacy_id = LEGACY_APP_IDS[0];
        let legacy = scratch.dir(legacy_id);
        fs::create_dir_all(legacy.join("profiles")).expect("create legacy root");
        fs::write(legacy.join("verge.yaml"), "activation_code: RQ-AAAA\n").expect("seed legacy file");

        let adopted = adopt_legacy_app_data_dir(&scratch.root, APP_ID, LEGACY_APP_IDS).expect("migration runs");

        assert_eq!(
            adopted,
            Some(DataRootAdoption::Relocated { from: legacy.clone() }),
            "should report what it relocated, and that it was a relocation"
        );
        assert!(!legacy.exists(), "the legacy directory should be gone, not copied");
        let moved = scratch.dir(APP_ID).join("verge.yaml");
        assert!(moved.is_file(), "state must land in the new root");
        assert_eq!(
            fs::read_to_string(&moved).expect("read moved file"),
            "activation_code: RQ-AAAA\n",
            "contents must survive the move"
        );
    }

    /// A non-empty legacy root is **moved aside**, not ignored, when the current
    /// root already exists.
    ///
    /// THIS IS THE REGRESSION THIS TEST EXISTS FOR. The adoption used to return
    /// early whenever the current root existed, which is the state an update
    /// produces: the new build starts once, creates an empty root, and from then
    /// on every launch declares "already adopted" and leaves the activation code
    /// in a directory nothing reads. From the student's seat that is a lost code,
    /// and it has no error anywhere.
    ///
    /// Demonstrated failing by restoring the early `return Ok(None)` — the
    /// assertions below then see `None` and no sibling directory.
    #[test]
    fn a_populated_legacy_root_is_moved_aside_when_the_current_one_exists() {
        let scratch = Scratch::new("aside");
        let current = scratch.dir(APP_ID);
        fs::create_dir_all(&current).expect("create current root");
        fs::write(current.join("verge.yaml"), "current\n").expect("seed current file");

        let legacy_id = LEGACY_APP_IDS[0];
        let legacy = scratch.dir(legacy_id);
        fs::create_dir_all(&legacy).expect("create legacy root");
        fs::write(legacy.join("verge.yaml"), "activation_code: RQ-PAID\n").expect("seed legacy");

        let adopted = adopt_legacy_app_data_dir(&scratch.root, APP_ID, LEGACY_APP_IDS).expect("migration runs");

        let Some(DataRootAdoption::AdoptedAside { from, to }) = adopted else {
            panic!("a populated legacy root beside a current one must be moved aside, got {adopted:?}");
        };
        assert_eq!(from, legacy);
        assert!(
            to.starts_with(&current) || to.parent() == current.parent(),
            "the aside directory must be a sibling of the current root: {to:?}"
        );
        assert!(
            to.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(legacy_id)),
            "the aside directory must name the product whose data it holds, got {to:?}"
        );
        assert!(
            !legacy.exists(),
            "the legacy root must have moved, not been left in place"
        );
        assert_eq!(
            fs::read_to_string(to.join("verge.yaml")).expect("read aside file"),
            "activation_code: RQ-PAID\n",
            "the code must survive the move — this is the whole point"
        );
        assert_eq!(
            fs::read_to_string(current.join("verge.yaml")).expect("read current file"),
            "current\n",
            "the current root must not be merged into or overwritten"
        );
    }

    /// An *empty* legacy root beside a current one is a quiet no-op.
    ///
    /// Without this, every launch would litter the data directory with an empty
    /// `....adopted-from-...` folder, and a second run would find the first one's
    /// leftovers and keep going.
    #[test]
    fn an_empty_legacy_root_is_not_worth_moving_aside() {
        let scratch = Scratch::new("aside-empty");
        fs::create_dir_all(scratch.dir(APP_ID)).expect("create current root");
        let legacy_id = LEGACY_APP_IDS[0];
        fs::create_dir_all(scratch.dir(legacy_id)).expect("create empty legacy root");

        let adopted = adopt_legacy_app_data_dir(&scratch.root, APP_ID, LEGACY_APP_IDS).expect("migration runs");

        assert_eq!(adopted, None, "an empty legacy root carries nothing to preserve");
        assert!(
            scratch.dir(legacy_id).exists(),
            "an empty legacy root is left where it is rather than renamed"
        );
    }

    /// A candidate equal to the current id must never be "adopted": that is a
    /// self-rename, which `fs::rename` reports as success while moving nothing.
    #[test]
    fn the_current_identity_is_never_its_own_candidate() {
        let scratch = Scratch::new("self");
        let current = scratch.dir(APP_ID);
        fs::create_dir_all(&current).expect("create current root");
        fs::write(current.join("verge.yaml"), "keep\n").expect("seed");

        let adopted = adopt_legacy_app_data_dir(&scratch.root, APP_ID, &[APP_ID]).expect("migration runs");

        assert_eq!(adopted, None);
        assert_eq!(
            fs::read_to_string(current.join("verge.yaml")).expect("read current file"),
            "keep\n",
            "an id equal to the current one must be skipped, not renamed onto itself"
        );
    }

    /// An existing Chrysalis root is never **overwritten or merged into**. This is
    /// what makes it safe to call the migration unconditionally on every launch.
    ///
    /// The legacy root is no longer "left alone", and that change is the point of
    /// [`a_populated_legacy_root_is_moved_aside_when_the_current_one_exists`]: the
    /// old assertion here (`assert!(legacy.exists())`) encoded the defect. Leaving
    /// a populated legacy root in place is safe for the current launch and fatal
    /// for the student, because nothing ever looks at that directory again.
    #[test]
    fn never_overwrites_an_existing_chrysalis_root() {
        let scratch = Scratch::new("idempotent");
        let current = scratch.dir(APP_ID);
        fs::create_dir_all(&current).expect("create current root");
        fs::write(current.join("verge.yaml"), "current\n").expect("seed current file");

        let legacy_id = LEGACY_APP_IDS[0];
        let legacy = scratch.dir(legacy_id);
        fs::create_dir_all(&legacy).expect("create legacy root");
        fs::write(legacy.join("verge.yaml"), "legacy\n").expect("seed legacy file");

        let adopted = adopt_legacy_app_data_dir(&scratch.root, APP_ID, LEGACY_APP_IDS).expect("migration runs");

        assert!(
            matches!(adopted, Some(DataRootAdoption::AdoptedAside { .. })),
            "a populated legacy root beside a current one is preserved aside, got {adopted:?}"
        );
        assert_eq!(
            fs::read_to_string(current.join("verge.yaml")).expect("read current file"),
            "current\n",
            "the current root must be left exactly as it was — this is the half that \
             must never change, because merging two profile sets is worse than either"
        );
        assert!(
            !legacy.exists(),
            "the legacy root must have moved aside rather than being left where nothing reads it"
        );
    }

    /// No legacy directory is the normal first-install case: it must be a quiet
    /// no-op, not an error.
    #[test]
    fn a_fresh_install_is_a_no_op() {
        let scratch = Scratch::new("fresh");
        let adopted = adopt_legacy_app_data_dir(&scratch.root, APP_ID, LEGACY_APP_IDS).expect("migration runs");
        assert!(adopted.is_none());
        assert!(
            !scratch.dir(APP_ID).exists(),
            "nothing should be created when there is nothing to adopt"
        );
    }

    /// Candidates are tried in order, so a newer legacy identity wins over an
    /// older one when both are somehow present.
    #[test]
    fn prefers_the_first_candidate_that_exists() {
        let scratch = Scratch::new("order");
        let first = "com.example.newer";
        let second = "com.example.older";
        fs::create_dir_all(scratch.dir(second)).expect("create older root");
        fs::write(scratch.dir(second).join("marker"), "older\n").expect("seed older");
        fs::create_dir_all(scratch.dir(first)).expect("create newer root");
        fs::write(scratch.dir(first).join("marker"), "newer\n").expect("seed newer");

        let adopted = adopt_legacy_app_data_dir(&scratch.root, APP_ID, &[first, second]).expect("migration runs");

        assert_eq!(
            adopted,
            Some(DataRootAdoption::Relocated {
                from: scratch.dir(first)
            })
        );
        assert_eq!(
            fs::read_to_string(scratch.dir(APP_ID).join("marker")).expect("read adopted marker"),
            "newer\n"
        );
        assert!(scratch.dir(second).exists(), "the unchosen candidate must be untouched");
    }

    /// The identity literals are load-bearing: they are what the migration looks
    /// for. Pinning them here means a careless rename shows up as a test failure
    /// rather than as silently-abandoned user state in the field.
    #[test]
    fn the_upstream_identity_is_still_a_migration_candidate() {
        assert!(
            LEGACY_APP_IDS.contains(&"io.github.clash-verge-rev.clash-verge-rev"),
            "the identity the fork was copied from must remain a candidate, or existing \
             installs will silently start from an empty data root"
        );
        assert!(
            !LEGACY_APP_IDS.contains(&APP_ID),
            "the current identity must not be a migration candidate — it would match a root \
             this build just created"
        );
    }

    /// **THE 2026-11-25 IDENTITY MOVE, PINNED.**
    ///
    /// `APP_ID` moved from `com.locus.client`, and every installed machine has its
    /// activation code, device fingerprint and tier under the old root. This test
    /// asserts the two things that make the move survivable, in the order that
    /// makes it survivable:
    ///
    ///   1. the old id IS a candidate — delete it and every existing install
    ///      starts from an empty profile set and has to re-activate;
    ///   2. it is checked BEFORE the upstream Clash Verge Rev id — the loop takes
    ///      the first existing candidate, and a machine that was never copied from
    ///      upstream (most of them) has no upstream directory, while a machine that
    ///      HAS one must still prefer its own data.
    ///
    /// A test rather than a comment because the failure mode is silent: nothing
    /// errors, the app simply starts empty and the user re-enters their code.
    #[test]
    fn the_pre_move_identity_is_a_candidate_and_precedes_upstream() {
        let moved_from = if cfg!(feature = "verge-dev") {
            "com.locus.client.dev"
        } else {
            "com.locus.client"
        };
        let moved_from_idx = LEGACY_APP_IDS.iter().position(|id| *id == moved_from);
        assert!(
            moved_from_idx.is_some(),
            "`{moved_from}` must stay a migration candidate: it is the data root every \
             install before 2026-11-25 writes its activation code, device fingerprint and \
             tier into. Without it those installs start from an empty root."
        );

        let upstream = if cfg!(feature = "verge-dev") {
            "io.github.clash-verge-rev.clash-verge-rev.dev"
        } else {
            "io.github.clash-verge-rev.clash-verge-rev"
        };
        if let Some(upstream_idx) = LEGACY_APP_IDS.iter().position(|id| *id == upstream) {
            assert!(
                moved_from_idx.expect("asserted above") < upstream_idx,
                "the pre-move identity must be tried BEFORE {upstream}: the adoption loop \
                 takes the first candidate that exists, and preferring the upstream \
                 directory would hand an install someone else's old profile set. \
                 Actual order: {LEGACY_APP_IDS:?}"
            );
        }
    }

    /// The release identity and the dev identity must not be reachable from each
    /// other's candidate list, or a dev build would adopt a release data root (or
    /// the reverse) and two installations would fight over one directory.
    #[test]
    fn the_other_channels_identity_is_not_a_candidate() {
        let other = if cfg!(feature = "verge-dev") {
            "com.chrysalis.client"
        } else {
            "com.chrysalis.client.dev"
        };
        assert!(
            !LEGACY_APP_IDS.contains(&other),
            "`{other}` belongs to the other channel and must never be adopted — the two \
             roots are deliberately separate"
        );
    }

    /// The migration must not silently degrade into a copy. A rename that cannot
    /// happen is reported so the user can move the directory by hand, instead of
    /// losing data to a half-written root.
    ///
    /// The failure is provoked the only way it can happen in practice on a
    /// same-filesystem rename: by making the destination un-renamable. An
    /// unwritable parent directory gives EACCES on the rename without the
    /// destination existing, which is exactly the "current root missing but the
    /// move failed" path.
    #[test]
    #[cfg(unix)]
    fn a_failed_rename_is_reported_rather_than_copied() {
        use std::os::unix::fs::PermissionsExt as _;

        let scratch = Scratch::new("rename-failure");
        let legacy_id = LEGACY_APP_IDS[0];
        let legacy = scratch.dir(legacy_id);
        fs::create_dir_all(&legacy).expect("create legacy root");

        // Make the data root read-only so the rename into it cannot proceed.
        // Skipped when running as root, where permissions do not apply.
        let original = fs::metadata(&scratch.root).expect("stat scratch root").permissions();
        fs::set_permissions(&scratch.root, fs::Permissions::from_mode(0o555)).expect("chmod scratch root");

        let result = adopt_legacy_app_data_dir(&scratch.root, APP_ID, LEGACY_APP_IDS);

        // Restore before asserting so the Drop cleanup can still remove the tree.
        fs::set_permissions(&scratch.root, original).expect("restore scratch root permissions");

        if unsafe { libc::geteuid() } == 0 {
            // root ignores the directory bits, so the failure cannot be provoked.
            return;
        }

        assert!(
            result.is_err(),
            "a rename that cannot happen must be an error, not a silent copy"
        );
        assert!(legacy.exists(), "the original must be left intact for a manual move");
    }

    /// A stale file (not a directory) sitting where the app data root belongs is
    /// treated as "the current root exists" and nothing is adopted.
    ///
    /// This pins the guard's actual predicate, which is `exists()` rather than
    /// `is_dir()`. That is the safe choice: the alternative would try to rename a
    /// directory over an unexpected file, and the app would then fail to start on
    /// a data root that is not a directory — a worse outcome than skipping an
    /// adoption the user can perform by hand.
    #[test]
    fn a_file_where_the_root_belongs_skips_the_migration() {
        let scratch = Scratch::new("file-in-the-way");
        let legacy_id = LEGACY_APP_IDS[0];
        let legacy = scratch.dir(legacy_id);
        fs::create_dir_all(&legacy).expect("create legacy root");
        fs::write(scratch.dir(APP_ID), "not a data root").expect("block the destination");

        let adopted = adopt_legacy_app_data_dir(&scratch.root, APP_ID, LEGACY_APP_IDS).expect("no error");

        assert!(adopted.is_none(), "an occupied destination is not adopted over");
        assert!(legacy.exists(), "the legacy directory must be left intact");
    }
}

#[async_trait]
pub trait PathBufExec {
    async fn remove_if_exists(&self) -> Result<()>;
}

#[async_trait]
impl PathBufExec for PathBuf {
    async fn remove_if_exists(&self) -> Result<()> {
        if self.exists() {
            tokio::fs::remove_file(self).await?;
            logging!(debug, Type::File, "Removed file: {:?}", self);
        }
        Ok(())
    }
}

#[cfg(test)]
mod windows_pipe_name_tests {
    use super::sidecar_pipe_name;
    use clash_verge_service_ipc::OwnerIdentity;

    #[test]
    fn windows_sidecar_pipe_separates_dev_and_release_for_the_same_owner() {
        let identity = OwnerIdentity::Windows {
            sid: "S-1-5-21-1000".to_owned(),
        };
        let owner_key = clash_verge_service_ipc::owner_key(&identity);

        assert_eq!(
            sidecar_pipe_name(&identity, false),
            format!(r"\\.\pipe\verge-mihomo-sidecar-release-{owner_key}")
        );
        assert_eq!(
            sidecar_pipe_name(&identity, true),
            format!(r"\\.\pipe\verge-mihomo-sidecar-dev-{owner_key}")
        );
    }
}
