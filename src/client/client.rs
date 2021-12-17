use tonic::{transport::Server, Request, Response, Status};

use metrics::metrics_client::MetricsClient;

pub mod metrics {
    tonic::include_proto!("goodmetrics");
}


#[tokio::main]
async fn main() {
    println!("Hello, world!");
}
