use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub sensors: Vec<SensorConfig>,
    pub discovery: DiscoveryConfig,
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
pub struct SensorConfig {
    pub name: String,
    pub device: String,
    pub slave_id: u8,
    pub baud_rate: u32,
    pub timeout_ms: u64,
    pub enabled: bool,
}

impl Default for SensorConfig {
    fn default() -> Self {
        Self {
            name: "Sensor 1".to_string(),
            device: "/dev/ttyUSB0".to_string(),
            slave_id: 1,
            baud_rate: 115200,
            timeout_ms: 5000,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub auto_scan: bool,
    pub slave_id: u8,
    pub baud_rate: u32,
    pub probe_timeout_ms: u64,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            auto_scan: true,
            slave_id: 1,
            baud_rate: 115200,
            probe_timeout_ms: 2000,
        }
    }
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
            sensors: vec![SensorConfig::default()],
            discovery: DiscoveryConfig::default(),
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
        if self.server.port == 0 {
            anyhow::bail!("Server port cannot be 0");
        }

        if self.sensors.len() > 4 {
            anyhow::bail!("Maximum 4 sensors supported");
        }

        for (i, sensor) in self.sensors.iter().enumerate() {
            if sensor.slave_id == 0 {
                anyhow::bail!("Sensor {} slave ID cannot be 0", i + 1);
            }
            if ![115200u32, 3000000].contains(&sensor.baud_rate) {
                tracing::warn!(
                    "Sensor {}: unusual baud rate {}. Supports 115200 or 3000000 bps",
                    i + 1,
                    sensor.baud_rate
                );
            }
        }

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

        config.server.port = 0;
        assert!(config.validate().is_err());

        config.server.port = 3000;
        config.sensors[0].slave_id = 0;
        assert!(config.validate().is_err());

        config.sensors[0].slave_id = 1;
        assert!(config.validate().is_ok());
    }
}
