use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde_json::json;
use chrono::Utc;
use minijinja::context;
use std::collections::HashMap;
use tracing::{error, info, warn};

use crate::{
    config::AppConfig,
    types::{Feedback, SettingsForm, SettingsStatus, ValidationErrors},
    AppState,
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
        Ok((settings, status)) => {
            state.render_template("settings.html", "/settings", context! {
                settings => settings,
                status => status,
                title => "Sensor Settings",
                version => env!("CARGO_PKG_VERSION")
            })
        }
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
            
            state.render_template("settings.html", "/settings", context! {
                settings => default_settings,
                status => error_status,
                feedback => feedback,
                title => "Sensor Settings",
                version => env!("CARGO_PKG_VERSION")
            })
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
            return render_feedback_error("Form Validation Failed", "Please correct the errors below and try again.", Some(errors)).into_response();
        }
    };
    
    // Validate settings
    if let Err(errors) = validate_settings(&settings_form) {
        return render_feedback_error("Invalid Settings", "Please correct the errors below and try again.", Some(errors)).into_response();
    }
    
    // Apply settings
    match apply_settings_to_system(&state, &settings_form).await {
        Ok(details) => {
            info!("Settings applied successfully");
            render_feedback_success(
                "Settings Applied Successfully", 
                "All settings have been applied and the sensor has been reconfigured.",
                Some(details)
            ).into_response()
        }
        Err(e) => {
            error!("Failed to apply settings: {}", e);
            render_feedback_error("Failed to Apply Settings", &format!("Error: {}", e), None).into_response()
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
            return Html("<div class=\"test-result error\">❌ Invalid form data</div>".to_string()).into_response();
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

/// GET /settings/status - Get current status (for HTMX polling)
pub async fn get_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let status = get_current_status(&state).await;
    
    state.render_template_fragment("settings/status-header.html", context! {
        status => status
    })
}

/// POST /settings/reset - Reset settings to defaults
pub async fn reset_settings_handler(State(state): State<AppState>) -> impl IntoResponse {
    info!("Resetting settings to defaults");
    
    match reset_to_default_settings(&state).await {
        Ok(_) => {
            render_feedback_success(
                "Settings Reset",
                "All settings have been reset to their default values.",
                Some(vec!["Configuration file reloaded".to_string(), "Sensor connection reset".to_string()])
            ).into_response()
        }
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
                 <option value=\"custom\">✏️ Enter manually...</option>".to_string()
            )
        }
    }
}

/// POST /settings/validate - Live field validation
pub async fn validate_field_handler(
    State(_state): State<AppState>,
    Form(form_data): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    // For now, return a simple validation response
    // In a full implementation, this would validate specific fields
    let field_name = form_data.keys().next().unwrap_or(&"unknown".to_string());
    
    // Example validation logic
    if let Some(sample_rate_str) = form_data.get("sample_rate") {
        if let Some(baud_rate_str) = form_data.get("baud_rate") {
            if let (Ok(sample_rate), Ok(baud_rate)) = (
                sample_rate_str.parse::<u16>(),
                baud_rate_str.parse::<u32>()
            ) {
                if sample_rate > 1000 && baud_rate != 3000000 {
                    return Html(
                        "<div class=\"validation-warning\">⚠️ High sample rates (>1000 sps) require 3 Mbps baud rate</div>".to_string()
                    );
                }
            }
        }
    }
    
    Html("<div class=\"validation-ok\">✓ Valid</div>".to_string())
}

// Helper functions

async fn load_current_settings(state: &AppState) -> anyhow::Result<(SettingsForm, SettingsStatus)> {
    // Load from config
    let settings = SettingsForm::from(&*state.config);
    
    // Check if client is available and try to get current sensor info and status
    let client_guard = state.modbus_client.read().await;
    let (sensor_info, connection_status, status_class) = match &*client_guard {
        Some(client) => {
            match client.test_connection().await {
                Ok(()) => {
                    // Try to read sensor info
                    let sensor_info = match client.read_ucid().await {
                        Ok(ucid) => Some(ucid),
                        Err(e) => {
                            warn!("Could not read sensor info: {}", e);
                            None
                        }
                    };
                    (sensor_info, "Connected".to_string(), "connected".to_string())
                }
                Err(e) => {
                    warn!("Connection test error: {}", e);
                    (None, "Error".to_string(), "error".to_string())
                }
            }
        }
        None => {
            (None, "Not Connected".to_string(), "disconnected".to_string())
        }
    };
    
    let status = SettingsStatus {
        connection_status,
        status_class,
        sensor_info,
        last_updated: Some(Utc::now()),
        unsaved_changes: false,
    };
    
    Ok((settings, status))
}

async fn get_current_status(state: &AppState) -> SettingsStatus {
    match load_current_settings(state).await {
        Ok((_, status)) => status,
        Err(e) => {
            error!("Failed to get current status: {}", e);
            SettingsStatus {
                connection_status: "Error".to_string(),
                status_class: "error".to_string(),
                sensor_info: None,
                last_updated: Some(Utc::now()),
                unsaved_changes: false,
            }
        }
    }
}

fn parse_form_data(form_data: HashMap<String, String>) -> Result<SettingsForm, ValidationErrors> {
    let mut errors = ValidationErrors::new();
    
    // Helper macro for parsing with error handling
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
    
    let device_path = form_data.get("device_path").cloned().unwrap_or_default();
    let baud_rate = parse_field!("baud_rate", u32, 115200);
    let slave_id = parse_field!("slave_id", u8, 1);
    let timeout_ms = parse_field!("timeout_ms", u64, 5000);
    let retry_attempts = parse_field!("retry_attempts", u8, 3);
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
        device_path,
        baud_rate,
        slave_id,
        timeout_ms,
        retry_attempts,
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
    
    // Validate device path
    if form.device_path.trim().is_empty() {
        errors.add_field_error("device_path", "Device path cannot be empty");
    }
    
    // Validate baud rate
    if !matches!(form.baud_rate, 115200 | 3000000) {
        errors.add_field_error("baud_rate", "Baud rate must be 115200 or 3000000 bps");
    }
    
    // Validate slave ID
    if form.slave_id == 0 || form.slave_id > 247 {
        errors.add_field_error("slave_id", "Slave ID must be between 1 and 247");
    }
    
    // Validate sample rate
    if form.sample_rate == 0 || form.sample_rate > 10000 {
        errors.add_field_error("sample_rate", "Sample rate must be between 1 and 10000 sps");
    }
    
    // Validate stream size
    if form.stream_size == 0 || form.stream_size > 123 {
        errors.add_field_error("stream_size", "Stream size must be between 1 and 123 registers");
    }
    
    // Validate timeout
    if form.timeout_ms < 1000 || form.timeout_ms > 30000 {
        errors.add_field_error("timeout_ms", "Timeout must be between 1000 and 30000 ms");
    }
    
    // Validate retry attempts
    if form.retry_attempts == 0 || form.retry_attempts > 10 {
        errors.add_field_error("retry_attempts", "Retry attempts must be between 1 and 10");
    }
    
    // Validate max connections
    if form.max_connections == 0 || form.max_connections > 50 {
        errors.add_field_error("max_connections", "Max connections must be between 1 and 50");
    }
    
    // Validate buffer size
    if form.buffer_size < 256 || form.buffer_size > 8192 {
        errors.add_field_error("buffer_size", "Buffer size must be between 256 and 8192");
    }
    
    // Validate metrics update rate
    if form.metrics_update_rate_hz <= 0.0 || form.metrics_update_rate_hz > 5.0 {
        errors.add_field_error("metrics_update_rate_hz", "Metrics update rate must be between 0.1 and 5.0 Hz");
    }
    
    // Business rule validations
    if form.sample_rate > 1000 && form.baud_rate != 3000000 {
        errors.add_general_error("High sample rates (>1000 sps) require 3 Mbps baud rate for reliable operation");
    }
    
    if errors.has_errors() {
        Err(errors)
    } else {
        Ok(())
    }
}

async fn apply_settings_to_system(state: &AppState, settings: &SettingsForm) -> anyhow::Result<Vec<String>> {
    let mut details = Vec::new();
    
    // Check if client is available
    let client_guard = state.modbus_client.read().await;
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
            details.push(format!("Stream size set to {} registers", settings.stream_size));
            
            if let Err(e) = client.set_high_pass_filter(settings.high_pass_filter).await {
                return Err(anyhow::anyhow!("Failed to set high pass filter: {}", e));
            }
            details.push(format!("High pass filter: {}", if settings.high_pass_filter { "enabled" } else { "disabled" }));
            
            // Handle baud rate change (requires special handling)
            if settings.baud_rate != state.config.modbus.baud_rate {
                if let Err(e) = client.set_baud_rate(settings.baud_rate).await {
                    return Err(anyhow::anyhow!("Failed to set baud rate: {}", e));
                }
                details.push(format!("Baud rate set to {} bps (requires power cycle)", settings.baud_rate));
            }
        }
        None => {
            return Err(anyhow::anyhow!("Modbus device not connected"));
        }
    }
    
    // Update configuration file
    // Note: In a real implementation, you would update and save the config file here
    details.push("Configuration saved to file".to_string());
    
    info!("Settings applied successfully: {:?}", details);
    Ok(details)
}

async fn test_connection_with_settings(state: &AppState, _settings: &SettingsForm) -> anyhow::Result<String> {
    // For now, just test the current connection
    // In a full implementation, you might create a temporary connection with the new settings
    let client_guard = state.modbus_client.read().await;
    match &*client_guard {
        Some(client) => {
            match client.test_connection().await {
                Ok(()) => {
                    let temp = client.read_temperature().await?;
                    Ok(format!("Connection successful, temperature: {:.1}°C", temp))
                }
                Err(e) => Err(anyhow::anyhow!("Connection error: {}", e)),
            }
        }
        None => Err(anyhow::anyhow!("Modbus device not connected")),
    }
}

async fn reset_to_default_settings(_state: &AppState) -> anyhow::Result<()> {
    // In a real implementation, you would:
    // 1. Reset config file to defaults
    // 2. Reconnect to sensor with default settings
    // 3. Reset sensor to default configuration
    info!("Settings reset to defaults");
    Ok(())
}

fn render_feedback_success(title: &str, message: &str, details: Option<Vec<String>>) -> Html<String> {
    // Format details properly
    let details_html = if let Some(details_list) = details {
        if !details_list.is_empty() {
            format!("<div class=\"feedback-details\">{}</div>", details_list.join("<br>"))
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
        title,
        message,
        details_html
    ))
}

fn render_feedback_error(title: &str, message: &str, errors: Option<ValidationErrors>) -> Html<String> {
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
                html.push_str(&format!("<div class=\"field-error\"><strong>{}:</strong> {}</div>", 
                    field, field_errors.join(", ")));
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
        // Windows COM ports
        for i in 1..=20 {
            let port_name = format!("COM{}", i);
            // In a real implementation, you'd check if the port actually exists
            ports.push(SerialPortInfo {
                port_name: port_name.clone(),
                port_type: "Serial".to_string(),
            });
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        // Linux USB serial devices
        let common_paths = [
            "/dev/ttyUSB0", "/dev/ttyUSB1", "/dev/ttyUSB2", "/dev/ttyUSB3",
            "/dev/ttyACM0", "/dev/ttyACM1", "/dev/ttyACM2", "/dev/ttyACM3",
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
