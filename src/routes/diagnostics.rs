use axum::{
    extract::State,
    response::{IntoResponse, Json},
};
use serde_json::json;
use tracing::error;

use crate::AppState;

/// Get system diagnostics and sensor status
pub async fn get_diagnostics(State(state): State<AppState>) -> impl IntoResponse {
    let mut diagnostics = json!({
        "timestamp": chrono::Utc::now(),
        "service": "modbus-stream",
        "version": env!("CARGO_PKG_VERSION"),
        "config": {
            "device": state.config.modbus.device,
            "baudRate": state.config.modbus.baud_rate,
            "slaveId": state.config.modbus.slave_id,
            "timeout": state.config.modbus.timeout_ms,
            "retryAttempts": state.config.modbus.retry_attempts
        }
    });

    // Check if client is available
    let client_guard = state.modbus_client.read().await;
    let connected = match &*client_guard {
        Some(client) => {
            match client.test_connection().await {
                Ok(()) => true,
                Err(_) => false,
            }
        }
        None => false,
    };

    if connected {
        if let Some(client) = &*client_guard {
            diagnostics["connection"] = json!({
                "status": "connected",
                "connected": true
            });

            // Try to read some basic information
            let mut sensor_info = json!({});

            // Read temperature
            match client.read_temperature().await {
                Ok(temp) => {
                    sensor_info["temperature"] = json!({
                        "value": temp,
                        "unit": "°C",
                        "status": "ok"
                    });
                }
                Err(e) => {
                    sensor_info["temperature"] = json!({
                        "status": "error",
                        "error": e.to_string()
                    });
                }
            }

            // Read UCID
            match client.read_ucid().await {
                Ok(ucid) => {
                    sensor_info["ucid"] = json!({
                        "model": ucid.model,
                        "gain": ucid.gain,
                        "serialNumber": ucid.serial_number,
                        "status": "ok"
                    });
                }
                Err(e) => {
                    sensor_info["ucid"] = json!({
                        "status": "error",
                        "error": e.to_string()
                    });
                }
            }

            // Read firmware version
            match client.read_firmware_version().await {
                Ok(version) => {
                    sensor_info["firmwareVersion"] = json!({
                        "value": version,
                        "status": "ok"
                    });
                }
                Err(e) => {
                    sensor_info["firmwareVersion"] = json!({
                        "status": "error",
                        "error": e.to_string()
                    });
                }
            }

            // Read FIFO buffer size
            match client.read_fifo_buffer_size().await {
                Ok(size) => {
                    sensor_info["fifoBufferSize"] = json!({
                        "value": size,
                        "status": "ok"
                    });
                }
                Err(e) => {
                    sensor_info["fifoBufferSize"] = json!({
                        "status": "error", 
                        "error": e.to_string()
                    });
                }
            }

            diagnostics["sensor"] = sensor_info;
        }
    } else {
        // Connection failed or no client available
        if client_guard.is_none() {
            diagnostics["connection"] = json!({
                "status": "no_device",
                "connected": false,
                "error": "Modbus device not connected"
            });
        } else if let Some(client) = &*client_guard {
            // Connection failed - get the actual error for better diagnostics
            match client.test_connection().await {
                Ok(()) => unreachable!(), // We already know it failed
                Err(e) => {
                    error!("Failed to test connection: {}", e);
                    diagnostics["connection"] = json!({
                        "status": "error",
                        "connected": false,
                        "error": e.to_string()
                    });
                }
            }
        }
    }

    // Add streaming capability check
    let streaming_capable = if state.config.modbus.baud_rate >= 3000000 {
        "full" // Raw data + metrics streaming
    } else {
        "metrics-only" // Only metrics streaming at lower baud rates
    };

    diagnostics["streaming"] = json!({
        "capability": streaming_capable,
        "maxConnections": state.config.streaming.max_connections,
        "bufferSize": state.config.streaming.buffer_size,
        "metricsUpdateRate": state.config.streaming.metrics_update_rate_hz,
        "rawDataMaxSamples": state.config.streaming.raw_data_max_samples
    });

    // Add system information
    diagnostics["system"] = json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "rustVersion": option_env!("RUSTC_SEMVER").unwrap_or("unknown")
    });

    Json(diagnostics).into_response()
}
