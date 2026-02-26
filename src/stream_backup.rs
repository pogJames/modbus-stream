use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::info;

use crate::{modbus::ModbusClient, types::WebSocketMessage};

pub mod manager;

pub use manager::StreamManager;

/// Global stream coordinator
pub struct StreamCoordinator {
    raw_tx: broadcast::Sender<WebSocketMessage>,
    metrics_tx: broadcast::Sender<WebSocketMessage>,
    _raw_rx: broadcast::Receiver<WebSocketMessage>,
    _metrics_rx: broadcast::Receiver<WebSocketMessage>,
}

impl StreamCoordinator {
    pub fn new() -> Self {
        let (raw_tx, raw_rx) = broadcast::channel(1000);
        let (metrics_tx, metrics_rx) = broadcast::channel(100);

        Self {
            raw_tx,
            metrics_tx,
            _raw_rx: raw_rx,
            _metrics_rx: metrics_rx,
        }
    }

    pub fn subscribe_raw(&self) -> broadcast::Receiver<WebSocketMessage> {
        self.raw_tx.subscribe()
    }

    pub fn subscribe_metrics(&self) -> broadcast::Receiver<WebSocketMessage> {
        self.metrics_tx.subscribe()
    }

    pub async fn start_streaming(&self, modbus_client: Arc<ModbusClient>) {
        let stream_manager = StreamManager::new(modbus_client);

        // Start background tasks
        let raw_tx = self.raw_tx.clone();
        let metrics_tx = self.metrics_tx.clone();

        tokio::spawn(async move {
            let _ = stream_manager.start_raw_streaming(raw_tx).await;
        });

        tokio::spawn(async move {
            let _ = stream_manager.start_metrics_streaming(metrics_tx).await;
        });

        info!("Stream coordinator started");
    }
}

impl Default for StreamCoordinator {
    fn default() -> Self {
        Self::new()
    }
}
