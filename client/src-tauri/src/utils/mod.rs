// Not macOS-only any more: the connection screen's traffic measurement reads the
// Core's `/traffic` stream on every platform, and the tray rate task (macOS-only)
// is now just one of its two writers. See `core::manager::traffic_probe`.
pub mod connections_stream;
pub mod dirs;
pub mod help;
pub mod init;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos_launch_guard;
pub(crate) mod network;
pub mod notification;
pub mod port;
pub mod resolve;
#[cfg(target_os = "windows")]
pub mod schtasks;
pub mod server;
pub mod singleton;
pub mod speed;
pub(crate) mod startup;
pub mod tmpl;
#[cfg(target_os = "macos")]
pub mod tray_speed;
pub mod window_manager;
pub mod yaml_emitter;
