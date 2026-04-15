use anyhow::{Context as AnyhowContext, Result};
use chrono::Utc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_modbus::prelude::*;
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tracing::{debug, error, info, warn};

use crate::types::{
    registers, AccelerationData, AllMetricsResponse, DeviceInfo, GravityMetrics, LatestRawResponse,
    UcidInfo, VelocityMetrics,
};

pub struct ModbusClient {
    context: Mutex<tokio_modbus::client::Context>,
    slave_id: u8,
    device_path: String,
    baud_rate: u32,
    scale_factor: Mutex<Option<f64>>,
}

impl ModbusClient {
    pub async fn new(device_path: &str, baud_rate: u32, slave_id: u8) -> Result<Self> {
        let serial_builder = tokio_serial::new(device_path, baud_rate)
            .data_bits(tokio_serial::DataBits::Eight)
            .stop_bits(tokio_serial::StopBits::One)
            .parity(tokio_serial::Parity::None)
            .timeout(Duration::from_millis(5000));

        let port = SerialStream::open(&serial_builder)
            .with_context(|| format!("Failed to open serial port {}", device_path))?;

        // Use the correct API for tokio-modbus 0.14 with RTU feature
        let context = rtu::attach_slave(port, Slave(slave_id));

        info!(
            "Connected to Modbus device: {} at {} bps, slave ID {}",
            device_path, baud_rate, slave_id
        );

        Ok(Self {
            context: Mutex::new(context),
            slave_id,
            device_path: device_path.to_string(),
            baud_rate,
            scale_factor: Mutex::new(None),
        })
    }

    /// Reconnect to the Modbus device
    pub async fn reconnect(&self) -> Result<()> {
        let serial_builder = tokio_serial::new(&self.device_path, self.baud_rate)
            .data_bits(tokio_serial::DataBits::Eight)
            .stop_bits(tokio_serial::StopBits::One)
            .parity(tokio_serial::Parity::None)
            .timeout(Duration::from_millis(5000));

        let port = SerialStream::open(&serial_builder)
            .with_context(|| format!("Failed to open serial port {}", self.device_path))?;

        let new_context = rtu::attach_slave(port, Slave(self.slave_id));

        let mut context = self.context.lock().await;
        *context = new_context;

        info!("Reconnected to Modbus device");
        Ok(())
    }

    /// Read holding registers (FC03)
    async fn read_holding_registers(&self, address: u16, count: u16) -> Result<Vec<u16>> {
        let mut context = self.context.lock().await;
        let result = context
            .read_holding_registers(address, count)
            .await
            .map_err(|e| anyhow::anyhow!("IO error reading holding registers at address 0x{:04X}, count {}: {}", address, count, e))?
            .map_err(|e| anyhow::anyhow!("Modbus exception reading holding registers at address 0x{:04X}, count {}: {:?}", address, count, e))?;
        debug!(
            "Read holding registers 0x{:04X}+{}: {:?}",
            address, count, result
        );
        Ok(result)
    }

    /// Read input registers (FC04)
    async fn read_input_registers(&self, address: u16, count: u16) -> Result<Vec<u16>> {
        let mut context = self.context.lock().await;
        let result = context
            .read_input_registers(address, count)
            .await
            .map_err(|e| anyhow::anyhow!("IO error reading input registers at address 0x{:04X}, count {}: {}", address, count, e))?
            .map_err(|e| anyhow::anyhow!("Modbus exception reading input registers at address 0x{:04X}, count {}: {:?}", address, count, e))?;
        debug!(
            "Read input registers 0x{:04X}+{}: {:?}",
            address, count, result
        );
        Ok(result)
    }

    /// Write single register (FC06)
    async fn write_single_register(&self, address: u16, value: u16) -> Result<()> {
        let mut context = self.context.lock().await;
        context
            .write_single_register(address, value)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to write register at address 0x{:04X} with value 0x{:04X}: {}", address, value, e))?;
        debug!("Wrote register 0x{:04X} = 0x{:04X}", address, value);
        Ok(())
    }

    /// Set sample rate
    pub async fn set_sample_rate(&self, sample_rate: u16) -> Result<()> {
        self.write_single_register(registers::SAMPLE_RATE, sample_rate)
            .await?;
        info!("Set sample rate to {} sps", sample_rate);
        Ok(())
    }

    /// Set baud rate (requires power cycle)
    pub async fn set_baud_rate(&self, baud_rate: u32) -> Result<()> {
        let high = ((baud_rate >> 16) & 0xFFFF) as u16;
        let low = (baud_rate & 0xFFFF) as u16;

        self.write_single_register(registers::BAUD_RATE_HIGH, high)
            .await?;
        self.write_single_register(registers::BAUD_RATE_LOW, low)
            .await?;

        warn!(
            "Set baud rate to {} bps. Power cycle the sensor to take effect.",
            baud_rate
        );
        Ok(())
    }

    /// Enable/disable high pass filter
    pub async fn set_high_pass_filter(&self, enabled: bool) -> Result<()> {
        let value = if enabled { 1 } else { 0 };
        self.write_single_register(registers::HIGH_PASS_ENABLE, value)
            .await?;
        info!("High pass filter: {}", if enabled { "enabled" } else { "disabled" });
        Ok(())
    }

    /// Set stream size for bulk transfer
    pub async fn set_stream_size(&self, size: u16) -> Result<()> {
        if size > 123 {
            anyhow::bail!("Stream size cannot exceed 123 registers");
        }
        self.write_single_register(registers::STREAM_SIZE, size)
            .await?;
        info!("Set stream size to {} registers", size);
        Ok(())
    }

    /// Read temperature
    pub async fn read_sample_rate(&self) -> Result<u16> {
        let data = self.read_holding_registers(registers::SAMPLE_RATE, 1).await?;
        Ok(data[0])
    }

    pub async fn read_temperature(&self) -> Result<f64> {
        let data = self
            .read_holding_registers(registers::TEMPERATURE, 1)
            .await?;
        let temp = data[0] as f64 / 100.0; // Convert from raw value
        debug!("Temperature: {:.2}°C", temp);
        Ok(temp)
    }

    /// Read UCID information
    pub async fn read_ucid(&self) -> Result<UcidInfo> {
        let data = self.read_holding_registers(registers::UCID, 2)
            .await
            .with_context(|| "Failed to read UCID registers")?;

        if data.len() != 2 {
            anyhow::bail!("Expected 2 UCID registers, got {}", data.len());
        }

        let raw_value = ((data[0] as u32) << 16) | (data[1] as u32);
        let ucid = UcidInfo::from_raw(raw_value);
        debug!("UCID: {:?}", ucid);
        Ok(ucid)
    }

    /// Get the scale factor for converting raw values to g's, reading UCID if not cached
    pub async fn get_scale_factor(&self) -> f64 {
        // Check cache first
        let cached = self.scale_factor.lock().await;
        if let Some(factor) = *cached {
            return factor;
        }
        drop(cached);

        // Read UCID and calculate scale factor
        let factor = match self.read_ucid().await {
            Ok(ucid) => {
                let factor = ucid.scale_factor();
                debug!("Determined scale factor {} from gain {}", factor, ucid.gain);
                factor
            }
            Err(e) => {
                warn!("Failed to read UCID for scale factor, using default 4G: {}", e);
                1.0 / 8192.0 // Default to 4G (32768 / 4 = 8192 counts/g)
            }
        };

        // Cache the result
        let mut cached = self.scale_factor.lock().await;
        *cached = Some(factor);
        factor
    }

    /// Clear the cached scale factor (call when sensor is reconfigured)
    pub async fn clear_scale_factor_cache(&self) {
        let mut cached = self.scale_factor.lock().await;
        *cached = None;
        debug!("Scale factor cache cleared");
    }

    /// Read firmware version — returns "n/a" if the register is not supported by this sensor
    pub async fn read_firmware_version(&self) -> Result<String> {
        let data = match self.read_holding_registers(registers::FIRMWARE_VERSION, 1).await {
            Ok(d)  => d,
            Err(e) => {
                debug!("Firmware version register unsupported on this sensor: {}", e);
                return Ok("n/a".to_string());
            }
        };
        let version = format!("{}.{}", (data[0] >> 8) & 0xFF, data[0] & 0xFF);
        debug!("Firmware version: {}", version);
        Ok(version)
    }

    /// Read chip ID
    pub async fn read_chip_id(&self) -> Result<Vec<u16>> {
        let data = self.read_input_registers(registers::CHIP_ID, 3)
            .await
            .with_context(|| "Failed to read chip ID registers")?;
        
        if data.len() != 3 {
            anyhow::bail!("Expected 3 chip ID registers, got {}", data.len());
        }
        
        debug!("Chip ID: {:?}", data);
        Ok(data)
    }

    /// Read FIFO buffer size
    pub async fn read_fifo_buffer_size(&self) -> Result<u16> {
        let data = self
            .read_input_registers(registers::FIFO_BUFFER_SIZE, 1)
            .await?;
        debug!("FIFO buffer size: {}", data[0]);
        Ok(data[0])
    }

    /// Read latest raw data (X, Y, Z)
    pub async fn read_latest_raw(&self) -> Result<LatestRawResponse> {
        let data = self
            .read_input_registers(registers::RAW_DATA_LATEST_X, 3)
            .await?;

        // Get scale factor based on sensor gain setting (cached after first read)
        let scale_factor = self.get_scale_factor().await;

        let x = (data[0] as i16) as f64 * scale_factor;
        let y = (data[1] as i16) as f64 * scale_factor;
        let z = (data[2] as i16) as f64 * scale_factor;

        debug!("Latest raw data: X={:.6}g, Y={:.6}g, Z={:.6}g", x, y, z);

        Ok(LatestRawResponse {
            timestamp: Utc::now(),
            x,
            y,
            z,
            unit: "g".to_string(),
        })
    }

    /// Read gravity RMS values
    pub async fn read_gravity_rms(&self) -> Result<AccelerationData> {
        let data = self
            .read_holding_registers(registers::GRAVITY_RMS, 3)
            .await?;
        let rms = AccelerationData {
            x: data[0] as f64 / 1000.0,
            y: data[1] as f64 / 1000.0,
            z: data[2] as f64 / 1000.0,
        };
        debug!("Gravity RMS: {:?}", rms);
        Ok(rms)
    }

    /// Read gravity peak values
    pub async fn read_gravity_peak(&self) -> Result<AccelerationData> {
        let data = self
            .read_holding_registers(registers::GRAVITY_PEAK, 3)
            .await?;
        let peak = AccelerationData {
            x: data[0] as f64 / 1000.0,
            y: data[1] as f64 / 1000.0,
            z: data[2] as f64 / 1000.0,
        };
        debug!("Gravity Peak: {:?}", peak);
        Ok(peak)
    }

    /// Read gravity crest factor
    pub async fn read_gravity_crest_factor(&self) -> Result<AccelerationData> {
        let data = self
            .read_holding_registers(registers::GRAVITY_CREST_FACTOR, 3)
            .await?;
        let crest = AccelerationData {
            x: data[0] as f64 / 1000.0,
            y: data[1] as f64 / 1000.0,
            z: data[2] as f64 / 1000.0,
        };
        debug!("Gravity Crest Factor: {:?}", crest);
        Ok(crest)
    }

    /// Read gravity skewness
    pub async fn read_gravity_skewness(&self) -> Result<AccelerationData> {
        let data = self
            .read_holding_registers(registers::GRAVITY_SKEWNESS, 3)
            .await?;
        let skewness = AccelerationData {
            x: data[0] as f64 / 1000.0,
            y: data[1] as f64 / 1000.0,
            z: data[2] as f64 / 1000.0,
        };
        debug!("Gravity Skewness: {:?}", skewness);
        Ok(skewness)
    }

    /// Read gravity kurtosis
    pub async fn read_gravity_kurtosis(&self) -> Result<AccelerationData> {
        let data = self
            .read_holding_registers(registers::GRAVITY_KURTOSIS, 3)
            .await?;
        let kurtosis = AccelerationData {
            x: data[0] as f64 / 1000.0,
            y: data[1] as f64 / 1000.0,
            z: data[2] as f64 / 1000.0,
        };
        debug!("Gravity Kurtosis: {:?}", kurtosis);
        Ok(kurtosis)
    }

    /// Read gravity primary frequency
    pub async fn read_gravity_primary_frequency(&self) -> Result<f64> {
        let data = self
            .read_holding_registers(registers::GRAVITY_PRIMARY_FREQ, 1)
            .await?;
        let freq = data[0] as f64;
        debug!("Gravity Primary Frequency: {:.2} Hz", freq);
        Ok(freq)
    }

    /// Read velocity RMS values
    pub async fn read_velocity_rms(&self) -> Result<AccelerationData> {
        let data = self
            .read_holding_registers(registers::VELOCITY_RMS, 3)
            .await?;
        let rms = AccelerationData {
            x: data[0] as f64 / 100.0,
            y: data[1] as f64 / 100.0,
            z: data[2] as f64 / 100.0,
        };
        debug!("Velocity RMS: {:?}", rms);
        Ok(rms)
    }

    /// Read velocity peak values
    pub async fn read_velocity_peak(&self) -> Result<AccelerationData> {
        let data = self
            .read_holding_registers(registers::VELOCITY_PEAK, 3)
            .await?;
        let peak = AccelerationData {
            x: data[0] as f64 / 100.0,
            y: data[1] as f64 / 100.0,
            z: data[2] as f64 / 100.0,
        };
        debug!("Velocity Peak: {:?}", peak);
        Ok(peak)
    }

    /// Read velocity crest factor
    pub async fn read_velocity_crest_factor(&self) -> Result<AccelerationData> {
        let data = self
            .read_holding_registers(registers::VELOCITY_CREST_FACTOR, 3)
            .await?;
        let crest = AccelerationData {
            x: data[0] as f64 / 100.0,
            y: data[1] as f64 / 100.0,
            z: data[2] as f64 / 100.0,
        };
        debug!("Velocity Crest Factor: {:?}", crest);
        Ok(crest)
    }

    /// Read velocity primary frequency
    pub async fn read_velocity_primary_frequency(&self) -> Result<f64> {
        let data = self
            .read_holding_registers(registers::VELOCITY_PRIMARY_FREQ, 1)
            .await?;
        let freq = data[0] as f64;
        debug!("Velocity Primary Frequency: {:.2} Hz", freq);
        Ok(freq)
    }

    /// Read all gravity metrics
    pub async fn read_gravity_metrics(&self) -> Result<GravityMetrics> {
        let rms = self.read_gravity_rms().await?;
        let peak = self.read_gravity_peak().await?;
        let crest_factor = self.read_gravity_crest_factor().await?;
        let skewness = self.read_gravity_skewness().await?;
        let kurtosis = self.read_gravity_kurtosis().await?;
        let primary_frequency = self.read_gravity_primary_frequency().await?;

        Ok(GravityMetrics {
            rms,
            peak,
            crest_factor,
            skewness,
            kurtosis,
            primary_frequency,
        })
    }

    /// Read all velocity metrics
    pub async fn read_velocity_metrics(&self) -> Result<VelocityMetrics> {
        let rms = self.read_velocity_rms().await?;
        let peak = self.read_velocity_peak().await?;
        let crest_factor = self.read_velocity_crest_factor().await?;
        let primary_frequency = self.read_velocity_primary_frequency().await?;

        Ok(VelocityMetrics {
            rms,
            peak,
            crest_factor,
            primary_frequency,
        })
    }

    /// Read all metrics (gravity + velocity)
    pub async fn read_all_metrics(&self) -> Result<AllMetricsResponse> {
        let gravity = self.read_gravity_metrics().await?;
        let velocity = self.read_velocity_metrics().await?;

        Ok(AllMetricsResponse {
            timestamp: Utc::now(),
            gravity,
            velocity,
        })
    }

    /// Read device information
    pub async fn read_device_info(&self) -> Result<DeviceInfo> {
        let ucid = self.read_ucid().await?;
        let firmware_version = self.read_firmware_version().await?;
        let chip_id = self.read_chip_id().await?;
        let temperature = self.read_temperature().await?;

        Ok(DeviceInfo {
            ucid,
            firmware_version,
            chip_id,
            temperature,
        })
    }

    /// Read FIFO buffer size AND raw data in a single Modbus transaction.
    /// Reads `1 + count` input registers starting at 0x0002 (FIFO_BUFFER_SIZE):
    ///   result[0]   = updated FIFO fill level (use as `count` for the next call)
    ///   result[1..] = `count` raw XYZ registers converted to AccelerationData
    /// One round trip instead of two — same technique as the vendor's Python DAQ.
    pub async fn read_fifo_combined(&self, count: u16) -> Result<(u16, Vec<AccelerationData>)> {
        if count == 0 || count > 123 {
            anyhow::bail!("count must be 1–123, got {}", count);
        }

        let regs = self
            .read_input_registers(registers::FIFO_BUFFER_SIZE, 1 + count)
            .await?;

        let next_size = regs[0];
        let scale_factor = self.get_scale_factor().await;

        let mut samples = Vec::new();
        for chunk in regs[1..].chunks_exact(3) {
            let x = (chunk[0] as i16) as f64 * scale_factor;
            let y = (chunk[1] as i16) as f64 * scale_factor;
            let z = (chunk[2] as i16) as f64 * scale_factor;
            samples.push(AccelerationData { x, y, z });
        }

        Ok((next_size, samples))
    }

    /// Read raw data buffer (up to 123 registers)
    /// Note: count should be divisible by 3 since each sample requires 3 registers (X, Y, Z)
    pub async fn read_raw_data_buffer(&self, count: u16) -> Result<Vec<AccelerationData>> {
        if count == 0 {
            anyhow::bail!("Count cannot be zero");
        }
        if count > 123 {
            anyhow::bail!("Cannot read more than 123 registers (sensor limitation)");
        }
        if count % 3 != 0 {
            warn!("Register count {} is not divisible by 3. Some data may be incomplete.", count);
        }

        let data = self
            .read_input_registers(registers::RAW_DATA_START, count)
            .await
            .with_context(|| format!("Failed to read {} raw data registers", count))?;

        // Convert raw data to acceleration values
        // Each triplet represents X, Y, Z
        let mut result = Vec::new();
        let scale_factor = self.get_scale_factor().await;

        for chunk in data.chunks_exact(3) {
            let x = (chunk[0] as i16) as f64 * scale_factor;
            let y = (chunk[1] as i16) as f64 * scale_factor;
            let z = (chunk[2] as i16) as f64 * scale_factor;

            result.push(AccelerationData { x, y, z });
        }

        // Handle remaining registers if count wasn't divisible by 3
        let remainder = data.len() % 3;
        if remainder > 0 {
            warn!("Discarded {} incomplete register(s) at end of buffer", remainder);
        }

        debug!("Read {} complete raw data samples from {} registers", result.len(), data.len());
        Ok(result)
    }

    /// Test connection by reading a known register
    pub async fn test_connection(&self) -> Result<()> {
        self.read_temperature().await
            .map(|_| {
                debug!("Connection test successful");
            })
            .map_err(|e| {
                error!("Connection test failed: {}", e);
                e
            })
    }

    /// Test connection and return boolean result (for simple checking)
    pub async fn is_connected(&self) -> bool {
        match self.test_connection().await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}
