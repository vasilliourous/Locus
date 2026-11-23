use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;

use crate::{
    config::Config,
    core::{
        CoreManager, Timer,
        handle::Handle,
        hotkey::Hotkey,
        logger,
        service::{SERVICE_MANAGER, ServiceManager},
        tray::Tray,
    },
    feat,
    module::{auto_backup::AutoBackupManager, lightweight::auto_lightweight_boot},
    process::AsyncHandler,
    utils::{init, server, window_manager::WindowManager},
};
use clash_verge_logging::{Type, logging, logging_error};
use clash_verge_signal;

pub mod dns;
pub(crate) mod window;
mod window_script;

static RESOLVE_DONE: AtomicBool = AtomicBool::new(false);

pub(crate) fn init_work_dir_and_logger() -> anyhow::Result<()> {
    AsyncHandler::block_on(async {
        init_work_config().await;
        logging!(info, Type::Setup, "Initializing logger");
        logger::init().await?;
        Ok(())
    })
}

pub(crate) fn resolve_setup_sync() {
    AsyncHandler::spawn(|| async {
        AsyncHandler::spawn_blocking(server::embed_server);
        #[cfg(target_os = "linux")]
        AsyncHandler::spawn_blocking(watch_linux_theme_changed);
    });
}

pub(crate) fn resolve_setup_async() {
    AsyncHandler::spawn(resolve_setup);
}

#[tracing::instrument(skip_all, level = "info")]
async fn resolve_setup() {
    logging!(info, Type::App, "Version: {}", env!("CARGO_PKG_VERSION"));

    // Migrate before windows or timers can change the loaded config.
    logging_error!(Type::Setup, init::migrate_short_update_intervals().await);

    #[cfg(target_os = "macos")]
    resolve_dock_show().await;
    init_startup_script().await;
    init_service_manager().await;
    let config_initialized = init_verge_config_before_window().await;
    init_window().await;
    feat::reconcile_startup_tun_availability().await;
    init_resources().await;
    if let Err(e) = init::init_dns_config().await {
        logging!(warn, Type::Setup, "DNS config initialization failed: {}", e);
    }
    if config_initialized {
        init_verge_config().await;
    }
    Config::verify_config_initialization().await;

    // Live before the core starts, so a service appearing mid-start is not missed.
    #[cfg(target_os = "macos")]
    crate::core::network_watch::start();

    // A synchronous body in an `async` block: `join!` needs a future, and
    // `init_core_manager` no longer has an await of its own (it stopped starting
    // the core — see its doc comment). The block is the await point.
    let core_init = AsyncHandler::spawn(|| async {
        init_core_manager();
    });

    let _ = futures::join!(
        core_init,
        init_tray(),
        init_timer(),
        init_hotkey(),
        init_auto_lightweight_boot(),
        init_auto_backup(),
        init_chrysalis_heartbeat(),
        async { init_chrysalis_update_poll() },
    );

    // Before `refresh_tray_menu` and before the window is shown, in this order:
    // a launch that follows a failed install records `not-replaced`, and the
    // Account page reads that record. Spawned rather than awaited because hashing
    // the executable is file I/O on the start-up path, and a slow disk must not
    // delay the first paint — the verdict is only ever displayed.
    init_chrysalis_update_relabel();

    refresh_tray_menu().await;
    resolve_done();
}

pub async fn resolve_reset_async() -> Result<(), anyhow::Error> {
    CoreManager::global().stop_core().await?;

    #[cfg(target_os = "macos")]
    {
        use dns::restore_public_dns;
        restore_public_dns().await;
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn watch_linux_theme_changed() {
    match dark_light::subscribe() {
        Ok(watcher) => {
            for mode in watcher.iter() {
                use crate::core::notification;

                let theme = match mode {
                    dark_light::Mode::Light => tauri::Theme::Light,
                    dark_light::Mode::Dark => tauri::Theme::Dark,
                    dark_light::Mode::Unspecified => tauri::Theme::Light, // fallback to light
                };
                notification::NotificationSystem::send_event(
                    Handle::app_handle().clone(),
                    notification::FrontendEvent::ThemeChanged { theme },
                );
            }
        }
        Err(e) => {
            logging_error!(Type::Setup, "Fail to watch linux theme changed: {}", e);
        }
    }
}

async fn init_resources() {
    logging_error!(Type::Setup, init::init_resources().await);
}

async fn init_startup_script() {
    logging_error!(Type::Setup, init::startup_script().await);
}

async fn init_timer() {
    logging_error!(Type::Setup, Timer::global().init().await);
}

async fn init_hotkey() {
    let skip_register_hotkeys = !Config::verge().await.latest_arc().enable_global_hotkey.unwrap_or(true);
    logging_error!(Type::Setup, Hotkey::global().init(skip_register_hotkeys).await);
}

async fn init_auto_lightweight_boot() {
    logging_error!(Type::Setup, auto_lightweight_boot().await);
}

async fn init_auto_backup() {
    logging_error!(Type::Setup, AutoBackupManager::global().init().await);
}

/// Starts the periodic update check.
///
/// Deliberately NOT inside [`init_chrysalis_heartbeat`], and not gated on activation.
///
/// Two different reasons for the same placement:
///
///  * An **unactivated** device is the one most in need of an update — it cannot
///    connect, cannot reach the hub through the tunnel, and an update is the only
///    thing that can fix a broken build. The check itself is unauthenticated
///    (`/api/update`), so there is nothing to be entitled to.
///  * Chrysalis is a background VPN. "Check on launch" is approximately "check
///    never", because the app stays open for days — see
///    [`crate::chrysalis::update::poll`] for the cadence and its reasoning.
///
/// `handle` is available here; the heartbeat needs none because it is reached
/// through a stored credential rather than a live window.
fn init_chrysalis_update_poll() {
    crate::chrysalis::update::poll::start(Handle::app_handle().clone());
}

/// Settles whether the last update attempt actually replaced this application.
///
/// Runs at launch, before anything can offer another update, because this is the
/// first moment the answer exists: on Windows the installer is launched and the
/// process exits without ever returning, so the attempt itself cannot know its own
/// outcome. The record it left behind is compared against the running executable
/// here — see [`crate::chrysalis::update::relabel`], which is where the reasoning
/// lives.
///
/// Placed in `resolve_setup`'s join rather than in the heartbeat because it must
/// run for an **unactivated** device too: nothing about a failed install depends on
/// being entitled, and an unactivated device stranded on an old build is the case
/// an update is the only cure for.
fn init_chrysalis_update_relabel() {
    AsyncHandler::spawn(|| async {
        crate::chrysalis::update::relabel::classify_on_launch(env!("CARGO_PKG_VERSION")).await;
    });
}

/// Resumes the heartbeat for an already-activated device.
///
/// Started at launch rather than only at activation, because the common case is
/// a student opening the app on a device that was activated last term. Without
/// this the entitlement would never refresh, a suspension would go unnoticed,
/// and the grace period would silently run out.
async fn init_chrysalis_heartbeat() {
    // Rehydrate from the durable mirror BEFORE reading the record. An uninstall
    // or a config reset can leave `verge.yaml` without the code while the
    // machine-scoped copy still holds it; without this the device would resume
    // as if unactivated and send the student back to the code screen for a code
    // they already own. This also writes the recovered code back, so the launch
    // is not a one-off.
    let _ = crate::chrysalis::store::read_code_rehydrating().await;

    let verge = crate::config::Config::verge().await;
    let activation = match crate::chrysalis::store::read(&verge.latest_arc()) {
        Some(activation) => activation,
        None => {
            // Not activated yet is the normal first-run state, not an error.
            logging!(debug, Type::Setup, "[chrysalis] no activation stored; heartbeat idle");
            return;
        }
    };

    crate::chrysalis::runtime::start(activation);
}

pub(crate) fn init_signal() {
    logging!(debug, Type::Setup, "Initializing signal handlers...");
    clash_verge_signal::register(feat::quit);
}

async fn init_work_config() {
    logging_error!(Type::Setup, init::init_config().await);
}

async fn init_tray() {
    logging_error!(Type::Setup, Tray::global().init().await);
}

async fn init_verge_config() {
    logging_error!(Type::Setup, Config::init_runtime_config().await);
}

async fn init_verge_config_before_window() -> bool {
    let result = Config::init_config_before_window().await;
    let success = result.is_ok();
    logging_error!(Type::Setup, result);
    success
}

async fn init_service_manager() {
    clash_verge_service_ipc::set_config(Some(ServiceManager::config())).await;

    SERVICE_MANAGER.detect_startup_status().await;
}

/// Reports whether the core is running at launch. It is not, and that is the
/// point — see `CoreManager::init`.
///
/// Not `async` for the same reason its callee is not: it has nothing to await
/// any more, and `clippy -D warnings` rejects an `async fn` with no await. The
/// spawn site below wraps it in an `async` block, which supplies the await point
/// the `join!` needs.
fn init_core_manager() -> bool {
    match CoreManager::global().init() {
        Ok(initialized) => initialized,
        Err(error) => {
            logging!(error, Type::Setup, "core manager initialization failed: {error:#}");
            false
        }
    }
}

async fn refresh_tray_menu() {
    logging_error!(Type::Setup, Tray::global().update_part().await);
}

async fn init_window() {
    let is_silent_start = Config::verge().await.data_arc().enable_silent_start.unwrap_or(false);
    WindowManager::create_window(!is_silent_start).await;
}

#[cfg(target_os = "macos")]
async fn resolve_dock_show() {
    let is_silent_start = Config::verge().await.data_arc().enable_silent_start.unwrap_or(false);
    if is_silent_start {
        Handle::global().set_activation_policy_accessory();
    }
}

fn resolve_done() {
    RESOLVE_DONE.store(true, Ordering::Release);
}

pub(crate) fn is_resolve_done() -> bool {
    RESOLVE_DONE.load(Ordering::Acquire)
}
