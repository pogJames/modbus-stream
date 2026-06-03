use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub sensors: Vec<ModbusConfig>,
    pub streaming: StreamingConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusConfig {
    pub device: String,
    pub baud_rate: u32,
    pub slave_id: u8,
    pub timeout_ms: u64,
    pub retry_attempts: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    pub max_connections: usize,
    pub buffer_size: usize,
    pub metrics_update_rate_hz: f64,
    pub raw_data_max_samples: usize,
    pub websocket_ping_interval_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

fn default_sensor(device: &str) -> ModbusConfig {
    ModbusConfig {
        device: device.to_string(),
        baud_rate: 115200,
        slave_id: 1,
        timeout_ms: 5000,
        retry_attempts: 3,
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
                cors_origins: vec!["*".to_string()],
            },
            sensors: vec![
                default_sensor("/dev/ttyUSB0"),
                default_sensor("/dev/ttyUSB1"),
                default_sensor("/dev/ttyUSB2"),
                default_sensor("/dev/ttyUSB3"),
            ],
            streaming: StreamingConfig {
                max_connections: 10,
                buffer_size: 1024,
                metrics_update_rate_hz: 5.0,
                raw_data_max_samples: 123,
                websocket_ping_interval_sec: 30,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "pretty".to_string(),
            },
        }
    }
}

impl AppConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        if !path.as_ref().exists() {
            // Create default config file if it doesn't exist
            let default_config = Self::default();
            default_config.save(&path)?;
            tracing::info!("Created default configuration file at {:?}", path.as_ref());
            return Ok(default_config);
        }

        let content = std::fs::read_to_string(&path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        // Validate server config
        if self.server.port == 0 {
            anyhow::bail!("Server port cannot be 0");
        }

        // Validate sensor configs
        if self.sensors.is_empty() {
            anyhow::bail!("At least one sensor must be configured");
        }

        for (i, sensor) in self.sensors.iter().enumerate() {
            let label = format!("sensor{}", i + 1);
            if sensor.slave_id == 0 {
                anyhow::bail!("{} slave ID cannot be 0", label);
            }
            if ![115200, 3000000].contains(&sensor.baud_rate) {
                tracing::warn!(
                    "Unusual baud rate {} for {}. Sensor supports 115200 or 3000000 bps",
                    sensor.baud_rate,
                    label
                );
            }
        }

        // Validate streaming config
        if self.streaming.max_connections == 0 {
            anyhow::bail!("Maximum connections cannot be 0");
        }

        if self.streaming.raw_data_max_samples > 123 {
            anyhow::bail!("Raw data max samples cannot exceed 123 (sensor limitation)");
        }

        if self.streaming.metrics_update_rate_hz > 5.0 {
            tracing::warn!(
                "Metrics update rate {} Hz exceeds sensor maximum of 5 Hz",
                self.streaming.metrics_update_rate_hz
            );
        }

        Ok(())
    }
}

// Helper function to create a sample config.toml
pub fn create_sample_config() -> String {
    toml::to_string_pretty(&AppConfig::default()).unwrap_or_else(|_| {
        r#"[server]
host = "127.0.0.1"
port = 3000
cors_origins = ["*"]

[modbus]
device = "/dev/ttyUSB0"  # Use "COM3" on Windows
baud_rate = 115200       # 115200 or 3000000 (3 Mbps for streaming)
slave_id = 1
timeout_ms = 5000
retry_attempts = 3

[streaming]
max_connections = 10
buffer_size = 1024
metrics_update_rate_hz = 5.0
raw_data_max_samples = 123
websocket_ping_interval_sec = 30

[logging]
level = "info"
format = "pretty"
"#
        .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_save_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let config = AppConfig::default();

        config.save(temp_file.path()).unwrap();
        let loaded_config = AppConfig::load(temp_file.path()).unwrap();

        assert_eq!(config.server.port, loaded_config.server.port);
        assert_eq!(
            config.sensors[0].slave_id,
            loaded_config.sensors[0].slave_id
        );
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::default();

        // Test invalid port
        config.server.port = 0;
        assert!(config.validate().is_err());

        // Test invalid slave ID
        config.server.port = 3000;
        config.sensors[0].slave_id = 0;
        assert!(config.validate().is_err());

        // Test valid config
        config.sensors[0].slave_id = 1;
        assert!(config.validate().is_ok());
    }
}
