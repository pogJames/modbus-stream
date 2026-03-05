# Modbus Stream - Tri-axial Accelerometer Web Interface

A high-performance Rust web server providing REST API, WebSocket streaming, and a browser-based UI for tri-axial accelerometer vibration sensors via Modbus RTU communication.

## Features

- **Browser-based dashboard** with live charts and navigation
- **Settings page** with live validation, connection testing, and hot-reload (HTMX)
- **Complete REST API** for sensor configuration and data reading
- **Real-time WebSocket streaming** for continuous raw data and metrics
- **Modbus RTU communication** with full register support
- **High-performance streaming** at up to 3 Mbps baud rate
- **Comprehensive metrics** including RMS, Peak, Crest Factor, Skewness, Kurtosis, Primary Frequency
- **CSV recording** — capture 10-second bursts (~78k samples) direct to disk
- **CSV viewer** — browser-based interactive chart for recorded files
- **Offline mode** — server starts and serves UI even when no sensor is connected
- **FTDI latency optimisation** — automatically lowers USB serial latency timer to 1 ms
- **Startup diagnostics** — 4-step connection test printed before the server starts
- **Built with async Rust** (Axum, Tokio, Minijinja)

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

# Run the server (config.toml is created automatically on first run)
./target/release/modbus-stream --device /dev/ttyUSB0 --baud-rate 3000000
```

Open `http://localhost:3000` in a browser to access the dashboard.

### Configuration

The server reads `config.toml` on startup and creates a default file if it does not exist. Edit it to match your hardware:

```toml
[server]
host = "127.0.0.1"
port = 3000
cors_origins = ["*"]

[modbus]
device = "/dev/ttyUSB0"  # Use "COM3" on Windows
baud_rate = 3000000      # 115200 for metrics-only; 3000000 for continuous raw streaming
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
```

## Web UI

| URL | Description |
|-----|-------------|
| `/` | Dashboard — navigation hub |
| `/settings` | Sensor & connection settings (HTMX live form) |
| `/view/raw` | Live raw X/Y/Z waveform chart |
| `/view/metrics` | Live metrics chart (RMS, peak, etc.) |
| `/view/latest-raw` | Single-shot latest X/Y/Z reading |
| `/view/all-metrics` | Full metrics snapshot |
| `/view/health` | Server health |
| `/view/diagnostics` | System diagnostics |
| `/view/csv` | CSV file browser & interactive chart |
| `/view/csv/{filename}` | View a specific recorded CSV file |

## API Reference

### Configuration Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/config` | Get current sensor configuration |
| PUT | `/config/sample-rate` | Set sample rate (sps) |
| PUT | `/config/baud-rate` | Set baud rate (requires power cycle) |
| PUT | `/config/high-pass-filter` | Enable/disable high-pass filter |
| PUT | `/config/stream-size` | Set bulk transfer size (1–123 registers) |

### Data Reading Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/read/temperature` | Read sensor temperature (°C) |
| GET | `/read/ucid` | Read device model, gain, serial number |
| GET | `/read/firmware-version` | Read firmware version |
| GET | `/read/chip-id` | Read chip ID |
| GET | `/read/fifo-buffer-size` | Read FIFO buffer fill level |
| GET | `/read/latest-raw` | Read latest X, Y, Z raw values |
| GET | `/read/gravity/rms` | Gravity RMS (g) |
| GET | `/read/gravity/peak` | Gravity peak (g) |
| GET | `/read/gravity/crest-factor` | Gravity crest factor |
| GET | `/read/gravity/skewness` | Gravity skewness |
| GET | `/read/gravity/kurtosis` | Gravity kurtosis |
| GET | `/read/gravity/primary-frequency` | Gravity primary frequency (Hz) |
| GET | `/read/velocity/rms` | Velocity RMS (mm/s) |
| GET | `/read/velocity/peak` | Velocity peak (mm/s) |
| GET | `/read/velocity/crest-factor` | Velocity crest factor |
| GET | `/read/velocity/primary-frequency` | Velocity primary frequency (Hz) |
| GET | `/read/all-metrics` | All gravity + velocity metrics in one response |

### Streaming Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| WS | `/stream/raw` | WebSocket — continuous raw data (downsampled) |
| WS | `/stream/metrics` | WebSocket — live metrics (~1 Hz) |
| POST | `/stream/start` | Start streaming mode (validates baud rate) |
| POST | `/stream/stop` | Stop streaming mode |
| GET | `/stream/status` | Get streaming status |

### Settings Endpoints (HTMX)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/settings` | Settings page |
| POST | `/settings/apply` | Apply and save settings |
| POST | `/settings/test` | Test connection with given settings |
| GET | `/settings/status` | Connection status fragment (for polling) |
| POST | `/settings/reset` | Reset settings to defaults |
| GET | `/settings/ports` | List available serial ports |
| POST | `/settings/validate` | Live field validation |

### Recording Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/record/start` | Start a 10-second burst recording to CSV |
| GET | `/api/record/status` | Get recording progress/status |

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
  -d '{"sampleRate": 7812}'
```

### Read Temperature

```bash
curl http://localhost:3000/read/temperature
```

### Read All Metrics

```bash
curl http://localhost:3000/read/all-metrics
```

### WebSocket Streaming (JavaScript)

```javascript
// Connect to raw data stream (downsampled — one representative sample per packet)
const rawSocket = new WebSocket('ws://localhost:3000/stream/raw');
rawSocket.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Raw data:', data);
};

// Connect to metrics stream (shared background reader, ~1 Hz)
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

Command-line `--device` and `--baud-rate` override the values in `config.toml`.

## Performance Notes

### Baud Rate Requirements

- **115200 bps**: Suitable for periodic metrics reading (~1 Hz) and single-shot reads
- **3 Mbps**: Required for continuous raw data streaming (up to 7812 sps)

### Raw Data Streaming

The raw WebSocket stream uses a pipeline read strategy (reads FIFO size + data in one Modbus round trip). To reduce WebSocket payload and frontend memory the stream is **downsampled**: only the middle sample of each Modbus packet is forwarded to clients (~41× reduction). Full-resolution data can be captured via the CSV recorder.

### Metrics Architecture

- A single shared background task reads all metrics once per second and broadcasts to all subscribed WebSocket clients.
- Skewness and kurtosis update slowly on the sensor (~every 5 s per datasheet); they are read on a separate slow-path task and cached, so the fast-path never stalls.

### Limits

- **Raw data**: Up to 7812 samples/second at 3 Mbps
- **Metrics**: ~1 Hz update rate
- **WebSocket connections**: Up to 10 concurrent clients (configurable)

## Data Formats

### Raw Data WebSocket Message

```json
{
  "type": "raw",
  "timestamp": "2026-03-04T10:30:00.123Z",
  "sequence": 12345,
  "data": [
    {"x": 0.125, "y": -0.089, "z": 1.001}
  ]
}
```

> Each message contains one downsampled representative sample from the FIFO batch.

### Metrics WebSocket Message

```json
{
  "type": "metrics",
  "timestamp": "2026-03-04T10:30:00Z",
  "gravity": {
    "rms": {"x": 0.125, "y": 0.089, "z": 1.001},
    "peak": {"x": 0.245, "y": 0.189, "z": 1.201},
    "crestFactor": {"x": 1.96, "y": 2.12, "z": 1.20},
    "skewness": {"x": 0.01, "y": -0.02, "z": 0.03},
    "kurtosis": {"x": 3.0, "y": 2.9, "z": 3.1},
    "primaryFrequency": 120.5
  },
  "velocity": {
    "rms": {"x": 2.5, "y": 1.8, "z": 0.5},
    "peak": {"x": 4.2, "y": 3.1, "z": 0.9},
    "crestFactor": {"x": 1.68, "y": 1.72, "z": 1.80},
    "primaryFrequency": 118.2
  },
  "temperature": 25.5
}
```

### Error Response

```json
{
  "error": "Failed to read temperature: Device not responding",
  "code": "TEMPERATURE_READ_ERROR",
  "timestamp": "2026-03-04T10:30:00Z"
}
```

When the sensor is not connected all `/read/*` and `/config/*` endpoints return HTTP 503 with `"code": "DEVICE_NOT_CONNECTED"`.

## CSV Recording

POST `/api/record/start` captures ~10 seconds of raw data (78,120 samples at 7812 Hz) to a timestamped file in the `data/` directory (e.g. `data/record_20260304_103000.csv`). Poll `/api/record/status` for progress. Completed files are immediately available in the CSV viewer at `/view/csv`.

## Project Structure

```
src/
├── main.rs              # Entry point, routing, startup diagnostics, recording, CSV viewer
├── config.rs            # Configuration loading/saving (config.toml)
├── modbus.rs            # Modbus RTU client (all sensor reads/writes)
├── types.rs             # Shared data types, register addresses, settings forms
└── routes/
    ├── mod.rs           # Route module declarations
    ├── config.rs        # /config/* endpoints
    ├── read.rs          # /read/* endpoints
    ├── stream.rs        # /stream/* WebSocket + REST endpoints
    ├── settings.rs      # /settings/* HTMX settings page
    ├── view.rs          # /view/* HTML page endpoints
    └── diagnostics.rs   # /diagnostics endpoint
templates/               # Minijinja HTML templates (auto-reloaded on change)
├── base.html
├── dashboard.html
├── settings.html
├── view_raw.html
├── view_metrics.html
├── view_latest_raw.html
├── view_all_metrics.html
├── view_health.html
├── view_diagnostics.html
├── view_csv.html
└── settings/
    ├── form.html
    ├── feedback.html
    └── status-header.html
data/                    # Recorded CSV files (served at /data/*)
static/                  # Static assets (served at /static/*)
config.toml              # Runtime configuration (auto-created on first run)
```

## Logging

Configure logging via environment variables:

```bash
RUST_LOG=modbus_stream=debug cargo run
RUST_LOG=debug ./target/release/modbus-stream
```

Log levels: `error`, `warn`, `info`, `debug`, `trace`

## Building for Production

```bash
# Optimised release build
cargo build --release

# Strip debug symbols (Linux)
strip target/release/modbus-stream

# Cross-compile for ARM (e.g. Raspberry Pi)
cargo build --target armv7-unknown-linux-gnueabihf --release
```

## Troubleshooting

The server prints a 4-step startup diagnostic before listening:

```
════════════════════════════════════════════════════════════
  Modbus Device Startup Diagnostics
════════════════════════════════════════════════════════════
  [1/4] Checking device path ...        OK
  [2/4] Checking permissions ...        OK (read + write)
  [3/4] Opening serial port ...         OK (12 ms)
  [4/4] Testing Modbus communication ... OK (45 ms)
         Model:    KAX301 | Gain: 4G
         Serial:   12345
         Temp:     24.3°C
         Firmware: 1.2.0

  Result: CONNECTED
```

If it fails at any step the server still starts in **offline mode** — the UI and API are available but all sensor reads return HTTP 503.

### Common Issues

1. **Permission Denied on Serial Port**
   ```bash
   sudo usermod -aG dialout $USER
   # Log out and back in (or: newgrp dialout)
   # Or temporarily: sudo chmod a+rw /dev/ttyUSB0
   ```

2. **Modbus Timeout (step 4 fails, port opens fine)**
   - FT232R is a plain UART with no automatic RS485 direction control. Your adapter must handle DE/RE switching itself.
   - Check that A/B wires are not swapped.
   - Verify slave ID and baud rate match the sensor.
   - Try broadcasting: set `slave_id = 0` in `config.toml`.
   - Add a 120 Ω termination resistor if the cable is long.
   - Test with `mbpoll`: `mbpoll -a 1 -b 3000000 /dev/ttyUSB0 -t 3 -r 20`

3. **WSL / usbipd**
   ```bash
   # Attach USB device to WSL
   usbipd attach --wsl --busid <id>
   # Find the port
   dmesg | grep tty | tail -5
   ```

4. **Streaming Performance Issues**
   - Use 3 Mbps baud rate for raw streaming.
   - Monitor "Client lagging" warnings in the browser console.
   - Check system USB/serial buffer sizes.

## License

MIT OR Apache-2.0
