use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use chrono::Utc;
use minijinja::context;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::{
    AppState,
    config::AppConfig,
    modbus::ModbusClient,
    types::{
        Feedback, SensorConnectionForm, SensorStatus, SettingsForm, SettingsStatus,
        ValidationErrors,
    },
};

#[derive(Debug)]
struct SerialPortInfo {
    port_name: String,
    port_type: String,
}

/// Helper to handle when device is not connected
async fn handle_no_device() -> String {
    "Device not connected".to_string()
}

/// GET /settings - Main settings page
pub async fn settings_page_handler(State(state): State<AppState>) -> impl IntoResponse {
    match load_current_settings(&state).await {
        Ok((settings, status)) => state.render_template(
            "settings.html",
            "/settings",
            context! {
                settings => settings,
                status => status,
                title => "Sensor Settings",
                version => env!("CARGO_PKG_VERSION")
            },
        ),
        Err(e) => {
            error!("Failed to load settings: {}", e);
            let feedback = Feedback {
                feedback_type: "error".to_string(),
                title: Some("Configuration Error".to_string()),
                message: format!("Failed to load current settings: {}", e),
                details: None,
                field_errors: None,
            };

            // Return error page with basic settings
            let default_config = AppConfig::default();
            let default_settings = SettingsForm::from(&default_config);
            let error_status = SettingsStatus {
                connection_status: "Error".to_string(),
                status_class: "error".to_string(),
                sensor_info: None,
                last_updated: Some(Utc::now()),
                unsaved_changes: false,
            };

            state.render_template(
                "settings.html",
                "/settings",
                context! {
                    settings => default_settings,
                    status => error_status,
                    feedback => feedback,
                    title => "Sensor Settings",
                    version => env!("CARGO_PKG_VERSION")
                },
            )
        }
    }
}

/// POST /settings/apply - Apply configuration changes
pub async fn apply_settings_handler(
    State(state): State<AppState>,
    Form(form_data): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    info!("Applying settings: {:?}", form_data);

    // Parse form data into SettingsForm
    let settings_form = match parse_form_data(form_data) {
        Ok(form) => form,
        Err(errors) => {
            return render_feedback_error(
                "Form Validation Failed",
                "Please correct the errors below and try again.",
                Some(errors),
            )
            .into_response();
        }
    };

    // Validate settings
    if let Err(errors) = validate_settings(&settings_form) {
        return render_feedback_error(
            "Invalid Settings",
            "Please correct the errors below and try again.",
            Some(errors),
        )
        .into_response();
    }

    // Apply settings
    match apply_settings_to_system(&state, &settings_form).await {
        Ok(details) => {
            info!("Settings applied successfully");
            render_feedback_success(
                "Settings Applied Successfully",
                "All settings have been applied and the sensor has been reconfigured.",
                Some(details),
            )
            .into_response()
        }
        Err(e) => {
            error!("Failed to apply settings: {}", e);
            render_feedback_error("Failed to Apply Settings", &format!("Error: {}", e), None)
                .into_response()
        }
    }
}

/// POST /settings/test - Test connection with current/new settings
pub async fn test_connection_handler(
    State(state): State<AppState>,
    Form(form_data): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    info!("Testing connection with form data: {:?}", form_data);

    // Parse form data
    let settings_form = match parse_form_data(form_data) {
        Ok(form) => form,
        Err(_) => {
            return Html("<div class=\"test-result error\">❌ Invalid form data</div>".to_string())
                .into_response();
        }
    };

    // Test connection
    match test_connection_with_settings(&state, &settings_form).await {
        Ok(result) => {
            Html(format!(
                "<div class=\"test-result success\">✅ Connection test successful<br><small>{}</small></div>", 
                result
            )).into_response()
        }
        Err(error) => {
            Html(format!(
                "<div class=\"test-result error\">❌ Connection test failed<br><small>{}</small></div>", 
                error
            )).into_response()
        }
    }
}

/// GET /settings/status - Get current status for all sensors (for HTMX polling)
pub async fn get_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let now = Utc::now();
    let mut cards_html = String::new();
    for (i, client_arc) in state.modbus_clients.iter().enumerate() {
        let label = format!("Sensor {}", i + 1);
        let status = check_sensor(client_arc, &label).await;
        let card = state.render_template_fragment(
            "settings/status-header.html",
            context! {
                sensor => status,
            },
        );
        cards_html.push_str(&card.0);
    }
    Html(format!(
        r#"<div class="sensor-status-grid">{}</div><div class="sensor-status-footer">Last updated: {}</div>"#,
        cards_html,
        now.format("%H:%M:%S UTC")
    ))
}

/// POST /settings/reset - Reset settings to defaults
pub async fn reset_settings_handler(State(state): State<AppState>) -> impl IntoResponse {
    info!("Resetting settings to defaults");

    match reset_to_default_settings(&state).await {
        Ok(_) => render_feedback_success(
            "Settings Reset",
            "All settings have been reset to their default values.",
            Some(vec![
                "Configuration file reloaded".to_string(),
                "Sensor connection reset".to_string(),
            ]),
        )
        .into_response(),
        Err(e) => {
            error!("Failed to reset settings: {}", e);
            render_feedback_error("Reset Failed", &format!("Error: {}", e), None).into_response()
        }
    }
}

/// GET /settings/ports - Get available serial ports
pub async fn get_available_ports() -> impl IntoResponse {
    match get_serial_ports() {
        Ok(ports) => {
            let mut options = String::new();
            options.push_str("<option value=\"auto\">🔍 Auto-detect</option>");

            for port in ports {
                options.push_str(&format!(
                    "<option value=\"{}\">📟 {} ({})</option>",
                    port.port_name, port.port_name, port.port_type
                ));
            }

            options.push_str("<option value=\"custom\">✏️ Enter manually...</option>");

            Html(options)
        }
        Err(e) => {
            warn!("Failed to enumerate serial ports: {}", e);
            Html(
                "<option value=\"auto\">🔍 Auto-detect</option>
                 <option value=\"custom\">✏️ Enter manually...</option>"
                    .to_string(),
            )
        }
    }
}

/// POST /settings/validate - Live field validation
pub async fn validate_field_handler(
    State(_state): State<AppState>,
    Form(form_data): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    // Validate individual fields
    for (field_name, value) in &form_data {
        let validation_result = match field_name.as_str() {
            "device_path" => {
                if value.trim().is_empty() {
                    Some("Device path cannot be empty")
                } else if !value.starts_with("/dev/") && !value.starts_with("COM") {
                    Some("Device path should start with /dev/ (Linux/macOS) or COM (Windows)")
                } else {
                    None
                }
            }
            "baud_rate" => match value.parse::<u32>() {
                Ok(rate) if rate == 115200 || rate == 3000000 => None,
                Ok(_) => Some("Baud rate must be 115200 or 3000000 bps"),
                Err(_) => Some("Invalid baud rate value"),
            },
            "slave_id" => match value.parse::<u8>() {
                Ok(id) if id >= 1 && id <= 247 => None,
                Ok(_) => Some("Slave ID must be between 1 and 247"),
                Err(_) => Some("Invalid slave ID value"),
            },
            "sample_rate" => match value.parse::<u16>() {
                Ok(rate) if rate >= 1 && rate <= 10000 => None,
                Ok(_) => Some("Sample rate must be between 1 and 10000 sps"),
                Err(_) => Some("Invalid sample rate value"),
            },
            "stream_size" => match value.parse::<u16>() {
                Ok(size) if size >= 1 && size <= 123 => None,
                Ok(_) => Some("Stream size must be between 1 and 123 registers"),
                Err(_) => Some("Invalid stream size value"),
            },
            "timeout_ms" => match value.parse::<u64>() {
                Ok(ms) if ms >= 1000 && ms <= 30000 => None,
                Ok(_) => Some("Timeout must be between 1000 and 30000 ms"),
                Err(_) => Some("Invalid timeout value"),
            },
            "metrics_update_rate_hz" => match value.parse::<f64>() {
                Ok(rate) if rate > 0.0 && rate <= 5.0 => None,
                Ok(_) => Some("Metrics update rate must be between 0.1 and 5.0 Hz"),
                Err(_) => Some("Invalid metrics update rate value"),
            },
            _ => None,
        };

        if let Some(error_msg) = validation_result {
            return Html(format!(
                "<div class=\"validation-error\">❌ {}</div>",
                error_msg
            ));
        }
    }

    // Cross-field validation: sample rate vs baud rate
    if let (Some(sample_rate_str), Some(baud_rate_str)) =
        (form_data.get("sample_rate"), form_data.get("baud_rate"))
    {
        if let (Ok(sample_rate), Ok(baud_rate)) =
            (sample_rate_str.parse::<u16>(), baud_rate_str.parse::<u32>())
        {
            if sample_rate > 1000 && baud_rate != 3000000 {
                return Html(
                    "<div class=\"validation-warning\">⚠️ High sample rates (>1000 sps) require 3 Mbps baud rate</div>".to_string()
                );
            }
        }
    }

    Html("<div class=\"validation-ok\">✓ Valid</div>".to_string())
}

// Helper functions

async fn load_current_settings(state: &AppState) -> anyhow::Result<(SettingsForm, SettingsStatus)> {
    let settings = SettingsForm::from(&*state.config);
    // Status is now handled by the per-sensor HTMX polling endpoints,
    // so return a placeholder to avoid blocking the page on Modbus I/O.
    let status = SettingsStatus {
        connection_status: "Loading…".to_string(),
        status_class: "loading".to_string(),
        sensor_info: None,
        last_updated: None,
        unsaved_changes: false,
    };

    Ok((settings, status))
}

async fn check_sensor(
    client_arc: &Arc<tokio::sync::RwLock<Option<ModbusClient>>>,
    label: &str,
) -> SensorStatus {
    let guard = client_arc.read().await;
    match &*guard {
        Some(client) => match client.test_connection().await {
            Ok(()) => {
                let (sensor_info, firmware_version) =
                    tokio::join!(client.read_ucid(), client.read_firmware_version(),);
                SensorStatus {
                    label: label.to_string(),
                    connection_status: "Connected".to_string(),
                    status_class: "connected".to_string(),
                    sensor_info: sensor_info.ok(),
                    firmware_version: firmware_version.ok(),
                }
            }
            Err(_) => SensorStatus {
                label: label.to_string(),
                connection_status: "Error".to_string(),
                status_class: "error".to_string(),
                sensor_info: None,
                firmware_version: None,
            },
        },
        None => SensorStatus {
            label: label.to_string(),
            connection_status: "Not Connected".to_string(),
            status_class: "disconnected".to_string(),
            sensor_info: None,
            firmware_version: None,
        },
    }
}

fn parse_form_data(form_data: HashMap<String, String>) -> Result<SettingsForm, ValidationErrors> {
    let mut errors = ValidationErrors::new();

    macro_rules! parse_field {
        ($field:expr, $type:ty, $default:expr) => {
            match form_data.get($field).and_then(|v| v.parse::<$type>().ok()) {
                Some(val) => val,
                None => {
                    errors.add_field_error($field, &format!("Invalid {}", $field));
                    $default
                }
            }
        };
    }

    // Parse per-sensor connection settings (sensor_0_*, sensor_1_*, ...)
    let mut sensors = Vec::new();
    for i in 0..4 {
        let device_key = format!("sensor_{}_device", i);
        if let Some(device) = form_data.get(&device_key).cloned() {
            if !device.is_empty() {
                let baud_rate = form_data
                    .get(&format!("sensor_{}_baud_rate", i))
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(115200);
                let slave_id = form_data
                    .get(&format!("sensor_{}_slave_id", i))
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(1);
                let timeout_ms = form_data
                    .get(&format!("sensor_{}_timeout_ms", i))
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(5000);
                let retry_attempts = form_data
                    .get(&format!("sensor_{}_retry_attempts", i))
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(3);
                sensors.push(SensorConnectionForm {
                    device,
                    baud_rate,
                    slave_id,
                    timeout_ms,
                    retry_attempts,
                });
            }
        }
    }

    if sensors.is_empty() {
        errors.add_general_error("At least one sensor must be configured");
    }

    let sample_rate = parse_field!("sample_rate", u16, 7812);
    let stream_size = parse_field!("stream_size", u16, 123);
    let high_pass_filter = form_data.contains_key("high_pass_filter");
    let max_connections = parse_field!("max_connections", usize, 10);
    let buffer_size = parse_field!("buffer_size", usize, 1024);
    let metrics_update_rate_hz = parse_field!("metrics_update_rate_hz", f64, 5.0);
    let websocket_ping_interval_sec = parse_field!("websocket_ping_interval_sec", u64, 30);

    if errors.has_errors() {
        return Err(errors);
    }

    Ok(SettingsForm {
        sensors,
        sample_rate,
        stream_size,
        high_pass_filter,
        max_connections,
        buffer_size,
        metrics_update_rate_hz,
        websocket_ping_interval_sec,
    })
}

fn validate_settings(form: &SettingsForm) -> Result<(), ValidationErrors> {
    let mut errors = ValidationErrors::new();

    if form.sensors.is_empty() {
        errors.add_general_error("At least one sensor must be configured");
    }

    for (i, sensor) in form.sensors.iter().enumerate() {
        if sensor.device.trim().is_empty() {
            errors.add_field_error(
                &format!("sensor_{}_device", i),
                "Device path cannot be empty",
            );
        }
        if !matches!(sensor.baud_rate, 115200 | 3000000) {
            errors.add_field_error(
                &format!("sensor_{}_baud_rate", i),
                "Baud rate must be 115200 or 3000000 bps",
            );
        }
        if sensor.slave_id == 0 || sensor.slave_id > 247 {
            errors.add_field_error(
                &format!("sensor_{}_slave_id", i),
                "Slave ID must be between 1 and 247",
            );
        }
        if sensor.timeout_ms < 1000 || sensor.timeout_ms > 30000 {
            errors.add_field_error(
                &format!("sensor_{}_timeout_ms", i),
                "Timeout must be between 1000 and 30000 ms",
            );
        }
        if sensor.retry_attempts == 0 || sensor.retry_attempts > 10 {
            errors.add_field_error(
                &format!("sensor_{}_retry_attempts", i),
                "Retry attempts must be between 1 and 10",
            );
        }
    }

    if form.sample_rate == 0 || form.sample_rate > 10000 {
        errors.add_field_error("sample_rate", "Sample rate must be between 1 and 10000 sps");
    }
    if form.stream_size == 0 || form.stream_size > 123 {
        errors.add_field_error(
            "stream_size",
            "Stream size must be between 1 and 123 registers",
        );
    }
    if form.max_connections == 0 || form.max_connections > 50 {
        errors.add_field_error(
            "max_connections",
            "Max connections must be between 1 and 50",
        );
    }
    if form.buffer_size < 256 || form.buffer_size > 8192 {
        errors.add_field_error("buffer_size", "Buffer size must be between 256 and 8192");
    }
    if form.metrics_update_rate_hz <= 0.0 || form.metrics_update_rate_hz > 5.0 {
        errors.add_field_error(
            "metrics_update_rate_hz",
            "Metrics update rate must be between 0.1 and 5.0 Hz",
        );
    }

    if errors.has_errors() {
        Err(errors)
    } else {
        Ok(())
    }
}

async fn apply_settings_to_system(
    state: &AppState,
    settings: &SettingsForm,
) -> anyhow::Result<Vec<String>> {
    let mut details = Vec::new();

    // Check if client is available (applies settings to sensor 1)
    let client_arc = state
        .modbus_clients
        .first()
        .ok_or_else(|| anyhow::anyhow!("No sensors configured"))?;
    let client_guard = client_arc.read().await;
    match &*client_guard {
        Some(client) => {
            // Apply sensor settings via Modbus
            if let Err(e) = client.set_sample_rate(settings.sample_rate).await {
                return Err(anyhow::anyhow!("Failed to set sample rate: {}", e));
            }
            details.push(format!("Sample rate set to {} sps", settings.sample_rate));

            if let Err(e) = client.set_stream_size(settings.stream_size).await {
                return Err(anyhow::anyhow!("Failed to set stream size: {}", e));
            }
            details.push(format!(
                "Stream size set to {} registers",
                settings.stream_size
            ));

            if let Err(e) = client.set_high_pass_filter(settings.high_pass_filter).await {
                return Err(anyhow::anyhow!("Failed to set high pass filter: {}", e));
            }
            details.push(format!(
                "High pass filter: {}",
                if settings.high_pass_filter {
                    "enabled"
                } else {
                    "disabled"
                }
            ));

            // Handle baud rate change for sensor 1 (requires special handling)
            let new_baud = settings
                .sensors
                .first()
                .map(|s| s.baud_rate)
                .unwrap_or(115200);
            if new_baud
                != state
                    .config
                    .sensors
                    .first()
                    .map(|s| s.baud_rate)
                    .unwrap_or(115200)
            {
                if let Err(e) = client.set_baud_rate(new_baud).await {
                    return Err(anyhow::anyhow!("Failed to set baud rate: {}", e));
                }
                details.push(format!(
                    "Baud rate set to {} bps (requires power cycle)",
                    new_baud
                ));
            }

            // Clear scale factor cache since sensor may have been reconfigured
            client.clear_scale_factor_cache().await;
        }
        None => {
            return Err(anyhow::anyhow!("Modbus device not connected"));
        }
    }

    // Update configuration file with new per-sensor connection params
    let new_sensors = settings
        .sensors
        .iter()
        .map(|s| crate::config::ModbusConfig {
            device: s.device.clone(),
            baud_rate: s.baud_rate,
            slave_id: s.slave_id,
            timeout_ms: s.timeout_ms,
            retry_attempts: s.retry_attempts,
        })
        .collect();
    let new_config = AppConfig {
        server: state.config.server.clone(),
        sensors: new_sensors,
        streaming: crate::config::StreamingConfig {
            max_connections: settings.max_connections,
            buffer_size: settings.buffer_size,
            metrics_update_rate_hz: settings.metrics_update_rate_hz,
            raw_data_max_samples: state.config.streaming.raw_data_max_samples,
            websocket_ping_interval_sec: settings.websocket_ping_interval_sec,
        },
        logging: state.config.logging.clone(),
    };

    if let Err(e) = new_config.save(&state.config_path) {
        warn!("Failed to save configuration file: {}", e);
        details.push(format!("Warning: Could not save config file: {}", e));
    } else {
        details.push("Configuration saved to file".to_string());
    }

    info!("Settings applied successfully: {:?}", details);
    Ok(details)
}

async fn test_connection_with_settings(
    state: &AppState,
    settings: &SettingsForm,
) -> anyhow::Result<String> {
    let s0 = settings
        .sensors
        .first()
        .ok_or_else(|| anyhow::anyhow!("No sensors configured"))?;

    // Check if settings differ from current configuration (sensor 1)
    let settings_differ = state.config.sensors.first().map_or(true, |cur| {
        s0.device != cur.device || s0.baud_rate != cur.baud_rate || s0.slave_id != cur.slave_id
    });

    if settings_differ {
        info!(
            "Testing connection with new settings: {} @ {} bps, slave {}",
            s0.device, s0.baud_rate, s0.slave_id
        );

        match ModbusClient::new(&s0.device, s0.baud_rate, s0.slave_id).await {
            Ok(temp_client) => {
                match temp_client.test_connection().await {
                    Ok(()) => {
                        let temp = temp_client.read_temperature().await?;
                        Ok(format!(
                            "Connection successful with new settings, temperature: {:.1}°C",
                            temp
                        ))
                    }
                    Err(e) => Err(anyhow::anyhow!(
                        "Connection test failed with new settings: {}",
                        e
                    )),
                }
                // temp_client is dropped here, closing the temporary connection
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to connect with new settings: {}",
                e
            )),
        }
    } else {
        // Test the existing connection (sensor 1)
        let client_arc = state
            .modbus_clients
            .first()
            .ok_or_else(|| anyhow::anyhow!("No sensors configured"))?;
        let client_guard = client_arc.read().await;
        match &*client_guard {
            Some(client) => match client.test_connection().await {
                Ok(()) => {
                    let temp = client.read_temperature().await?;
                    Ok(format!("Connection successful, temperature: {:.1}°C", temp))
                }
                Err(e) => Err(anyhow::anyhow!("Connection error: {}", e)),
            },
            None => Err(anyhow::anyhow!("Modbus device not connected")),
        }
    }
}

async fn reset_to_default_settings(state: &AppState) -> anyhow::Result<()> {
    // Create and save default configuration
    let default_config = AppConfig::default();
    default_config.save(&state.config_path)?;
    info!("Configuration file reset to defaults");

    // Reset sensor settings via Modbus if connected (sensor 1)
    let client_arc = match state.modbus_clients.first() {
        Some(c) => c,
        None => return Ok(()),
    };
    let client_guard = client_arc.read().await;
    if let Some(client) = &*client_guard {
        // Reset to default sensor settings
        if let Err(e) = client.set_sample_rate(7812).await {
            warn!("Failed to reset sample rate: {}", e);
        }
        if let Err(e) = client.set_stream_size(123).await {
            warn!("Failed to reset stream size: {}", e);
        }
        if let Err(e) = client.set_high_pass_filter(false).await {
            warn!("Failed to reset high pass filter: {}", e);
        }
        // Clear scale factor cache
        client.clear_scale_factor_cache().await;
        info!("Sensor settings reset to defaults");
    }

    info!("Settings reset to defaults");
    Ok(())
}

fn render_feedback_success(
    title: &str,
    message: &str,
    details: Option<Vec<String>>,
) -> Html<String> {
    // Format details properly
    let details_html = if let Some(details_list) = details {
        if !details_list.is_empty() {
            format!(
                "<div class=\"feedback-details\">{}</div>",
                details_list.join("<br>")
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // In a real implementation, you would render the template properly
    // For now, return a simple HTML response
    Html(format!(
        r#"<div class="feedback success">
            <div class="feedback-icon">✅</div>
            <div class="feedback-content">
                <h4>{}</h4>
                <p>{}</p>
                {}
            </div>
        </div>"#,
        title, message, details_html
    ))
}

fn render_feedback_error(
    title: &str,
    message: &str,
    errors: Option<ValidationErrors>,
) -> Html<String> {
    let mut html = format!(
        r#"<div class="feedback error">
            <div class="feedback-icon">❌</div>
            <div class="feedback-content">
                <h4>{}</h4>
                <p>{}</p>"#,
        title, message
    );

    if let Some(errors) = errors {
        if !errors.field_errors.is_empty() {
            html.push_str("<div class=\"field-errors\">");
            for (field, field_errors) in errors.field_errors {
                html.push_str(&format!(
                    "<div class=\"field-error\"><strong>{}:</strong> {}</div>",
                    field,
                    field_errors.join(", ")
                ));
            }
            html.push_str("</div>");
        }

        if !errors.general_errors.is_empty() {
            html.push_str("<div class=\"feedback-details\">");
            for error in errors.general_errors {
                html.push_str(&format!("<div>{}</div>", error));
            }
            html.push_str("</div>");
        }
    }

    html.push_str("</div></div>");
    Html(html)
}

/// Get available serial ports
fn get_serial_ports() -> anyhow::Result<Vec<SerialPortInfo>> {
    let mut ports = Vec::new();

    // For cross-platform compatibility, we'll do a simple check
    // In a real implementation, you might use the serialport crate

    #[cfg(target_os = "windows")]
    {
        // Windows COM ports - check if they actually exist by trying to query them
        use std::fs::OpenOptions;

        for i in 1..=20 {
            let port_name = format!("COM{}", i);
            // Use the Windows device path format for checking
            let device_path = format!("\\\\.\\{}", port_name);

            // Try to open the port briefly to check if it exists
            // This is a non-blocking check that just verifies the port is available
            match OpenOptions::new().read(true).write(true).open(&device_path) {
                Ok(_) => {
                    ports.push(SerialPortInfo {
                        port_name: port_name.clone(),
                        port_type: "Serial".to_string(),
                    });
                }
                Err(e) => {
                    // ERROR_FILE_NOT_FOUND (2) or ERROR_ACCESS_DENIED (5) means port exists but may be in use
                    let raw_error = e.raw_os_error().unwrap_or(0);
                    if raw_error == 5 {
                        // Access denied - port exists but is in use
                        ports.push(SerialPortInfo {
                            port_name: port_name.clone(),
                            port_type: "Serial (in use)".to_string(),
                        });
                    }
                    // ERROR_FILE_NOT_FOUND means port doesn't exist, skip it
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Linux USB serial devices
        let common_paths = [
            "/dev/ttyUSB0",
            "/dev/ttyUSB1",
            "/dev/ttyUSB2",
            "/dev/ttyUSB3",
            "/dev/ttyACM0",
            "/dev/ttyACM1",
            "/dev/ttyACM2",
            "/dev/ttyACM3",
        ];

        for path in &common_paths {
            if std::path::Path::new(path).exists() {
                ports.push(SerialPortInfo {
                    port_name: path.to_string(),
                    port_type: "USB Serial".to_string(),
                });
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS USB serial devices
        if let Ok(entries) = std::fs::read_dir("/dev") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("cu.usbserial") || name.starts_with("cu.usbmodem") {
                    ports.push(SerialPortInfo {
                        port_name: format!("/dev/{}", name),
                        port_type: "USB Serial".to_string(),
                    });
                }
            }
        }
    }

    Ok(ports)
}
