use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub modbus1: ModbusConfig,
    pub modbus2: ModbusConfig,
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

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
                cors_origins: vec!["*".to_string()],
            },
            modbus1: ModbusConfig {
                device: "/dev/ttyUSB0".to_string(),
                baud_rate: 115200,
                slave_id: 1,
                timeout_ms: 5000,
                retry_attempts: 3,
            },
            modbus2: ModbusConfig {
                device: "/dev/ttyUSB1".to_string(),
                baud_rate: 115200,
                slave_id: 1,
                timeout_ms: 5000,
                retry_attempts: 3,
            },
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

        // Validate modbus configs
        if self.modbus1.slave_id == 0 {
            anyhow::bail!("Modbus1 slave ID cannot be 0");
        }
        if self.modbus2.slave_id == 0 {
            anyhow::bail!("Modbus2 slave ID cannot be 0");
        }

        for (label, baud) in [("modbus1", self.modbus1.baud_rate), ("modbus2", self.modbus2.baud_rate)] {
            if ![115200, 3000000].contains(&baud) {
                tracing::warn!(
                    "Unusual baud rate {} for {}. Sensor supports 115200 or 3000000 bps",
                    baud, label
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
"#.to_string()
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
        assert_eq!(config.modbus1.slave_id, loaded_config.modbus1.slave_id);
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::default();
        
        // Test invalid port
        config.server.port = 0;
        assert!(config.validate().is_err());
        
        // Test invalid slave ID
        config.server.port = 3000;
        config.modbus1.slave_id = 0;
        assert!(config.validate().is_err());

        // Test valid config
        config.modbus1.slave_id = 1;
        assert!(config.validate().is_ok());
    }
}
