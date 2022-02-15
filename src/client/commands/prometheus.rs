pub async fn poll_prometheus(poll_endpoint: String, interval_seconds: u32) {
    log::info!("polling: {} every: {}s", poll_endpoint, interval_seconds);
}
