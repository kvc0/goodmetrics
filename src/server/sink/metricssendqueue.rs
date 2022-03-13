use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::proto::goodmetrics::Datum;

use super::{ErrorCode, MetricsSink};

#[derive(Debug, Clone)]
pub struct MetricsSendQueue {
    tx: Sender<Vec<Datum>>,
}

pub struct MetricsReceiveQueue {
    pub rx: Receiver<Vec<Datum>>,
}

impl MetricsSink for MetricsSendQueue {
    fn drain(&self, metrics: Vec<Datum>) -> Result<String, super::ErrorCode> {
        match self.tx.try_send(metrics) {
            Ok(_) => Ok("collected".to_string()),
            Err(e) => {
                log::warn!("queue error: {:?}", e);
                Err(ErrorCode::QueueFull)
            }
        }
    }
}

impl MetricsSendQueue {
    pub fn new() -> (MetricsSendQueue, MetricsReceiveQueue) {
        let (tx, rx) = mpsc::channel(100);

        (MetricsSendQueue { tx }, MetricsReceiveQueue { rx })
    }
}

impl MetricsReceiveQueue {
    pub async fn recv(&mut self) -> Option<Vec<Datum>> {
        self.rx.recv().await
    }
}
