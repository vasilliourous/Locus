#![cfg(target_os = "windows")]

use crate::utils::dirs;
use anyhow::{Result, bail};
use runas::Command as RunasCommand;
use std::process::Command as StdCommand;

pub fn invoke_uwptools() -> Result<()> {
    let resource_dir = dirs::app_resources_dir()?;
    let tool_path = resource_dir.join("enableLoopback.exe");

    if !tool_path.exists() {
        bail!("enableLoopback exe not found");
    }

    // Shared probe, not `deelevate::privilege_level()`: the app declares
    // `requireAdministrator`, so it is always elevated, and the old classifier
    // can report `NotPrivileged` for a non-UAC elevated token — which would spawn
    // a redundant `runas` (a second, pointless UAC prompt). See
    // `tauri_plugin_clash_verge_sysinfo::is_process_elevated`.
    if crate::utils::help::is_process_elevated() {
        StdCommand::new(tool_path).status()?;
    } else {
        RunasCommand::new(tool_path).status()?;
    }

    Ok(())
}
