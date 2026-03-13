use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Tri-axial acceleration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccelerationData {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Raw sensor reading with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawReading {
    pub timestamp: DateTime<Utc>,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub unit: String,
}

/// Latest raw data response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestRawResponse {
    pub timestamp: DateTime<Utc>,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub unit: String,
}

/// Gravity metrics for all three axes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GravityMetrics {
    pub rms: AccelerationData,
    pub peak: AccelerationData,
    #[serde(rename = "crestFactor")]
    pub crest_factor: AccelerationData,
    pub skewness: AccelerationData,
    pub kurtosis: AccelerationData,
    #[serde(rename = "primaryFrequency")]
    pub primary_frequency: f64,
}

/// Velocity metrics for all three axes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VelocityMetrics {
    pub rms: AccelerationData,
    pub peak: AccelerationData,
    #[serde(rename = "crestFactor")]
    pub crest_factor: AccelerationData,
    #[serde(rename = "primaryFrequency")]
    pub primary_frequency: f64,
}

/// Combined metrics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllMetricsResponse {
    pub timestamp: DateTime<Utc>,
    pub gravity: GravityMetrics,
    pub velocity: VelocityMetrics,
}

/// UCID information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UcidInfo {
    pub model: String,
    pub gain: String,
    #[serde(rename = "serialNumber")]
    pub serial_number: u32,
    pub raw_value: u32,
}

/// Device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub ucid: UcidInfo,
    #[serde(rename = "firmwareVersion")]
    pub firmware_version: String,
    #[serde(rename = "chipId")]
    pub chip_id: Vec<u16>,
    pub temperature: f64,
}

/// Configuration responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    #[serde(rename = "sampleRate")]
    pub sample_rate: u16,
    #[serde(rename = "baudRate")]
    pub baud_rate: u32,
    #[serde(rename = "highPassFilter")]
    pub high_pass_filter: bool,
    #[serde(rename = "streamSize")]
    pub stream_size: u16,
    pub temperature: f64,
    #[serde(rename = "firmwareVersion")]
    pub firmware_version: String,
    pub ucid: UcidInfo,
}

/// Configuration requests
#[derive(Debug, Deserialize)]
pub struct SampleRateRequest {
    #[serde(rename = "sampleRate")]
    pub sample_rate: u16,
}

#[derive(Debug, Deserialize)]
pub struct BaudRateRequest {
    #[serde(rename = "baudRate")]
    pub baud_rate: u32,
}

#[derive(Debug, Deserialize)]
pub struct HighPassFilterRequest {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct StreamSizeRequest {
    #[serde(rename = "streamSize")]
    pub stream_size: u16,
}

/// Stream control
#[derive(Debug, Deserialize)]
pub struct StreamStartRequest {
    #[serde(rename = "type")]
    pub stream_type: StreamType,
    #[serde(rename = "sampleRate")]
    pub sample_rate: Option<u16>,
    #[serde(rename = "bufferSize")]
    pub buffer_size: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamType {
    Raw,
    Metrics,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStatus {
    pub active: bool,
    #[serde(rename = "type")]
    pub stream_type: Option<StreamType>,
    #[serde(rename = "startTime")]
    pub start_time: Option<DateTime<Utc>>,
    #[serde(rename = "sampleCount")]
    pub sample_count: u64,
    #[serde(rename = "bufferUtilization")]
    pub buffer_utilization: f64,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    #[serde(rename = "raw")]
    RawData {
        timestamp: DateTime<Utc>,
        sequence: u64,
        data: Vec<AccelerationData>,
    },
    #[serde(rename = "metrics")]
    Metrics {
        timestamp: DateTime<Utc>,
        gravity: GravityMetrics,
        velocity: VelocityMetrics,
        temperature: f64,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
        code: Option<String>,
    },
    #[serde(rename = "status")]
    Status {
        connected: bool,
        streaming: bool,
    },
}

/// Error response
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Modbus register addresses (from documentation)
pub mod registers {
    pub const SAMPLE_RATE: u16 = 0x0001;
    pub const FIFO_BUFFER_SIZE: u16 = 0x0002;
    pub const RAW_DATA_START: u16 = 0x0003;
    pub const RAW_DATA_END: u16 = 0x007D;
    pub const TEMPERATURE: u16 = 0x0014;
    pub const STREAM_SIZE: u16 = 0x0015;
    pub const BAUD_RATE_HIGH: u16 = 0x0017;
    pub const BAUD_RATE_LOW: u16 = 0x0018;
    pub const UCID: u16 = 0x001B;
    pub const HIGH_PASS_ENABLE: u16 = 0x001C;
    pub const FIRMWARE_VERSION: u16 = 0x001D;
    
    // Gravity metrics
    pub const GRAVITY_RMS: u16 = 0x001E;
    pub const GRAVITY_PEAK: u16 = 0x001F;
    pub const GRAVITY_CREST_FACTOR: u16 = 0x0020;
    pub const GRAVITY_SKEWNESS: u16 = 0x0021;
    pub const GRAVITY_KURTOSIS: u16 = 0x0022;
    pub const GRAVITY_PRIMARY_FREQ: u16 = 0x003D;
    
    // Velocity metrics
    pub const VELOCITY_RMS: u16 = 0x0032;
    pub const VELOCITY_PEAK: u16 = 0x0033;
    pub const VELOCITY_CREST_FACTOR: u16 = 0x0034;
    pub const VELOCITY_PRIMARY_FREQ: u16 = 0x003C;
    
    // Latest raw data
    pub const RAW_DATA_LATEST_X: u16 = 0x0083;
    pub const RAW_DATA_LATEST_Y: u16 = 0x0084;
    pub const RAW_DATA_LATEST_Z: u16 = 0x0085;
    
    // Chip ID
    pub const CHIP_ID: u16 = 0x0080;
}

/// Modbus function codes
pub mod function_codes {
    pub const READ_HOLDING_REGISTERS: u8 = 0x03;
    pub const READ_INPUT_REGISTERS: u8 = 0x04;
    pub const WRITE_SINGLE_REGISTER: u8 = 0x06;
}

/// Data conversion utilities
/// Settings form data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsForm {
    // Connection settings
    pub device_path: String,
    pub baud_rate: u32,
    pub slave_id: u8,
    pub timeout_ms: u64,
    pub retry_attempts: u8,
    
    // Sensor settings  
    pub sample_rate: u16,
    pub stream_size: u16,
    pub high_pass_filter: bool,
    
    // Streaming settings
    pub max_connections: usize,
    pub buffer_size: usize,
    pub metrics_update_rate_hz: f64,
    pub websocket_ping_interval_sec: u64,
}

/// Settings status for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsStatus {
    pub connection_status: String,
    pub status_class: String,
    pub sensor_info: Option<UcidInfo>,
    pub last_updated: Option<DateTime<Utc>>,
    pub unsaved_changes: bool,
}

/// Validation errors for form feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationErrors {
    pub field_errors: HashMap<String, Vec<String>>,
    pub general_errors: Vec<String>,
}

/// Feedback for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feedback {
    #[serde(rename = "type")]
    pub feedback_type: String,
    pub title: Option<String>,
    pub message: String,
    pub details: Option<Vec<String>>,
    pub field_errors: Option<HashMap<String, Vec<String>>>,
}

impl ValidationErrors {
    pub fn new() -> Self {
        Self {
            field_errors: HashMap::new(),
            general_errors: Vec::new(),
        }
    }
    
    pub fn add_field_error(&mut self, field: &str, error: &str) {
        self.field_errors
            .entry(field.to_string())
            .or_insert_with(Vec::new)
            .push(error.to_string());
    }
    
    pub fn add_general_error(&mut self, error: &str) {
        self.general_errors.push(error.to_string());
    }
    
    pub fn has_errors(&self) -> bool {
        !self.field_errors.is_empty() || !self.general_errors.is_empty()
    }
}

impl From<&crate::config::AppConfig> for SettingsForm {
    fn from(config: &crate::config::AppConfig) -> Self {
        Self {
            device_path: config.modbus.device.clone(),
            baud_rate: config.modbus.baud_rate,
            slave_id: config.modbus.slave_id,
            timeout_ms: config.modbus.timeout_ms,
            retry_attempts: config.modbus.retry_attempts,
            sample_rate: 7812, // Default, will be read from sensor
            stream_size: 123,   // Default max
            high_pass_filter: false, // Default
            max_connections: config.streaming.max_connections,
            buffer_size: config.streaming.buffer_size,
            metrics_update_rate_hz: config.streaming.metrics_update_rate_hz,
            websocket_ping_interval_sec: config.streaming.websocket_ping_interval_sec,
        }
    }
}

impl UcidInfo {
    pub fn from_raw(raw_value: u32) -> Self {
        let model_bits = (raw_value >> 28) & 0xF;
        let gain_bits = (raw_value >> 24) & 0xF;
        let serial_number = raw_value & 0xFFFFFF;

        let model = match model_bits {
            0 => "12B",
            1 => "15B", 
            2 => "KAX301",
            3 => "KAX302",
            4 => "S6S",
            _ => "Unknown",
        }.to_string();

        let gain = match gain_bits {
            0 => "4G",
            1 => "2G",
            2 => "8G", 
            3 => "16G",
            4 => "32G",
            5 => "64G",
            _ => "Unknown",
        }.to_string();

        Self {
            model,
            gain,
            serial_number,
            raw_value,
        }
    }

    /// Get the scale factor for converting raw sensor values to g's based on gain setting.
    /// For a 16-bit signed ADC: counts_per_g = 32768 / full_scale_g
    pub fn scale_factor(&self) -> f64 {
        match self.gain.as_str() {
            "2G"  => 1.0 / 16384.0,
            "4G"  => 1.0 / 8192.0,
            "8G"  => 1.0 / 4096.0,
            "16G" => 1.0 / 2048.0,
            "32G" => 1.0 / 1024.0,
            "64G" => 1.0 / 512.0,
            _     => 1.0 / 8192.0, // Default to 4G (matches Python's hardcoded turn_gravity=8192)
        }
    }
}
