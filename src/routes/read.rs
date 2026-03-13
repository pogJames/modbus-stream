use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use tracing::error;

use crate::{types::ErrorResponse, AppState};

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
pub async fn get_temperature(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_ucid(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_firmware_version(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_chip_id(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_fifo_buffer_size(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_latest_raw(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_gravity_rms(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_gravity_peak(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_gravity_crest_factor(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_gravity_skewness(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_gravity_kurtosis(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_gravity_primary_frequency(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_velocity_rms(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_velocity_peak(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_velocity_crest_factor(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_velocity_primary_frequency(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
pub async fn get_all_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let client_guard = state.modbus_client.read().await;
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
