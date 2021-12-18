use tonic::{transport::Server, Request, Response, Status};

use metrics::metrics_client::MetricsClient;
use metrics::{Histogram, Gauge, Dimension, Bucket};

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
    #[structopt(about = "Send a measurement")]
    Send {
        #[structopt(subcommand)]
        operation: SendCommand,
    },
}

#[derive(StructOpt)]
enum SendCommand {
    #[structopt(about = "Send some gauge values")]
    Gauge {
        #[structopt(parse(try_from_str = serde_json::from_str))]
        gauges: Vec<Gauge>,
    }
}

#[tokio::main]
async fn main() {
    // metrics::metrics_client::MetricsClient::connect("");
    let args = Client::from_args();

    let log_level = if args.verbose { "debug" } else { "info" };

    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(log_level)
            .default_write_style_or("always"),
    )
    .init();


    match args.command {
        Subcommand::Send { operation } => {
            match operation {
                SendCommand::Gauge { gauges } => {
                    for gauge in gauges {
                        // log::info!("dimension: {name:>16} -> {value:16}", name=gauge.name, value=gauge.value);
                        log::info!("gauge: {value}", value=gauge.value);
                    }
                },
            }
        },
    }
}
