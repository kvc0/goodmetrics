use crate::proto::metrics::pb::WorkflowMetric;

pub mod postgres_sink;
pub mod metricssendqueue;

pub trait MetricsSink: Send {
    fn drain(&self, metrics: Vec<WorkflowMetric>) -> Result<String, ErrorCode>;
}

#[derive(Debug)]
pub enum ErrorCode {
    QueueFull
}
