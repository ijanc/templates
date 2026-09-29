// SPDX-License-Identifier: ISC
// SPDX-FileCopyrightText: {{year}} {{authors}}

use std::sync::OnceLock;

use axum::{Router, routing::get};
use axum_prometheus::{
    PrometheusMetricLayer, PrometheusMetricLayerBuilder,
    metrics_exporter_prometheus::PrometheusHandle,
};

use crate::AppState;

/// Prefix of every metric: `<prefix>_http_requests_total`, ...
pub const PREFIX: &str = "{{crate_name}}";

/// Path serving the Prometheus text format.
pub const PATH: &str = "/metrics";

// The recorder is process global and can only be installed once, but
// `app()` may be called many times (tests).
fn pair() -> &'static (PrometheusMetricLayer<'static>, PrometheusHandle) {
    static PAIR: OnceLock<(PrometheusMetricLayer<'static>, PrometheusHandle)> =
        OnceLock::new();
    PAIR.get_or_init(|| {
        PrometheusMetricLayerBuilder::new()
            .with_prefix(PREFIX)
            .with_default_metrics()
            .build_pair()
    })
}

/// Middleware recording request count and latency per route.
pub fn layer() -> PrometheusMetricLayer<'static> {
    pair().0.clone()
}

/// `GET /metrics`.
pub fn router() -> Router<AppState> {
    Router::new().route(PATH, get(|| async { pair().1.render() }))
}
