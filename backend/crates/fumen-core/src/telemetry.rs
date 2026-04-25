use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use opentelemetry::{KeyValue, global, trace::TracerProvider as _};
use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    Resource,
    propagation::TraceContextPropagator,
    trace::{Sampler, SdkTracerProvider},
};
use reqwest::blocking::Client;
use std::{collections::HashMap, env, fmt};
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    EnvFilter,
    fmt::{
        FmtContext,
        format::{FormatEvent, FormatFields, Writer},
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    util::SubscriberInitExt,
};

const DEFAULT_DEPLOYMENT_ENVIRONMENT: &str = "production";
const DEFAULT_OTLP_HTTP_TRACES_URL: &str = "http://127.0.0.1:4318/v1/traces";

struct PlainTextEventFormatter;

pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
}

impl<S, N> FormatEvent<S, N> for PlainTextEventFormatter
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let metadata = event.metadata();
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        write!(
            writer,
            "{} {:>5} {}: ",
            timestamp,
            metadata.level(),
            metadata.target()
        )?;
        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
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

pub fn init_tracing(
    default_filter: &str,
    default_service_name: &str,
    default_service_version: &str,
    tracer_name: &'static str,
) -> Result<TelemetryGuard> {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .event_format(PlainTextEventFormatter)
        .with_ansi(is_ansi_enabled());

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
    let service_name = env_var_or_default("OTEL_SERVICE_NAME", default_service_name);
    let service_version = env_var_or_default("OTEL_SERVICE_VERSION", default_service_version);
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
    let tracer = tracer_provider.tracer(tracer_name);

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

fn is_ansi_enabled() -> bool {
    matches!(
        env::var("LOG_ANSI")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase()),
        Some(value) if matches!(value.as_str(), "1" | "true" | "yes" | "on" | "always")
    ) || matches!(
        env::var("RUST_LOG_STYLE")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase()),
        Some(value) if matches!(value.as_str(), "always" | "yes" | "true" | "on")
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
