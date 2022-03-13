use std::collections::HashMap;

use crate::proto::channel_connection::get_channel;
use crate::proto::metrics::pb::{metrics_client::MetricsClient, Datum, MetricsRequest};

pub async fn send_metrics(metrics: Vec<Datum>, endpoint: &str) {
    for metric in &metrics {
        log::debug!("parsed: {}", serde_json::to_string_pretty(&metric).unwrap());
    }

    let mut client = match get_channel(endpoint).await {
        Ok(channel) => {
            log::debug!("connected: {:?}", channel);
            MetricsClient::new(channel)
        }
        Err(e) => {
            log::error!("failed to connect: {:?}", e);
            std::process::exit(2);
        }
    };

    let result = client
        .send_metrics(MetricsRequest {
            shared_dimensions: HashMap::new(),
            metrics,
        })
        .await;
    match result {
        Ok(r) => {
            log::info!("result: {:?}", r);
        }
        Err(e) => {
            log::error!("error: {:?}", e);
        }
    }
}
