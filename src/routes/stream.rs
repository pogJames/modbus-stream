use axum::{
    extract::{Path as AxumPath, State, WebSocketUpgrade, ws::{Message, WebSocket}},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use futures::stream::StreamExt;
use std::{sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, warn};
use anyhow::Result;
use chrono::Utc;

use crate::{
    modbus::ModbusClient,
    types::{ErrorResponse, StreamStartRequest, StreamStatus, StreamType, WebSocketMessage},
    AppState,
};

// ── StreamManager ──────────────────────────────────────────────────────────────

struct StreamManager {
    modbus_client: Arc<RwLock<Option<ModbusClient>>>,
}

impl StreamManager {
    fn new(modbus_client: Arc<RwLock<Option<ModbusClient>>>) -> Self {
        Self { modbus_client }
    }

    async fn start_raw_streaming(
        &self,
        tx: broadcast::Sender<WebSocketMessage>,
        sensor_id: usize,
    ) -> Result<()> {
        info!("Starting raw data streaming for sensor {}", sensor_id);

        let mut sequence = 0u64;
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

            let result: Result<(u16, Vec<_>)> = if next_count <= 6 {
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
                        tokio::time::sleep(Duration::from_millis(1)).await;
                        continue;
                    }

                    let mid = data[data.len() / 2].clone();
                    let last = data[data.len() - 1].clone();
                    let data = if data.len() == 1 { vec![mid] } else { vec![mid, last] };

                    let message = WebSocketMessage::RawData {
                        sensor_id,
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

        Ok(())
    }
}

// ── Backwards-compat WebSocket handlers (sensor 0) ────────────────────────────

pub async fn websocket_raw_handler_compat(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    let Some(sensor) = state.sensors.get(0) else {
        return (StatusCode::SERVICE_UNAVAILABLE, "No sensors available").into_response();
    };
    let client = sensor.client.clone();
    ws.on_upgrade(move |socket| handle_raw_websocket(socket, client, 0)).into_response()
}

pub async fn websocket_metrics_handler_compat(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    let Some(sensor) = state.sensors.get(0) else {
        return (StatusCode::SERVICE_UNAVAILABLE, "No sensors available").into_response();
    };
    let metrics_tx = sensor.metrics_tx.clone();
    ws.on_upgrade(move |socket| handle_metrics_websocket(socket, metrics_tx)).into_response()
}

// ── Per-sensor WebSocket handlers ─────────────────────────────────────────────

pub async fn websocket_raw_handler(
    AxumPath(sensor_id): AxumPath<usize>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return (StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: format!("Sensor {} not found", sensor_id),
            code: Some("SENSOR_NOT_FOUND".to_string()),
            timestamp: chrono::Utc::now(),
        })).into_response();
    };
    let client = sensor.client.clone();
    ws.on_upgrade(move |socket| handle_raw_websocket(socket, client, sensor_id)).into_response()
}

pub async fn websocket_metrics_handler(
    AxumPath(sensor_id): AxumPath<usize>,
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return (StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: format!("Sensor {} not found", sensor_id),
            code: Some("SENSOR_NOT_FOUND".to_string()),
            timestamp: chrono::Utc::now(),
        })).into_response();
    };
    let metrics_tx = sensor.metrics_tx.clone();
    ws.on_upgrade(move |socket| handle_metrics_websocket(socket, metrics_tx)).into_response()
}

// ── Stream control stubs ───────────────────────────────────────────────────────

pub async fn start_stream(
    State(state): State<AppState>,
    Json(payload): Json<StreamStartRequest>,
) -> impl IntoResponse {
    match payload.stream_type {
        StreamType::Raw => {
            let baud = state.config.sensors.first().map(|s| s.baud_rate).unwrap_or(115200);
            if baud != 3000000 {
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

pub async fn stop_stream(State(_state): State<AppState>) -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "success": true, "message": "Streaming stopped" })))
}

pub async fn get_stream_status(State(_state): State<AppState>) -> impl IntoResponse {
    let status = StreamStatus {
        active: false,
        stream_type: None,
        start_time: None,
        sample_count: 0,
        buffer_utilization: 0.0,
    };
    Json(status)
}

// ── WebSocket connection handlers ──────────────────────────────────────────────

async fn handle_raw_websocket(
    mut socket: WebSocket,
    modbus_client: Arc<RwLock<Option<ModbusClient>>>,
    sensor_id: usize,
) {
    info!("New raw data WebSocket connection (sensor {})", sensor_id);

    let status_message = match serde_json::to_string(&WebSocketMessage::Status {
        connected: true,
        streaming: false,
    }) {
        Ok(msg) => msg,
        Err(e) => { error!("Failed to serialize status message: {}", e); return; }
    };

    if socket.send(Message::Text(status_message.into())).await.is_err() {
        error!("Failed to send initial status");
        return;
    }

    let stream_manager = Arc::new(StreamManager::new(modbus_client));
    let (tx, mut rx) = broadcast::channel(1000);
    let sm_clone = stream_manager.clone();

    let read_task = tokio::spawn(async move {
        if let Err(e) = sm_clone.start_raw_streaming(tx, sensor_id).await {
            error!("Raw streaming task failed: {}", e);
        }
    });

    loop {
        tokio::select! {
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
                    Some(Ok(Message::Close(_))) => { info!("WebSocket connection closed"); break; }
                    Some(Err(e)) => { error!("WebSocket error: {}", e); break; }
                    None => break,
                    _ => {}
                }
            }
            data = rx.recv() => {
                match data {
                    Ok(message) => {
                        let json = match serde_json::to_string(&message) {
                            Ok(j) => j,
                            Err(e) => { error!("Failed to serialize message: {}", e); continue; }
                        };
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            warn!("Failed to send data to WebSocket");
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("WebSocket client lagging, skipped {} messages", skipped);
                        let error_msg = WebSocketMessage::Error {
                            message: format!("Client lagging, skipped {} messages", skipped),
                            code: Some("CLIENT_LAGGING".to_string()),
                        };
                        if let Ok(json) = serde_json::to_string(&error_msg) {
                            let _ = socket.send(Message::Text(json.into())).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => { info!("Stream closed"); break; }
                }
            }
        }
    }

    read_task.abort();
    info!("Raw data WebSocket connection closed (sensor {})", sensor_id);
}

async fn handle_metrics_websocket(
    mut socket: WebSocket,
    metrics_tx: broadcast::Sender<WebSocketMessage>,
) {
    info!("New metrics WebSocket connection");

    let status_message = match serde_json::to_string(&WebSocketMessage::Status {
        connected: true,
        streaming: true,
    }) {
        Ok(msg) => msg,
        Err(e) => { error!("Failed to serialize status message: {}", e); return; }
    };

    if socket.send(Message::Text(status_message.into())).await.is_err() {
        return;
    }

    let mut rx = metrics_tx.subscribe();

    loop {
        tokio::select! {
            msg = socket.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if text == "ping" {
                            if socket.send(Message::Text("pong".into())).await.is_err() { break; }
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
                                if socket.send(Message::Text(json.into())).await.is_err() { break; }
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
