use commands::send_metrics::send_metrics;
use config::{cli_config::get_args, options::Subcommand};

pub mod metrics {
    tonic::include_proto!("goodmetrics");
}
mod commands;
mod config;

#[tokio::main]
async fn main() {
    let args = get_args();

    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(&args.log_level)
            .default_write_style_or("always"),
    )
    .init();

    //    log::info!("{}", serde_json::to_string(&Measurement { name: "asd".to_string(), measurement_type: Some(metrics::measurement::MeasurementType::Gauge(Gauge { value: 42.0 })) }).unwrap());

    match args.command {
        Subcommand::Send { metrics } => send_metrics(metrics, &args.goodmetrics_server).await,
    }
}
