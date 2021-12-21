use crate::proto::metrics::pb::Datum;

pub mod postgres_sink;
pub mod metricssendqueue;

pub trait MetricsSink: Send {
    fn drain(&self, metrics: Vec<Datum>) -> Result<String, ErrorCode>;
}

#[derive(Debug)]
pub enum ErrorCode {
    QueueFull
}
