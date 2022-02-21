use std::collections::HashMap;

use crate::metrics::{Datum, MetricsRequest};

use super::client_connection::get_client;

pub async fn send_metrics(metrics: Vec<Datum>, endpoint: &str) {
    for metric in &metrics {
        log::info!("parsed: {}", serde_json::to_string_pretty(&metric).unwrap());
    }

    let mut client = match get_client(endpoint).await {
        Ok(c) => {
            log::debug!("connected: {:?}", c);
            c
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