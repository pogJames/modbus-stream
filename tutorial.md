                                   
  ---                                                                                               
  Architecture Overview                                                                             
                                                                                                    
  Physical Sensor (RS485)                                                                           
          │                                                                                       
          │ Serial cable (USB-RS485 adapter → /dev/ttyUSB0)
          ▼
  ┌─────────────────────────────────────────────────────┐
  │  modbus-stream (Rust/Axum process)                  │
  │                                                     │
  │  main.rs ─── startup diagnostics, AppState, routes  │
  │     │                                               │
  │     ├── config.rs       (config.toml ↔ structs)     │
  │     ├── modbus.rs       (all hardware I/O)          │
  │     ├── types.rs        (all shared data types)     │
  │     └── routes/                                     │
  │           ├── config.rs    PUT /config/*            │
  │           ├── read.rs      GET /read/*              │
  │           ├── stream.rs    WS  /stream/*            │
  │           ├── settings.rs  GET/POST /settings       │
  │           ├── diagnostics.rs GET /diagnostics       │
  │           └── view.rs      GET /view/*              │
  │                                                     │
  │  templates/  (minijinja HTML, hot-reload)           │
  │  static/     (CSS, JS assets)                       │
  └─────────────────────────────────────────────────────┘
          │
          │ HTTP / WebSocket
          ▼
     Browser (dashboard, charts, settings UI)

  ---
  Layer by Layer

  1. Startup (main.rs)

  Before the web server binds, three things happen in order:

  a) FTDI latency fix (set_ftdi_latency)
  The FT232R USB-serial chip has a 16ms receive buffer timer by default, meaning every Modbus
  response gets an extra 16ms delay. This writes "1" to the kernel sysfs file to lower it to 1ms.
  This is non-fatal — if it fails (e.g. no permission), it just prints a hint.

  b) Startup diagnostics (run_startup_diagnostics)
  Runs 4 numbered steps before the server starts:
  1. Does /dev/ttyUSB0 exist?
  2. Can we open it (read+write permissions)?
  3. Can we open the serial port as a Modbus RTU connection?
  4. Does the sensor actually reply? — wrapped in tokio::time::timeout because async Modbus doesn't
  respect the serial port .timeout() setting.

  If any step fails, the server starts anyway in "offline mode" — modbus_client is None instead of
  Some(client).

  c) AppState creation
  pub struct AppState {
      modbus_client: Arc<RwLock<Option<ModbusClient>>>,  // None = offline
      config:        Arc<AppConfig>,                      // read-only after startup
      config_path:   String,                              // path to config.toml
      template_env:  Arc<AutoReloader>,                   // minijinja with hot-reload
  }
  This is cloned cheaply (via Arc) into every request handler. The RwLock allows many readers (route
   handlers) but exclusive write access when reconnecting.

  ---
  2. Configuration (config.rs)

  config.toml is parsed at startup into AppConfig, which has four nested structs:

  AppConfig
  ├── ServerConfig    host, port, cors_origins
  ├── ModbusConfig    device, baud_rate, slave_id, timeout_ms, retry_attempts
  ├── StreamingConfig max_connections, buffer_size, metrics_update_rate_hz, ...
  └── LoggingConfig   level, format

  AppConfig::load() creates a default file if it doesn't exist. AppConfig::save() serialises back to
   TOML. This is how settings changes persist to disk. The config_path string in AppState is what
  settings.rs would use to call config.save().

  ---
  3. Hardware Layer (modbus.rs)

  This is the only file that touches the physical sensor.

  ModbusClient struct:
  struct ModbusClient {
      context:      Mutex<tokio_modbus::client::Context>,  // the serial line
      slave_id:     u8,
      device_path:  String,
      baud_rate:    u32,
      scale_factor: Mutex<Option<f64>>,  // cached to avoid re-reading UCID
  }

  Three primitive Modbus operations (private):
  - read_holding_registers(addr, count) → FC03 — for config registers (RMS, peak, UCID, temperature)
  - read_input_registers(addr, count) → FC04 — for live data registers (FIFO buffer, latest XYZ)
  - write_single_register(addr, value) → FC06 — for setting sample rate, baud rate, etc.

  Everything else is built on top of these three.

  Scale factor caching (get_scale_factor):
  The sensor's gain setting (2G, 4G, 8G...) determines how to convert raw i16 integers to physical g
   values. This is stored in the UCID register. Rather than reading it on every sample, it's read
  once and cached in scale_factor: Mutex<Option<f64>>. Cache is invalidated if you change the sensor
   config.

  Key register map (from types.rs::registers):

  ┌───────────────────────┬───────────────┬────────────────────────────────────┐
  │       Register        │    Address    │              Function              │
  ├───────────────────────┼───────────────┼────────────────────────────────────┤
  │ SAMPLE_RATE           │ 0x0001        │ How fast sensor samples (Hz)       │
  ├───────────────────────┼───────────────┼────────────────────────────────────┤
  │ FIFO_BUFFER_SIZE      │ 0x0002        │ How many samples are waiting       │
  ├───────────────────────┼───────────────┼────────────────────────────────────┤
  │ RAW_DATA_START        │ 0x0003        │ Start of FIFO data block           │
  ├───────────────────────┼───────────────┼────────────────────────────────────┤
  │ TEMPERATURE           │ 0x0014        │ Temp in units of 0.01°C            │
  ├───────────────────────┼───────────────┼────────────────────────────────────┤
  │ UCID                  │ 0x001B        │ Model + gain + serial (packed u32) │
  ├───────────────────────┼───────────────┼────────────────────────────────────┤
  │ FIRMWARE_VERSION      │ 0x001D        │ Not supported on all sensors       │
  ├───────────────────────┼───────────────┼────────────────────────────────────┤
  │ GRAVITY_RMS           │ 0x001E        │ Pre-computed RMS X/Y/Z             │
  ├───────────────────────┼───────────────┼────────────────────────────────────┤
  │ GRAVITY_PEAK–KURTOSIS │ 0x001F–0x0022 │ Pre-computed stats                 │
  ├───────────────────────┼───────────────┼────────────────────────────────────┤
  │ VELOCITY_RMS          │ 0x0032        │ Pre-computed velocity RMS          │
  ├───────────────────────┼───────────────┼────────────────────────────────────┤
  │ RAW_DATA_LATEST_X     │ 0x0083        │ Most recent single sample          │
  └───────────────────────┴───────────────┴────────────────────────────────────┘

  The sensor does a lot of signal processing internally — it computes RMS, peak, crest factor,
  skewness, kurtosis, and primary frequency itself. You just read the result registers.

  ---
  4. Data Types (types.rs)

  All the shared Rust structs and enums live here. Three categories:

  Wire types (serialized to/from JSON for HTTP responses):
  - AccelerationData — {x, y, z} in g's
  - GravityMetrics — RMS, peak, crestFactor, skewness, kurtosis, primaryFrequency
  - VelocityMetrics — RMS, peak, crestFactor, primaryFrequency
  - UcidInfo — model, gain, serialNumber (decoded from packed 32-bit UCID register)

  WebSocket envelope (WebSocketMessage enum with #[serde(tag="type")]):
  enum WebSocketMessage {
      RawData  { timestamp, sequence, data: Vec<AccelerationData> }  // type="raw"
      Metrics  { timestamp, gravity, velocity, temperature }          // type="metrics"
      Error    { message, code }                                       // type="error"
      Status   { connected, streaming }                               // type="status"
  }
  The #[serde(tag="type")] means every JSON message has a "type" field the browser uses to dispatch.

  Form/UI types — SettingsForm, ValidationErrors, Feedback — used by the settings page.

  UcidInfo::from_raw(u32) decodes the packed UCID register: bits 31-28 = model, bits 27-24 = gain,
  bits 23-0 = serial number.

  ---
  5. Route Handlers

  routes/read.rs — One-shot REST reads

  Simple GET handlers that call ModbusClient, return JSON. Used for:
  - /read/temperature, /read/ucid, /read/firmware-version, /read/chip-id
  - /read/gravity/rms, /read/gravity/peak, etc.
  - /read/all-metrics — reads everything in one call
  - /read/latest-raw — reads the most recent single XYZ sample

  All follow the same pattern: acquire the RwLock, check Option<ModbusClient>, call the client,
  return Json(result) or a 503 error.

  routes/config.rs — Sensor configuration writes

  PUT handlers to change sensor settings in real time:
  - /config/sample-rate — writes to register 0x0001
  - /config/baud-rate — writes to registers 0x0017 + 0x0018 (split 32-bit)
  - /config/high-pass-filter — writes 1 or 0 to register 0x001C
  - /config/stream-size — writes to register 0x0015

  routes/stream.rs — WebSocket streaming (the most complex part)

  When a browser opens /stream/raw or /stream/metrics, this is what happens:

  Browser connects to /stream/raw
          │
          ▼
  websocket_raw_handler()
    → Axum upgrades HTTP → WebSocket
          │
          ▼
  handle_raw_websocket(socket, state)
    1. Sends initial {"type":"status","connected":true,"streaming":false}
    2. Creates a broadcast::channel(1000)  ← this is the fan-out bus
    3. Spawns a background task that loops:
         every 10ms → read_raw_data_batch() → tx.send(WebSocketMessage)
    4. Main loop (tokio::select!):
         - receives from rx → serializes JSON → sends to socket
         - receives from socket → handles ping/close
    5. On disconnect → read_task.abort()

  Key insight: each WebSocket connection spawns its own sensor polling loop. There's no shared
  background reader — if two browsers open /stream/raw, the sensor gets polled twice simultaneously.
   This works but means two Modbus reads are competing for the same serial port lock.

  The broadcast::channel with capacity 1000 (raw) or 100 (metrics) is a ring buffer. If the browser
  can't keep up, it gets a Lagged error and some frames are dropped.

  read_raw_data_batch() reads the FIFO buffer:
  1. FC04 to FIFO_BUFFER_SIZE (0x0002) — how many samples are ready
  2. FC04 to RAW_DATA_START (0x0003) for min(buffer_size, 123) registers
  3. Converts each group of 3 registers (X, Y, Z) to AccelerationData using the cached scale factor

  routes/settings.rs — Settings web page

  Handles the HTML settings form (HTMX-powered):
  - GET /settings — renders the settings page template
  - POST /settings/apply — applies new settings to the sensor + saves config
  - POST /settings/test — tests connection with current or new settings
  - GET /settings/status — returns connection status HTML fragment (polled every 5s by HTMX)
  - POST /settings/reset — resets to defaults
  - GET /settings/ports — lists available serial ports

  routes/diagnostics.rs — System health

  GET /diagnostics returns a JSON report with connection status, config info, and sensor readings if
   connected.

  routes/view.rs — Chart pages

  Two simple handlers that just render templates:
  - GET /view/raw → view_raw.html
  - GET /view/metrics → view_metrics.html

  ---
  6. Templates (Jinja2-like via minijinja)

  All extend base.html which provides the page shell, CSS variables, and HTMX script tag.

  - dashboard.html — landing page, links to all sections, HTMX status widget
  - settings.html — full settings form with HTMX live validation
  - view_raw.html — Chart.js rolling waveform, connects to /stream/raw, buffers in JS, renders at
  30fps
  - view_metrics.html — metrics grid + RMS trend, connects to /stream/metrics

  The AutoReloader means you can edit a template file and refresh the browser without restarting the
   server — useful during development.

  ---
  Data Flow Summary

  Sensor ADC (7812 sps) → internal FIFO buffer
                                  │
                every 10ms: Modbus FC04 → FIFO_BUFFER_SIZE
                                  │
                FC04 → RAW_DATA_START (up to 123 registers = 41 XYZ samples)
                                  │
                           raw i16 × scale_factor → AccelerationData
                                  │
                      broadcast::channel → WebSocket → Browser
                                  │
                           Chart.js rolling array (500 pts)
                           setInterval 33ms → chart.update('none')

  For metrics, the sensor computes everything internally. The Rust server just polls the result
  registers at 5 Hz (every 200ms) and forwards them via WebSocket.