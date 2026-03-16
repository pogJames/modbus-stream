use axum::{extract::{Path, State}, response::{Html, Redirect, Response, IntoResponse}};
use minijinja::context;
use crate::AppState;

pub async fn raw_stream_page(State(_state): State<AppState>) -> Response {
    Redirect::permanent("/1/view/raw").into_response()
}

pub async fn raw_stream_page_sensor(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> Html<String> {
    state.render_template("view_raw.html", "", context! {
        title => "Raw Data Stream",
        sensor => sensor,
        sensor_count => state.sensor_count(),
    })
}

pub async fn metrics_stream_page(State(state): State<AppState>) -> Response {
    Redirect::permanent("/1/view/metrics").into_response()
}

pub async fn metrics_stream_page_sensor(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> Html<String> {
    state.render_template("view_metrics.html", "", context! {
        title => "Metrics Stream",
        sensor => sensor,
        sensor_count => state.sensor_count(),
    })
}

pub async fn latest_raw_page(State(state): State<AppState>) -> Html<String> {
    state.render_template("view_latest_raw.html", "/view/latest-raw", context! {
        title => "Latest Raw Reading"
    })
}

pub async fn all_metrics_page(State(state): State<AppState>) -> Response {
    Redirect::permanent("/1/view/all-metrics").into_response()
}

pub async fn all_metrics_page_sensor(
    Path(sensor): Path<u8>,
    State(state): State<AppState>,
) -> Html<String> {
    state.render_template("view_all_metrics.html", "", context! {
        title => "Sensor Metrics",
        sensor => sensor,
        sensor_count => state.sensor_count(),
    })
}

pub async fn diagnostics_page(State(state): State<AppState>) -> Html<String> {
    state.render_template("view_diagnostics.html", "/view/diagnostics", context! {
        title => "Diagnostics"
    })
}
