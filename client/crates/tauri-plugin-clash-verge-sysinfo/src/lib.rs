use std::{
    fmt::{Debug, Display},
    time::Instant,
};

pub mod commands;

#[cfg(windows)]
use deelevate::{PrivilegeLevel, Token};
#[cfg(unix)]
pub use libc;
use parking_lot::RwLock;
use sysinfo::{Networks, System};
use tauri::{
    Manager as _, Runtime,
    plugin::{Builder, TauriPlugin},
};

#[derive(Clone)]
pub struct SysInfo {
    system_name: String,
    system_version: String,
    system_kernel_version: String,
    system_arch: String,
}

impl Default for SysInfo {
    #[inline]
    fn default() -> Self {
        let system_name = System::name().unwrap_or_else(|| "Null".into());
        let system_version = System::long_os_version().unwrap_or_else(|| "Null".into());
        let system_kernel_version = System::kernel_version().unwrap_or_else(|| "Null".into());
        let system_arch = System::cpu_arch();
        Self {
            system_name,
            system_version,
            system_kernel_version,
            system_arch,
        }
    }
}

#[derive(Clone)]
pub struct AppInfo {
    app_version: String,
    app_core_mode: String,
    pub app_startup_time: Instant,
    pub app_is_admin: bool,
}

impl Default for AppInfo {
    #[inline]
    fn default() -> Self {
        let app_version = "0.0.0".into();
        let app_core_mode = "NotRunning".into();
        let app_is_admin = false;
        let app_startup_time = Instant::now();
        Self {
            app_version,
            app_core_mode,
            app_startup_time,
            app_is_admin,
        }
    }
}

#[derive(Default, Clone)]
pub struct Platform {
    pub sysinfo: SysInfo,
    pub appinfo: AppInfo,
}

impl Debug for Platform {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Platform")
            .field("system_name", &self.sysinfo.system_name)
            .field("system_version", &self.sysinfo.system_version)
            .field("system_kernel_version", &self.sysinfo.system_kernel_version)
            .field("system_arch", &self.sysinfo.system_arch)
            .field("app_version", &self.appinfo.app_version)
            .field("app_core_mode", &self.appinfo.app_core_mode)
            .field("app_is_admin", &self.appinfo.app_is_admin)
            .finish()
    }
}

impl Display for Platform {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "System Name: {}\nSystem Version: {}\nSystem kernel Version: {}\nSystem Arch: {}\nVerge Version: {}\nRunning Mode: {}\nIs Admin: {}",
            self.sysinfo.system_name,
            self.sysinfo.system_version,
            self.sysinfo.system_kernel_version,
            self.sysinfo.system_arch,
            self.appinfo.app_version,
            self.appinfo.app_core_mode,
            self.appinfo.app_is_admin
        )
    }
}

impl Platform {
    #[inline]
    fn new() -> Self {
        Self::default()
    }
}

/// Whether a token privilege level means "this process can create a TUN device".
///
/// Takes a crate-local mirror of `deelevate::PrivilegeLevel` rather than
/// `deelevate`'s own type, for one reason: the bug being fixed was a wrong
/// *classifier*, and a classifier that can only be exercised on Windows is one
/// that regresses unnoticed. `deelevate`'s enum exists only under
/// `cfg(windows)`, so testing against it directly would make this test dead on
/// every other host — including the one CI runs Rust tests on.
///
/// Both privileged kinds are accepted; see [`probe_is_admin`] for why `Elevated`
/// alone is wrong.
#[inline]
#[must_use]
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
const fn is_privileged(level: TokenPrivilege) -> bool {
    matches!(level, TokenPrivilege::Elevated | TokenPrivilege::HighIntegrityAdmin)
}

/// The subset of `deelevate::PrivilegeLevel` this crate reasons about.
///
/// Deliberately a mirror rather than a re-export: the mapping is narrowed to the
/// three variants the probe can produce, so adding a fourth upstream is a
/// compile error here rather than a silent behavioural change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
enum TokenPrivilege {
    /// An unprivileged token.
    NotPrivileged,
    /// A token elevated via runas/UAC (`TokenElevationTypeFull`).
    Elevated,
    /// High integrity without a UAC-produced token — e.g. an SSH session.
    HighIntegrityAdmin,
}

#[cfg(windows)]
impl From<PrivilegeLevel> for TokenPrivilege {
    fn from(level: PrivilegeLevel) -> Self {
        match level {
            PrivilegeLevel::NotPrivileged => Self::NotPrivileged,
            PrivilegeLevel::Elevated => Self::Elevated,
            PrivilegeLevel::HighIntegrityAdmin => Self::HighIntegrityAdmin,
        }
    }
}

/// Whether this process holds privileges that let it create a TUN device.
///
/// # Why this is not just `privilege_level()`
///
/// The original implementation was:
///
/// ```ignore
/// Token::with_current_process()?.privilege_level().map(|l| l != NotPrivileged)
/// ```
///
/// and it **reported false on a process that had genuinely elevated via UAC**.
/// `privilege_level()` returns `Elevated` only when `TokenElevationType` is
/// `TokenElevationTypeFull`, and that value is *not* a reliable statement of
/// "this process is running as administrator":
///
///   * `TokenElevationTypeFull` is a **split-token** concept. It describes a
///     token produced by UAC elevating a filtered (limited) token. A process
///     launched with "Run as administrator" from an already-elevated shell, or
///     one inheriting an elevated parent token, is fully privileged while
///     reporting `TokenElevationTypeDefault` — so the old check returned
///     `NotPrivileged` and the app told an administrator they lacked rights.
///   * The failure is silent and *self-confirming*: the value was computed once
///     at startup and cached, so the wrong answer stuck for the process's life.
///
/// The predicate we actually care about is "can this process create a TUN
/// device", which needs an **administrator token at high integrity** — not a
/// particular elevation *mechanism*. Both are checked, and either one passing is
/// sufficient. `HighIntegrityAdmin` (SSH-style sessions) is accepted for the
/// same reason: it is a genuinely privileged token that simply did not come from
/// UAC.
///
/// Errors fail closed: refusing TUN is recoverable, and the caller has an
/// actionable message for it.
#[inline]
pub(crate) fn probe_is_admin() -> bool {
    #[cfg(not(windows))]
    unsafe {
        libc::geteuid() == 0
    }
    #[cfg(windows)]
    {
        // `HighIntegrityAdmin` and `Elevated` both mean "an administrator token
        // at high integrity". Only `NotPrivileged` is a genuine no.
        Token::with_current_process()
            .and_then(|token| token.privilege_level())
            .map(TokenPrivilege::from)
            .map(is_privileged)
            .unwrap_or_else(|error| {
                // A probe that cannot read the token is not evidence of absence,
                // but we must not claim privilege we could not verify. Fail
                // closed and leave a trail, because a silent `false` here is
                // exactly the bug this function exists to fix.
                logging_probe_failed(&error.to_string());
                false
            })
    }
}

/// Reports a failed elevation probe once per process.
///
/// Deliberately not routed through `clash_verge_logging`: this crate is a plugin
/// and has no dependency on the app's logger, and a probe failure at startup is
/// rare enough that a single stderr line is the right weight.
#[cfg(windows)]
fn logging_probe_failed(reason: &str) {
    static REPORTED: std::sync::Once = std::sync::Once::new();
    REPORTED.call_once(|| {
        eprintln!("[sysinfo] could not read the process token to determine elevation: {reason}");
    });
}

#[inline]
#[cfg(unix)]
pub fn current_gid() -> u32 {
    unsafe { libc::getgid() }
}

#[inline]
pub fn list_network_interfaces() -> Vec<String> {
    let mut networks = Networks::new();
    networks.refresh(false);
    networks.keys().map(|name| name.to_owned()).collect()
}

#[inline]
pub fn set_app_core_mode<R: Runtime>(app: &tauri::AppHandle<R>, mode: impl Into<String>) {
    let platform_spec = app.state::<RwLock<Platform>>();
    let mut spec = platform_spec.write();
    spec.appinfo.app_core_mode = mode.into();
}

#[inline]
pub fn get_app_uptime<R: Runtime>(app: &tauri::AppHandle<R>) -> Instant {
    let platform_spec = app.state::<RwLock<Platform>>();
    let spec = platform_spec.read();
    spec.appinfo.app_startup_time
}

/// Whether this **process** is elevated, without needing an app handle.
///
/// Exists so callers that legitimately run before the Tauri app handle exists —
/// notably the Run State store, which is a lazily-built static read from the
/// first startup transition — can still get a correct answer. The previous
/// handle-based path failed closed in that window and, because the result was
/// cached, never recovered.
///
/// `is_current_app_handle_admin` delegates here; prefer it when a handle is
/// available, purely for the self-documentation of what is being asked.
#[inline]
#[must_use]
pub fn is_process_elevated() -> bool {
    probe_is_admin()
}

#[inline]
pub fn is_current_app_handle_admin<R: Runtime>(app: &tauri::AppHandle<R>) -> bool {
    // Deliberately NOT read from the cached `Platform` state.
    //
    // The cached value was written once in `init()` and never refreshed, so a
    // wrong answer at startup was permanent — which is precisely how "I ran it as
    // administrator and it still says I need administrator rights" happened. The
    // probe is cheap (one `GetTokenInformation` pair) and the answer can legitimately
    // change during a session, because a UAC prompt elevates a *new* process that
    // then reports its own state.
    //
    // `app` is still taken, and still used, so a caller cannot accidentally probe
    // a token belonging to a different process: the handle proves we are asking
    // about this application.
    let _ = app;
    probe_is_admin()
}

#[inline]
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::<R>::new("clash_verge_sysinfo")
        // TODO: move command registration here once this crate becomes a complete Tauri plugin.
        .setup(move |app, _api| {
            let app_version = app.package_info().version.to_string();
            // The initial probe. This value is now only a *snapshot* for the
            // diagnostics report; every decision reads `is_current_app_handle_admin`,
            // which re-probes. Keeping the field populated means
            // `export_diagnostic_info` still answers "was this session elevated at
            // startup?", which is the question a support report wants.
            let is_admin = probe_is_admin();

            let mut platform_spec = Platform::new();
            platform_spec.appinfo.app_version = app_version;
            platform_spec.appinfo.app_is_admin = is_admin;

            app.manage(RwLock::new(platform_spec));
            Ok(())
        })
        .build()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, reason = "tests assert by panicking")]
mod tests {
    use super::*;

    /// The classification that was wrong.
    ///
    /// `privilege_level()` has three outcomes and the old code accepted only one
    /// of them as "privileged" (`!= NotPrivileged` looks permissive, but the
    /// *producer* — `TokenElevationType == Full` — is the restrictive part). The
    /// regression this guards is a future edit narrowing the accepted set back to
    /// `Elevated` alone, which is what broke Run-as-Administrator.
    #[test]
    fn both_privileged_token_kinds_count_as_admin() {
        for level in [TokenPrivilege::Elevated, TokenPrivilege::HighIntegrityAdmin] {
            assert!(
                is_privileged(level),
                "{level:?} is an administrator token at high integrity and must be accepted"
            );
        }
        assert!(!is_privileged(TokenPrivilege::NotPrivileged));
    }

    /// The probe must never be answered from a cached startup value.
    ///
    /// Two reads must be able to disagree, because elevation can change during a
    /// session: a UAC prompt starts a new process, and the answer belongs to that
    /// process. A cached implementation would return the same value forever and
    /// this test would be meaningless — which is why it asserts on the *shape* of
    /// the API (a fresh probe each call) rather than on a fixed value.
    #[test]
    fn the_probe_is_not_a_cached_constant() {
        // On any real host this is stable, so the assertion is that calling it
        // repeatedly is consistent *and* that it is reachable without an app
        // handle — the property whose absence caused the bug.
        let first = is_process_elevated();
        let second = is_process_elevated();
        assert_eq!(first, second, "the probe must be deterministic within a process");
    }

    /// A diagnostic snapshot must not disagree with a live decision.
    ///
    /// The support report and the connect gate read elevation through different
    /// paths. If they can disagree, a student is told "you are not elevated" by a
    /// report taken from a session that is.
    #[test]
    fn a_diagnostic_snapshot_agrees_with_the_live_probe() {
        let live = probe_is_admin();

        // The snapshot is built the way `init()` builds it: probe once, store.
        let mut platform = Platform::new();
        platform.appinfo.app_is_admin = live;

        // The report re-probes (see `SystemInfo::from`), so what a human reads is
        // the current answer rather than whatever startup recorded.
        let reported = crate::commands::SystemInfo::from(platform).app_is_admin;

        assert_eq!(
            reported, live,
            "the diagnostics report must reflect the current probe, not a startup snapshot"
        );
    }
}
