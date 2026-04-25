use anyhow::Result;
const TRACER_NAME: &str = "fumen-processor";

pub use fumen_core::telemetry::TelemetryGuard;

pub fn init() -> Result<TelemetryGuard> {
    let default_filter = if std::env::var("OTEL_ENABLED")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
    {
        "fumen_processor=info,diesel=debug"
    } else {
        "fumen_processor=info"
    };
    fumen_core::telemetry::init_tracing(
        default_filter,
        "fumen-processor",
        env!("CARGO_PKG_VERSION"),
        TRACER_NAME,
    )
}
