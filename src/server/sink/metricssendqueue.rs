use tokio::sync::mpsc::{self, Sender, Receiver};

use crate::proto::metrics::pb::WorkflowMetric;

use super::MetricsSink;


#[derive(Debug, Clone)]
pub struct MetricsSendQueue {
    tx: Sender<Vec<WorkflowMetric>>,
}

pub struct MetricsReceiveQueue {
    pub rx: Receiver<Vec<WorkflowMetric>>,
}

impl MetricsSink for MetricsSendQueue {
    fn drain(&self, metrics: Vec<WorkflowMetric>) -> Result<String, super::Error> {
        log::debug!("collecting: {:?}", metrics);

        Ok("collected".to_string())
    }
}

impl MetricsSendQueue {
    pub fn new() -> (MetricsSendQueue, MetricsReceiveQueue) {
        let (mut tx, mut rx) = mpsc::channel(100);

        (MetricsSendQueue {tx}, MetricsReceiveQueue {rx})
    }    
}
