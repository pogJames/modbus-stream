use anyhow::Result;
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Redirect, Response},
    routing::{get, post, put, get_service},
    Router,
};
use clap::Parser;
use minijinja::{context, Environment};
use minijinja_autoreload::AutoReloader;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::Path, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use std::time::Duration;
use tokio::time::{Instant, timeout};
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::{error, info};

mod config;
mod modbus;
mod routes;
mod types;

use config::AppConfig;
use modbus::ModbusClient;

/// Modbus Stream Server - Web interface for tri-axial accelerometer
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Server bind address
    #[arg(short, long, default_value = "0.0.0.0:3000")]
    bind: String,

    /// Modbus device path (e.g., /dev/ttyUSB0 or COM3)
    #[arg(short, long)]
    device: Option<String>,

    /// Modbus baud rate
    #[arg(short = 'r', long, default_value = "115200")]
    baud_rate: u32,

    /// Modbus slave ID
    #[arg(short = 's', long, default_value = "1")]
    slave_id: u8,
}

#[derive(Clone, Serialize)]
struct RecordingState {
    active: bool,
    samples: u64,
    total: u64,
    filename: Option<String>,
    error: Option<String>,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self { active: false, samples: 0, total: 78120, filename: None, error: None }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub modbus_clients: Vec<Arc<tokio::sync::RwLock<Option<ModbusClient>>>>,
    config: Arc<AppConfig>,
    config_path: String,
    template_env: Arc<AutoReloader>,
    /// Per-sensor metrics broadcasters — one background reader per sensor, all WS clients subscribe.
    pub metrics_txs: Vec<broadcast::Sender<types::WebSocketMessage>>,
    recording: Arc<tokio::sync::Mutex<RecordingState>>,
}

impl AppState {
    pub fn sensor_count(&self) -> usize {
        self.modbus_clients.len()
    }
}

/// Lower the FTDI USB serial latency timer from 16ms → 1ms before opening the port.
/// Without this, every Modbus transaction has a 16ms extra delay.
fn set_ftdi_latency(device: &str) {
    if let Some(name) = device.strip_prefix("/dev/") {
        let path = format!("/sys/bus/usb-serial/devices/{}/latency_timer", name);
        match std::fs::write(&path, "1") {
            Ok(_)  => info!("FTDI latency timer → 1 ms"),
            Err(e) => println!("  Note: could not set FTDI latency ({}) — try: echo 1 | sudo tee {}", e, path),
        }
    }
}

/// Runs pre-startup device diagnostics and returns a ModbusClient if connection succeeds.
/// Prints a clear step-by-step report to stdout before the server starts.
async fn run_startup_diagnostics(
    device_path: &str,
    baud_rate: u32,
    slave_id: u8,
    timeout_ms: u64,
) -> Option<ModbusClient> {
    let border = "═".repeat(60);
    println!("\n{}", border);
    println!("  Modbus Device Startup Diagnostics");
    println!("{}", border);
    println!("  Device:    {}", device_path);
    println!("  Baud rate: {} bps", baud_rate);
    println!("  Slave ID:  {}", slave_id);
    println!("  Timeout:   {} ms", timeout_ms);
    println!();

    // Step 1: Check device path exists
    print!("  [1/4] Checking device path ...        ");
    if !Path::new(device_path).exists() {
        println!("FAIL");
        println!("         ✗ {} not found", device_path);
        println!("         → Check USB is connected:  ls /dev/ttyUSB* /dev/ttyACM*");
        println!("         → If using WSL/usbipd:     usbipd attach --wsl --busid <id>");
        println!("         → Then check which port:   dmesg | grep tty | tail -5");
        println!("\n  Result: NOT CONNECTED — server starting in offline mode");
        println!("{}\n", border);
        return None;
    }
    println!("OK");

    // Step 2: Check read/write permissions
    print!("  [2/4] Checking permissions ...        ");
    match std::fs::OpenOptions::new().read(true).write(true).open(device_path) {
        Ok(_) => println!("OK (read + write)"),
        Err(e) => {
            println!("FAIL");
            println!("         ✗ Cannot open {}: {}", device_path, e);
            println!("         → Add yourself to dialout group:");
            println!("             sudo usermod -aG dialout $USER");
            println!("           Then log out and back in (or: newgrp dialout)");
            println!("         → Or temporarily:  sudo chmod a+rw {}", device_path);
            println!("\n  Result: PERMISSION DENIED — server starting in offline mode");
            println!("{}\n", border);
            return None;
        }
    }

    // Step 3: Open serial port
    print!("  [3/4] Opening serial port ...         ");
    let t = Instant::now();
    let client = match ModbusClient::new(device_path, baud_rate, slave_id).await {
        Ok(c) => {
            println!("OK ({} ms)", t.elapsed().as_millis());
            c
        }
        Err(e) => {
            println!("FAIL ({} ms)", t.elapsed().as_millis());
            println!("         ✗ {}", e);
            println!("         → Port may be in use by another process");
            println!("         → Check: fuser {}", device_path);
            println!("\n  Result: PORT ERROR — server starting in offline mode");
            println!("{}\n", border);
            return None;
        }
    };

    // Step 4: Test actual Modbus communication
    print!("  [4/4] Testing Modbus communication ... ");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let t = Instant::now();
    let test_result = timeout(
        Duration::from_millis(timeout_ms),
        client.test_connection(),
    ).await;
    let elapsed = t.elapsed().as_millis();

    match test_result {
        Ok(Ok(())) => {
            println!("OK ({} ms)", elapsed);

            // Read extra device info to confirm it's the right sensor
            if let Ok(ucid) = client.read_ucid().await {
                println!("         Model:    {} | Gain: {}", ucid.model, ucid.gain);
                println!("         Serial:   {}", ucid.serial_number);
            }
            if let Ok(temp) = client.read_temperature().await {
                println!("         Temp:     {:.1}°C", temp);
            }
            if let Ok(fw) = client.read_firmware_version().await {
                println!("         Firmware: {}", fw);
            }

            println!("\n  Result: CONNECTED");
            println!("{}\n", border);
            Some(client)
        }
        Ok(Err(e)) => {
            println!("FAIL ({} ms)", elapsed);
            println!("         ✗ {}", e);
            println!("         → Device found but not responding to Modbus queries");
            println!("         → Verify slave ID {} matches sensor configuration", slave_id);
            println!("         → Verify baud rate {} bps matches sensor", baud_rate);
            println!("         → Check sensor is powered and Modbus mode is enabled");
            println!("\n  Result: NOT RESPONDING — server starting in offline mode");
            println!("{}\n", border);
            None
        }
        Err(_) => {
            println!("TIMED OUT ({} ms)", elapsed);
            println!("         ✗ No response after {} ms", timeout_ms);
            println!();
            println!("         Serial port opened OK but sensor is not replying.");
            println!("         Most likely causes:");
            println!();
            println!("         [RS485 direction control]");
            println!("           FT232R is a plain UART — it has no automatic RS485");
            println!("           direction switching. Your adapter must handle DE/RE");
            println!("           itself, or the request is sent but TX is never enabled.");
            println!("           → Check your USB-RS485 adapter has auto direction control");
            println!("           → Or try an adapter with automatic flow control (e.g. CH340)");
            println!();
            println!("         [Wrong slave ID]");
            println!("           Config has slave ID {}. Sensor default is usually 1.", slave_id);
            println!("           → Try broadcasting: set slave_id = 0 in config.toml");
            println!();
            println!("         [Wrong baud rate]");
            println!("           Config has {} bps. Sensor default is 115200.", baud_rate);
            println!("           → Verify with sensor documentation or try 9600");
            println!();
            println!("         [Wiring]");
            println!("           → Verify A/B lines are not swapped");
            println!("           → Check termination resistor (120Ω) if cable is long");
            println!();
            println!("         → Run with debug logging for raw bytes:");
            println!("           RUST_LOG=debug cargo run");
            println!("         → Or test with mbpoll: mbpoll -a {} -b {} {} -t 3 -r 20", slave_id, baud_rate, device_path);
            println!("\n  Result: TIMED OUT — server starting in offline mode");
            println!("{}\n", border);
            None
        }
    }
}

/// Spawn a metrics background task for one sensor.
/// Reads all metrics at the configured rate and broadcasts to subscribers.
fn spawn_metrics_tasks(
    mc: Arc<tokio::sync::RwLock<Option<ModbusClient>>>,
    tx: broadcast::Sender<types::WebSocketMessage>,
    rate_hz: f64,
) {
    let interval_ms = (1000.0 / rate_hz.max(0.1)) as u64;
    tokio::spawn(async move {
        loop {
            if tx.receiver_count() == 0 {
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }

            let cycle_start = tokio::time::Instant::now();
            let guard = mc.read().await;
            match &*guard {
                Some(client) => {
                    let g = client.read_gravity_metrics().await;
                    let v = client.read_velocity_metrics().await;
                    let t = client.read_temperature().await;
                    drop(guard);
                    match (g, v, t) {
                        (Ok(gravity), Ok(velocity), Ok(temperature)) => {
                            let _ = tx.send(types::WebSocketMessage::Metrics {
                                timestamp: chrono::Utc::now(),
                                gravity,
                                velocity,
                                temperature,
                            });
                        }
                        _ => {
                            error!("Metrics read error");
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }
                    }
                }
                None => {
                    drop(guard);
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    continue;
                }
            }

            let elapsed = cycle_start.elapsed();
            let target = Duration::from_millis(interval_ms);
            if elapsed < target {
                tokio::time::sleep(target - elapsed).await;
            }
        }
    });
}

// ── CSV viewer ────────────────────────────────────────────────────────────────

const CSV_DATA_DIR: &str = "data";

fn list_csv_files() -> Vec<String> {
    let dir = std::path::PathBuf::from(CSV_DATA_DIR);
    if !dir.exists() {
        return vec![];
    }
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.to_lowercase().ends_with(".csv") { Some(name) } else { None }
        })
        .collect();
    files.sort();
    files
}

fn is_safe_csv_filename(name: &str) -> bool {
    !name.is_empty()
        && !name.contains("..")
        && !name.contains('/')
        && !name.contains('\\')
        && name.to_lowercase().ends_with(".csv")
        && name.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// GET /view/csv — redirect to the first file, or show empty state.
async fn csv_list_page(State(state): State<AppState>) -> Response {
    let files = list_csv_files();
    if let Some(first) = files.first().cloned() {
        Redirect::to(&format!("/view/csv/{}", first)).into_response()
    } else {
        state.render_template("view_csv.html", "/view/csv", context! {
            title => "CSV Viewer",
            files => files,
            current_file => "",
        }).into_response()
    }
}

/// GET /view/csv/:filename — render the canvas viewer for one CSV file.
async fn csv_viewer_page(
    AxumPath(filename): AxumPath<String>,
    State(state): State<AppState>,
) -> Response {
    if !is_safe_csv_filename(&filename) {
        return (StatusCode::BAD_REQUEST, Html("<p>Invalid filename</p>".to_string()))
            .into_response();
    }
    if !std::path::PathBuf::from(CSV_DATA_DIR).join(&filename).exists() {
        return (StatusCode::NOT_FOUND, Html("<p>File not found</p>".to_string()))
            .into_response();
    }
    let files = list_csv_files();
    state.render_template("view_csv.html", "/view/csv", context! {
        title => format!("CSV — {}", filename),
        files => files,
        current_file => filename,
    }).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("modbus_stream=debug".parse()?),
        )
        .init();

    let args = Args::parse();

    // Load configuration
    let config = AppConfig::load(&args.config)?;
    info!("Loaded configuration from {}", args.config);

    // Setup template auto-reloader
    let template_path = "templates";
    let template_env = Arc::new(AutoReloader::new(move |notifier| {
        let mut env = Environment::new();
        notifier.watch_path(template_path, true);
        env.set_loader(minijinja::path_loader(template_path));

        // Add the url_for function to the environment
        env.add_function("url_for", url_for);

        Ok(env)
    }));

    // Resolve sensor 1 device/baud from CLI args (overrides config for sensor 1 only)
    let device1 = args.device.unwrap_or_else(|| {
        config.sensors.first().map(|s| s.device.clone()).unwrap_or_else(|| "/dev/ttyUSB0".to_string())
    });
    let baud1 = if args.baud_rate != 115200 {
        args.baud_rate
    } else {
        config.sensors.first().map(|s| s.baud_rate).unwrap_or(115200)
    };

    // Run startup diagnostics and spawn background tasks for each configured sensor
    let mut modbus_arcs: Vec<Arc<tokio::sync::RwLock<Option<ModbusClient>>>> = Vec::new();
    let mut metrics_txs_vec: Vec<broadcast::Sender<types::WebSocketMessage>> = Vec::new();

    for (i, sensor_cfg) in config.sensors.iter().enumerate() {
        let device = if i == 0 { device1.clone() } else { sensor_cfg.device.clone() };
        let baud   = if i == 0 { baud1 } else { sensor_cfg.baud_rate };
        let slave  = if i == 0 { args.slave_id } else { sensor_cfg.slave_id };

        set_ftdi_latency(&device);
        let client = run_startup_diagnostics(&device, baud, slave, sensor_cfg.timeout_ms).await;

        let arc = Arc::new(tokio::sync::RwLock::new(client));
        let (tx, _) = broadcast::channel::<types::WebSocketMessage>(16);

        spawn_metrics_tasks(arc.clone(), tx.clone(), config.streaming.metrics_update_rate_hz);

        modbus_arcs.push(arc);
        metrics_txs_vec.push(tx);
    }

    // Create application state
    let state = AppState {
        modbus_clients: modbus_arcs,
        config: Arc::new(config),
        config_path: args.config.clone(),
        template_env,
        metrics_txs: metrics_txs_vec,
        recording: Arc::new(tokio::sync::Mutex::new(RecordingState::default())),
    };

    // Build our application with routes
    let app = Router::new()
        // Health check
        .route("/health", get(health_check))
        
        // Dashboard
        .route("/", get(dashboard_handler))
        
        // Configuration routes
        .route("/config", get(routes::config::get_config))
        .route("/config/sample-rate", put(routes::config::set_sample_rate))
        .route("/config/baud-rate", put(routes::config::set_baud_rate))
        .route("/config/high-pass-filter", put(routes::config::set_high_pass_filter))
        .route("/config/stream-size", put(routes::config::set_stream_size))
        
        // Read routes - System info
        .route("/{sensor}/read/temperature", get(routes::read::get_temperature))
        .route("/{sensor}/read/ucid", get(routes::read::get_ucid))
        .route("/{sensor}/read/firmware-version", get(routes::read::get_firmware_version))
        .route("/{sensor}/read/chip-id", get(routes::read::get_chip_id))
        .route("/{sensor}/read/fifo-buffer-size", get(routes::read::get_fifo_buffer_size))
        .route("/{sensor}/read/latest-raw", get(routes::read::get_latest_raw))

        // Read routes - Gravity metrics
        .route("/{sensor}/read/gravity/rms", get(routes::read::get_gravity_rms))
        .route("/{sensor}/read/gravity/peak", get(routes::read::get_gravity_peak))
        .route("/{sensor}/read/gravity/crest-factor", get(routes::read::get_gravity_crest_factor))
        .route("/{sensor}/read/gravity/skewness", get(routes::read::get_gravity_skewness))
        .route("/{sensor}/read/gravity/kurtosis", get(routes::read::get_gravity_kurtosis))
        .route("/{sensor}/read/gravity/primary-frequency", get(routes::read::get_gravity_primary_frequency))

        // Read routes - Velocity metrics
        .route("/{sensor}/read/velocity/rms", get(routes::read::get_velocity_rms))
        .route("/{sensor}/read/velocity/peak", get(routes::read::get_velocity_peak))
        .route("/{sensor}/read/velocity/crest-factor", get(routes::read::get_velocity_crest_factor))
        .route("/{sensor}/read/velocity/primary-frequency", get(routes::read::get_velocity_primary_frequency))

        // Read routes - Bulk
        .route("/{sensor}/read/all-metrics", get(routes::read::get_all_metrics))
        .route("/read/latest-raw", get(routes::read::get_latest_raw_combined))

        // Stream routes
        .route("/{sensor}/stream/raw", get(routes::stream::websocket_raw_handler))
        .route("/{sensor}/stream/metrics", get(routes::stream::websocket_metrics_handler))
        .route("/{sensor}/stream/start", post(routes::stream::start_stream))
        .route("/{sensor}/stream/stop", post(routes::stream::stop_stream))
        .route("/{sensor}/stream/status", get(routes::stream::get_stream_status))

        // Settings routes
        .route("/settings", get(routes::settings::settings_page_handler))
        .route("/settings/apply", post(routes::settings::apply_settings_handler))
        .route("/settings/test", post(routes::settings::test_connection_handler))
        .route("/settings/status", get(routes::settings::get_status_handler))
        .route("/settings/reset", post(routes::settings::reset_settings_handler))
        .route("/settings/ports", get(routes::settings::get_available_ports))
        .route("/settings/validate", post(routes::settings::validate_field_handler))
        
        // View pages (charts + data)
        .route("/view/raw", get(routes::view::raw_stream_page))
        .route("/{sensor}/view/raw", get(routes::view::raw_stream_page_sensor))
        .route("/view/metrics", get(routes::view::metrics_stream_page))
        .route("/{sensor}/view/metrics", get(routes::view::metrics_stream_page_sensor))
        .route("/view/latest-raw", get(routes::view::latest_raw_page))
        .route("/view/all-metrics", get(routes::view::all_metrics_page))
        .route("/{sensor}/view/all-metrics", get(routes::view::all_metrics_page_sensor))
        .route("/view/health", get(|| async { Redirect::permanent("/health") }))
        .route("/view/diagnostics", get(routes::view::diagnostics_page))

        // Recording
        .route("/api/record/start", post(record_start_handler))
        .route("/api/record/status", get(record_status_handler))

        // CSV viewer
        .route("/view/csv", get(csv_list_page))
        .route("/view/csv/{filename}", get(csv_viewer_page))

        // Static files and diagnostics
        .route("/diagnostics", get(routes::diagnostics::get_diagnostics))
        .nest_service("/static", get_service(ServeDir::new("static")).handle_error(|e| async move {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Error serving static file: {}", e))
        }))
        .nest_service("/data", get_service(ServeDir::new("data")).handle_error(|e| async move {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Error serving data file: {}", e))
        }))
        
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Parse bind address
    let addr: SocketAddr = args.bind.parse()?;
    let listener = TcpListener::bind(addr).await?;
    
    info!("Starting server on {}", addr);
    info!("API Documentation:");
    info!("  Health: GET /health");
    info!("  Config: GET/PUT /config/*");
    info!("  Read:   GET /read/*");
    info!("  Stream: WS /stream/raw, /stream/metrics");
    
    axum::serve(listener, app).await?;

    Ok(())
}

// ── Recording ─────────────────────────────────────────────────────────────────

const RECORD_TARGET: u64 = 78120; // 10 seconds at 7812 Hz

async fn record_start_handler(State(state): State<AppState>) -> impl IntoResponse {
    let mut rec = state.recording.lock().await;
    if rec.active {
        return Json(serde_json::json!({ "success": false, "error": "Already recording" }));
    }
    rec.active = true;
    rec.samples = 0;
    rec.filename = None;
    rec.error = None;
    drop(rec);

    let modbus = match state.modbus_clients.first() {
        Some(c) => c.clone(),
        None => return Json(serde_json::json!({ "success": false, "error": "No sensors configured" })),
    };
    let recording = state.recording.clone();
    tokio::spawn(async move { run_recording(modbus, recording).await });

    Json(serde_json::json!({ "success": true }))
}

async fn record_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let rec = state.recording.lock().await;
    Json(rec.clone())
}

async fn run_recording(
    modbus_client: Arc<tokio::sync::RwLock<Option<ModbusClient>>>,
    recording: Arc<tokio::sync::Mutex<RecordingState>>,
) {
    use std::io::Write;

    let filename = format!("record_{}.csv", chrono::Local::now().format("%Y%m%d_%H%M%S"));
    let path = format!("{}/{}", CSV_DATA_DIR, filename);

    let file = match std::fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            let mut rec = recording.lock().await;
            rec.active = false;
            rec.error = Some(format!("Failed to create file: {}", e));
            return;
        }
    };

    {
        let mut rec = recording.lock().await;
        rec.filename = Some(filename.clone());
    }

    let mut writer = std::io::BufWriter::new(file);
    let _ = writeln!(writer, "x,y,z");

    let mut next_count: u16 = 0;
    let mut total: u64 = 0;

    loop {
        if total >= RECORD_TARGET {
            break;
        }

        let guard = modbus_client.read().await;
        let client = match &*guard {
            Some(c) => c,
            None => {
                drop(guard);
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };

        let result: anyhow::Result<(u16, Vec<types::AccelerationData>)> = if next_count <= 6 {
            client.read_fifo_buffer_size().await.map(|sz| (sz, vec![]))
        } else {
            let count = next_count.min(123);
            client.read_fifo_combined(count).await
        };

        drop(guard);

        match result {
            Ok((new_size, data)) => {
                next_count = new_size;
                if data.is_empty() {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                    continue;
                }
                for sample in &data {
                    if total >= RECORD_TARGET { break; }
                    let _ = writeln!(writer, "{},{},{}", sample.x, sample.y, sample.z);
                    total += 1;
                }
                recording.lock().await.samples = total;
            }
            Err(e) => {
                error!("Recording read failed: {}", e);
                next_count = 0;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }

    let _ = writer.flush();
    let mut rec = recording.lock().await;
    rec.active = false;
    rec.samples = total;
}

// ─────────────────────────────────────────────────────────────────────────────

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "service": "modbus-stream"
    }))
}

async fn dashboard_handler(State(state): State<AppState>) -> impl IntoResponse {
    state.render_template("dashboard.html", "/", context! {
        title => "Modbus Stream Dashboard",
        version => env!("CARGO_PKG_VERSION")
    })
}

// Helper function to generate URLs in templates
fn url_for(name: &str, _args: Vec<minijinja::Value>) -> Result<minijinja::Value, minijinja::Error> {
    match name {
        "index" => Ok(minijinja::Value::from("/")),
        "health" => Ok(minijinja::Value::from("/health")),
        "settings" => Ok(minijinja::Value::from("/settings")),
        "apply_settings" => Ok(minijinja::Value::from("/settings/apply")),
        "test_connection" => Ok(minijinja::Value::from("/settings/test")),
        "reset_settings" => Ok(minijinja::Value::from("/settings/reset")),
        "get_ports" => Ok(minijinja::Value::from("/settings/ports")),
        "validate_settings" => Ok(minijinja::Value::from("/settings/validate")),
        "diagnostics"   => Ok(minijinja::Value::from("/diagnostics")),
        "view_raw"        => Ok(minijinja::Value::from("/view/raw")),
        "view_metrics"    => Ok(minijinja::Value::from("/view/metrics")),
        "view_latest_raw"   => Ok(minijinja::Value::from("/view/latest-raw")),
        "view_all_metrics"  => Ok(minijinja::Value::from("/view/all-metrics")),
        "view_diagnostics"  => Ok(minijinja::Value::from("/view/diagnostics")),
        "view_csv"          => Ok(minijinja::Value::from("/view/csv")),
        _ => Err(minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("unknown route: {}", name),
        )),
    }
}

// Template rendering helpers for AppState
impl AppState {
    pub fn render_template(&self, template_name: &str, _path: &str, context: minijinja::Value) -> Html<String> {
        match self.template_env.acquire_env() {
            Ok(env) => {
                match env.get_template(template_name) {
                    Ok(template) => {
                        match template.render(context) {
                            Ok(rendered) => Html(rendered),
                            Err(e) => {
                                tracing::error!("Template render error: {}", e);
                                Html(format!("<div class='error'>Template render error: {}</div>", e))
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Template not found: {} - {}", template_name, e);
                        Html(format!("<div class='error'>Template not found: {}</div>", template_name))
                    }
                }
            }
            Err(e) => {
                tracing::error!("Template environment error: {}", e);
                Html(format!("<div class='error'>Template environment error</div>"))
            }
        }
    }
    
    pub fn render_template_fragment(&self, template_name: &str, context: minijinja::Value) -> Html<String> {
        self.render_template(template_name, "", context)
    }
}
