use tracing::*;
use tracing_slog::TracingSlogDrain;
use tracing_subscriber::{fmt::Subscriber as TracingSubscriber, EnvFilter as TracingEnvFilter};

mod nested {
    pub fn log_something(slogger: &slog::Logger) {
        slog::info!(slogger, "logged using slog from a nested module");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let drain = TracingSlogDrain;
    let slogger = slog::Logger::root(drain, slog::o!());

    TracingSubscriber::builder()
        .with_env_filter(TracingEnvFilter::from_default_env())
        .with_file(true)
        .with_line_number(true)
        .json()
        .init();

    info!("json tracing example");

    slog::info!(slogger, "logged using slog"; "arg1" => "val1", "arg2"=>"val2");
    nested::log_something(&slogger);

    log::info!("logged using plain log");

    Ok(())
}
