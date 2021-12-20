use tokio::sync::mpsc::{self, Sender, Receiver};

use crate::proto::metrics::pb::WorkflowMetric;

use super::{MetricsSink, ErrorCode};


#[derive(Debug, Clone)]
pub struct MetricsSendQueue {
    tx: Sender<Vec<WorkflowMetric>>,
}

pub struct MetricsReceiveQueue {
    pub rx: Receiver<Vec<WorkflowMetric>>,
}

impl MetricsSink for MetricsSendQueue {
    fn drain(&self, metrics: Vec<WorkflowMetric>) -> Result<String, super::ErrorCode> {
        match self.tx.try_send(metrics) {
            Ok(_) => {
                Ok("collected".to_string())
            },
            Err(e) => {
                Err(ErrorCode::QueueFull)
            },
        }
    }
}

impl MetricsSendQueue {
    pub fn new() -> (MetricsSendQueue, MetricsReceiveQueue) {
        let (mut tx, mut rx) = mpsc::channel(100);

        (MetricsSendQueue {tx}, MetricsReceiveQueue {rx})
    }    
}
