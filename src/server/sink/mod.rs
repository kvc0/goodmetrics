use crate::proto::metrics::pb::Datum;

pub mod metricssendqueue;
mod opentelemetry_sink;
pub mod postgres_sink;
pub mod sink_error;

pub trait MetricsSink: Send {
    fn drain(&self, metrics: Vec<Datum>) -> Result<String, ErrorCode>;
}

#[derive(Debug)]
pub enum ErrorCode {
    QueueFull,
}
