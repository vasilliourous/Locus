use parking_lot::RwLock;
use serde::Serialize;
use tauri::{AppHandle, Runtime, State, command};
use tauri_plugin_clipboard_manager::{ClipboardExt as _, Error};

use crate::Platform;

#[derive(Serialize)]
pub struct SystemInfo {
    pub system_name: String,
    pub system_version: String,
    pub system_kernel_version: String,
    pub system_arch: String,
    pub app_version: String,
    pub app_core_mode: String,
    pub app_is_admin: bool,
}

impl From<Platform> for SystemInfo {
    fn from(platform: Platform) -> Self {
        Self {
            system_name: platform.sysinfo.system_name,
            system_version: platform.sysinfo.system_version,
            system_kernel_version: platform.sysinfo.system_kernel_version,
            system_arch: platform.sysinfo.system_arch,
            app_version: platform.appinfo.app_version,
            app_core_mode: platform.appinfo.app_core_mode,
            // Re-probed, NOT read from the cached field.
            //
            // The cached value is the one written at startup, and a support
            // report showing `Is Admin: false` for a session the user ran
            // elevated is worse than no line at all: it contradicts what they
            // did and sends the investigation the wrong way. The cached field
            // remains on `Platform` for the startup snapshot, but anything a
            // human reads must be current.
            app_is_admin: crate::probe_is_admin(),
        }
    }
}

#[command]
pub fn get_system_info(state: State<'_, RwLock<Platform>>) -> Result<SystemInfo, Error> {
    let platform = state.inner().read();
    Ok(SystemInfo::from(platform.clone()))
}

/// 获取应用的运行时间（毫秒）
#[command]
pub fn get_app_uptime(state: State<'_, RwLock<Platform>>) -> Result<u128, Error> {
    Ok(state.inner().read().appinfo.app_startup_time.elapsed().as_millis())
}

#[command]
pub fn export_diagnostic_info<R: Runtime>(
    app_handle: AppHandle<R>,
    state: State<'_, RwLock<Platform>>,
) -> Result<(), Error> {
    let mut platform = state.inner().read().clone();
    // Same reason as `SystemInfo::from`: the report is read by a human during a
    // support conversation, so the elevation line must be the current answer.
    platform.appinfo.app_is_admin = crate::probe_is_admin();
    let clipboard = app_handle.clipboard();
    clipboard.write_text(platform.to_string())
}
