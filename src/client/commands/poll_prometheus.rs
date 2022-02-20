use std::{time::{Duration, SystemTime, UNIX_EPOCH}, collections::HashMap};

use tokio::time;

use crate::{prometheus::reader::read_prometheus, metrics::Dimension};

pub async fn poll_prometheus(poll_endpoint: String, interval_seconds: u32, bonus_dimensions: HashMap<String, Dimension>) {
    log::info!("polling: {} every: {}s", poll_endpoint, interval_seconds);
    let mut interval = time::interval(time::Duration::from_secs(interval_seconds as u64));
    loop {
        match read_prometheus(
            &poll_endpoint,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_else(|_| Duration::from_secs(0))
                .as_nanos() as u64,
                &bonus_dimensions,
        )
        .await
        {
            Ok(result) => {
                log::info!("here's some stuff: {:?}", result);
            }
            Err(error) => log::error!("error talking to prometheus endpoint: {:?}", error),
        }
        interval.tick().await;
    }
}
