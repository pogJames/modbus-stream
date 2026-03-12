use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use tracing::error;

use crate::{types::ErrorResponse, AppState};

fn sensor_not_found(sensor_id: usize) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: format!("Sensor {} not found", sensor_id),
            code: Some("SENSOR_NOT_FOUND".to_string()),
            timestamp: chrono::Utc::now(),
        }),
    )
        .into_response()
}

async fn handle_no_device() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "Modbus device not connected".to_string(),
            code: Some("DEVICE_NOT_CONNECTED".to_string()),
            timestamp: chrono::Utc::now(),
        }),
    )
        .into_response()
}

// ── Inner implementations ──────────────────────────────────────────────────────

async fn temperature_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_temperature().await {
            Ok(temperature) => Json(serde_json::json!({
                "temperature": temperature, "unit": "°C", "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read temperature: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read temperature: {}", e),
                    code: Some("TEMPERATURE_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn ucid_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_ucid().await {
            Ok(ucid) => Json(ucid).into_response(),
            Err(e) => {
                error!("Failed to read UCID: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read UCID: {}", e),
                    code: Some("UCID_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn firmware_version_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_firmware_version().await {
            Ok(version) => Json(serde_json::json!({
                "firmwareVersion": version, "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read firmware version: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read firmware version: {}", e),
                    code: Some("FIRMWARE_VERSION_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn chip_id_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_chip_id().await {
            Ok(chip_id) => Json(serde_json::json!({
                "chipId": chip_id, "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read chip ID: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read chip ID: {}", e),
                    code: Some("CHIP_ID_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn fifo_buffer_size_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_fifo_buffer_size().await {
            Ok(size) => Json(serde_json::json!({
                "fifoBufferSize": size, "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read FIFO buffer size: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read FIFO buffer size: {}", e),
                    code: Some("FIFO_BUFFER_SIZE_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn latest_raw_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_latest_raw().await {
            Ok(data) => Json(data).into_response(),
            Err(e) => {
                error!("Failed to read latest raw data: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read latest raw data: {}", e),
                    code: Some("LATEST_RAW_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn gravity_rms_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_gravity_rms().await {
            Ok(rms) => Json(serde_json::json!({
                "rms": rms, "unit": "g", "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read gravity RMS: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read gravity RMS: {}", e),
                    code: Some("GRAVITY_RMS_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn gravity_peak_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_gravity_peak().await {
            Ok(peak) => Json(serde_json::json!({
                "peak": peak, "unit": "g", "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read gravity peak: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read gravity peak: {}", e),
                    code: Some("GRAVITY_PEAK_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn gravity_crest_factor_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_gravity_crest_factor().await {
            Ok(cf) => Json(serde_json::json!({
                "crestFactor": cf, "unit": "g", "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read gravity crest factor: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read gravity crest factor: {}", e),
                    code: Some("GRAVITY_CREST_FACTOR_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn gravity_skewness_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_gravity_skewness().await {
            Ok(s) => Json(serde_json::json!({
                "skewness": s, "unit": "g", "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read gravity skewness: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read gravity skewness: {}", e),
                    code: Some("GRAVITY_SKEWNESS_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn gravity_kurtosis_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_gravity_kurtosis().await {
            Ok(k) => Json(serde_json::json!({
                "kurtosis": k, "unit": "g", "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read gravity kurtosis: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read gravity kurtosis: {}", e),
                    code: Some("GRAVITY_KURTOSIS_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn gravity_primary_frequency_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_gravity_primary_frequency().await {
            Ok(freq) => Json(serde_json::json!({
                "primaryFrequency": freq, "unit": "Hz", "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read gravity primary frequency: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read gravity primary frequency: {}", e),
                    code: Some("GRAVITY_PRIMARY_FREQUENCY_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn velocity_rms_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_velocity_rms().await {
            Ok(rms) => Json(serde_json::json!({
                "rms": rms, "unit": "mm/s", "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read velocity RMS: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read velocity RMS: {}", e),
                    code: Some("VELOCITY_RMS_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn velocity_peak_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_velocity_peak().await {
            Ok(peak) => Json(serde_json::json!({
                "peak": peak, "unit": "mm/s", "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read velocity peak: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read velocity peak: {}", e),
                    code: Some("VELOCITY_PEAK_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn velocity_crest_factor_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_velocity_crest_factor().await {
            Ok(cf) => Json(serde_json::json!({
                "crestFactor": cf, "unit": "mm/s", "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read velocity crest factor: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read velocity crest factor: {}", e),
                    code: Some("VELOCITY_CREST_FACTOR_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn velocity_primary_frequency_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_velocity_primary_frequency().await {
            Ok(freq) => Json(serde_json::json!({
                "primaryFrequency": freq, "unit": "Hz", "timestamp": chrono::Utc::now()
            })).into_response(),
            Err(e) => {
                error!("Failed to read velocity primary frequency: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read velocity primary frequency: {}", e),
                    code: Some("VELOCITY_PRIMARY_FREQUENCY_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

async fn all_metrics_for(state: &AppState, sensor_id: usize) -> Response {
    let Some(sensor) = state.sensors.get(sensor_id) else {
        return sensor_not_found(sensor_id);
    };
    let guard = sensor.client.read().await;
    match &*guard {
        Some(client) => match client.read_all_metrics().await {
            Ok(metrics) => Json(metrics).into_response(),
            Err(e) => {
                error!("Failed to read all metrics: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                    error: format!("Failed to read all metrics: {}", e),
                    code: Some("ALL_METRICS_READ_ERROR".to_string()),
                    timestamp: chrono::Utc::now(),
                })).into_response()
            }
        },
        None => handle_no_device().await,
    }
}

// ── Backwards-compat handlers (sensor 0) ──────────────────────────────────────

pub async fn get_temperature(State(state): State<AppState>) -> Response { temperature_for(&state, 0).await }
pub async fn get_ucid(State(state): State<AppState>) -> Response { ucid_for(&state, 0).await }
pub async fn get_firmware_version(State(state): State<AppState>) -> Response { firmware_version_for(&state, 0).await }
pub async fn get_chip_id(State(state): State<AppState>) -> Response { chip_id_for(&state, 0).await }
pub async fn get_fifo_buffer_size(State(state): State<AppState>) -> Response { fifo_buffer_size_for(&state, 0).await }
pub async fn get_latest_raw(State(state): State<AppState>) -> Response { latest_raw_for(&state, 0).await }
pub async fn get_gravity_rms(State(state): State<AppState>) -> Response { gravity_rms_for(&state, 0).await }
pub async fn get_gravity_peak(State(state): State<AppState>) -> Response { gravity_peak_for(&state, 0).await }
pub async fn get_gravity_crest_factor(State(state): State<AppState>) -> Response { gravity_crest_factor_for(&state, 0).await }
pub async fn get_gravity_skewness(State(state): State<AppState>) -> Response { gravity_skewness_for(&state, 0).await }
pub async fn get_gravity_kurtosis(State(state): State<AppState>) -> Response { gravity_kurtosis_for(&state, 0).await }
pub async fn get_gravity_primary_frequency(State(state): State<AppState>) -> Response { gravity_primary_frequency_for(&state, 0).await }
pub async fn get_velocity_rms(State(state): State<AppState>) -> Response { velocity_rms_for(&state, 0).await }
pub async fn get_velocity_peak(State(state): State<AppState>) -> Response { velocity_peak_for(&state, 0).await }
pub async fn get_velocity_crest_factor(State(state): State<AppState>) -> Response { velocity_crest_factor_for(&state, 0).await }
pub async fn get_velocity_primary_frequency(State(state): State<AppState>) -> Response { velocity_primary_frequency_for(&state, 0).await }
pub async fn get_all_metrics(State(state): State<AppState>) -> Response { all_metrics_for(&state, 0).await }

// ── Per-sensor handlers ────────────────────────────────────────────────────────

pub async fn get_temperature_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { temperature_for(&state, id).await }
pub async fn get_ucid_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { ucid_for(&state, id).await }
pub async fn get_firmware_version_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { firmware_version_for(&state, id).await }
pub async fn get_chip_id_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { chip_id_for(&state, id).await }
pub async fn get_fifo_buffer_size_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { fifo_buffer_size_for(&state, id).await }
pub async fn get_latest_raw_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { latest_raw_for(&state, id).await }
pub async fn get_gravity_rms_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { gravity_rms_for(&state, id).await }
pub async fn get_gravity_peak_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { gravity_peak_for(&state, id).await }
pub async fn get_gravity_crest_factor_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { gravity_crest_factor_for(&state, id).await }
pub async fn get_gravity_skewness_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { gravity_skewness_for(&state, id).await }
pub async fn get_gravity_kurtosis_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { gravity_kurtosis_for(&state, id).await }
pub async fn get_gravity_primary_frequency_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { gravity_primary_frequency_for(&state, id).await }
pub async fn get_velocity_rms_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { velocity_rms_for(&state, id).await }
pub async fn get_velocity_peak_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { velocity_peak_for(&state, id).await }
pub async fn get_velocity_crest_factor_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { velocity_crest_factor_for(&state, id).await }
pub async fn get_velocity_primary_frequency_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { velocity_primary_frequency_for(&state, id).await }
pub async fn get_all_metrics_sensor(AxumPath(id): AxumPath<usize>, State(state): State<AppState>) -> Response { all_metrics_for(&state, id).await }
