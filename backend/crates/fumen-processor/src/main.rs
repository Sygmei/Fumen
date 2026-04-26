use anyhow::{Context, Result, anyhow};
use fumen_processor::{telemetry, worker};

const DEFAULT_PROCESSOR_THREAD_STACK_BYTES: usize = 16 * 1024 * 1024;

fn main() -> Result<()> {
    let cli = match parse_processor_cli(std::env::args())? {
        ParsedCli::Run(cli) => cli,
        ParsedCli::Help { usage } => {
            println!("{usage}");
            return Ok(());
        }
    };
    let stack_size = processor_thread_stack_size();
    let processor_thread = std::thread::Builder::new()
        .name("fumen-processor-runtime".to_owned())
        .stack_size(stack_size)
        .spawn(move || run_processor(stack_size, cli))
        .context("failed to spawn processor runtime thread")?;

    processor_thread.join().map_err(|panic| {
        anyhow!(
            "processor runtime thread panicked: {}",
            panic_message(panic)
        )
    })?
}

fn run_processor(stack_size: usize, cli: ProcessorCli) -> Result<()> {
    let _telemetry = telemetry::init()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(stack_size)
        .enable_all()
        .build()
        .context("failed to build Tokio runtime")?;
    let result = runtime.block_on(worker::run(cli.run_mode));
    drop(runtime);
    result
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProcessorCli {
    run_mode: worker::RunMode,
}

enum ParsedCli {
    Run(ProcessorCli),
    Help { usage: String },
}

fn parse_processor_cli<I>(args: I) -> Result<ParsedCli>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let program = args
        .next()
        .unwrap_or_else(|| "/usr/local/bin/fumen-processor".to_owned());
    let mut oneshot = false;
    let mut job_id: Option<String> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--oneshot" => {
                oneshot = true;
            }
            "--job-id" => {
                let value = args.next().ok_or_else(|| {
                    anyhow!(
                        "missing value for --job-id\n\n{}",
                        processor_usage(&program)
                    )
                })?;
                if value.trim().is_empty() {
                    return Err(anyhow!(
                        "--job-id requires a non-empty value\n\n{}",
                        processor_usage(&program)
                    ));
                }
                if job_id.replace(value).is_some() {
                    return Err(anyhow!(
                        "--job-id can only be provided once\n\n{}",
                        processor_usage(&program)
                    ));
                }
                oneshot = true;
            }
            "-h" | "--help" => {
                return Ok(ParsedCli::Help {
                    usage: processor_usage(&program),
                });
            }
            _ => {
                return Err(anyhow!(
                    "unknown argument: {arg}\n\n{}",
                    processor_usage(&program)
                ));
            }
        }
    }

    let run_mode = if oneshot {
        worker::RunMode::Oneshot { music_id: job_id }
    } else {
        worker::RunMode::Service
    };

    Ok(ParsedCli::Run(ProcessorCli { run_mode }))
}

fn processor_usage(program: &str) -> String {
    format!(
        "Usage: {program} [--oneshot] [--job-id <music-id>]\n\nOptions:\n  --oneshot           Claim at most one processing job and then exit.\n  --job-id <music-id> Claim a specific queued or expired-running job, then exit.\n  -h, --help          Show this help text."
    )
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

#[cfg(test)]
mod tests {
    use super::{ParsedCli, parse_processor_cli};
    use fumen_processor::worker::RunMode;

    #[test]
    fn parses_default_service_mode() {
        let cli = parse_processor_cli(vec!["fumen-processor".to_owned()]).unwrap();
        match cli {
            ParsedCli::Run(cli) => assert_eq!(cli.run_mode, RunMode::Service),
            ParsedCli::Help { .. } => panic!("expected run mode"),
        }
    }

    #[test]
    fn parses_oneshot_mode() {
        let cli = parse_processor_cli(vec![
            "fumen-processor".to_owned(),
            "--oneshot".to_owned(),
        ])
        .unwrap();
        match cli {
            ParsedCli::Run(cli) => {
                assert_eq!(cli.run_mode, RunMode::Oneshot { music_id: None })
            }
            ParsedCli::Help { .. } => panic!("expected run mode"),
        }
    }

    #[test]
    fn parses_targeted_job_mode() {
        let cli = parse_processor_cli(vec![
            "fumen-processor".to_owned(),
            "--job-id".to_owned(),
            "music-123".to_owned(),
        ])
        .unwrap();
        match cli {
            ParsedCli::Run(cli) => {
                assert_eq!(
                    cli.run_mode,
                    RunMode::Oneshot {
                        music_id: Some("music-123".to_owned())
                    }
                );
            }
            ParsedCli::Help { .. } => panic!("expected run mode"),
        }
    }
}
