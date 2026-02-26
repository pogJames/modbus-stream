# Modbus Stream - Tri-axial Accelerometer Web Interface

A high-performance Rust web server providing REST API and WebSocket streaming for tri-axial accelerometer vibration sensors via Modbus RTU communication.

## Features

- **Complete REST API** for sensor configuration and data reading
- **Real-time WebSocket streaming** for continuous data collection
- **Modbus RTU communication** with full register support
- **High-performance streaming** at up to 3 Mbps baud rate
- **Comprehensive metrics** including RMS, Peak, Crest Factor, Skewness, Kurtosis
- **Web-based configuration** and monitoring
- **Built with modern async Rust** for maximum performance

## Supported Sensors

Compatible with tri-axial accelerometers featuring:
- RS485 Modbus RTU interface
- Function codes: FC03 (Read Holding), FC04 (Read Input), FC06 (Write Single)
- Big Endian byte order
- Models: 12B, 15B, KAX301, KAX302, S6S
- Gain ranges: 2G, 4G, 8G, 16G, 32G, 64G

## Quick Start

### Prerequisites

- Rust 2024 edition or later
- Serial/USB connection to accelerometer
- Windows, Linux, or macOS

### Installation

```bash
# Clone and build
git clone <repository>
cd modbus-stream
cargo build --release

# Create configuration
cp config.toml.example config.toml
# Edit config.toml with your device settings

# Run the server
./target/release/modbus-stream --device /dev/ttyUSB0 --baud-rate 115200
```

### Configuration

Create a `config.toml` file:

```toml
[server]
host = "127.0.0.1"
port = 3000
cors_origins = ["*"]

[modbus]
device = "/dev/ttyUSB0"  # Use "COM3" on Windows
baud_rate = 115200       # 115200 or 3000000 for streaming
slave_id = 1
timeout_ms = 5000
retry_attempts = 3

[streaming]
max_connections = 10
buffer_size = 1024
metrics_update_rate_hz = 5.0
raw_data_max_samples = 123
websocket_ping_interval_sec = 30
```

## API Reference

### Configuration Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/config` | Get current configuration |
| PUT | `/config/sample-rate` | Set sample rate (sps) |
| PUT | `/config/baud-rate` | Set baud rate (requires power cycle) |
| PUT | `/config/high-pass-filter` | Enable/disable high pass filter |
| PUT | `/config/stream-size` | Set bulk transfer size |

### Data Reading Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/read/temperature` | Read sensor temperature |
| GET | `/read/ucid` | Read device information |
| GET | `/read/firmware-version` | Read firmware version |
| GET | `/read/chip-id` | Read chip ID |
| GET | `/read/latest-raw` | Read latest X,Y,Z values |
| GET | `/read/gravity/rms` | Read gravity RMS values |
| GET | `/read/gravity/peak` | Read gravity peak values |
| GET | `/read/velocity/rms` | Read velocity RMS values |
| GET | `/read/all-metrics` | Read all metrics at once |

### Streaming Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| WS | `/stream/raw` | WebSocket for raw data streaming |
| WS | `/stream/metrics` | WebSocket for metrics streaming |
| POST | `/stream/start` | Start streaming mode |
| POST | `/stream/stop` | Stop streaming mode |
| GET | `/stream/status` | Get streaming status |

### Utility Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | API health check |
| GET | `/diagnostics` | System diagnostics |

## Usage Examples

### Set Sample Rate

```bash
curl -X PUT http://localhost:3000/config/sample-rate \
  -H "Content-Type: application/json" \
  -d '{"sampleRate": 1600}'
```

### Read Temperature

```bash
curl http://localhost:3000/read/temperature
```

### WebSocket Streaming (JavaScript)

```javascript
// Connect to raw data stream
const rawSocket = new WebSocket('ws://localhost:3000/stream/raw');

rawSocket.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Raw data:', data);
};

// Connect to metrics stream
const metricsSocket = new WebSocket('ws://localhost:3000/stream/metrics');

metricsSocket.onmessage = (event) => {
  const metrics = JSON.parse(event.data);
  console.log('Metrics:', metrics);
};
```

## Command Line Options

```
USAGE:
    modbus-stream [OPTIONS]

OPTIONS:
    -c, --config <FILE>      Configuration file path [default: config.toml]
    -b, --bind <ADDR>        Server bind address [default: 127.0.0.1:3000]
    -d, --device <DEVICE>    Modbus device path (e.g., /dev/ttyUSB0 or COM3)
    -r, --baud-rate <RATE>   Modbus baud rate [default: 115200]
    -s, --slave-id <ID>      Modbus slave ID [default: 1]
    -h, --help               Print help information
    -V, --version            Print version information
```

## Performance Notes

### Baud Rate Requirements

- **115200 bps**: Suitable for periodic metrics reading (5 Hz)
- **3 Mbps**: Required for continuous raw data streaming (7.8 kHz)

### Streaming Performance

- **Raw Data**: Up to 7812 samples/second at 3 Mbps
- **Metrics**: Up to 5 Hz update rate
- **WebSocket Connections**: Up to 10 concurrent clients
- **Buffer Management**: Automatic FIFO buffer monitoring

## Data Formats

### Raw Data Message

```json
{
  "type": "raw",
  "timestamp": "2025-06-18T10:30:00.123Z",
  "sequence": 12345,
  "data": [
    {"x": 0.125, "y": -0.089, "z": 1.001},
    {"x": 0.126, "y": -0.088, "z": 1.002}
  ]
}
```

### Metrics Message

```json
{
  "type": "metrics",
  "timestamp": "2025-06-18T10:30:00Z",
  "gravity": {
    "rms": {"x": 0.125, "y": 0.089, "z": 1.001},
    "peak": {"x": 0.245, "y": 0.189, "z": 1.201},
    "crestFactor": {"x": 1.96, "y": 2.12, "z": 1.20},
    "primaryFrequency": 120.5
  },
  "velocity": {
    "rms": {"x": 2.5, "y": 1.8, "z": 0.5},
    "primaryFrequency": 118.2
  },
  "temperature": 25.5
}
```

## Error Handling

The API provides comprehensive error responses:

```json
{
  "error": "Failed to read temperature: Device not responding",
  "code": "TEMPERATURE_READ_ERROR",
  "timestamp": "2025-06-18T10:30:00Z"
}
```

## Logging

Configure logging via environment variables:
```bash
RUST_LOG=modbus_stream=debug,tokio_modbus=info cargo run
```

Log levels: `error`, `warn`, `info`, `debug`, `trace`

## Development

### Project Structure

```
src/
├── main.rs              # Application entry point
├── config.rs            # Configuration management
├── modbus.rs            # Modbus client implementation
├── types.rs             # Data types and structures
├── routes/              # HTTP route handlers
│   ├── config.rs        # Configuration endpoints
│   ├── read.rs          # Data reading endpoints
│   ├── stream.rs        # Streaming endpoints
│   └── diagnostics.rs   # System diagnostics
└── stream/              # Streaming implementation
    └── manager.rs       # Stream management
```

### Building for Production

```bash
# Optimized release build
cargo build --release

# Strip debug symbols
strip target/release/modbus-stream

# Cross-compilation (example for ARM)
cargo build --target armv7-unknown-linux-gnueabihf --release
```

## Troubleshooting

### Common Issues

1. **Permission Denied on Serial Port**
   ```bash
   sudo usermod -a -G dialout $USER
   # Log out and back in
   ```

2. **Connection Timeout**
   - Check device path and baud rate
   - Verify Modbus slave ID
   - Test with simple Modbus tool first

3. **Streaming Performance Issues**
   - Ensure 3 Mbps baud rate for raw streaming
   - Monitor WebSocket client lag warnings
   - Check system USB/serial buffer sizes

### Debug Mode

Run with debug logging:
```bash
RUST_LOG=debug ./target/release/modbus-stream
```

## License

MIT OR Apache-2.0

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## Support

For issues and questions:
- Check the troubleshooting section
- Review sensor documentation
- Open a GitHub issue with debug logs
