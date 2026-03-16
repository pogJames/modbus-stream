use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use tracing::error;

use crate::{types::ErrorResponse, AppState};

/// Get latest raw data for all sensors combined
pub async fn get_latest_raw_combined(State(state): State<AppState>) -> impl IntoResponse {
    let mut map = serde_json::Map::new();
    for (i, client_arc) in state.modbus_clients.iter().enumerate() {
        let g = client_arc.read().await;
        let val = match &*g {
            Some(c) => c.read_latest_raw().await.ok(),
            None => None,
        };
        map.insert(
            format!("sensor{}", i + 1),
            serde_json::to_value(val).unwrap_or(serde_json::Value::Null),
        );
    }
    Json(serde_json::Value::Object(map))
}

macro_rules! resolve_client {
    ($sensor:expr, $state:expr) => {{
        let _idx = ($sensor as usize).wrapping_sub(1);
        if _idx >= $state.modbus_clients.len() {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Unknown sensor: {}", $sensor),
                    code: Some("UNKNOWN_SENSOR".to_string()),
                    timestamp: chrono::Utc::now(),
                }),
            ).into_response();
        }
        &$state.modbus_clients[_idx]
    }};
}

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

/// Get temperature
pub async fn get_temperature(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_temperature().await {
                Ok(temperature) => Json(serde_json::json!({
                    "temperature": temperature,
                    "unit": "°C",
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read temperature: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read temperature: {}", e),
                            code: Some("TEMPERATURE_READ_ERROR".to_string()),
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

/// Get UCID information
pub async fn get_ucid(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_ucid().await {
                Ok(ucid) => Json(ucid).into_response(),
                Err(e) => {
                    error!("Failed to read UCID: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read UCID: {}", e),
                            code: Some("UCID_READ_ERROR".to_string()),
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

/// Get firmware version
pub async fn get_firmware_version(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_firmware_version().await {
                Ok(version) => Json(serde_json::json!({
                    "firmwareVersion": version,
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read firmware version: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read firmware version: {}", e),
                            code: Some("FIRMWARE_VERSION_READ_ERROR".to_string()),
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

/// Get chip ID
pub async fn get_chip_id(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_chip_id().await {
                Ok(chip_id) => Json(serde_json::json!({
                    "chipId": chip_id,
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read chip ID: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read chip ID: {}", e),
                            code: Some("CHIP_ID_READ_ERROR".to_string()),
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

/// Get FIFO buffer size
pub async fn get_fifo_buffer_size(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_fifo_buffer_size().await {
                Ok(size) => Json(serde_json::json!({
                    "fifoBufferSize": size,
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read FIFO buffer size: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read FIFO buffer size: {}", e),
                            code: Some("FIFO_BUFFER_SIZE_READ_ERROR".to_string()),
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

/// Get latest raw data
pub async fn get_latest_raw(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_latest_raw().await {
                Ok(data) => Json(data).into_response(),
                Err(e) => {
                    error!("Failed to read latest raw data: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read latest raw data: {}", e),
                            code: Some("LATEST_RAW_READ_ERROR".to_string()),
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

/// Get gravity RMS
pub async fn get_gravity_rms(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_gravity_rms().await {
                Ok(rms) => Json(serde_json::json!({
                    "rms": rms,
                    "unit": "g",
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read gravity RMS: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read gravity RMS: {}", e),
                            code: Some("GRAVITY_RMS_READ_ERROR".to_string()),
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

/// Get gravity peak
pub async fn get_gravity_peak(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_gravity_peak().await {
                Ok(peak) => Json(serde_json::json!({
                    "peak": peak,
                    "unit": "g",
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read gravity peak: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read gravity peak: {}", e),
                            code: Some("GRAVITY_PEAK_READ_ERROR".to_string()),
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

/// Get gravity crest factor
pub async fn get_gravity_crest_factor(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_gravity_crest_factor().await {
                Ok(crest_factor) => Json(serde_json::json!({
                    "crestFactor": crest_factor,
                    "unit": "g",
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read gravity crest factor: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read gravity crest factor: {}", e),
                            code: Some("GRAVITY_CREST_FACTOR_READ_ERROR".to_string()),
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

/// Get gravity skewness
pub async fn get_gravity_skewness(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_gravity_skewness().await {
                Ok(skewness) => Json(serde_json::json!({
                    "skewness": skewness,
                    "unit": "g",
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read gravity skewness: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read gravity skewness: {}", e),
                            code: Some("GRAVITY_SKEWNESS_READ_ERROR".to_string()),
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

/// Get gravity kurtosis
pub async fn get_gravity_kurtosis(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_gravity_kurtosis().await {
                Ok(kurtosis) => Json(serde_json::json!({
                    "kurtosis": kurtosis,
                    "unit": "g",
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read gravity kurtosis: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read gravity kurtosis: {}", e),
                            code: Some("GRAVITY_KURTOSIS_READ_ERROR".to_string()),
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

/// Get gravity primary frequency
pub async fn get_gravity_primary_frequency(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_gravity_primary_frequency().await {
                Ok(frequency) => Json(serde_json::json!({
                    "primaryFrequency": frequency,
                    "unit": "Hz",
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read gravity primary frequency: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read gravity primary frequency: {}", e),
                            code: Some("GRAVITY_PRIMARY_FREQUENCY_READ_ERROR".to_string()),
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

/// Get velocity RMS
pub async fn get_velocity_rms(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_velocity_rms().await {
                Ok(rms) => Json(serde_json::json!({
                    "rms": rms,
                    "unit": "mm/s",
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read velocity RMS: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read velocity RMS: {}", e),
                            code: Some("VELOCITY_RMS_READ_ERROR".to_string()),
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

/// Get velocity peak
pub async fn get_velocity_peak(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_velocity_peak().await {
                Ok(peak) => Json(serde_json::json!({
                    "peak": peak,
                    "unit": "mm/s",
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read velocity peak: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read velocity peak: {}", e),
                            code: Some("VELOCITY_PEAK_READ_ERROR".to_string()),
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

/// Get velocity crest factor
pub async fn get_velocity_crest_factor(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_velocity_crest_factor().await {
                Ok(crest_factor) => Json(serde_json::json!({
                    "crestFactor": crest_factor,
                    "unit": "mm/s",
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read velocity crest factor: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read velocity crest factor: {}", e),
                            code: Some("VELOCITY_CREST_FACTOR_READ_ERROR".to_string()),
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

/// Get velocity primary frequency
pub async fn get_velocity_primary_frequency(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_velocity_primary_frequency().await {
                Ok(frequency) => Json(serde_json::json!({
                    "primaryFrequency": frequency,
                    "unit": "Hz",
                    "timestamp": chrono::Utc::now()
                }))
                .into_response(),
                Err(e) => {
                    error!("Failed to read velocity primary frequency: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read velocity primary frequency: {}", e),
                            code: Some("VELOCITY_PRIMARY_FREQUENCY_READ_ERROR".to_string()),
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

/// Get all metrics (gravity + velocity)
pub async fn get_all_metrics(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let client_guard = resolve_client!(sensor, state).read().await;
    match &*client_guard {
        Some(client) => {
            match client.read_all_metrics().await {
                Ok(metrics) => Json(metrics).into_response(),
                Err(e) => {
                    error!("Failed to read all metrics: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to read all metrics: {}", e),
                            code: Some("ALL_METRICS_READ_ERROR".to_string()),
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
