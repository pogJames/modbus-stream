# Sensor Integration Summary

## Key Changes Made

### 1. **Enhanced Modbus Client** (`src/modbus.rs`)
- ✅ **Multi-baud rate support**: Tries 3 Mbps first, falls back to 115.2k (matching your test script)
- ✅ **Sensor-specific methods**: All functions from your working test script
- ✅ **Proper serial configuration**: 8N1, no flow control, 1000ms timeout
- ✅ **Error handling**: Uses `thiserror` for structured errors
- ✅ **Cross-platform compatibility**: Combines auto-detection with sensor specifics

### 2. **Updated Configuration** (`src/config.rs`)
- ✅ **Sensor-specific settings**: Sample rate, auto-init, polling intervals
- ✅ **Multi-baud rate configuration**: Array of baud rates to try
- ✅ **Enhanced validation**: Checks for supported baud rates and parameters
- ✅ **Platform overrides**: Still supports cross-platform port specification

### 3. **Sensor Data Types** (`src/types.rs`)
- ✅ **Complete register map**: All registers from sensor documentation
- ✅ **Data conversion utilities**: Temperature, gravity, velocity conversions
- ✅ **API structures**: WebSocket messages, streaming config, status
- ✅ **Validation functions**: Input validation for all sensor parameters

### 4. **Updated Dependencies** (`Cargo.toml`)
- ✅ **Matching versions**: `tokio-modbus = "0.7"`, `tokio-serial = "5.4"`
- ✅ **Error handling**: `thiserror = "1.0"`
- ✅ **Web framework**: Full Axum stack for API and streaming
- ✅ **Optimized builds**: Release profile optimizations

## Integration Steps

### 1. **Update Your Project Files**
Replace these files in your `modbus-stream` project:
- `src/modbus.rs` → Enhanced Modbus Client
- `src/config.rs` → Updated Configuration  
- `src/types.rs` → Sensor Data Types
- `Cargo.toml` → Updated Dependencies
- `config.toml` → Complete Sensor Configuration

### 2. **Update Your Main Application**
```rust
// src/main.rs
use modbus::{create_sensor_client, SensorError};
use config::AppConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = AppConfig::load_or_create_default("config.toml")?;
    config.validate()?; // Validate sensor-specific settings
    
    // Create sensor client with initialization
    let sensor_client = create_sensor_client(
        config.modbus.clone(), 
        config.modbus.sensor.sample_rate
    ).await?;
    
    // Test sensor communication
    test_sensor_communication(&sensor_client).await?;
    
    // Start your web server...
    Ok(())
}

async fn test_sensor_communication(client: &modbus::ModbusClient) -> Result<(), SensorError> {
    println!("Testing sensor communication...");
    
    // Read all sensor data (like your test script)
    let sensor_data = client.read_all_sensor_data().await?;
    
    println!("✓ Chip ID: {:?}", sensor_data.chip_id);
    println!("✓ Temperature: {:.2}°C", sensor_data.temperature);
    println!("✓ UCID - Model: {}, Gain: {}G, Serial: {}", 
             sensor_data.ucid_info.model, 
             sensor_data.ucid_info.gain, 
             sensor_data.ucid_info.serial_number);
    println!("✓ Latest XYZ: X:{}, Y:{}, Z:{}", 
             sensor_data.latest_xyz.0, 
             sensor_data.latest_xyz.1, 
             sensor_data.latest_xyz.2);
    println!("✓ RMS Gravity: X:{:.3}g, Y:{:.3}g, Z:{:.3}g", 
             sensor_data.rms_gravity[0], 
             sensor_data.rms_gravity[1], 
             sensor_data.rms_gravity[2]);
    println!("✓ Velocity RMS: X:{:.2}mm/s, Y:{:.2}mm/s, Z:{:.2}mm/s", 
             sensor_data.velocity_rms[0], 
             sensor_data.velocity_rms[1], 
             sensor_data.velocity_rms[2]);
    
    Ok(())
}
```

### 3. **Build and Test**
```bash
# Clean build with new dependencies
cargo clean
cargo build

# Test sensor connection
cargo run

# Run your original test script for comparison
cargo run --bin sensor-test  # If you create the binary
```

## Configuration Options

### **Auto-Detection (Recommended)**
```toml
[modbus]
device_path = "auto"
baud_rates = [3000000, 115200]
```

### **Platform-Specific**
```toml
[modbus.platform_overrides]
windows = "COM3"
linux = "/dev/ttyUSB0"
macos = "/dev/cu.usbserial-0001"
```

### **High-Speed Streaming** (3 Mbps required)
```toml
[modbus.sensor]
sample_rate = 7812
polling_interval_ms = 50
stream_buffer_size = 500
```

### **Low-Bandwidth Monitoring** (115.2k compatible)
```toml
[modbus.sensor]
sample_rate = 800
polling_interval_ms = 1000  
stream_buffer_size = 10
```

## API Endpoints

The enhanced implementation provides these endpoints:

- `GET /api/ports` - List available serial ports
- `GET /api/sensor/status` - Sensor connection status
- `GET /api/sensor/data` - Current sensor readings
- `POST /api/sensor/config` - Update sensor configuration
- `GET /api/sensor/stream` - WebSocket for real-time data
- `POST /api/sensor/initialize` - Re-initialize sensor

## Key Benefits

### ✅ **Proven Communication Logic**
- Uses the exact same baud rate fallback strategy as your working test script
- Identical serial port settings (8N1, no flow control, 1000ms timeout)
- Same register addresses and function codes from sensor documentation
- Matching data conversion formulas (temp/100, gravity/1000, velocity/100)

### ✅ **Enhanced Cross-Platform Support**
- Auto-detects serial ports on Windows, Linux, and macOS
- Platform-specific overrides when needed
- Graceful fallback if configured port is unavailable
- Better error messages showing available ports

### ✅ **Production-Ready Features**
- Structured error handling with `thiserror`
- Configuration validation for sensor parameters
- WebSocket streaming for real-time data
- Health checks and status monitoring
- Automatic reconnection on connection loss

### ✅ **Flexible Configuration**
- Multiple deployment scenarios (streaming vs. monitoring)
- Environment-specific settings
- Hot-reloadable configuration
- Comprehensive documentation

## Troubleshooting

### **Connection Issues**
If you get connection errors:

1. **Check available ports**:
   ```bash
   # The app will show available ports on startup
   cargo run
   ```

2. **Try manual port specification**:
   ```toml
   [modbus]
   device_path = "COM3"  # Windows
   # device_path = "/dev/ttyUSB0"  # Linux
   ```

3. **Verify baud rate support**:
   ```bash
   # Check if sensor responds at 115.2k first
   # Your test script logic handles this automatically
   ```

### **Data Reading Issues**
If sensor data appears incorrect:

1. **Verify sample rate initialization**:
   ```toml
   [modbus.sensor]
   auto_initialize = true
   sample_rate = 1600
   ```

2. **Check data conversions**:
   ```rust
   // Temperature: raw_value / 100.0 = °C
   // Gravity: raw_value / 1000.0 = g  
   // Velocity: raw_value / 100.0 = mm/s
   ```

3. **Validate UCID parsing**:
   ```rust
   // Model, gain, and serial number should match sensor specs
   ```

### **Performance Optimization**
For high-speed applications:

1. **Use 3 Mbps connection**:
   ```toml
   [modbus]
   baud_rates = [3000000]  # Force 3 Mbps only
   ```

2. **Optimize polling**:
   ```toml
   [modbus.sensor]
   polling_interval_ms = 50   # 20 Hz
   stream_buffer_size = 500   # Larger buffer
   ```

3. **Enable streaming mode**:
   ```rust
   // Use bulk transfer registers for continuous data
   // Register 0x0015 for stream size configuration
   ```

## Next Steps

### **Immediate Actions**
1. ✅ Replace the files in your project with the enhanced versions
2. ✅ Update `Cargo.toml` dependencies to match working versions
3. ✅ Test basic sensor communication with `cargo run`
4. ✅ Verify all sensor readings match your test script output

### **Development Workflow**
1. **Start with auto-detection**:
   ```toml
   device_path = "auto"
   ```

2. **Test sensor initialization**:
   ```bash
   RUST_LOG=debug cargo run
   ```

3. **Verify data accuracy**:
   - Compare with your standalone test script
   - Check temperature, UCID, and XYZ readings
   - Validate RMS and velocity calculations

4. **Configure for your use case**:
   - High-speed streaming vs. periodic monitoring
   - Platform-specific port settings
   - Polling intervals and buffer sizes

### **Advanced Integration**
1. **Real-time streaming**:
   ```javascript
   // WebSocket client for live data
   const ws = new WebSocket('ws://localhost:3000/api/sensor/stream');
   ws.onmessage = (event) => {
     const data = JSON.parse(event.data);
     console.log('Sensor data:', data);
   };
   ```

2. **REST API integration**:
   ```bash
   # Get current sensor status
   curl http://localhost:3000/api/sensor/status
   
   # Get latest readings
   curl http://localhost:3000/api/sensor/data
   
   # Update configuration
   curl -X POST http://localhost:3000/api/sensor/config \
        -H "Content-Type: application/json" \
        -d '{"sample_rate": 3200}'
   ```

3. **Data persistence**:
   ```rust
   // Add database integration for historical data
   // Implement data logging and analytics
   // Create data export functionality
   ```

## File Structure Overview

```
modbus-stream/
├── src/
│   ├── main.rs              # Application entry point
│   ├── config.rs            # ✅ Enhanced configuration
│   ├── modbus.rs            # ✅ Enhanced sensor client
│   ├── types.rs             # ✅ Sensor data types
│   ├── serial_utils.rs      # Cross-platform serial support
│   ├── routes/              # API endpoints
│   │   ├── mod.rs
│   │   ├── sensor.rs        # Sensor API routes
│   │   ├── stream.rs        # WebSocket streaming
│   │   └── settings.rs      # Configuration API
│   └── bin/
│       └── sensor_test.rs   # Optional: standalone sensor test
├── config.toml              # ✅ Complete sensor configuration
├── Cargo.toml               # ✅ Updated dependencies
└── README.md                # Updated documentation
```

## Compatibility Matrix

| Platform | Port Format | Auto-Detection | Manual Config |
|----------|-------------|----------------|---------------|
| Windows | `COM3`, `COM4` | ✅ | ✅ |
| Linux | `/dev/ttyUSB0` | ✅ | ✅ |
| macOS | `/dev/cu.usbserial-*` | ✅ | ✅ |

| Baud Rate | Streaming | Periodic | Use Case |
|-----------|-----------|----------|----------|
| 3 Mbps | ✅ Full | ✅ | Real-time applications |
| 115.2k | ❌ Limited | ✅ | Monitoring applications |

| Sample Rate | I-type | K-type | Notes |
|-------------|--------|--------|-------|
| Default | 7812 Hz | 6400 Hz | Maximum performance |
| Recommended | 1600 Hz | 1600 Hz | Balanced performance |
| Minimum | 100 Hz | 100 Hz | Low-bandwidth mode |

This integration gives you the best of both worlds: the proven sensor communication logic from your working test script combined with robust cross-platform support and production-ready features for your web application.