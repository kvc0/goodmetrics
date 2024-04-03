use tokio::sync::broadcast::{Receiver, Sender};

use communication::proto::goodmetrics::Datum;

use super::{ErrorCode, MetricsSink};

#[derive(Debug, Clone)]
pub struct MetricsSendQueue {
    pub tx: Sender<Vec<Datum>>,
}

pub struct MetricsReceiveQueue {
    pub rx: Receiver<Vec<Datum>>,
}

impl MetricsSink for MetricsSendQueue {
    fn drain(&self, metrics: Vec<Datum>) -> Result<String, super::ErrorCode> {
        match self.tx.send(metrics) {
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
        let (tx, rx) = tokio::sync::broadcast::channel(4096);

        (MetricsSendQueue { tx }, MetricsReceiveQueue { rx })
    }
}

impl MetricsReceiveQueue {
    pub async fn recv(&mut self) -> Option<Vec<Datum>> {
        match self.rx.recv().await {
            Ok(some_datums) => Some(some_datums),
            Err(error) => {
                log::error!("failed to receive some datums: {:?}", error);
                None
            }
        }
    }
}
