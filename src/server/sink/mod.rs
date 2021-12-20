use crate::proto::metrics::pb::WorkflowMetric;

pub mod postgres_sink;
pub mod metricssendqueue;

pub trait MetricsSink: Send {
    fn drain(&self, metrics: Vec<WorkflowMetric>) -> Result<String, Error>;
}

#[derive(Debug)]
pub enum Error {
    QueueFull
}
