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
    modbus_client: Arc<tokio::sync::RwLock<Option<ModbusClient>>>,
    config: Arc<AppConfig>,
    config_path: String,
    template_env: Arc<AutoReloader>,
    /// Shared metrics broadcaster — one background reader, all WebSocket clients subscribe here.
    pub metrics_tx: broadcast::Sender<types::WebSocketMessage>,
    recording: Arc<tokio::sync::Mutex<RecordingState>>,
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

    // Lower FTDI latency before touching the serial port
    let device_path = args.device.unwrap_or_else(|| config.modbus.device.clone());
    set_ftdi_latency(&device_path);
    let baud_rate = if args.baud_rate != 115200 {
        args.baud_rate
    } else {
        config.modbus.baud_rate
    };

    let modbus_client = run_startup_diagnostics(
        &device_path,
        baud_rate,
        args.slave_id,
        config.modbus.timeout_ms,
    ).await;

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

    let modbus_arc = Arc::new(tokio::sync::RwLock::new(modbus_client));

    // Shared metrics broadcast channel (capacity 16 — clients only need the latest)
    let (metrics_tx, _) = broadcast::channel::<types::WebSocketMessage>(16);

    // Single shared background metrics reader — runs only when clients are subscribed.
    // Skewness and kurtosis update every 2–5 s on the sensor (per datasheet), so they
    // are read on a slow path every 5 s and cached; all other metrics are read every cycle.
    // Shared cache for skewness + kurtosis (updated every 5 s on a separate task).
    let slow_cache: Arc<tokio::sync::Mutex<Option<(types::AccelerationData, types::AccelerationData)>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    // Slow-path task: reads skewness + kurtosis every 5 s, but only when metrics
    // clients are connected — skips entirely otherwise so the raw stream is never blocked.
    {
        let mc = modbus_arc.clone();
        let cache = slow_cache.clone();
        let tx = metrics_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if tx.receiver_count() == 0 {
                    continue;
                }
                let guard = mc.read().await;
                if let Some(client) = &*guard {
                    match (client.read_gravity_skewness().await,
                           client.read_gravity_kurtosis().await) {
                        (Ok(s), Ok(k)) => { *cache.lock().await = Some((s, k)); }
                        (Err(e), _) | (_, Err(e)) => {
                            error!("Slow metrics read error: {}", e);
                        }
                    }
                }
                drop(guard);
            }
        });
    }

    // Fast-path task: reads 6 metrics every ~1 s, uses cached skew/kurt — no stutter.
    {
        let mc = modbus_arc.clone();
        let tx = metrics_tx.clone();
        let cache = slow_cache;
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
                        let g_rms  = client.read_gravity_rms().await;
                        let g_peak = client.read_gravity_peak().await;
                        let g_cf   = client.read_gravity_crest_factor().await;
                        let g_freq = client.read_gravity_primary_frequency().await;
                        let vel    = client.read_velocity_metrics().await;
                        let temp   = client.read_temperature().await;
                        drop(guard);

                        match (g_rms, g_peak, g_cf, g_freq, vel, temp) {
                            (Ok(rms), Ok(peak), Ok(crest_factor), Ok(primary_frequency),
                             Ok(velocity), Ok(temperature)) => {
                                if let Some((skewness, kurtosis)) = cache.lock().await.clone() {
                                    let _ = tx.send(types::WebSocketMessage::Metrics {
                                        timestamp: chrono::Utc::now(),
                                        gravity: types::GravityMetrics {
                                            rms,
                                            peak,
                                            crest_factor,
                                            skewness,
                                            kurtosis,
                                            primary_frequency,
                                        },
                                        velocity,
                                        temperature,
                                    });
                                }
                                let elapsed = cycle_start.elapsed();
                                if elapsed < Duration::from_secs(1) {
                                    tokio::time::sleep(Duration::from_secs(1) - elapsed).await;
                                }
                            }
                            _ => {
                                error!("Fast metrics read error");
                                tokio::time::sleep(Duration::from_millis(500)).await;
                            }
                        }
                    }
                    None => {
                        drop(guard);
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                    }
                }
            }
        });
    }

    // Create application state
    let state = AppState {
        modbus_client: modbus_arc,
        config: Arc::new(config),
        config_path: args.config.clone(),
        template_env,
        metrics_tx,
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
        .route("/read/temperature", get(routes::read::get_temperature))
        .route("/read/ucid", get(routes::read::get_ucid))
        .route("/read/firmware-version", get(routes::read::get_firmware_version))
        .route("/read/chip-id", get(routes::read::get_chip_id))
        .route("/read/fifo-buffer-size", get(routes::read::get_fifo_buffer_size))
        .route("/read/latest-raw", get(routes::read::get_latest_raw))
        
        // Read routes - Gravity metrics
        .route("/read/gravity/rms", get(routes::read::get_gravity_rms))
        .route("/read/gravity/peak", get(routes::read::get_gravity_peak))
        .route("/read/gravity/crest-factor", get(routes::read::get_gravity_crest_factor))
        .route("/read/gravity/skewness", get(routes::read::get_gravity_skewness))
        .route("/read/gravity/kurtosis", get(routes::read::get_gravity_kurtosis))
        .route("/read/gravity/primary-frequency", get(routes::read::get_gravity_primary_frequency))
        
        // Read routes - Velocity metrics
        .route("/read/velocity/rms", get(routes::read::get_velocity_rms))
        .route("/read/velocity/peak", get(routes::read::get_velocity_peak))
        .route("/read/velocity/crest-factor", get(routes::read::get_velocity_crest_factor))
        .route("/read/velocity/primary-frequency", get(routes::read::get_velocity_primary_frequency))
        
        // Read routes - Bulk
        .route("/read/all-metrics", get(routes::read::get_all_metrics))
        
        // Stream routes
        .route("/stream/raw", get(routes::stream::websocket_raw_handler))
        .route("/stream/metrics", get(routes::stream::websocket_metrics_handler))
        .route("/stream/start", post(routes::stream::start_stream))
        .route("/stream/stop", post(routes::stream::stop_stream))
        .route("/stream/status", get(routes::stream::get_stream_status))
        
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
        .route("/view/metrics", get(routes::view::metrics_stream_page))
        .route("/view/latest-raw", get(routes::view::latest_raw_page))
        .route("/view/all-metrics", get(routes::view::all_metrics_page))
        .route("/view/health", get(routes::view::health_page))
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

    let modbus = state.modbus_client.clone();
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
        "get_status" => Ok(minijinja::Value::from("/settings/status")),
        "reset_settings" => Ok(minijinja::Value::from("/settings/reset")),
        "get_ports" => Ok(minijinja::Value::from("/settings/ports")),
        "validate_settings" => Ok(minijinja::Value::from("/settings/validate")),
        "diagnostics"   => Ok(minijinja::Value::from("/diagnostics")),
        "view_raw"        => Ok(minijinja::Value::from("/view/raw")),
        "view_metrics"    => Ok(minijinja::Value::from("/view/metrics")),
        "view_latest_raw"   => Ok(minijinja::Value::from("/view/latest-raw")),
        "view_all_metrics"  => Ok(minijinja::Value::from("/view/all-metrics")),
        "view_health"       => Ok(minijinja::Value::from("/view/health")),
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
