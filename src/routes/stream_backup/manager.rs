use anyhow::Result;
use chrono::Utc;
use std::{sync::Arc, time::Duration};
use tokio::{sync::broadcast, time::interval};
use tracing::{debug, error, info, warn};

use crate::{
    modbus::ModbusClient,
    types::{AccelerationData, WebSocketMessage},
};

pub struct StreamManager {
    modbus_client: Arc<ModbusClient>,
}

impl StreamManager {
    pub fn new(modbus_client: Arc<ModbusClient>) -> Self {
        Self { modbus_client }
    }

    /// Start raw data streaming task
    pub async fn start_raw_streaming(
        &self,
        tx: broadcast::Sender<WebSocketMessage>,
    ) -> Result<()> {
        info!("Starting raw data streaming");

        let mut sequence = 0u64;
        let mut interval = interval(Duration::from_millis(10)); // 100 Hz max

        loop {
            interval.tick().await;

            match self.read_raw_data_batch().await {
                Ok(data) => {
                    let message = WebSocketMessage::RawData {
                        timestamp: Utc::now(),
                        sequence,
                        data,
                    };

                    if let Err(e) = tx.send(message) {
                        // All receivers dropped
                        if matches!(e, broadcast::error::SendError(_)) {
                            info!("All raw data receivers dropped, stopping stream");
                            break;
                        }
                    }

                    sequence += 1;
                }
                Err(e) => {
                    error!("Failed to read raw data: {}", e);

                    let error_message = WebSocketMessage::Error {
                        message: format!("Failed to read raw data: {}", e),
                        code: Some("RAW_DATA_READ_ERROR".to_string()),
                    };

                    if let Err(send_error) = tx.send(error_message) {
                        if matches!(send_error, broadcast::error::SendError(_)) {
                            info!("All receivers dropped, stopping stream");
                            break;
                        }
                    }

                    // Wait a bit before retrying
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        info!("Raw data streaming stopped");
        Ok(())
    }

    /// Start metrics streaming task
    pub async fn start_metrics_streaming(
        &self,
        tx: broadcast::Sender<WebSocketMessage>,
    ) -> Result<()> {
        info!("Starting metrics streaming");

        let mut interval = interval(Duration::from_millis(200)); // 5 Hz

        loop {
            interval.tick().await;

            match self.read_metrics_data().await {
                Ok(message) => {
                    if let Err(e) = tx.send(message) {
                        // All receivers dropped
                        if matches!(e, broadcast::error::SendError(_)) {
                            info!("All metrics receivers dropped, stopping stream");
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to read metrics: {}", e);

                    let error_message = WebSocketMessage::Error {
                        message: format!("Failed to read metrics: {}", e),
                        code: Some("METRICS_READ_ERROR".to_string()),
                    };

                    if let Err(send_error) = tx.send(error_message) {
                        if matches!(send_error, broadcast::error::SendError(_)) {
                            info!("All receivers dropped, stopping stream");
                            break;
                        }
                    }

                    // Wait a bit before retrying
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }

        info!("Metrics streaming stopped");
        Ok(())
    }

    /// Read a batch of raw data from the sensor
    async fn read_raw_data_batch(&self) -> Result<Vec<AccelerationData>> {
        // Check FIFO buffer size first
        let buffer_size = self.modbus_client.read_fifo_buffer_size().await?;

        if buffer_size == 0 {
            debug!("FIFO buffer is empty");
            return Ok(vec![]);
        }

        // Read up to the buffer size or maximum registers (123)
        let read_count = std::cmp::min(buffer_size, 123);
        let raw_data = self.modbus_client.read_raw_data_buffer(read_count).await?;

        debug!("Read {} raw data samples", raw_data.len());
        Ok(raw_data)
    }

    /// Read metrics data and create WebSocket message
    async fn read_metrics_data(&self) -> Result<WebSocketMessage> {
        // Read all metrics in parallel for better performance
        let (gravity_result, velocity_result, temperature_result) = tokio::join!(
            self.modbus_client.read_gravity_metrics(),
            self.modbus_client.read_velocity_metrics(),
            self.modbus_client.read_temperature()
        );

        let gravity = gravity_result?;
        let velocity = velocity_result?;
        let temperature = temperature_result?;

        Ok(WebSocketMessage::Metrics {
            timestamp: Utc::now(),
            gravity,
            velocity,
            temperature,
        })
    }

    /// Test the connection to the Modbus device
    pub async fn test_connection(&self) -> bool {
        self.modbus_client.test_connection().await.unwrap_or(false)
    }

    /// Reconnect to the Modbus device
    pub async fn reconnect(&self) -> Result<()> {
        self.modbus_client.reconnect().await
    }
}
