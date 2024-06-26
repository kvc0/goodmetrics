use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::time;

use communication::{
    get_channel,
    proto::goodmetrics::{metrics_client::MetricsClient, Dimension, MetricsRequest},
};

use crate::prometheus::reader::read_prometheus;

pub async fn poll_prometheus(
    poll_endpoint: String,
    interval_seconds: u32,
    bonus_dimensions: HashMap<String, Dimension>,
    table_prefix: String,
    goodmetrics_endpoint: &str,
    insecure_goodmetrics: bool,
) {
    log::info!("polling: {} every: {}s", poll_endpoint, interval_seconds);
    let mut interval = time::interval(time::Duration::from_secs(interval_seconds as u64));

    loop {
        match read_prometheus(
            &poll_endpoint,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_else(|_| Duration::from_secs(0))
                .as_nanos() as u64,
            &table_prefix,
        )
        .await
        {
            Ok(datums) => {
                log::debug!("lines: {:?}", datums);

                match get_channel(goodmetrics_endpoint, insecure_goodmetrics).await {
                    Ok(channel) => {
                        log::debug!("connected: {}", goodmetrics_endpoint);
                        let mut client = MetricsClient::new(channel);
                        let result = client
                            .send_metrics(MetricsRequest {
                                shared_dimensions: bonus_dimensions.clone(),
                                metrics: datums,
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
                    Err(e) => {
                        log::error!("failed to connect to goodmetrics: {:?}", e);
                    }
                };
            }
            Err(error) => log::error!("error talking to prometheus endpoint: {:?}", error),
        }
        interval.tick().await;
    }
}
