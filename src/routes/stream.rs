use axum::{
    extract::{Path, State, WebSocketUpgrade, ws::{Message, WebSocket}},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::{sync::Arc, time::Duration};
use tokio::{sync::broadcast, time::interval};
use tracing::{debug, error, info, warn};
use anyhow::Result;
use chrono::Utc;

use crate::{
    modbus::ModbusClient,
    types::{AccelerationData, ErrorResponse, StreamStartRequest, StreamStatus, StreamType, WebSocketMessage},
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
                let count = next_count.min(123);
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

                    // Downsample: keep middle + last sample per Modbus packet.
                    let mid = data[data.len() / 2].clone();
                    let last = data[data.len() - 1].clone();
                    let data = if data.len() == 1 { vec![mid] } else { vec![mid, last] };

                    let message = WebSocketMessage::RawData {
                        timestamp: Utc::now(),
                        sequence,
                        data,
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
                }
            }
        }

        info!("Raw data streaming stopped");
        Ok(())
    }

    /// Start metrics streaming task
    async fn start_metrics_streaming(
        &self,
        tx: broadcast::Sender<WebSocketMessage>,
    ) -> Result<()> {
        info!("Starting metrics streaming");

        let mut interval = interval(Duration::from_millis(200)); // 5 Hz

        loop {
            interval.tick().await;

            match self.read_metrics_data().await {
                Ok(message) => {
                    if let Err(e) = tx.send(message) {
                        // All receivers dropped
                        if matches!(e, broadcast::error::SendError(_)) {
                            info!("All metrics receivers dropped, stopping stream");
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to read metrics: {}", e);

                    let error_message = WebSocketMessage::Error {
                        message: format!("Failed to read metrics: {}", e),
                        code: Some("METRICS_READ_ERROR".to_string()),
                    };

                    if let Err(send_error) = tx.send(error_message) {
                        if matches!(send_error, broadcast::error::SendError(_)) {
                            info!("All receivers dropped, stopping stream");
                            break;
                        }
                    }

                    // Wait a bit before retrying
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }

        info!("Metrics streaming stopped");
        Ok(())
    }

    /// Read a batch of raw data from the sensor
    async fn read_raw_data_batch(&self) -> Result<Vec<AccelerationData>> {
        let client_guard = self.modbus_client.read().await;
        match &*client_guard {
            Some(client) => {
                // Check FIFO buffer size first
                let buffer_size = client.read_fifo_buffer_size().await?;

                if buffer_size == 0 {
                    debug!("FIFO buffer is empty");
                    return Ok(vec![]);
                }

                // Read up to the buffer size or maximum registers (123)
                let read_count = std::cmp::min(buffer_size, 123);
                let raw_data = client.read_raw_data_buffer(read_count).await?;

                debug!("Read {} raw data samples", raw_data.len());
                Ok(raw_data)
            }
            None => Err(anyhow::anyhow!("Modbus device not connected")),
        }
    }

    /// Read metrics data and create WebSocket message
    async fn read_metrics_data(&self) -> Result<WebSocketMessage> {
        let client_guard = self.modbus_client.read().await;
        match &*client_guard {
            Some(client) => {
                // Read all metrics in parallel for better performance
                let (gravity_result, velocity_result, temperature_result) = tokio::join!(
                    client.read_gravity_metrics(),
                    client.read_velocity_metrics(),
                    client.read_temperature()
                );

                let gravity = gravity_result?;
                let velocity = velocity_result?;
                let temperature = temperature_result?;

                Ok(WebSocketMessage::Metrics {
                    timestamp: Utc::now(),
                    gravity,
                    velocity,
                    temperature,
                })
            }
            None => Err(anyhow::anyhow!("Modbus device not connected")),
        }
    }
}

/// WebSocket handler for raw data streaming
pub async fn websocket_raw_handler(
    Path(sensor): Path<u8>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client = match sensor {
        1 => state.modbus_client1.clone(),
        2 => state.modbus_client2.clone(),
        _ => {
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
    let rx = match sensor {
        1 => state.metrics_tx.subscribe(),
        2 => state.metrics_tx2.subscribe(),
        _ => {
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
    let baud_rate = match sensor {
        1 => state.config.modbus1.baud_rate,
        2 => state.config.modbus2.baud_rate,
        _ => {
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

    // Clean up
    read_task.abort();
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
