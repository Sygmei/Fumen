use anyhow::{Context, Result, anyhow};
use fumen_processor::{telemetry, worker};

const DEFAULT_PROCESSOR_THREAD_STACK_BYTES: usize = 16 * 1024 * 1024;

fn main() -> Result<()> {
    let stack_size = processor_thread_stack_size();
    let processor_thread = std::thread::Builder::new()
        .name("fumen-processor-runtime".to_owned())
        .stack_size(stack_size)
        .spawn(move || run_processor(stack_size))
        .context("failed to spawn processor runtime thread")?;

    processor_thread.join().map_err(|panic| {
        anyhow!(
            "processor runtime thread panicked: {}",
            panic_message(panic)
        )
    })?
}

fn run_processor(stack_size: usize) -> Result<()> {
    let _telemetry = telemetry::init()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(stack_size)
        .enable_all()
        .build()
        .context("failed to build Tokio runtime")?;
    let result = runtime.block_on(worker::run());
    drop(runtime);
    result
}

fn processor_thread_stack_size() -> usize {
    std::env::var("PROCESSOR_THREAD_STACK_BYTES")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value >= 1024 * 1024)
        .unwrap_or(DEFAULT_PROCESSOR_THREAD_STACK_BYTES)
}

fn panic_message(panic: Box<dyn std::any::Any + Send + 'static>) -> String {
    if let Some(message) = panic.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = panic.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_owned()
    }
}
