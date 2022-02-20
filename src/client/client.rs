use commands::{poll_prometheus::poll_prometheus, send_metrics::send_metrics};
use config::{cli_config::get_args, options::Subcommand};

pub mod metrics {
    tonic::include_proto!("goodmetrics");
}
mod commands;
mod config;
mod prometheus;

#[tokio::main]
async fn main() {
    let args = get_args();

    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(&args.log_level)
            .default_write_style_or("always"),
    )
    .init();

    match args.command {
        Subcommand::Send { metrics } => send_metrics(metrics, &args.goodmetrics_server).await,
        Subcommand::PollPrometheus {
            poll_endpoint,
            interval_seconds,
            bonus_dimensions,
            prefix,
        } => {
            poll_prometheus(
                poll_endpoint,
                interval_seconds,
                bonus_dimensions,
                underscore_suffix(prefix),
            )
            .await
        }
    }
}

fn underscore_suffix(s: String) -> String {
    if s.is_empty() {
        return s;
    }
    if s.ends_with('_') {
        return s;
    }
    s + "_"
}
