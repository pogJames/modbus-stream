# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo check                    # Quick type check
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # Run all tests
cargo clippy                   # Linter
cargo fmt                      # Formatter
```

Cross-compile for aarch64 (primary deployment target — NXP i.MX93):
```bash
cargo build --release --target aarch64-unknown-linux-gnu
```

Run a single test:
```bash
cargo test <test_name>
```

The build script `build_and_test.sh` also checks environment dependencies (libudev, serial ports) and config before building.

## Architecture

This is an Axum/Tokio web server that bridges **Modbus RTU accelerometer sensors** (up to 4) to a browser dashboard via REST and WebSocket.

### Request Flow

1. HTTP/WebSocket → Axum router → route handlers in `src/routes/`
2. Route handlers access shared state (`AppState`) containing per-sensor `Arc<RwLock<Option<ModbusClient>>>`
3. Modbus operations go through `src/modbus.rs` over serial (RS485)
4. Background tasks per sensor push metrics to `broadcast::Sender<Metrics>` channels
5. WebSocket handlers subscribe to broadcast channels and stream to clients

### Key Files

| File | Role |
|------|------|
| `src/main.rs` | Entry point, routing, startup diagnostics, CSV recording, background task spawning |
| `src/modbus.rs` | All Modbus RTU operations (FIFO reads, metrics, config) |
| `src/config.rs` | TOML config load/save; sensor + server settings |
| `src/types.rs` | Shared structs: `AccelerationData`, `Metrics`, WebSocket message types |
| `src/tss_ml.rs` | FFI bindings + safe wrapper for the NXP TSS SVM model; `OnceLock` singleton |
| `src/routes/stream.rs` | WebSocket handlers — raw FIFO streaming and metrics broadcast |
| `src/routes/read.rs` | REST endpoints for sensor reads and ML inference (`/csv/infer`) |
| `src/routes/config.rs` | REST endpoints for changing sensor config (sample rate, baud rate, filters) |
| `src/routes/settings.rs` | HTMX settings page with live validation and connection testing |

### Multi-Sensor Design

- Sensors are numbered 1–4; API paths are `/{n}/...`
- Each sensor has its own `Arc<RwLock<Option<ModbusClient>>>` and metrics broadcast channel
- One background task per sensor reads metrics at ~1 Hz
- Server starts in "offline mode" if sensors are not connected

### Streaming Pipeline

- **Raw stream** (`/{n}/stream/raw`): Reads FIFO size from previous cycle, fetches that many samples, downsamples ~41× for WebSocket delivery
- **Metrics stream** (`/{n}/stream/metrics`): Background task broadcasts; WebSocket clients subscribe
- FIFO reads are aligned to multiples of 3 (XYZ triplets) via `fifo_read_count()`

### CSV Recording

Two-phase in `main.rs`: Phase 1 collects samples asynchronously, Phase 2 writes to disk via `spawn_blocking`. Includes stuck detection (5s warning) and 60s timeout. Output format: `x,y,z` header + 78120 rows of g-unit floats.

### ML Inference (`src/tss_ml.rs`)

NXP eIQ Time Series Studio SVM model, accessed via FFI to `lib/libtss_svm.a`.

- `MlModel::new()` calls `tss_get_task_ops()`, then `cls_ops.init()` (reads `algorithm.dat` from the working directory), then `algo_attribute()` to read `data_tab` (0 = interleaved, 1 = channels-first) and `data_len`
- `predict_window(&[(f32,f32,f32)])` flattens samples using the layout declared by `data_tab`, calls `cls_ops.predict()`, returns a 1-based class label (1–4) and probability array
- The entire FFI block is `#[cfg(target_arch = "aarch64")]`-gated; the server builds and runs on x86_64 but the infer endpoint returns an error
- `POST /{n}/csv/infer?file=<name>` in `routes/read.rs` reads the CSV, slices the middle `data_len` samples (`start = (78120 - data_len) / 2`), and calls `predict_window`

### External Libraries (`lib/`)

| File | Purpose |
|------|---------|
| `libtss_svm.a` | NXP TSS SVM classification model (aarch64, linked via `build.rs`) |
| `libtss.a` | NXP TSS stats library (present but not currently linked) |
| `TimeSeries.h` | C API header shared by all TSS libraries |
| `algorithm.dat` | Model weights — must be in the working directory at runtime |

### Build Notes

- `build.rs` links `libtss_svm` and `m` for aarch64 targets only
- `.cargo/config.toml` sets `tokio_unstable` globally; sets `linker`, `target-cpu=cortex-a55`, and `target-feature=+neon` for the aarch64 target
- Release profile uses LTO and aggressive optimization
- Templates (Minijinja) auto-reload in debug builds via `minijinja-autoreload`
