use crate::{AppState, services::auth};
use anyhow::Result;
use axum::{
    body::Body,
    extract::MatchedPath,
    extract::State,
    http::{
        HeaderValue, Request, Response,
        header::{HeaderName, USER_AGENT},
    },
    middleware::Next,
    response::IntoResponse,
};
use opentelemetry::{global, trace::TraceContextExt};
use opentelemetry_http::HeaderExtractor;
use std::{time::Duration, time::Instant};
use tower_http::classify::ServerErrorsFailureClass;
use tracing::{Instrument, Span, field::Empty};
use tracing_opentelemetry::OpenTelemetrySpanExt;

const TRACER_NAME: &str = "fumen-backend";
pub(crate) const TRACE_ID_HEADER_NAME: HeaderName = HeaderName::from_static("x-trace-id");

#[derive(Clone, Debug)]
pub(crate) struct ServerErrorContext {
    pub(crate) message: String,
}

pub(crate) use fumen_core::telemetry::TelemetryGuard;

pub(crate) fn init() -> Result<TelemetryGuard> {
    let default_filter = if std::env::var("OTEL_ENABLED")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
    {
        "fumen_backend=info,fumen_core=info,processor=info,diesel=debug,tower_http=info"
    } else {
        "fumen_backend=info,fumen_core=info,processor=info,tower_http=info"
    };

    fumen_core::telemetry::init_tracing(
        default_filter,
        "fumen-backend",
        env!("CARGO_PKG_VERSION"),
        TRACER_NAME,
    )
}

pub(crate) fn make_operation_span(operation_id: &'static str, request: &Request<Body>) -> Span {
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
        otel.name = %operation_id,
        http.request.method = %request.method(),
        url.path = %request.uri().path(),
        url.query = Empty,
        http.route = Empty,
        user_agent.original = Empty,
        network.protocol.version = ?request.version(),
        http.response.status_code = Empty,
        error.type = Empty,
        operation.id = %operation_id,
        token.sub = Empty,
        token.sid = Empty,
        token.exp = Empty,
        token.iat = Empty,
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
    let trace_id = span.context().span().span_context().trace_id().to_string();
    tracing::info!(
        parent: span,
        latency_ms = latency.as_millis() as u64,
        status = response.status().as_u16(),
        trace_id = %trace_id,
        "http response"
    );
}

pub(crate) fn on_http_failure(response: &Response<Body>, latency: Duration, span: &Span) {
    let failure = format!(
        "{:?}",
        ServerErrorsFailureClass::StatusCode(response.status())
    );
    let trace_id = span.context().span().span_context().trace_id().to_string();
    let message = response
        .extensions()
        .get::<ServerErrorContext>()
        .map(|details| details.message.as_str())
        .unwrap_or("no AppError details attached");

    span.record("error.type", failure.as_str());
    tracing::error!(
        parent: span,
        latency_ms = latency.as_millis() as u64,
        status = response.status().as_u16(),
        trace_id = %trace_id,
        error.type = %failure,
        error.message = %message,
        "http request failed"
    );
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

pub(crate) async fn trace_operation_request(
    State((state, operation_id)): State<(AppState, &'static str)>,
    request: Request<Body>,
    next: Next,
) -> axum::response::Response {
    let span = make_operation_span(operation_id, &request);
    record_token_claims(&span, request.headers(), &state);
    let trace_id = span.context().span().span_context().trace_id().to_string();
    let start = Instant::now();
    let response = next.run(request).instrument(span.clone()).await;
    let latency = start.elapsed();
    let status = response.status();

    if status.is_server_error() {
        on_http_failure(&response, latency, &span);
    } else {
        on_http_response(&response, latency, &span);
    }

    let mut response = response.into_response();
    if !trace_id.is_empty() && !trace_id.chars().all(|ch| ch == '0') {
        if let Ok(trace_id) = HeaderValue::from_str(&trace_id) {
            response
                .headers_mut()
                .insert(TRACE_ID_HEADER_NAME, trace_id);
        }
    }

    response
}

fn record_token_claims(span: &Span, headers: &axum::http::HeaderMap, state: &AppState) {
    let Some(header_value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return;
    };

    let Ok(authorization) = header_value.to_str() else {
        return;
    };

    let Some(token) = authorization
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return;
    };

    let Ok(claims) = auth::decode_access_token_claims(token, &state.config.jwt_secret) else {
        return;
    };

    span.record("token.sub", claims.sub.as_str());
    span.record("token.sid", claims.sid.as_str());
    span.record("token.exp", claims.exp);
    span.record("token.iat", claims.iat);
}

#[macro_export]
macro_rules! op_get {
    ($state:expr, $path:expr, $handler:ident) => {
        axum::routing::get($handler).route_layer(axum::middleware::from_fn_with_state(
            ($state.clone(), stringify!($handler)),
            $crate::telemetry::trace_operation_request,
        ))
    };
}

#[macro_export]
macro_rules! op_post {
    ($state:expr, $path:expr, $handler:ident) => {
        axum::routing::post($handler).route_layer(axum::middleware::from_fn_with_state(
            ($state.clone(), stringify!($handler)),
            $crate::telemetry::trace_operation_request,
        ))
    };
}

#[macro_export]
macro_rules! op_patch {
    ($state:expr, $path:expr, $handler:ident) => {
        axum::routing::patch($handler).route_layer(axum::middleware::from_fn_with_state(
            ($state.clone(), stringify!($handler)),
            $crate::telemetry::trace_operation_request,
        ))
    };
}

#[macro_export]
macro_rules! op_delete {
    ($state:expr, $path:expr, $handler:ident) => {
        axum::routing::delete($handler).route_layer(axum::middleware::from_fn_with_state(
            ($state.clone(), stringify!($handler)),
            $crate::telemetry::trace_operation_request,
        ))
    };
}
