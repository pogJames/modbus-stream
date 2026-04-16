use axum::{
    extract::{Path, State, WebSocketUpgrade, ws::{Message, WebSocket}},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use futures::{sink::SinkExt, stream::StreamExt};
use rustfft::{FftPlanner, num_complex::Complex};
use std::{f32::consts::PI, sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use anyhow::Result;
use chrono::Utc;

const FFT_WINDOW: usize = 4096;
const SAMPLE_RATE_HZ: f32 = 7812.0;
const FFT_BIN_START: usize = 2;
const FFT_BIN_END:   usize = 262;

/// Apply a Hann window in-place to reduce spectral leakage.
fn apply_hann(buf: &mut [f32]) {
    let n = buf.len();
    for (i, s) in buf.iter_mut().enumerate() {
        let w = 0.5 * (1.0 - (2.0 * PI * i as f32 / (n - 1) as f32).cos());
        *s *= w;
    }
}

/// Compute the magnitude spectrum and return only bins `[FFT_BIN_START..=FFT_BIN_END]`.
/// Values are peak-amplitude scaled (2/N for non-DC bins).
fn fft_magnitudes(samples: &[f32]) -> Vec<f32> {
    let n = samples.len();
    let mut buf: Vec<Complex<f32>> = samples.iter().map(|&s| Complex { re: s, im: 0.0 }).collect();
    let mut planner = FftPlanner::new();
    planner.plan_fft_forward(n).process(&mut buf);
    let inv_n = 1.0 / n as f32;
    buf[FFT_BIN_START..=FFT_BIN_END]
        .iter()
        .map(|c| c.norm() * 2.0 * inv_n)
        .collect()
}

/// Build the frequency axis (Hz) for bins `[FFT_BIN_START..=FFT_BIN_END]`.
fn freq_bins(n: usize, sample_rate: f32) -> Vec<f32> {
    (FFT_BIN_START..=FFT_BIN_END)
        .map(|i| i as f32 * sample_rate / n as f32)
        .collect()
}

use crate::{
    modbus::ModbusClient,
    types::{ErrorResponse, StreamStartRequest, StreamStatus, StreamType, WebSocketMessage},
    AppState,
};

// StreamManager implementation inlined
struct StreamManager {
    modbus_client: Arc<tokio::sync::RwLock<Option<ModbusClient>>>,
}

impl StreamManager {
    fn new(modbus_client: Arc<tokio::sync::RwLock<Option<ModbusClient>>>) -> Self {
        Self { modbus_client }
    }

    /// Start raw data streaming task — pipeline style (one combined read per round trip).
    /// Uses the same technique as the vendor Python DAQ: reads FIFO size + data in a single
    /// Modbus transaction starting at 0x0002, using the previous cycle's size as the count.
    ///
    /// Emits two message types on every cycle:
    /// - `RawData` (downsampled, mid + last of each batch) for the time-domain plot.
    /// - `FftData` whenever the internal accumulation buffer fills `FFT_WINDOW` samples,
    ///   computed from the full-rate data before downsampling.
    async fn start_raw_streaming(
        &self,
        tx: broadcast::Sender<WebSocketMessage>,
    ) -> Result<()> {
        info!("Starting raw data streaming");

        let mut sequence = 0u64;
        // next_count: how many data registers to request on the next read.
        // Seeded from the current FIFO fill level; updated each cycle from the
        // buffer-size register that comes back with every combined read.
        let mut next_count: u16 = 0;

        // FFT accumulation buffers — collect full-rate samples before downsampling.
        let mut fft_x: Vec<f32> = Vec::with_capacity(FFT_WINDOW);
        let mut fft_y: Vec<f32> = Vec::with_capacity(FFT_WINDOW);
        let mut fft_z: Vec<f32> = Vec::with_capacity(FFT_WINDOW);
        let mut fft_seq: u64 = 0;

        loop {
            let client_guard = self.modbus_client.read().await;
            let client = match &*client_guard {
                Some(c) => c,
                None => {
                    drop(client_guard);
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    continue;
                }
            };

            // One Modbus round trip: size register + data
            let result: Result<(u16, Vec<_>)> = if next_count <= 6 {
                // Buffer nearly empty — just refresh the size, skip data this cycle
                client.read_fifo_buffer_size().await.map(|sz| (sz, vec![]))
            } else {
                let count = crate::fifo_read_count(next_count);
                client.read_fifo_combined(count).await
            };

            drop(client_guard);

            match result {
                Ok((new_size, data)) => {
                    next_count = new_size;

                    if data.is_empty() {
                        // Buffer was too small; yield briefly then try again
                        tokio::time::sleep(Duration::from_millis(1)).await;
                        continue;
                    }

                    // --- Accumulate full-rate samples for FFT ---
                    for sample in &data {
                        fft_x.push(sample.x as f32);
                        fft_y.push(sample.y as f32);
                        fft_z.push(sample.z as f32);
                    }

                    // Emit an FFT frame whenever we have a full window.
                    while fft_x.len() >= FFT_WINDOW {
                        let mut wx: Vec<f32> = fft_x.drain(..FFT_WINDOW).collect();
                        let mut wy: Vec<f32> = fft_y.drain(..FFT_WINDOW).collect();
                        let mut wz: Vec<f32> = fft_z.drain(..FFT_WINDOW).collect();

                        apply_hann(&mut wx);
                        apply_hann(&mut wy);
                        apply_hann(&mut wz);

                        let fft_msg = WebSocketMessage::FftData {
                            timestamp: Utc::now(),
                            sequence: fft_seq,
                            window: FFT_WINDOW,
                            sample_rate_hz: SAMPLE_RATE_HZ,
                            frequencies: freq_bins(FFT_WINDOW, SAMPLE_RATE_HZ),
                            x: fft_magnitudes(&wx),
                            y: fft_magnitudes(&wy),
                            z: fft_magnitudes(&wz),
                        };

                        if let Err(broadcast::error::SendError(_)) = tx.send(fft_msg) {
                            info!("All raw data receivers dropped, stopping stream");
                            return Ok(());
                        }
                        fft_seq += 1;
                    }

                    // --- Downsample: keep middle + last sample per Modbus packet ---
                    let mid = data[data.len() / 2].clone();
                    let last = data[data.len() - 1].clone();
                    let downsampled = if data.len() == 1 { vec![mid] } else { vec![mid, last] };

                    let message = WebSocketMessage::RawData {
                        timestamp: Utc::now(),
                        sequence,
                        data: downsampled,
                    };

                    if let Err(broadcast::error::SendError(_)) = tx.send(message) {
                        info!("All raw data receivers dropped, stopping stream");
                        break;
                    }

                    sequence += 1;
                }
                Err(e) => {
                    error!("Raw data read failed: {}", e);
                    next_count = 0;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    if tx.receiver_count() == 0 {
                        break;
                    }
                }
            }
        }

        info!("Raw data streaming stopped");
        Ok(())
    }

}

/// WebSocket handler for raw data streaming
pub async fn websocket_raw_handler(
    Path(sensor): Path<u8>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let idx = (sensor as usize).wrapping_sub(1);
    let client = match state.modbus_clients.get(idx) {
        Some(c) => c.clone(),
        None => {
            return (StatusCode::NOT_FOUND, Json(ErrorResponse {
                error: format!("Unknown sensor: {}", sensor),
                code: Some("UNKNOWN_SENSOR".to_string()),
                timestamp: chrono::Utc::now(),
            })).into_response();
        }
    };
    ws.on_upgrade(|socket| handle_raw_websocket(socket, client))
}

/// WebSocket handler for metrics streaming
pub async fn websocket_metrics_handler(
    Path(sensor): Path<u8>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let idx = (sensor as usize).wrapping_sub(1);
    let rx = match state.metrics_txs.get(idx) {
        Some(tx) => tx.subscribe(),
        None => {
            return (StatusCode::NOT_FOUND, Json(ErrorResponse {
                error: format!("Unknown sensor: {}", sensor),
                code: Some("UNKNOWN_SENSOR".to_string()),
                timestamp: chrono::Utc::now(),
            })).into_response();
        }
    };
    ws.on_upgrade(|socket| handle_metrics_websocket(socket, rx))
}

/// Start streaming
pub async fn start_stream(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
    Json(payload): Json<StreamStartRequest>,
) -> impl IntoResponse {
    let idx = (sensor as usize).wrapping_sub(1);
    let baud_rate = match state.config.sensors.get(idx) {
        Some(s) => s.baud_rate,
        None => {
            return (StatusCode::NOT_FOUND, Json(ErrorResponse {
                error: format!("Unknown sensor: {}", sensor),
                code: Some("UNKNOWN_SENSOR".to_string()),
                timestamp: chrono::Utc::now(),
            })).into_response();
        }
    };

    match payload.stream_type {
        StreamType::Raw => {
            if baud_rate != 3000000 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Raw data streaming requires 3 Mbps baud rate".to_string(),
                        code: Some("INSUFFICIENT_BAUD_RATE".to_string()),
                        timestamp: chrono::Utc::now(),
                    }),
                )
                    .into_response();
            }
        }
        _ => {}
    }

    info!("Starting {:?} stream", payload.stream_type);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": format!("Started {:?} streaming", payload.stream_type),
            "type": payload.stream_type
        })),
    )
        .into_response()
}

/// Stop streaming
pub async fn stop_stream(
    Path(sensor): Path<u8>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    info!("Stopping stream");
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "Streaming stopped"
        })),
    )
        .into_response()
}

/// Get stream status
pub async fn get_stream_status(
    Path(sensor): Path<u8>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // For now, return a static status
    // In a real implementation, this would check the actual streaming state
    let status = StreamStatus {
        active: false,
        stream_type: None,
        start_time: None,
        sample_count: 0,
        buffer_utilization: 0.0,
    };

    Json(status).into_response()
}

/// Handle raw data WebSocket connection
async fn handle_raw_websocket(mut socket: WebSocket, client: Arc<tokio::sync::RwLock<Option<ModbusClient>>>) {
    info!("New raw data WebSocket connection");

    // Send initial status
    let status_message = match serde_json::to_string(&WebSocketMessage::Status {
        connected: true,
        streaming: false,
    }) {
        Ok(msg) => msg,
        Err(e) => {
            error!("Failed to serialize status message: {}", e);
            return;
        }
    };

    if socket.send(Message::Text(status_message.into())).await.is_err() {
        error!("Failed to send initial status");
        return;
    }

    // Create a stream manager for this connection
    let stream_manager = Arc::new(StreamManager::new(client));

    // Start raw data streaming
    let (tx, mut rx) = broadcast::channel(1000);
    let stream_manager_clone = stream_manager.clone();

    // Spawn task to continuously read raw data
    let read_task = tokio::spawn(async move {
        if let Err(e) = stream_manager_clone.start_raw_streaming(tx).await {
            error!("Raw streaming task failed: {}", e);
        }
    });

    // Handle WebSocket messages and forward stream data
    loop {
        tokio::select! {
            // Handle incoming WebSocket messages
            msg = socket.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if text == "ping" {
                            if socket.send(Message::Text("pong".into())).await.is_err() {
                                warn!("Failed to send pong");
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("WebSocket connection closed");
                        break;
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
            // Forward stream data to WebSocket
            data = rx.recv() => {
                match data {
                    Ok(message) => {
                        let json = match serde_json::to_string(&message) {
                            Ok(json) => json,
                            Err(e) => {
                                error!("Failed to serialize message: {}", e);
                                continue;
                            }
                        };

                        if socket.send(Message::Text(json.into())).await.is_err() {
                            warn!("Failed to send data to WebSocket");
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("WebSocket client lagging, skipped {} messages", skipped);
                        // Send error message to client
                        let error_msg = WebSocketMessage::Error {
                            message: format!("Client lagging, skipped {} messages", skipped),
                            code: Some("CLIENT_LAGGING".to_string()),
                        };
                        if let Ok(json) = serde_json::to_string(&error_msg) {
                            let _ = socket.send(Message::Text(json.into())).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Stream closed");
                        break;
                    }
                }
            }
        }
    }

    // Let the read task self-terminate: now that rx is dropped, the next
    // tx.send() inside start_raw_streaming will return SendError and the
    // task will exit cleanly after finishing its current Modbus transaction.
    // Calling abort() here would cancel the task mid-transaction, leaving
    // stale response bytes in the serial buffer that corrupt the next session.
    drop(read_task);
    info!("Raw data WebSocket connection closed");
}

/// Handle metrics WebSocket connection — subscribes to the shared background reader in AppState.
async fn handle_metrics_websocket(mut socket: WebSocket, mut rx: broadcast::Receiver<WebSocketMessage>) {
    info!("New metrics WebSocket connection");

    let status_message = match serde_json::to_string(&WebSocketMessage::Status {
        connected: true,
        streaming: true,
    }) {
        Ok(msg) => msg,
        Err(e) => {
            error!("Failed to serialize status message: {}", e);
            return;
        }
    };

    if socket.send(Message::Text(status_message.into())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            msg = socket.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if text == "ping" {
                            if socket.send(Message::Text("pong".into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            data = rx.recv() => {
                match data {
                    Ok(message) => {
                        match serde_json::to_string(&message) {
                            Ok(json) => {
                                if socket.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => error!("Failed to serialize metrics: {}", e),
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Metrics WebSocket lagged, skipped {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    info!("Metrics WebSocket connection closed");
}
