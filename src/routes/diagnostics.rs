use axum::{
    extract::State,
    response::{IntoResponse, Json},
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{modbus::ModbusClient, AppState};

async fn read_sensor_data(
    client_arc: &Arc<RwLock<Option<ModbusClient>>>,
) -> (Value, Value) {
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
                Ok(temp) => info["temperature"] = json!({ "value": temp, "unit": "°C", "status": "ok" }),
                Err(e)   => info["temperature"] = json!({ "status": "error", "error": e.to_string() }),
            }
            match client.read_ucid().await {
                Ok(ucid) => info["ucid"] = json!({ "model": ucid.model, "gain": ucid.gain, "serialNumber": ucid.serial_number, "status": "ok" }),
                Err(e)   => info["ucid"] = json!({ "status": "error", "error": e.to_string() }),
            }
            match client.read_firmware_version().await {
                Ok(v)  => info["firmwareVersion"] = json!({ "value": v, "status": "ok" }),
                Err(e) => info["firmwareVersion"] = json!({ "status": "error", "error": e.to_string() }),
            }
            match client.read_fifo_buffer_size().await {
                Ok(sz) => info["fifoBufferSize"] = json!({ "value": sz, "status": "ok" }),
                Err(e) => info["fifoBufferSize"] = json!({ "status": "error", "error": e.to_string() }),
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
    let (conn1, sensor1) = read_sensor_data(&state.modbus_client1).await;
    let (conn2, sensor2) = read_sensor_data(&state.modbus_client2).await;

    let streaming_capable = if state.config.modbus1.baud_rate >= 3000000 {
        "full"
    } else {
        "metrics-only"
    };

    let mut diagnostics = json!({
        "timestamp": chrono::Utc::now(),
        "service": "modbus-stream",
        "version": env!("CARGO_PKG_VERSION"),
        "config1": {
            "device": state.config.modbus1.device,
            "baudRate": state.config.modbus1.baud_rate,
            "slaveId": state.config.modbus1.slave_id,
        },
        "config2": {
            "device": state.config.modbus2.device,
            "baudRate": state.config.modbus2.baud_rate,
            "slaveId": state.config.modbus2.slave_id,
        },
        "connection1": conn1,
        "connection2": conn2,
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

    if sensor1 != Value::Null { diagnostics["sensor1"] = sensor1; }
    if sensor2 != Value::Null { diagnostics["sensor2"] = sensor2; }

    Json(diagnostics).into_response()
}
