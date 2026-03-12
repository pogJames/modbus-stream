use anyhow::Result;
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Redirect, Response},
    routing::{get, post, get_service},
    Router,
};
use clap::Parser;
use minijinja::{context, Environment};
use minijinja_autoreload::AutoReloader;
use serde::Serialize;
use std::{net::SocketAddr, path::Path, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex, RwLock};
use std::time::Duration;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::{error, info, warn};

mod config;
mod modbus;
mod routes;
mod types;

use config::{AppConfig, SensorConfig};
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

// ── Per-sensor handle ──────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SensorHandle {
    pub id: usize,
    pub name: String,
    pub device: String,
    pub slave_id: u8,
    pub client: Arc<RwLock<Option<ModbusClient>>>,
    pub metrics_tx: broadcast::Sender<types::WebSocketMessage>,
    pub record_tx: tokio::sync::mpsc::Sender<Vec<types::AccelerationData>>,
    pub record_rx: Arc<Mutex<tokio::sync::mpsc::Receiver<Vec<types::AccelerationData>>>>,
}

// ── Recording state ────────────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
pub struct SensorRecordingState {
    pub sensor_id: usize,
    pub filename: String,
    pub samples: u64,
    pub complete: bool,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct MultiRecordingState {
    pub active: bool,
    pub total: u64,
    pub per_sensor: Vec<SensorRecordingState>,
}

impl Default for MultiRecordingState {
    fn default() -> Self {
        Self { active: false, total: RECORD_TARGET, per_sensor: vec![] }
    }
}

// ── AppState ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub sensors: Arc<Vec<SensorHandle>>,
    pub config: Arc<AppConfig>,
    pub config_path: String,
    pub template_env: Arc<AutoReloader>,
    pub recording: Arc<Mutex<MultiRecordingState>>,
}

// ── FTDI latency ──────────────────────────────────────────────────────────────

fn set_ftdi_latency(device: &str) {
    if let Some(name) = device.strip_prefix("/dev/") {
        let path = format!("/sys/bus/usb-serial/devices/{}/latency_timer", name);
        match std::fs::write(&path, "1") {
            Ok(_) => info!("FTDI latency timer → 1 ms ({})", device),
            Err(e) => println!("  Note: could not set FTDI latency ({}) — try: echo 1 | sudo tee {}", e, path),
        }
    }
}

// ── Port auto-discovery ────────────────────────────────────────────────────────

/// Enumerate /dev/ttyUSB* candidates (Linux) up to 4 devices.
fn candidate_devices() -> Vec<String> {
    let mut candidates = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev") {
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                if n.starts_with("ttyUSB") { Some(format!("/dev/{}", n)) } else { None }
            })
            .collect();
        names.sort();
        candidates.extend(names.into_iter().take(4));
    }
    candidates
}

/// Probe each candidate port; return a SensorHandle for every responsive sensor.
async fn discover_sensors(config: &AppConfig) -> Vec<SensorHandle> {
    let candidates: Vec<String> = if config.discovery.auto_scan {
        candidate_devices()
    } else {
        config.sensors.iter()
            .filter(|s| s.enabled)
            .map(|s| s.device.clone())
            .collect()
    };

    if candidates.is_empty() {
        warn!("No candidate serial ports found — starting with no sensors");
    }

    let mut handles = Vec::new();
    for (idx, device) in candidates.iter().enumerate() {
        set_ftdi_latency(device);

        let cfg = config.sensors.get(idx).cloned().unwrap_or_else(|| SensorConfig {
            name: format!("Sensor {}", idx + 1),
            device: device.clone(),
            slave_id: config.discovery.slave_id,
            baud_rate: config.discovery.baud_rate,
            timeout_ms: config.discovery.probe_timeout_ms,
            enabled: true,
        });

        if !cfg.enabled {
            continue;
        }

        // Brief existence / permission check
        if !Path::new(device).exists() {
            info!("Port {} not found, skipping", device);
            continue;
        }

        match ModbusClient::new(&cfg.device, cfg.baud_rate, cfg.slave_id).await {
            Ok(client) => {
                match client.read_temperature().await {
                    Ok(temp) => {
                        info!("Sensor {} detected on {} (temp {:.1}°C)", idx + 1, cfg.device, temp);
                        let (metrics_tx, _) = broadcast::channel(16);
                        let (record_tx, record_rx) = tokio::sync::mpsc::channel(3);
                        handles.push(SensorHandle {
                            id: idx,
                            name: cfg.name,
                            device: cfg.device,
                            slave_id: cfg.slave_id,
                            client: Arc::new(RwLock::new(Some(client))),
                            metrics_tx,
                            record_tx,
                            record_rx: Arc::new(Mutex::new(record_rx)),
                        });
                    }
                    Err(e) => warn!("Port {} opened but sensor silent: {}", device, e),
                }
            }
            Err(e) => info!("No sensor on {}: {}", device, e),
        }
    }

    if handles.is_empty() {
        // Offline mode: create one placeholder handle so the server still starts
        info!("No sensors detected — creating offline placeholder for {}",
              config.sensors.first().map(|s| s.device.as_str()).unwrap_or("/dev/ttyUSB0"));
        let cfg = config.sensors.first().cloned().unwrap_or_default();
        let (metrics_tx, _) = broadcast::channel(16);
        let (record_tx, record_rx) = tokio::sync::mpsc::channel(3);
        handles.push(SensorHandle {
            id: 0,
            name: cfg.name,
            device: cfg.device,
            slave_id: cfg.slave_id,
            client: Arc::new(RwLock::new(None)),
            metrics_tx,
            record_tx,
            record_rx: Arc::new(Mutex::new(record_rx)),
        });
    }

    handles
}

// ── Per-sensor FIFO loop ───────────────────────────────────────────────────────

/// Mirrors Python ServiceThread.run(): accumulates 7812 samples then sends to record channel.
async fn sensor_fifo_loop(sensor: SensorHandle) {
    const SAMPLES_PER_SEC: usize = 7812;
    let mut accumulator: Vec<types::AccelerationData> = Vec::with_capacity(SAMPLES_PER_SEC);
    let mut next_count: u16 = 0;

    loop {
        let guard = sensor.client.read().await;
        let Some(client) = &*guard else {
            drop(guard);
            tokio::time::sleep(Duration::from_millis(1000)).await;
            continue;
        };

        let result = if next_count <= 6 {
            client.read_fifo_buffer_size().await.map(|sz| (sz, vec![]))
        } else {
            client.read_fifo_combined(next_count.min(123)).await
        };
        drop(guard);

        match result {
            Ok((new_size, data)) => {
                next_count = new_size;
                if data.is_empty() {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                    continue;
                }

                accumulator.extend_from_slice(&data);

                if accumulator.len() >= SAMPLES_PER_SEC {
                    let chunk = std::mem::replace(
                        &mut accumulator,
                        Vec::with_capacity(SAMPLES_PER_SEC),
                    );
                    if sensor.record_tx.capacity() == 0 {
                        warn!("Sensor {}: record queue full, dropping chunk", sensor.id);
                    }
                    let _ = sensor.record_tx.try_send(chunk);
                }
            }
            Err(e) => {
                error!("Sensor {} FIFO read error: {}", sensor.id, e);
                next_count = 0;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

// ── Per-sensor metrics tasks ───────────────────────────────────────────────────

fn spawn_metrics_tasks(sensor: SensorHandle) {
    let slow_cache: Arc<Mutex<Option<(types::AccelerationData, types::AccelerationData)>>> =
        Arc::new(Mutex::new(None));

    // Slow path: skewness + kurtosis every 5 s
    {
        let mc = sensor.client.clone();
        let cache = slow_cache.clone();
        let tx = sensor.metrics_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                if tx.receiver_count() == 0 {
                    continue;
                }
                let guard = mc.read().await;
                if let Some(client) = &*guard {
                    match (client.read_gravity_skewness().await, client.read_gravity_kurtosis().await) {
                        (Ok(s), Ok(k)) => { *cache.lock().await = Some((s, k)); }
                        (Err(e), _) | (_, Err(e)) => {
                            error!("Sensor slow metrics read error: {}", e);
                        }
                    }
                }
                drop(guard);
            }
        });
    }

    // Fast path: 6 metrics every ~1 s
    {
        let mc = sensor.client.clone();
        let tx = sensor.metrics_tx.clone();
        let cache = slow_cache;
        let sensor_id = sensor.id;
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
                                        sensor_id,
                                        timestamp: chrono::Utc::now(),
                                        gravity: types::GravityMetrics {
                                            rms, peak, crest_factor, skewness, kurtosis, primary_frequency,
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
                                error!("Sensor {} fast metrics read error", sensor_id);
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
}

// ── CSV viewer ─────────────────────────────────────────────────────────────────

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

// ── Recording ──────────────────────────────────────────────────────────────────

const RECORD_TARGET: u64 = 78120; // 10 seconds at 7812 Hz

async fn record_start_handler(State(state): State<AppState>) -> impl IntoResponse {
    let mut rec = state.recording.lock().await;
    if rec.active {
        return Json(serde_json::json!({ "success": false, "error": "Already recording" }));
    }
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    rec.active = true;
    rec.per_sensor = state.sensors.iter().map(|s| SensorRecordingState {
        sensor_id: s.id,
        filename: format!("record_sensor{}_{}.csv", s.id + 1, ts),
        samples: 0,
        complete: false,
        error: None,
    }).collect();
    drop(rec);

    for sensor in state.sensors.iter() {
        let s = sensor.clone();
        let recording = state.recording.clone();
        tokio::spawn(run_sensor_recording(s, recording));
    }

    Json(serde_json::json!({ "success": true }))
}

async fn record_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let rec = state.recording.lock().await;
    Json(rec.clone())
}

async fn run_sensor_recording(
    sensor: SensorHandle,
    recording: Arc<Mutex<MultiRecordingState>>,
) {
    use std::io::Write;

    let filename = {
        let rec = recording.lock().await;
        rec.per_sensor
            .iter()
            .find(|s| s.sensor_id == sensor.id)
            .map(|s| s.filename.clone())
            .unwrap_or_else(|| format!("record_sensor{}_{}.csv", sensor.id + 1,
                chrono::Local::now().format("%Y%m%d_%H%M%S")))
    };

    let path = format!("{}/{}", CSV_DATA_DIR, filename);
    std::fs::create_dir_all(CSV_DATA_DIR).ok();

    let file = match std::fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            let mut rec = recording.lock().await;
            if let Some(s) = rec.per_sensor.iter_mut().find(|s| s.sensor_id == sensor.id) {
                s.error = Some(e.to_string());
                s.complete = true;
            }
            if rec.per_sensor.iter().all(|s| s.complete) {
                rec.active = false;
            }
            return;
        }
    };

    let mut writer = std::io::BufWriter::new(file);
    let _ = writeln!(writer, "x,y,z");

    let mut total: u64 = 0;
    let mut rx = sensor.record_rx.lock().await;

    while total < RECORD_TARGET {
        match rx.recv().await {
            Some(chunk) => {
                for sample in &chunk {
                    if total >= RECORD_TARGET { break; }
                    let _ = writeln!(writer, "{},{},{}", sample.x, sample.y, sample.z);
                    total += 1;
                }
                let mut rec = recording.lock().await;
                if let Some(s) = rec.per_sensor.iter_mut().find(|s| s.sensor_id == sensor.id) {
                    s.samples = total;
                }
            }
            None => break,
        }
    }

    let _ = writer.flush();
    let mut rec = recording.lock().await;
    if let Some(s) = rec.per_sensor.iter_mut().find(|s| s.sensor_id == sensor.id) {
        s.complete = true;
        s.samples = total;
    }
    if rec.per_sensor.iter().all(|s| s.complete) {
        rec.active = false;
    }
}

// ── Sensors list endpoint ──────────────────────────────────────────────────────

async fn get_sensors_handler(State(state): State<AppState>) -> impl IntoResponse {
    let list: Vec<_> = state.sensors.iter().map(|s| {
        serde_json::json!({
            "id": s.id,
            "name": s.name,
            "device": s.device,
            "slave_id": s.slave_id,
            "connected": true,
        })
    }).collect();
    Json(list)
}

// ── Main ───────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("modbus_stream=debug".parse()?),
        )
        .init();

    let args = Args::parse();

    let mut config = AppConfig::load(&args.config)?;
    info!("Loaded configuration from {}", args.config);

    // CLI overrides: if a device was specified, set it as sensor 0
    if let Some(dev) = &args.device {
        if let Some(sensor) = config.sensors.first_mut() {
            sensor.device = dev.clone();
        }
        if args.baud_rate != 115200 {
            if let Some(sensor) = config.sensors.first_mut() {
                sensor.baud_rate = args.baud_rate;
            }
        }
        if args.slave_id != 1 {
            if let Some(sensor) = config.sensors.first_mut() {
                sensor.slave_id = args.slave_id;
            }
        }
        // Disable auto-scan when device is explicitly specified
        config.discovery.auto_scan = false;
    }

    let sensors = discover_sensors(&config).await;
    info!("{} sensor(s) discovered", sensors.len());

    for sensor in &sensors {
        let s = sensor.clone();
        tokio::spawn(async move { sensor_fifo_loop(s).await });
        spawn_metrics_tasks(sensor.clone());
    }

    let template_path = "templates";
    let template_env = Arc::new(AutoReloader::new(move |notifier| {
        let mut env = Environment::new();
        notifier.watch_path(template_path, true);
        env.set_loader(minijinja::path_loader(template_path));
        env.add_function("url_for", url_for);
        Ok(env)
    }));

    let state = AppState {
        sensors: Arc::new(sensors),
        config: Arc::new(config),
        config_path: args.config.clone(),
        template_env,
        recording: Arc::new(Mutex::new(MultiRecordingState::default())),
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/", get(dashboard_handler))

        // Sensors list
        .route("/api/sensors", get(get_sensors_handler))

        // Configuration routes
        .route("/config", get(routes::config::get_config))
        .route("/config/sample-rate", axum::routing::put(routes::config::set_sample_rate))
        .route("/config/baud-rate", axum::routing::put(routes::config::set_baud_rate))
        .route("/config/high-pass-filter", axum::routing::put(routes::config::set_high_pass_filter))
        .route("/config/stream-size", axum::routing::put(routes::config::set_stream_size))

        // Read routes - backwards compat (sensor 0)
        .route("/read/temperature", get(routes::read::get_temperature))
        .route("/read/ucid", get(routes::read::get_ucid))
        .route("/read/firmware-version", get(routes::read::get_firmware_version))
        .route("/read/chip-id", get(routes::read::get_chip_id))
        .route("/read/fifo-buffer-size", get(routes::read::get_fifo_buffer_size))
        .route("/read/latest-raw", get(routes::read::get_latest_raw))
        .route("/read/gravity/rms", get(routes::read::get_gravity_rms))
        .route("/read/gravity/peak", get(routes::read::get_gravity_peak))
        .route("/read/gravity/crest-factor", get(routes::read::get_gravity_crest_factor))
        .route("/read/gravity/skewness", get(routes::read::get_gravity_skewness))
        .route("/read/gravity/kurtosis", get(routes::read::get_gravity_kurtosis))
        .route("/read/gravity/primary-frequency", get(routes::read::get_gravity_primary_frequency))
        .route("/read/velocity/rms", get(routes::read::get_velocity_rms))
        .route("/read/velocity/peak", get(routes::read::get_velocity_peak))
        .route("/read/velocity/crest-factor", get(routes::read::get_velocity_crest_factor))
        .route("/read/velocity/primary-frequency", get(routes::read::get_velocity_primary_frequency))
        .route("/read/all-metrics", get(routes::read::get_all_metrics))

        // Per-sensor read routes
        .route("/sensor/{sensor_id}/read/temperature", get(routes::read::get_temperature_sensor))
        .route("/sensor/{sensor_id}/read/ucid", get(routes::read::get_ucid_sensor))
        .route("/sensor/{sensor_id}/read/firmware-version", get(routes::read::get_firmware_version_sensor))
        .route("/sensor/{sensor_id}/read/chip-id", get(routes::read::get_chip_id_sensor))
        .route("/sensor/{sensor_id}/read/fifo-buffer-size", get(routes::read::get_fifo_buffer_size_sensor))
        .route("/sensor/{sensor_id}/read/latest-raw", get(routes::read::get_latest_raw_sensor))
        .route("/sensor/{sensor_id}/read/gravity/rms", get(routes::read::get_gravity_rms_sensor))
        .route("/sensor/{sensor_id}/read/gravity/peak", get(routes::read::get_gravity_peak_sensor))
        .route("/sensor/{sensor_id}/read/gravity/crest-factor", get(routes::read::get_gravity_crest_factor_sensor))
        .route("/sensor/{sensor_id}/read/gravity/skewness", get(routes::read::get_gravity_skewness_sensor))
        .route("/sensor/{sensor_id}/read/gravity/kurtosis", get(routes::read::get_gravity_kurtosis_sensor))
        .route("/sensor/{sensor_id}/read/gravity/primary-frequency", get(routes::read::get_gravity_primary_frequency_sensor))
        .route("/sensor/{sensor_id}/read/velocity/rms", get(routes::read::get_velocity_rms_sensor))
        .route("/sensor/{sensor_id}/read/velocity/peak", get(routes::read::get_velocity_peak_sensor))
        .route("/sensor/{sensor_id}/read/velocity/crest-factor", get(routes::read::get_velocity_crest_factor_sensor))
        .route("/sensor/{sensor_id}/read/velocity/primary-frequency", get(routes::read::get_velocity_primary_frequency_sensor))
        .route("/sensor/{sensor_id}/read/all-metrics", get(routes::read::get_all_metrics_sensor))

        // Stream routes (backwards compat, sensor 0)
        .route("/stream/raw", get(routes::stream::websocket_raw_handler_compat))
        .route("/stream/metrics", get(routes::stream::websocket_metrics_handler_compat))
        .route("/stream/start", post(routes::stream::start_stream))
        .route("/stream/stop", post(routes::stream::stop_stream))
        .route("/stream/status", get(routes::stream::get_stream_status))

        // Per-sensor stream routes
        .route("/sensor/{sensor_id}/stream/raw", get(routes::stream::websocket_raw_handler))
        .route("/sensor/{sensor_id}/stream/metrics", get(routes::stream::websocket_metrics_handler))

        // Settings routes
        .route("/settings", get(routes::settings::settings_page_handler))
        .route("/settings/apply", post(routes::settings::apply_settings_handler))
        .route("/settings/test", post(routes::settings::test_connection_handler))
        .route("/settings/status", get(routes::settings::get_status_handler))
        .route("/settings/reset", post(routes::settings::reset_settings_handler))
        .route("/settings/ports", get(routes::settings::get_available_ports))
        .route("/settings/validate", post(routes::settings::validate_field_handler))

        // View pages (backwards compat, sensor 0)
        .route("/view/raw", get(routes::view::raw_stream_page))
        .route("/view/metrics", get(routes::view::metrics_stream_page))
        .route("/view/latest-raw", get(routes::view::latest_raw_page))
        .route("/view/all-metrics", get(routes::view::all_metrics_page))
        .route("/view/health", get(routes::view::health_page))
        .route("/view/diagnostics", get(routes::view::diagnostics_page))

        // Per-sensor view pages
        .route("/sensor/{sensor_id}/view/raw", get(routes::view::raw_stream_page_sensor))
        .route("/sensor/{sensor_id}/view/metrics", get(routes::view::metrics_stream_page_sensor))

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

    let addr: SocketAddr = args.bind.parse()?;
    let listener = TcpListener::bind(addr).await?;

    info!("Starting server on {}", addr);
    info!("  GET /api/sensors       → list detected sensors");
    info!("  GET /read/*            → sensor 0 (backwards compat)");
    info!("  GET /sensor/{{id}}/read/* → per-sensor reads");
    info!("  WS  /sensor/{{id}}/stream/raw|metrics");

    axum::serve(listener, app).await?;

    Ok(())
}

// ── Health / Dashboard ─────────────────────────────────────────────────────────

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "service": "modbus-stream"
    }))
}

async fn dashboard_handler(State(state): State<AppState>) -> impl IntoResponse {
    let sensors_info: Vec<_> = state.sensors.iter().map(|s| {
        serde_json::json!({ "id": s.id, "name": s.name, "device": s.device })
    }).collect();
    state.render_template("dashboard.html", "/", context! {
        title => "Modbus Stream Dashboard",
        version => env!("CARGO_PKG_VERSION"),
        sensors => sensors_info,
    })
}

fn url_for(name: &str, _args: Vec<minijinja::Value>) -> Result<minijinja::Value, minijinja::Error> {
    match name {
        "index"             => Ok(minijinja::Value::from("/")),
        "health"            => Ok(minijinja::Value::from("/health")),
        "settings"          => Ok(minijinja::Value::from("/settings")),
        "apply_settings"    => Ok(minijinja::Value::from("/settings/apply")),
        "test_connection"   => Ok(minijinja::Value::from("/settings/test")),
        "get_status"        => Ok(minijinja::Value::from("/settings/status")),
        "reset_settings"    => Ok(minijinja::Value::from("/settings/reset")),
        "get_ports"         => Ok(minijinja::Value::from("/settings/ports")),
        "validate_settings" => Ok(minijinja::Value::from("/settings/validate")),
        "diagnostics"       => Ok(minijinja::Value::from("/diagnostics")),
        "view_raw"          => Ok(minijinja::Value::from("/view/raw")),
        "view_metrics"      => Ok(minijinja::Value::from("/view/metrics")),
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

// ── Template helpers ───────────────────────────────────────────────────────────

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
                Html("<div class='error'>Template environment error</div>".to_string())
            }
        }
    }

    pub fn render_template_fragment(&self, template_name: &str, context: minijinja::Value) -> Html<String> {
        self.render_template(template_name, "", context)
    }
}
