#[cfg(test)]
use super::claim_core_readiness_generation;
use super::{CoreManager, RunningMode};
use crate::{
    config::Config,
    core::{handle, service},
    utils::dirs,
};
use anyhow::Result;
use std::path::Path;
use tauri_plugin_mihomo::MihomoExt as _;

/// Starts the Core through the Service, retrying a transient refusal.
///
/// Windows-only because the Service is the only route to the Core there, and the
/// failure it retries is specific to that handshake: the Service releases the
/// ext-controller channel slightly AFTER the previous stop returns, so the first
/// start attempt can lose the race and succeed a moment later. Off Windows the
/// Service is not involved in the same way, so there is nothing to retry.
///
/// A refusal about the **bundle** is not retried. `StageRequest::is_about_the_bundle`
/// identifies one that would be presented identically on every attempt — a core the
/// Service will not accept no matter how often we ask — so retrying it would only
/// delay an error the student has to act on.
///
/// This is the helper behind `SERVICE_START_RETRIES`; deleting it and leaving the
/// call site compiles everywhere except a Windows target, which is why CI caught it
/// and a Linux `cargo check` did not.
#[cfg(target_os = "windows")]
async fn retry_service_start<Start, StartFuture>(
    attempts: usize,
    retry_delay: std::time::Duration,
    mut start: Start,
) -> Result<()>
where
    Start: FnMut() -> StartFuture,
    StartFuture: std::future::Future<Output = Result<()>>,
{
    use clash_verge_logging::{Type, logging};

    let mut last_error = None;
    for attempt in 0..attempts {
        match start().await {
            Ok(()) => return Ok(()),
            Err(error) => {
                logging!(
                    warn,
                    Type::Core,
                    "service start attempt {}/{} failed: {error:#}",
                    attempt + 1,
                    attempts
                );
                if error
                    .downcast_ref::<service::ServiceStartRefusal>()
                    .is_some_and(|refusal| service::StageRequest::is_about_the_bundle(refusal.code))
                {
                    return Err(error);
                }
                last_error = Some(error);
                if attempt + 1 < attempts {
                    tokio::time::sleep(retry_delay).await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("service start failed")))
}

impl CoreManager {
    pub async fn get_clash_logs(&self) -> Result<Vec<String>> {
        match *self.get_running_mode() {
            RunningMode::Service => service::get_clash_logs_by_service().await,
            RunningMode::NotRunning => Ok(Vec::new()),
        }
    }

    pub(super) async fn start_core_by_service(&self) -> Result<()> {
        self.core_starting();
        let service_ipc = dirs::ipc_path()?;
        let config_file = Config::generate_file().await?;
        handle::Handle::app_handle()
            .mihomo()
            .update_socket_path(dirs::path_to_str(&service_ipc)?.to_owned())?;

        self.start_core_by_service_with_config(&config_file).await
    }

    #[tracing::instrument(skip_all, level = "info", fields(config_file = %config_file.display()))]
    pub(super) async fn start_core_by_service_with_config(&self, config_file: &Path) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            use crate::constants::timing;
            retry_service_start(timing::SERVICE_START_RETRIES, timing::SERVICE_START_RETRY_DELAY, || {
                service::run_core_by_service(config_file)
            })
            .await?;
            self.mark_core_ready();
            self.core_started(RunningMode::Service);
            crate::config::profiles::restore_selected_nodes().await;
            service::request_runtime_provider_sync(timing::RUNTIME_PROVIDER_SYNC_DELAY);
            Ok(())
        }

        #[cfg(not(target_os = "windows"))]
        {
            service::run_core_by_service(config_file).await?;
            self.mark_core_ready();
            self.core_started(RunningMode::Service);
            crate::config::profiles::restore_selected_nodes().await;
            service::request_runtime_provider_sync(crate::constants::timing::RUNTIME_PROVIDER_SYNC_DELAY);
            Ok(())
        }
    }

    pub(super) async fn stop_core_by_service(&self) -> Result<()> {
        service::stop_core_by_service().await?;
        self.core_stopped();
        Ok(())
    }

}

/// Drops a `cache.db` the current user cannot write before handing the directory to the core.
///
/// mihomo keeps `profile.store-selected` in `cache.db` inside its data directory. Service builds
/// before the runtime staging rework ran the core as root against this same directory without a
/// umask, leaving the file as `root:staff 0644`: still readable, so the core loads stale
/// selections, but never writable again, so it silently stops recording new ones. Nothing
/// repairs it either, because the service-side cleanup only runs inside the service. Removing it
/// lets the core recreate the cache under the current user; the fake-ip leases and frozen
/// selections that go with it could not be updated anyway.
#[cfg(all(unix, test))]
fn discard_unwritable_core_cache(config_dir: &Path) {
    use clash_verge_logging::{Type, logging};

    let cache = config_dir.join("cache.db");
    // Appending neither creates nor truncates, so this only asks whether a write would be allowed.
    match std::fs::OpenOptions::new().append(true).open(&cache) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            // Unlinking is governed by the directory, which the current user owns.
            match std::fs::remove_file(&cache) {
                Ok(()) => logging!(
                    info,
                    Type::Core,
                    "Discarded a core cache the current user cannot write: {}",
                    cache.display()
                ),
                Err(error) => logging!(
                    warn,
                    Type::Core,
                    "Failed to discard the unwritable core cache {}: {error}",
                    cache.display()
                ),
            }
        }
        Err(error) => logging!(
            warn,
            Type::Core,
            "Failed to probe the core cache {}: {error}",
            cache.display()
        ),
    }
}

#[cfg(all(test, unix))]
mod core_cache_tests {
    use super::discard_unwritable_core_cache;
    use std::os::unix::fs::PermissionsExt as _;

    fn scratch(name: &str) -> anyhow::Result<std::path::PathBuf> {
        let root = std::env::temp_dir().join(format!("clash-verge-core-cache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        Ok(root)
    }

    #[test]
    fn an_unwritable_cache_is_discarded() -> anyhow::Result<()> {
        // root ignores the permission bits, so the probe cannot fail there.
        if unsafe { tauri_plugin_clash_verge_sysinfo::libc::geteuid() } == 0 {
            return Ok(());
        }
        let root = scratch("unwritable")?;
        let cache = root.join("cache.db");
        std::fs::write(&cache, b"stale")?;
        std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o444))?;

        discard_unwritable_core_cache(&root);

        assert!(!cache.exists(), "an unwritable cache must not be handed to the core");
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn a_writable_cache_is_kept() -> anyhow::Result<()> {
        let root = scratch("writable")?;
        let cache = root.join("cache.db");
        std::fs::write(&cache, b"live")?;

        discard_unwritable_core_cache(&root);

        assert_eq!(
            std::fs::read(&cache)?,
            b"live",
            "a writable cache carries the stored selections and must survive"
        );
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn a_missing_cache_is_not_an_error() -> anyhow::Result<()> {
        let root = scratch("missing")?;

        discard_unwritable_core_cache(&root);

        assert!(!root.join("cache.db").exists());
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }
}

#[cfg(test)]
mod readiness_tests {
    use super::claim_core_readiness_generation;
    use crate::core::manager::{CoreManager, RunningMode};
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn core_readiness_generation_can_only_be_claimed_once() {
        let generation = AtomicU64::new(7);

        assert!(claim_core_readiness_generation(&generation, 7));
        assert_eq!(generation.load(Ordering::Acquire), 8);
        assert!(!claim_core_readiness_generation(&generation, 7));
    }

    #[test]
    fn invalidated_core_readiness_cannot_be_recaptured_from_stale_mode() {
        let manager = CoreManager::isolated();
        manager.mark_core_ready();
        manager.core_started(RunningMode::Service);

        manager.invalidate_core_readiness();

        assert_eq!(*manager.get_running_mode(), RunningMode::Service);
        assert_eq!(manager.current_core_readiness_generation(), None);
    }
}
