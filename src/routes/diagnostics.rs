use axum::{
    extract::State,
    response::{IntoResponse, Json},
};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{AppState, modbus::ModbusClient};

async fn read_sensor_data(client_arc: &Arc<RwLock<Option<ModbusClient>>>) -> (Value, Value) {
    let guard = client_arc.read().await;
    let connected = match &*guard {
        Some(client) => client.test_connection().await.is_ok(),
        None => false,
    };

    let connection = if connected {
        json!({ "status": "connected", "connected": true })
    } else if guard.is_none() {
        json!({ "status": "no_device", "connected": false, "error": "Modbus device not connected" })
    } else {
        json!({ "status": "error", "connected": false, "error": "Connection test failed" })
    };

    let sensor = if connected {
        if let Some(client) = &*guard {
            let mut info = json!({});

            match client.read_temperature().await {
                Ok(temp) => {
                    info["temperature"] = json!({ "value": temp, "unit": "°C", "status": "ok" })
                }
                Err(e) => {
                    info["temperature"] = json!({ "status": "error", "error": e.to_string() })
                }
            }
            match client.read_ucid().await {
                Ok(ucid) => {
                    info["ucid"] = json!({ "model": ucid.model, "gain": ucid.gain, "serialNumber": ucid.serial_number, "status": "ok" })
                }
                Err(e) => info["ucid"] = json!({ "status": "error", "error": e.to_string() }),
            }
            match client.read_firmware_version().await {
                Ok(v) => info["firmwareVersion"] = json!({ "value": v, "status": "ok" }),
                Err(e) => {
                    info["firmwareVersion"] = json!({ "status": "error", "error": e.to_string() })
                }
            }
            match client.read_fifo_buffer_size().await {
                Ok(sz) => info["fifoBufferSize"] = json!({ "value": sz, "status": "ok" }),
                Err(e) => {
                    info["fifoBufferSize"] = json!({ "status": "error", "error": e.to_string() })
                }
            }
            info
        } else {
            Value::Null
        }
    } else {
        Value::Null
    };

    (connection, sensor)
}

/// Get system diagnostics and sensor status
pub async fn get_diagnostics(State(state): State<AppState>) -> impl IntoResponse {
    let streaming_capable = if state
        .config
        .sensors
        .first()
        .map(|s| s.baud_rate)
        .unwrap_or(0)
        >= 3000000
    {
        "full"
    } else {
        "metrics-only"
    };

    // Build per-sensor configs and connection data
    let mut sensor_configs = serde_json::Map::new();
    let mut sensor_connections = serde_json::Map::new();
    let mut sensor_data_map = serde_json::Map::new();

    for (i, cfg) in state.config.sensors.iter().enumerate() {
        let key = format!("sensor{}", i + 1);
        sensor_configs.insert(
            format!("config{}", i + 1),
            json!({
                "device": cfg.device,
                "baudRate": cfg.baud_rate,
                "slaveId": cfg.slave_id,
            }),
        );
        if let Some(client_arc) = state.modbus_clients.get(i) {
            let (conn, sensor) = read_sensor_data(client_arc).await;
            sensor_connections.insert(format!("connection{}", i + 1), conn);
            if sensor != Value::Null {
                sensor_data_map.insert(key, sensor);
            }
        }
    }

    let mut diagnostics = json!({
        "timestamp": chrono::Utc::now(),
        "service": "modbus-stream",
        "version": env!("CARGO_PKG_VERSION"),
        "streaming": {
            "capability": streaming_capable,
            "maxConnections": state.config.streaming.max_connections,
            "bufferSize": state.config.streaming.buffer_size,
            "metricsUpdateRate": state.config.streaming.metrics_update_rate_hz,
            "rawDataMaxSamples": state.config.streaming.raw_data_max_samples,
        },
        "system": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "rustVersion": option_env!("RUSTC_SEMVER").unwrap_or("unknown"),
        },
    });

    // Merge sensor configs, connections, and data into top-level object
    if let Some(obj) = diagnostics.as_object_mut() {
        for (k, v) in sensor_configs {
            obj.insert(k, v);
        }
        for (k, v) in sensor_connections {
            obj.insert(k, v);
        }
        for (k, v) in sensor_data_map {
            obj.insert(k, v);
        }
    }

    Json(diagnostics).into_response()
}
