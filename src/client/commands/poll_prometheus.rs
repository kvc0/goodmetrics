use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::prometheus::reader::read_prometheus;

pub async fn poll_prometheus(poll_endpoint: String, interval_seconds: u32) {
    log::info!("polling: {} every: {}s", poll_endpoint, interval_seconds);
    match read_prometheus(
        &poll_endpoint,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_nanos() as u64,
    )
    .await
    {
        Ok(_result) => {}
        Err(error) => log::error!("error talking to prometheus endpoint: {:?}", error),
    }
}
