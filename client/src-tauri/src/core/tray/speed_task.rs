// Every import below belongs to the macOS tray rate task. The one thing this
// module must provide on every platform is `TraySpeedController::
// is_traffic_task_running`, which needs none of them.
#[cfg(target_os = "macos")]
use crate::core::handle;
#[cfg(target_os = "macos")]
use crate::core::manager::traffic_probe;
#[cfg(target_os = "macos")]
use crate::process::AsyncHandler;
#[cfg(target_os = "macos")]
use crate::utils::connections_stream;
#[cfg(target_os = "macos")]
use crate::utils::tray_speed;
#[cfg(target_os = "macos")]
use crate::{Type, logging};
#[cfg(target_os = "macos")]
use parking_lot::Mutex;
#[cfg(target_os = "macos")]
use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::time::Duration;
#[cfg(target_os = "macos")]
use tauri::async_runtime::JoinHandle;
#[cfg(target_os = "macos")]
use tauri_plugin_mihomo::models::WsConnectionId;

#[cfg(target_os = "macos")]
const TRAY_SPEED_RETRY_DELAY: Duration = Duration::from_secs(1);
#[cfg(target_os = "macos")]
const TRAY_SPEED_STALE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct TraySpeedController {
    /// The running rate task, and the stream it owns.
    ///
    /// Present on every platform so `is_traffic_task_running` can be asked
    /// everywhere; only macOS ever puts a task in it.
    #[cfg(target_os = "macos")]
    speed_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    #[cfg(target_os = "macos")]
    speed_connection_id: Arc<Mutex<Option<WsConnectionId>>>,
}

// Not `#[derive(Default)]`: on macOS the two fields are `Arc<Mutex<Option<_>>>`,
// which derive fine, but the point of writing it out is that the non-macOS build
// has no fields at all and a derive would hide that the struct is deliberately
// empty there.
#[allow(clippy::derivable_impls)]
impl Default for TraySpeedController {
    fn default() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            speed_task: Arc::new(Mutex::new(None)),
            #[cfg(target_os = "macos")]
            speed_connection_id: Arc::new(Mutex::new(None)),
        }
    }
}

impl TraySpeedController {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(target_os = "macos")]
    pub fn update_task(&self, enable_tray_speed: bool) {
        if enable_tray_speed {
            self.start_task();
        } else {
            self.stop_task();
        }
    }

    #[cfg(target_os = "macos")]
    fn start_task(&self) {
        if handle::Handle::global().is_exiting() {
            return;
        }

        if !Self::has_main_tray() {
            logging!(warn, Type::Tray, "托盘不可用，跳过启动托盘速率任务");
            return;
        }

        let mut guard = self.speed_task.lock();
        if guard.as_ref().is_some_and(|task| !task.inner().is_finished()) {
            return;
        }

        let speed_connection_id = Arc::clone(&self.speed_connection_id);
        let task = AsyncHandler::spawn(move || async move {
            loop {
                if handle::Handle::global().is_exiting() {
                    break;
                }

                if !Self::has_main_tray() {
                    logging!(warn, Type::Tray, "托盘已不可用，停止托盘速率任务");
                    break;
                }

                let stream_connect_result = connections_stream::connect_traffic_stream().await;
                let mut speed_stream = match stream_connect_result {
                    Ok(stream) => stream,
                    Err(err) => {
                        logging!(debug, Type::Tray, "托盘速率流连接失败，稍后重试: {err}");
                        Self::blank_tray_title();
                        tokio::time::sleep(TRAY_SPEED_RETRY_DELAY).await;
                        continue;
                    }
                };

                Self::set_speed_connection_id(&speed_connection_id, Some(speed_stream.connection_id));

                loop {
                    let next_state = speed_stream
                        .next_event(TRAY_SPEED_STALE_TIMEOUT, || handle::Handle::global().is_exiting())
                        .await;

                    match next_state {
                        connections_stream::StreamConsumeState::Event(speed_event) => {
                            Self::apply_tray_speed(speed_event.up, speed_event.down);
                        }
                        connections_stream::StreamConsumeState::Stale => {
                            logging!(debug, Type::Tray, "托盘速率流长时间未收到有效数据，触发重连");
                            Self::blank_tray_title();
                            break;
                        }
                        connections_stream::StreamConsumeState::Closed
                        | connections_stream::StreamConsumeState::ExitRequested => {
                            break;
                        }
                    }
                }

                Self::disconnect_speed_connection(&speed_connection_id).await;

                if handle::Handle::global().is_exiting() || !Self::has_main_tray() {
                    break;
                }

                // `Stale` already reset the display; this covers remote close.
                Self::blank_tray_title();
                tokio::time::sleep(TRAY_SPEED_RETRY_DELAY).await;
            }

            Self::set_speed_connection_id(&speed_connection_id, None);
        });

        *guard = Some(task);
    }

    #[cfg(target_os = "macos")]
    fn stop_task(&self) {
        let task = self.speed_task.lock().take();
        let speed_connection_id = Arc::clone(&self.speed_connection_id);

        AsyncHandler::spawn(move || async move {
            // Await abort before disconnect so the task cannot consume and lose the connection ID.
            if let Some(task) = task {
                task.abort();
                let _ = task.await;
            }
            Self::disconnect_speed_connection(&speed_connection_id).await;
        });

        #[cfg(target_os = "macos")]
        {
            let app_handle = handle::Handle::app_handle();
            if let Some(tray) = app_handle.tray_by_id(super::TRAY_ID) {
                let result = tray.with_inner_tray_icon(|inner| {
                    if let Some(status_item) = inner.ns_status_item() {
                        tray_speed::clear_speed_attributed_title(&status_item);
                    }
                });
                if let Err(err) = result {
                    logging!(warn, Type::Tray, "清除富文本速率失败: {err}");
                }
            }
        }
    }

    /// Whether the tray rate task holds a subscription to the Core's `/traffic`
    /// stream right now.
    ///
    /// The connection screen needs the same numbers, and there is no reason to
    /// open a second websocket for a measurement that is already being fed. When
    /// this is `true` the screen reuses these numbers; when it is `false` — no
    /// tray, or the option is off — the screen subscribes itself.
    // Not `const`: on macOS this locks a mutex and inspects a task handle, which
    // clippy only reads as const because the non-macOS arm is a bare `false`.
    // Making it `const` would be a lie about the macOS branch.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn is_traffic_task_running(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.speed_task
                .lock()
                .as_ref()
                .is_some_and(|task| !task.inner().is_finished())
        }
        // No tray rate display exists off macOS, so nothing is being measured
        // there and the connection screen must take over.
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    #[cfg(target_os = "macos")]
    fn has_main_tray() -> bool {
        handle::Handle::app_handle().tray_by_id(super::TRAY_ID).is_some()
    }

    #[cfg(target_os = "macos")]
    fn set_speed_connection_id(
        speed_connection_id: &Arc<Mutex<Option<WsConnectionId>>>,
        connection_id: Option<WsConnectionId>,
    ) {
        *speed_connection_id.lock() = connection_id;
    }

    #[cfg(target_os = "macos")]
    fn take_speed_connection_id(speed_connection_id: &Arc<Mutex<Option<WsConnectionId>>>) -> Option<WsConnectionId> {
        speed_connection_id.lock().take()
    }

    #[cfg(target_os = "macos")]
    async fn disconnect_speed_connection(speed_connection_id: &Arc<Mutex<Option<WsConnectionId>>>) {
        if let Some(connection_id) = Self::take_speed_connection_id(speed_connection_id) {
            connections_stream::disconnect_connection(connection_id).await;
        }
    }

    /// Blanks the tray title without touching the shared measurement.
    ///
    /// Used wherever the tray wants an empty display: a failed connect, a stale
    /// stream, or a sweep before retrying. Those are statements about the tray,
    /// not about whether the tunnel is carrying bytes, and treating them as
    /// samples would let the tray's own housekeeping cancel the connection
    /// screen's proof — on macOS, with tray speed on, the only platform where
    /// both writers exist.
    #[cfg(target_os = "macos")]
    fn blank_tray_title() {
        traffic_probe::display_blanked();
        Self::paint_tray_title(0, 0);
    }

    #[cfg(target_os = "macos")]
    fn apply_tray_speed(up: u64, down: u64) {
        // Publish the *measurement* into the shared sink, so a machine that runs
        // this task feeds the connection screen as a side effect of painting the
        // tray: one subscription, two readers. Unconditional, unlike the paint
        // below — the paint needs a tray to exist, the measurement does not.
        //
        // A zero here is ambiguous and the two meanings must not be conflated:
        // `(0, 0)` from a *Core sample* is "quiet right now" and belongs in the
        // sink; `(0, 0)` from the tray blanking its title says nothing about the
        // tunnel and must not cancel a byte the Core reported. `blank_title`
        // carries that distinction, so the callers below can be read for which
        // one they mean.
        traffic_probe::publish_from_tray(up, down);

        Self::paint_tray_title(up, down);
    }

    /// Paints one rate pair into the tray title. Display only.
    #[cfg(target_os = "macos")]
    fn paint_tray_title(up: u64, down: u64) {
        let app_handle = handle::Handle::app_handle();
        if let Some(tray) = app_handle.tray_by_id(super::TRAY_ID) {
            let result = tray.with_inner_tray_icon(move |inner| {
                if let Some(status_item) = inner.ns_status_item() {
                    tray_speed::set_speed_attributed_title(&status_item, up, down);
                }
            });
            if let Err(err) = result {
                logging!(warn, Type::Tray, "设置富文本速率失败: {err}");
            }
        }
    }
}
