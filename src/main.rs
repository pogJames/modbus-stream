use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post, put, get_service},
    Router,
};
use clap::Parser;
use minijinja::{context, Environment};
use minijinja_autoreload::AutoReloader;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::{info, warn};

mod config;
mod modbus;
mod routes;
mod types;

use config::AppConfig;
use modbus::ModbusClient;
use types::*;

/// Modbus Stream Server - Web interface for tri-axial accelerometer
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Server bind address
    #[arg(short, long, default_value = "127.0.0.1:3000")]
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

#[derive(Clone)]
pub struct AppState {
    modbus_client: Arc<tokio::sync::RwLock<Option<ModbusClient>>>,
    config: Arc<AppConfig>,
    config_path: String,
    template_env: Arc<AutoReloader>,
}

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

    // Try to initialize Modbus client (don't fail if device not found)
    let device_path = args.device.unwrap_or_else(|| config.modbus.device.clone());
    let baud_rate = if args.baud_rate != 115200 {
        args.baud_rate
    } else {
        config.modbus.baud_rate
    };

    let modbus_client = ModbusClient::new(&device_path, baud_rate, args.slave_id).await
        .map_err(|e| {
            warn!("Device not found: {}. Starting in monitoring mode.", e);
            e
        })
        .ok();
    
    match &modbus_client {
        Some(_) => info!("Connected to Modbus device at {}", device_path),
        None => warn!("Modbus device not found at {}, starting in monitoring mode", device_path),
    }

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

    // Create application state
    let state = AppState {
        modbus_client: Arc::new(tokio::sync::RwLock::new(modbus_client)),
        config: Arc::new(config),
        config_path: args.config.clone(),
        template_env,
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
        
        // Static files and diagnostics
        .route("/diagnostics", get(routes::diagnostics::get_diagnostics))
        .nest_service("/static", get_service(ServeDir::new("static")).handle_error(|e| async move {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Error serving static file: {}", e))
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
        "diagnostics" => Ok(minijinja::Value::from("/diagnostics")),
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
