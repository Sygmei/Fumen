use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::MatchedPath,
    http::{
        HeaderValue, Request, Response,
        header::{HeaderName, USER_AGENT},
    },
};
use opentelemetry::{
    KeyValue, global,
    trace::{TraceContextExt, TracerProvider as _},
};
use opentelemetry_http::HeaderExtractor;
use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    Resource,
    propagation::TraceContextPropagator,
    trace::{Sampler, SdkTracerProvider},
};
use reqwest::Client;
use std::{collections::HashMap, env, time::Duration};
use tower_http::classify::ServerErrorsFailureClass;
use tracing::{Span, field::Empty};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_SERVICE_NAME: &str = "fumen-backend";
const DEFAULT_DEPLOYMENT_ENVIRONMENT: &str = "production";
const DEFAULT_OTLP_HTTP_TRACES_URL: &str = "http://127.0.0.1:4318/v1/traces";
const TRACER_NAME: &str = "fumen-backend";
pub(crate) const TRACE_ID_HEADER_NAME: HeaderName = HeaderName::from_static("x-trace-id");

pub(crate) struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(tracer_provider) = self.tracer_provider.take() {
            if let Err(error) = tracer_provider.shutdown() {
                eprintln!("[otel] shutdown failed: {error}");
            }
        }
    }
}

pub(crate) fn init() -> Result<TelemetryGuard> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("fumen_backend=info,tower_http=info"));
    let fmt_layer = tracing_subscriber::fmt::layer();

    if !is_enabled() {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
        return Ok(TelemetryGuard {
            tracer_provider: None,
        });
    }

    global::set_text_map_propagator(TraceContextPropagator::new());

    let traces_url = resolve_traces_url();
    let service_name = env_var_or_default("OTEL_SERVICE_NAME", DEFAULT_SERVICE_NAME);
    let service_version = env_var_or_default("OTEL_SERVICE_VERSION", env!("CARGO_PKG_VERSION"));
    let deployment_environment = env_var_or_default(
        "OTEL_DEPLOYMENT_ENVIRONMENT",
        DEFAULT_DEPLOYMENT_ENVIRONMENT,
    );

    let tracer_provider = build_tracer_provider(
        &traces_url,
        &service_name,
        &service_version,
        &deployment_environment,
    )?;
    let tracer = tracer_provider.tracer(TRACER_NAME);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    tracing::info!(
        exporter = %traces_url,
        service = %service_name,
        version = %service_version,
        environment = %deployment_environment,
        "OpenTelemetry exporter initialized"
    );

    Ok(TelemetryGuard {
        tracer_provider: Some(tracer_provider),
    })
}

pub(crate) fn make_http_request_span(request: &Request<Body>) -> Span {
    let matched_path = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str);
    let user_agent = request
        .headers()
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok());

    let span = tracing::info_span!(
        "http.request",
        otel.kind = "server",
        http.request.method = %request.method(),
        url.path = %request.uri().path(),
        url.query = Empty,
        http.route = Empty,
        user_agent.original = Empty,
        network.protocol.version = ?request.version(),
        http.response.status_code = Empty,
        error.type = Empty,
    );

    if let Some(query) = request.uri().query() {
        span.record("url.query", query);
    }

    if let Some(path) = matched_path {
        span.record("http.route", path);
    }

    if let Some(user_agent) = user_agent {
        span.record("user_agent.original", user_agent);
    }

    attach_remote_parent(&span, request);
    span
}

pub(crate) fn on_http_response(response: &Response<Body>, latency: Duration, span: &Span) {
    span.record("http.response.status_code", response.status().as_u16());
    tracing::info!(
        parent: span,
        latency_ms = latency.as_millis() as u64,
        status = response.status().as_u16(),
        "http response"
    );
}

pub(crate) fn on_http_failure(
    failure_classification: ServerErrorsFailureClass,
    latency: Duration,
    span: &Span,
) {
    let failure = format!("{failure_classification:?}");
    span.record("error.type", failure.as_str());
    tracing::error!(
        parent: span,
        latency_ms = latency.as_millis() as u64,
        error = %failure,
        "http request failed"
    );
}

pub(crate) fn current_trace_id_header_value() -> Option<HeaderValue> {
    let trace_id = Span::current().context().span().span_context().trace_id();
    let trace_id = trace_id.to_string();
    if trace_id.is_empty() || trace_id.chars().all(|ch| ch == '0') {
        return None;
    }

    HeaderValue::from_str(&trace_id).ok()
}

fn attach_remote_parent(span: &Span, request: &Request<Body>) {
    let parent_context = global::get_text_map_propagator(|propagator| {
        propagator.extract(&HeaderExtractor(request.headers()))
    });

    if !parent_context.span().span_context().is_valid() {
        return;
    }

    if let Err(error) = span.set_parent(parent_context) {
        tracing::warn!(error = %error, "failed to attach remote OpenTelemetry parent context");
    }
}

fn build_tracer_provider(
    traces_url: &str,
    service_name: &str,
    service_version: &str,
    deployment_environment: &str,
) -> Result<SdkTracerProvider> {
    let exporter = SpanExporter::builder()
        .with_http()
        .with_http_client(Client::new())
        .with_endpoint(traces_url)
        .with_headers(parse_key_value_pairs(
            env::var("OTEL_EXPORTER_OTLP_HEADERS").ok(),
        ))
        .build()
        .context("failed to build OTLP span exporter")?;

    Ok(SdkTracerProvider::builder()
        .with_sampler(Sampler::ParentBased(Box::new(Sampler::AlwaysOn)))
        .with_resource(build_resource(
            service_name,
            service_version,
            deployment_environment,
        ))
        .with_batch_exporter(exporter)
        .build())
}

fn build_resource(
    service_name: &str,
    service_version: &str,
    deployment_environment: &str,
) -> Resource {
    let mut attributes = parse_resource_attributes(env::var("OTEL_RESOURCE_ATTRIBUTES").ok())
        .into_iter()
        .map(|(key, value)| KeyValue::new(key, value))
        .collect::<Vec<_>>();
    attributes.push(KeyValue::new("service.name", service_name.to_owned()));
    attributes.push(KeyValue::new("service.version", service_version.to_owned()));
    attributes.push(KeyValue::new(
        "deployment.environment",
        deployment_environment.to_owned(),
    ));

    Resource::builder().with_attributes(attributes).build()
}

fn is_enabled() -> bool {
    matches!(
        env::var("OTEL_ENABLED")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase()),
        Some(value) if matches!(value.as_str(), "1" | "true" | "yes" | "on")
    )
}

fn resolve_traces_url() -> String {
    if let Some(explicit) = env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return explicit;
    }

    if let Some(base) = env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_owned())
        .filter(|value| !value.is_empty())
    {
        return format!("{base}/v1/traces");
    }

    DEFAULT_OTLP_HTTP_TRACES_URL.to_owned()
}

fn env_var_or_default(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn parse_resource_attributes(raw: Option<String>) -> HashMap<String, String> {
    parse_key_value_pairs(raw)
}

fn parse_key_value_pairs(raw: Option<String>) -> HashMap<String, String> {
    let mut pairs = HashMap::new();
    let source = raw.unwrap_or_default();
    for entry in source.split(',') {
        let item = entry.trim();
        if item.is_empty() {
            continue;
        }

        let Some((key, value)) = item.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }

        pairs.insert(key.to_owned(), value.to_owned());
    }
    pairs
}
