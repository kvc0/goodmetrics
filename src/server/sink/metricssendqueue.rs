use super::MetricsSink;


#[derive(Debug)]
pub struct MetricsSendQueue {
}

impl MetricsSink for MetricsSendQueue {
    fn drain(&self, metrics: Vec<crate::proto::metrics::pb::WorkflowMetric>) -> Result<String, super::Error> {
        log::debug!("collecting: {:?}", metrics);

        Ok("collected".to_string())
    }
}
