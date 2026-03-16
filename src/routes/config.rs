use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use tracing::{error, info};

use crate::{
    types::{
        BaudRateRequest, ConfigResponse, ErrorResponse, HighPassFilterRequest, SampleRateRequest,
        StreamSizeRequest,
    },
    AppState,
};

/// Helper to handle when device is not connected
async fn handle_no_device() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "Modbus device not connected".to_string(),
            code: Some("DEVICE_NOT_CONNECTED".to_string()),
            timestamp: chrono::Utc::now(),
        }),
    )
}

/// Get current configuration
pub async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    match read_current_config(&state).await {
        Ok(config) => Json(config).into_response(),
        Err(e) if e.to_string().contains("not connected") => handle_no_device().await.into_response(),
        Err(e) => {
            error!("Failed to read configuration: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to read configuration: {}", e),
                    code: Some("CONFIG_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                }),
            )
                .into_response()
        }
    }
}

/// Set sample rate
pub async fn set_sample_rate(
    State(state): State<AppState>,
    Json(payload): Json<SampleRateRequest>,
) -> impl IntoResponse {
    let client_guard = match state.modbus_clients.first() {
        Some(c) => c.read().await,
        None => return handle_no_device().await.into_response(),
    };
    match &*client_guard {
        Some(client) => {
            match client.set_sample_rate(payload.sample_rate).await {
                Ok(()) => {
                    info!("Sample rate set to {} sps", payload.sample_rate);
                    (StatusCode::OK, Json(serde_json::json!({
                        "success": true,
                        "message": format!("Sample rate set to {} sps", payload.sample_rate),
                        "sampleRate": payload.sample_rate
                    }))).into_response()
                }
                Err(e) => {
                    error!("Failed to set sample rate: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to set sample rate: {}", e),
                            code: Some("SAMPLE_RATE_ERROR".to_string()),
                            timestamp: chrono::Utc::now(),
                        }),
                    )
                        .into_response()
                }
            }
        }
        None => handle_no_device().await.into_response(),
    }
}

/// Set baud rate (requires power cycle)
pub async fn set_baud_rate(
    State(state): State<AppState>,
    Json(payload): Json<BaudRateRequest>,
) -> impl IntoResponse {
    // Validate baud rate
    if ![115200, 3000000].contains(&payload.baud_rate) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid baud rate. Supported rates: 115200, 3000000".to_string(),
                code: Some("INVALID_BAUD_RATE".to_string()),
                timestamp: chrono::Utc::now(),
            }),
        )
            .into_response();
    }

    let client_guard = match state.modbus_clients.first() {
        Some(c) => c.read().await,
        None => return handle_no_device().await.into_response(),
    };
    match &*client_guard {
        Some(client) => {
            match client.set_baud_rate(payload.baud_rate).await {
                Ok(()) => {
                    info!("Baud rate set to {} bps", payload.baud_rate);
                    (StatusCode::OK, Json(serde_json::json!({
                        "success": true,
                        "message": format!("Baud rate set to {} bps. Power cycle the sensor to take effect.", payload.baud_rate),
                        "baudRate": payload.baud_rate,
                        "powerCycleRequired": true
                    }))).into_response()
                }
                Err(e) => {
                    error!("Failed to set baud rate: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to set baud rate: {}", e),
                            code: Some("BAUD_RATE_ERROR".to_string()),
                            timestamp: chrono::Utc::now(),
                        }),
                    )
                        .into_response()
                }
            }
        }
        None => handle_no_device().await.into_response(),
    }
}

/// Set high pass filter
pub async fn set_high_pass_filter(
    State(state): State<AppState>,
    Json(payload): Json<HighPassFilterRequest>,
) -> impl IntoResponse {
    let client_guard = match state.modbus_clients.first() {
        Some(c) => c.read().await,
        None => return handle_no_device().await.into_response(),
    };
    match &*client_guard {
        Some(client) => {
            match client.set_high_pass_filter(payload.enabled).await {
                Ok(()) => {
                    info!("High pass filter: {}", if payload.enabled { "enabled" } else { "disabled" });
                    (StatusCode::OK, Json(serde_json::json!({
                        "success": true,
                        "message": format!("High pass filter {}", if payload.enabled { "enabled" } else { "disabled" }),
                        "enabled": payload.enabled
                    }))).into_response()
                }
                Err(e) => {
                    error!("Failed to set high pass filter: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to set high pass filter: {}", e),
                            code: Some("HIGH_PASS_FILTER_ERROR".to_string()),
                            timestamp: chrono::Utc::now(),
                        }),
                    )
                        .into_response()
                }
            }
        }
        None => handle_no_device().await.into_response(),
    }
}

/// Set stream size
pub async fn set_stream_size(
    State(state): State<AppState>,
    Json(payload): Json<StreamSizeRequest>,
) -> impl IntoResponse {
    if payload.stream_size > 123 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Stream size cannot exceed 123 registers".to_string(),
                code: Some("INVALID_STREAM_SIZE".to_string()),
                timestamp: chrono::Utc::now(),
            }),
        )
            .into_response();
    }

    let client_guard = match state.modbus_clients.first() {
        Some(c) => c.read().await,
        None => return handle_no_device().await.into_response(),
    };
    match &*client_guard {
        Some(client) => {
            match client.set_stream_size(payload.stream_size).await {
                Ok(()) => {
                    info!("Stream size set to {} registers", payload.stream_size);
                    (StatusCode::OK, Json(serde_json::json!({
                        "success": true,
                        "message": format!("Stream size set to {} registers", payload.stream_size),
                        "streamSize": payload.stream_size
                    }))).into_response()
                }
                Err(e) => {
                    error!("Failed to set stream size: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to set stream size: {}", e),
                            code: Some("STREAM_SIZE_ERROR".to_string()),
                            timestamp: chrono::Utc::now(),
                        }),
                    )
                        .into_response()
                }
            }
        }
        None => handle_no_device().await.into_response(),
    }
}

/// Helper function to read current configuration
async fn read_current_config(state: &AppState) -> anyhow::Result<ConfigResponse> {
    let client_arc = state.modbus_clients.first()
        .ok_or_else(|| anyhow::anyhow!("No sensors configured"))?;
    let client_guard = client_arc.read().await;
    match &*client_guard {
        Some(client) => {
            let temperature = client.read_temperature().await?;
            let firmware_version = client.read_firmware_version().await?;
            let ucid = client.read_ucid().await?;

            // Note: We can't directly read sample rate, baud rate, etc. from the sensor
            // so we'll use default/configured values
            Ok(ConfigResponse {
                sample_rate: 7812, // Default for I-type sensors
                baud_rate: state.config.sensors.first().map(|s| s.baud_rate).unwrap_or(115200),
                high_pass_filter: false, // Default state
                stream_size: 123, // Maximum
                temperature,
                firmware_version,
                ucid,
            })
        }
        None => Err(anyhow::anyhow!("Modbus device not connected")),
    }
}
