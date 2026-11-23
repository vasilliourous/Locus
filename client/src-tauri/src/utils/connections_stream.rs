use crate::{Type, core::handle, logging};
use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;
use tauri_plugin_mihomo::models::WsConnectionId;
use tokio::sync::mpsc;
use tokio::time::Instant;

/// Bounds memory use if the consumer falls behind the WebSocket.
const MIHOMO_WS_STREAM_BUFFER_SIZE: usize = 8;
const MIHOMO_WS_STREAM_CLOSE_CODE: u64 = 1000;

#[derive(Debug, Clone, Copy)]
pub struct TrafficSpeedEvent {
    pub up: u64,
    pub down: u64,
    /// The Core's **lifetime** byte totals, for the free tier's allowance.
    ///
    /// `None` when the Core did not send them — an older engine, or a payload
    /// shape that omitted the fields. Absent means "we cannot bill this sample",
    /// never "zero bytes moved": [crate::chrysalis::usage::billable_delta]
    /// distinguishes the two, and conflating them would leave a stale baseline
    /// that bills the whole gap on the next sample.
    ///
    /// Deliberately NOT fed to [`crate::core::manager::traffic_probe`]. That
    /// module's whole design is that a lifetime total is a latch rather than
    /// evidence (`upTotal > 0` is a Core that moved a byte at *some* point), and
    /// it has tests pinning that a total is never read as activity. Metering is
    /// the opposite question — how much, ever — so it gets its own field and its
    /// own consumer, and neither can corrupt the other.
    pub totals: Option<TrafficTotals>,
}

/// mihomo's cumulative counters, as reported on the `/traffic` sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrafficTotals {
    pub up_total: u64,
    pub down_total: u64,
}

pub enum StreamConsumeState<T> {
    Event(T),
    Closed,
    Stale,
    ExitRequested,
}

enum InternalWsEvent<T> {
    Data(T),
}

pub struct MihomoWsEventStream<T> {
    pub connection_id: WsConnectionId,
    receiver: mpsc::Receiver<InternalWsEvent<T>>,
    last_valid_event_at: Instant,
}

#[derive(Deserialize)]
struct TrafficPayload {
    up: u64,
    down: u64,
    /// Renamed on the wire to `upTotal`. Optional so a sample that omits it is
    /// still a usable *speed* sample — the readiness probe needs the rates and
    /// must not go blind because a total is missing.
    #[serde(rename = "upTotal")]
    up_total: Option<u64>,
    /// Renamed on the wire to `downTotal`. See `up_total`.
    #[serde(rename = "downTotal")]
    down_total: Option<u64>,
}

fn parse_traffic_event(data: &[u8]) -> Option<InternalWsEvent<TrafficSpeedEvent>> {
    let payload = serde_json::from_slice::<TrafficPayload>(data).ok()?;
    // Both or neither: a half-populated total pair cannot be billed, because a
    // missing half is indistinguishable from a legitimate zero. Treating one as
    // zero would silently under-bill; billing the present half would over-bill
    // against a baseline that was never established.
    let totals = match (payload.up_total, payload.down_total) {
        (Some(up_total), Some(down_total)) => Some(TrafficTotals { up_total, down_total }),
        _ => None,
    };
    Some(InternalWsEvent::Data(TrafficSpeedEvent {
        up: payload.up,
        down: payload.down,
        totals,
    }))
}

fn try_send_internal_event<T>(message_tx: &mpsc::Sender<InternalWsEvent<T>>, event: InternalWsEvent<T>) {
    if let Err(err) = message_tx.try_send(event) {
        match err {
            // A later real-time sample supersedes one dropped from a full queue.
            tokio::sync::mpsc::error::TrySendError::Full(_) => {}
            tokio::sync::mpsc::error::TrySendError::Closed(_) => {}
        }
    }
}

pub async fn connect_traffic_stream() -> Result<MihomoWsEventStream<TrafficSpeedEvent>> {
    let (message_tx, message_rx) = mpsc::channel::<InternalWsEvent<TrafficSpeedEvent>>(MIHOMO_WS_STREAM_BUFFER_SIZE);
    let connection_id = handle::Handle::mihomo()
        .ws_traffic({
            let message_tx = message_tx.clone();
            move |message| {
                if let Some(event) = parse_traffic_event(&message) {
                    try_send_internal_event(&message_tx, event);
                }
            }
        })
        .await?;
    drop(message_tx);
    Ok(MihomoWsEventStream {
        connection_id,
        receiver: message_rx,
        last_valid_event_at: Instant::now(),
    })
}

impl<T> MihomoWsEventStream<T> {
    pub async fn next_event<F>(&mut self, stale_timeout: Duration, should_exit: F) -> StreamConsumeState<T>
    where
        F: Fn() -> bool,
    {
        let sleep = tokio::time::sleep(stale_timeout);
        tokio::pin!(sleep);

        loop {
            if should_exit() {
                return StreamConsumeState::ExitRequested;
            }

            tokio::select! {
                maybe_event = self.receiver.recv() => {
                    match maybe_event {
                        Some(InternalWsEvent::Data(event)) => {
                            self.last_valid_event_at = Instant::now();
                            sleep.as_mut().reset(self.last_valid_event_at + stale_timeout);
                            return StreamConsumeState::Event(event);
                        }
                        None => return StreamConsumeState::Closed,
                    }
                }
                _ = &mut sleep => {
                    if self.last_valid_event_at.elapsed() >= stale_timeout {
                        return StreamConsumeState::Stale;
                    }
                    sleep.as_mut().reset(self.last_valid_event_at + stale_timeout);
                }
            }
        }
    }
}

pub async fn disconnect_connection(connection_id: WsConnectionId) {
    if let Err(err) = handle::Handle::mihomo()
        .disconnect(connection_id, Some(MIHOMO_WS_STREAM_CLOSE_CODE))
        .await
    {
        logging!(debug, Type::Tray, "断开 Mihomo WebSocket 连接失败: {err}");
    }
}
