use axum::{
    extract::State,
    response::{IntoResponse, Json},
};
use serde_json::json;

use crate::AppState;

pub async fn get_diagnostics(State(state): State<AppState>) -> impl IntoResponse {
    let sensor_cfg = state.config.sensors.first();
    let baud_rate = sensor_cfg.map(|s| s.baud_rate).unwrap_or(115200);

    let mut diagnostics = json!({
        "timestamp": chrono::Utc::now(),
        "service": "modbus-stream",
        "version": env!("CARGO_PKG_VERSION"),
        "config": {
            "device": sensor_cfg.map(|s| s.device.as_str()).unwrap_or("/dev/ttyUSB0"),
            "baudRate": baud_rate,
            "slaveId": sensor_cfg.map(|s| s.slave_id).unwrap_or(1),
            "timeout": sensor_cfg.map(|s| s.timeout_ms).unwrap_or(5000),
            "sensorCount": state.sensors.len()
        }
    });

    // Sensor 0 connection check
    if let Some(sensor) = state.sensors.get(0) {
        let client_guard = sensor.client.read().await;
        let connected = match &*client_guard {
            Some(client) => client.test_connection().await.is_ok(),
            None => false,
        };

        if connected {
            if let Some(client) = &*client_guard {
                diagnostics["connection"] = json!({ "status": "connected", "connected": true });

                let mut sensor_info = json!({});

                match client.read_temperature().await {
                    Ok(temp) => { sensor_info["temperature"] = json!({ "value": temp, "unit": "°C", "status": "ok" }); }
                    Err(e) => { sensor_info["temperature"] = json!({ "status": "error", "error": e.to_string() }); }
                }
                match client.read_ucid().await {
                    Ok(ucid) => { sensor_info["ucid"] = json!({ "model": ucid.model, "gain": ucid.gain, "serialNumber": ucid.serial_number, "status": "ok" }); }
                    Err(e) => { sensor_info["ucid"] = json!({ "status": "error", "error": e.to_string() }); }
                }
                match client.read_firmware_version().await {
                    Ok(v) => { sensor_info["firmwareVersion"] = json!({ "value": v, "status": "ok" }); }
                    Err(e) => { sensor_info["firmwareVersion"] = json!({ "status": "error", "error": e.to_string() }); }
                }
                match client.read_fifo_buffer_size().await {
                    Ok(sz) => { sensor_info["fifoBufferSize"] = json!({ "value": sz, "status": "ok" }); }
                    Err(e) => { sensor_info["fifoBufferSize"] = json!({ "status": "error", "error": e.to_string() }); }
                }

                diagnostics["sensor"] = sensor_info;
            }
        } else {
            diagnostics["connection"] = json!({
                "status": if client_guard.is_none() { "no_device" } else { "error" },
                "connected": false,
                "error": if client_guard.is_none() { "Modbus device not connected" } else { "Connection test failed" }
            });
        }
    } else {
        diagnostics["connection"] = json!({ "status": "no_device", "connected": false });
    }

    let streaming_capable = if baud_rate >= 3000000 { "full" } else { "metrics-only" };
    diagnostics["streaming"] = json!({
        "capability": streaming_capable,
        "maxConnections": state.config.streaming.max_connections,
        "bufferSize": state.config.streaming.buffer_size,
        "metricsUpdateRate": state.config.streaming.metrics_update_rate_hz,
        "rawDataMaxSamples": state.config.streaming.raw_data_max_samples
    });

    diagnostics["system"] = json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    });

    Json(diagnostics).into_response()
}
