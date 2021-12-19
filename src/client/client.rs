use config::config::{get_args, Subcommand};
use metrics::{WorkflowMetric, MetricsRequest};
use structopt::StructOpt;

pub mod metrics {
    tonic::include_proto!("goodmetrics");
}
mod config;

#[tokio::main]
async fn main() {
    let args = get_args();

    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(args.log_level)
            .default_write_style_or("always"),
    )
    .init();

//    log::info!("{}", serde_json::to_string(&Measurement { name: "asd".to_string(), measurement_type: Some(metrics::measurement::MeasurementType::Gauge(Gauge { value: 42.0 })) }).unwrap());

    match args.command {
        Subcommand::Send { metrics } => {
            for metric in &metrics {
                log::info!("parsed: {}", serde_json::to_string_pretty(&metric).unwrap());
            }
            let mut client = match metrics::metrics_client::MetricsClient::connect("http://localhost:9573").await {
                Ok(c) => {
                    log::debug!("connected: {:?}", c);
                    c
                },
                Err(e) => {
                    log::error!("failed to connect: {:?}", e);
                    std::process::exit(2);
                },
            };

            let result = client.send_metrics(MetricsRequest {
                shared_dimensions: vec!(),
                metrics: metrics,
            }).await;
            match result {
                Ok(r) => {
                    log::info!("result: {:?}", r);
                },
                Err(e) => {
                    log::error!("error: {:?}", e);
                },
            }
        },
    }
}
