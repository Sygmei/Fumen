use anyhow::{Context, Result};
use fumen_processor::{telemetry, worker};

fn main() -> Result<()> {
    let _telemetry = telemetry::init()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build Tokio runtime")?;
    let result = runtime.block_on(worker::run());
    drop(runtime);
    result
}
