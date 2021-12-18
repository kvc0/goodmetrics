use tonic::{transport::Server, Request, Response, Status};

use metrics::metrics_client::MetricsClient;
use metrics::{Measurement, Histogram, Gauge, Dimension, Bucket, WorkflowMetric, MetricsRequest};

use structopt::StructOpt;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};


pub mod metrics {
    tonic::include_proto!("goodmetrics");
}

#[derive(StructOpt)]
#[structopt(about = "Good metrics: I never said they were great.")]
struct Client {
    #[structopt(name = "verbose", global = true, long, short)]
    verbose: bool,

    #[structopt(subcommand)]
    command: Subcommand,
}

#[derive(StructOpt)]
enum Subcommand {
    #[structopt(about = "Send measurements")]
    Send {
        #[structopt(parse(try_from_str = serde_json::from_str))]
        metrics: Vec<WorkflowMetric>,
    },
}

#[tokio::main]
async fn main() {
    let args = Client::from_args();

    let log_level = if args.verbose { "debug" } else { "info" };

    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(log_level)
            .default_write_style_or("always"),
    )
    .init();

//    log::info!("{}", serde_json::to_string(&Measurement { name: "asd".to_string(), measurement_type: Some(metrics::measurement::MeasurementType::Gauge(Gauge { value: 42.0 })) }).unwrap());

    match args.command {
        Subcommand::Send { metrics } => {
            for metric in &metrics {
                log::info!("parsed: {}", serde_json::to_string_pretty(&metric).unwrap());
            }
            let mut client = match metrics::metrics_client::MetricsClient::connect("https://localhost:9753").await {
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
