use axum::{
    extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}},
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

    /// Start raw data streaming task
    async fn start_raw_streaming(
        &self,
        tx: broadcast::Sender<WebSocketMessage>,
    ) -> Result<()> {
        info!("Starting raw data streaming");

        let mut sequence = 0u64;
        let mut interval = interval(Duration::from_millis(10)); // 100 Hz max

        loop {
            interval.tick().await;

            match self.read_raw_data_batch().await {
                Ok(data) => {
                    let message = WebSocketMessage::RawData {
                        timestamp: Utc::now(),
                        sequence,
                        data,
                    };

                    if let Err(e) = tx.send(message) {
                        // All receivers dropped
                        if matches!(e, broadcast::error::SendError(_)) {
                            info!("All raw data receivers dropped, stopping stream");
                            break;
                        }
                    }

                    sequence += 1;
                }
                Err(e) => {
                    error!("Failed to read raw data: {}", e);

                    let error_message = WebSocketMessage::Error {
                        message: format!("Failed to read raw data: {}", e),
                        code: Some("RAW_DATA_READ_ERROR".to_string()),
                    };

                    if let Err(send_error) = tx.send(error_message) {
                        if matches!(send_error, broadcast::error::SendError(_)) {
                            info!("All receivers dropped, stopping stream");
                            break;
                        }
                    }

                    // Wait a bit before retrying
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
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_raw_websocket(socket, state))
}

/// WebSocket handler for metrics streaming
pub async fn websocket_metrics_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_metrics_websocket(socket, state))
}

/// Start streaming
pub async fn start_stream(
    State(state): State<AppState>,
    Json(payload): Json<StreamStartRequest>,
) -> impl IntoResponse {
    // Implementation for starting streaming
    match payload.stream_type {
        StreamType::Raw => {
            if state.config.modbus.baud_rate != 3000000 {
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
pub async fn stop_stream(State(_state): State<AppState>) -> impl IntoResponse {
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
pub async fn get_stream_status(State(_state): State<AppState>) -> impl IntoResponse {
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
async fn handle_raw_websocket(mut socket: WebSocket, state: AppState) {
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
    let stream_manager = Arc::new(StreamManager::new(state.modbus_client.clone()));

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

/// Handle metrics WebSocket connection
async fn handle_metrics_websocket(mut socket: WebSocket, state: AppState) {
    info!("New metrics WebSocket connection");

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
    let stream_manager = Arc::new(StreamManager::new(state.modbus_client.clone()));

    // Start metrics streaming
    let (tx, mut rx) = broadcast::channel(100);
    let stream_manager_clone = stream_manager.clone();

    // Spawn task to periodically read metrics
    let read_task = tokio::spawn(async move {
        if let Err(e) = stream_manager_clone.start_metrics_streaming(tx).await {
            error!("Metrics streaming task failed: {}", e);
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
    info!("Metrics WebSocket connection closed");
}
