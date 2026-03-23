# Modbus Stream — Tri-axial Accelerometer Web Interface

A high-performance Rust web server providing REST API, WebSocket streaming, browser-based UI, and on-device ML inference for tri-axial accelerometer vibration sensors via Modbus RTU.

## Features

- **Multi-sensor support** — up to 4 independent sensors, each on its own serial port
- **Browser-based dashboard** with live charts and per-sensor tab switching
- **Settings page** with live validation, connection testing, and hot-reload (HTMX)
- **Complete REST API** for sensor configuration and data reading
- **Real-time WebSocket streaming** for continuous raw data and metrics
- **Modbus RTU communication** with full register support
- **High-performance streaming** at up to 3 Mbps baud rate (7812 sps)
- **Comprehensive metrics** — RMS, Peak, Crest Factor, Skewness, Kurtosis, Primary Frequency
- **CSV recording** — capture 10-second bursts (~78k samples) to disk
- **CSV viewer** — interactive browser chart for recorded files
- **ML inference** — SVM classifier (NXP eIQ Time Series Studio) runs on recorded CSV files directly in the browser; returns class label 1–4 with per-class probabilities
- **Offline mode** — server starts and serves UI even when no sensor is connected
- **FTDI latency optimisation** — automatically lowers USB serial latency timer to 1 ms
- **Startup diagnostics** — 4-step connection test printed per sensor before the server starts
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
- Linux (primary target), Windows, or macOS

### Installation

```bash
git clone <repository>
cd modbus-stream
cargo build --release

./target/release/modbus-stream --device /dev/ttyUSB0 --baud-rate 3000000
```

Open `http://localhost:3000` in a browser.

### Configuration

The server reads `config.toml` on startup (created with defaults if missing). Each `[[sensors]]` block defines one sensor.

```toml
[server]
host = "0.0.0.0"
port = 3000
cors_origins = ["*"]

[[sensors]]
device = "/dev/ttyUSB0"   # use "COM3" on Windows
baud_rate = 3000000       # 115200 for metrics-only; 3000000 for raw streaming
slave_id = 1
timeout_ms = 5000
retry_attempts = 3

[[sensors]]
device = "/dev/ttyUSB1"   # sensor 2
baud_rate = 3000000
slave_id = 1
timeout_ms = 5000
retry_attempts = 3

# Add up to 4 [[sensors]] blocks total

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

Sensors are numbered by their order in the file. URL paths follow the same numbering: `/1/view/raw`, `/2/stream/metrics`, etc.

## Web UI

| URL | Description |
|-----|-------------|
| `/` | Dashboard — navigation hub |
| `/settings` | Sensor & connection settings (HTMX live form) |
| `/{n}/view/raw` | Live raw X/Y/Z waveform chart for sensor n |
| `/{n}/view/metrics` | Live metrics chart (RMS, peak, etc.) for sensor n |
| `/{n}/view/all-metrics` | Full metrics snapshot for sensor n |
| `/view/latest-raw` | Single-shot latest X/Y/Z reading (all sensors) |
| `/view/diagnostics` | System diagnostics |
| `/view/csv` | CSV file browser, interactive chart, and ML inference |
| `/view/csv/{filename}` | View a specific recorded CSV file |

`/view/raw`, `/view/metrics`, `/view/all-metrics` redirect to `/1/view/...` automatically.

## API Reference

### Configuration Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/config` | Get current sensor 1 configuration |
| PUT | `/config/sample-rate` | Set sample rate (sps) |
| PUT | `/config/baud-rate` | Set baud rate (requires power cycle) |
| PUT | `/config/high-pass-filter` | Enable/disable high-pass filter |
| PUT | `/config/stream-size` | Set bulk transfer size (1–123 registers) |

### Data Reading Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/{n}/read/temperature` | Read sensor temperature (°C) |
| GET | `/{n}/read/ucid` | Read device model, gain, serial number |
| GET | `/{n}/read/firmware-version` | Read firmware version |
| GET | `/{n}/read/chip-id` | Read chip ID |
| GET | `/{n}/read/fifo-buffer-size` | Read FIFO buffer fill level |
| GET | `/{n}/read/latest-raw` | Read latest X, Y, Z raw values |
| GET | `/{n}/read/gravity/rms` | Gravity RMS (g) |
| GET | `/{n}/read/gravity/peak` | Gravity peak (g) |
| GET | `/{n}/read/gravity/crest-factor` | Gravity crest factor |
| GET | `/{n}/read/gravity/skewness` | Gravity skewness |
| GET | `/{n}/read/gravity/kurtosis` | Gravity kurtosis |
| GET | `/{n}/read/gravity/primary-frequency` | Gravity primary frequency (Hz) |
| GET | `/{n}/read/velocity/rms` | Velocity RMS (mm/s) |
| GET | `/{n}/read/velocity/peak` | Velocity peak (mm/s) |
| GET | `/{n}/read/velocity/crest-factor` | Velocity crest factor |
| GET | `/{n}/read/velocity/primary-frequency` | Velocity primary frequency (Hz) |
| GET | `/{n}/read/all-metrics` | All gravity + velocity metrics in one response |
| GET | `/read/latest-raw` | Latest X/Y/Z for all sensors combined |

### Streaming Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| WS | `/{n}/stream/raw` | WebSocket — continuous raw data (downsampled ~41×) |
| WS | `/{n}/stream/metrics` | WebSocket — live metrics (~1 Hz) |
| POST | `/{n}/stream/start` | Start streaming mode |
| POST | `/{n}/stream/stop` | Stop streaming mode |
| GET | `/{n}/stream/status` | Get streaming status |

### Recording Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/record/start` | Start a 10-second burst recording to CSV |
| GET | `/api/record/status` | Get recording progress/status |

### ML Inference Endpoint

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/{n}/csv/infer?file=<filename>` | Run SVM classifier on a recorded CSV file |

**Request:** `POST /1/csv/infer?file=record_20260316_093913.csv`

**Response:**
```json
{
  "class": 2,
  "probabilities": [0.02, 0.94, 0.03, 0.01]
}
```

The endpoint slices the middle 1953 samples from the 78120-sample recording and runs the NXP eIQ TSS SVM model. The data layout (interleaved vs channels-first) is determined at runtime from the model's `algo_attribute`. Requires `algorithm.dat` to be present in the server's working directory.

### Settings Endpoints (HTMX)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/settings` | Settings page |
| POST | `/settings/apply` | Apply and save settings |
| POST | `/settings/test` | Test connection with given settings |
| GET | `/settings/status` | Connection status cards (for polling) |
| POST | `/settings/reset` | Reset settings to defaults |
| GET | `/settings/ports` | List available serial ports |
| POST | `/settings/validate` | Live field validation |

### Utility Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/health` | API health check |
| GET | `/diagnostics` | System diagnostics (all sensors) |

## CSV Recording

`POST /api/record/start` captures ~10 seconds of raw data (78,120 samples at 7812 Hz) from all connected sensors to timestamped files in `data/` (e.g. `data/record_20260304_103000_sensor1.csv`). Poll `/api/record/status` for progress. Completed files are immediately available in the CSV viewer at `/view/csv`.

CSV format: `x,y,z` header row followed by 78120 rows of g-unit float values.

## ML Inference

The CSV viewer at `/view/csv` shows an **Infer** button next to each file. Clicking it calls `POST /1/csv/infer?file=<name>` and displays the result as a **Class N** badge. Hovering the badge shows the raw per-class probabilities.

The model is an NXP eIQ Time Series Studio SVM classifier compiled for aarch64 (NXP i.MX93 / cortex-a55). It classifies a 1953-sample window extracted from the centre of the recording into one of 4 classes.

**Runtime requirement:** `algorithm.dat` must be present in the directory from which the server is run.

## Cross-Compilation (aarch64 / NXP i.MX93)

```bash
# Install cross-linker (Debian/Ubuntu)
sudo apt install gcc-aarch64-linux-gnu

# Build
cargo build --release --target aarch64-unknown-linux-gnu

# Copy to device
scp target/aarch64-unknown-linux-gnu/release/modbus-stream user@device:/opt/modbus-stream/
scp -r templates static config.toml lib/algorithm.dat user@device:/opt/modbus-stream/
```

Run on the device from the directory containing `templates/`, `static/`, `config.toml`, and `algorithm.dat`:

```bash
cd /opt/modbus-stream && ./modbus-stream
```

The `.cargo/config.toml` sets `linker = "aarch64-linux-gnu-gcc"`, `target-cpu=cortex-a55`, and `target-feature=+neon` for the aarch64 target automatically.

## Project Structure

```
src/
├── main.rs              # Entry point, routing, startup diagnostics, recording, CSV viewer
├── config.rs            # Configuration loading/saving (config.toml)
├── modbus.rs            # Modbus RTU client (all sensor reads/writes)
├── types.rs             # Shared data types, register addresses, settings forms
├── tss_ml.rs            # NXP TSS SVM FFI bindings + safe wrapper + OnceLock singleton
└── routes/
    ├── mod.rs           # Route module declarations
    ├── config.rs        # /config/* endpoints
    ├── read.rs          # /{n}/read/* and /{n}/csv/infer endpoints
    ├── stream.rs        # /{n}/stream/* WebSocket + REST endpoints
    ├── settings.rs      # /settings/* HTMX settings page
    ├── view.rs          # /view/* HTML page endpoints
    └── diagnostics.rs   # /diagnostics endpoint
lib/
├── libtss_svm.a         # NXP TSS SVM model (aarch64 static library)
├── libtss.a             # NXP TSS stats library (present, not currently linked)
├── TimeSeries.h         # C API header
└── algorithm.dat        # Model weights (must be in working dir at runtime)
templates/               # Minijinja HTML templates (auto-reloaded in debug builds)
data/                    # Recorded CSV files (served at /data/*)
static/                  # Static assets (served at /static/*)
build.rs                 # Links libtss_svm + libm for aarch64 targets
```

## Performance Notes

- **115200 bps**: Periodic metrics reading (~1 Hz) and single-shot reads
- **3 Mbps**: Required for continuous raw data streaming (7812 sps)
- Raw WebSocket stream is downsampled ~41×; use CSV recording for full-resolution data
- One background task per sensor reads metrics; broadcasts to all subscribed WebSocket clients
- ML inference runs in `spawn_blocking` — does not block the async runtime

## Logging

```bash
RUST_LOG=modbus_stream=debug cargo run
RUST_LOG=debug ./target/release/modbus-stream
```

## Troubleshooting

The server prints a 4-step startup diagnostic per sensor before listening:

```
════════════════════════════════════════════════════════════
  Modbus Device Startup Diagnostics
════════════════════════════════════════════════════════════
  [1/4] Checking device path ...        OK
  [2/4] Checking permissions ...        OK (read + write)
  [3/4] Opening serial port ...         OK (12 ms)
  [4/4] Testing Modbus communication ... OK (45 ms)
         Model:    KAX301 | Gain: 4G
  Result: CONNECTED
```

If a sensor fails at any step the server starts in **offline mode** for that sensor.

### Common Issues

**Permission denied on serial port:**
```bash
sudo usermod -aG dialout $USER
# Log out and back in, or: newgrp dialout
```

**Modbus timeout (port opens, step 4 fails):**
- FT232R has no automatic RS485 direction control — your adapter must handle DE/RE switching
- Verify slave ID and baud rate match the sensor
- Check A/B wires are not swapped; add 120 Ω termination if cable is long
- Test with `mbpoll -a 1 -b 3000000 /dev/ttyUSB0 -t 3 -r 20`

**ML inference returns error:**
- Check `algorithm.dat` is in the server's working directory
- Check server startup log for `TSS ML model ready: data_tab=…` — if absent, init failed
- Only available when running on aarch64; returns an error on x86_64

**WSL / usbipd:**
```bash
usbipd attach --wsl --busid <id>
dmesg | grep tty | tail -5
```

**Config not loading after upgrade:** The config format changed from `[modbus1]`/`[modbus2]` to `[[sensors]]` array syntax. Update `config.toml` accordingly.

## License

MIT OR Apache-2.0
