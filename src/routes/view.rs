use axum::{extract::State, response::Html};
use minijinja::context;
use crate::AppState;

pub async fn raw_stream_page(State(state): State<AppState>) -> Html<String> {
    state.render_template("view_raw.html", "/view/raw", context! {
        title => "Raw Data Stream"
    })
}

pub async fn metrics_stream_page(State(state): State<AppState>) -> Html<String> {
    state.render_template("view_metrics.html", "/view/metrics", context! {
        title => "Metrics Stream"
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
