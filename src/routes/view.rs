use axum::{extract::{Path as AxumPath, State}, response::Html};
use minijinja::context;
use crate::AppState;

pub async fn raw_stream_page(State(state): State<AppState>) -> Html<String> {
    state.render_template("view_raw.html", "/view/raw", context! {
        title => "Raw Data Stream",
        sensor_id => 0usize,
    })
}

pub async fn metrics_stream_page(State(state): State<AppState>) -> Html<String> {
    state.render_template("view_metrics.html", "/view/metrics", context! {
        title => "Metrics Stream",
        sensor_id => 0usize,
    })
}

pub async fn raw_stream_page_sensor(
    AxumPath(sensor_id): AxumPath<usize>,
    State(state): State<AppState>,
) -> Html<String> {
    let title = state.sensors.get(sensor_id)
        .map(|s| format!("Raw Stream — {}", s.name))
        .unwrap_or_else(|| format!("Raw Stream — Sensor {}", sensor_id));
    state.render_template("view_raw.html", "/view/raw", context! {
        title => title,
        sensor_id => sensor_id,
    })
}

pub async fn metrics_stream_page_sensor(
    AxumPath(sensor_id): AxumPath<usize>,
    State(state): State<AppState>,
) -> Html<String> {
    let title = state.sensors.get(sensor_id)
        .map(|s| format!("Metrics Stream — {}", s.name))
        .unwrap_or_else(|| format!("Metrics Stream — Sensor {}", sensor_id));
    state.render_template("view_metrics.html", "/view/metrics", context! {
        title => title,
        sensor_id => sensor_id,
    })
}

pub async fn latest_raw_page(State(state): State<AppState>) -> Html<String> {
    state.render_template("view_latest_raw.html", "/view/latest-raw", context! {
        title => "Latest Raw Reading"
    })
}

pub async fn all_metrics_page(State(state): State<AppState>) -> Html<String> {
    state.render_template("view_all_metrics.html", "/view/all-metrics", context! {
        title => "Sensor Metrics"
    })
}

pub async fn health_page(State(state): State<AppState>) -> Html<String> {
    state.render_template("view_health.html", "/view/health", context! {
        title => "Health"
    })
}

pub async fn diagnostics_page(State(state): State<AppState>) -> Html<String> {
    state.render_template("view_diagnostics.html", "/view/diagnostics", context! {
        title => "Diagnostics"
    })
}
