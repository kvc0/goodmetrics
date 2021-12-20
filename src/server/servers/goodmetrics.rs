use tonic::Response;

use crate::proto::metrics::pb::metrics_server::Metrics;
use crate::proto::metrics::pb::{MetricsRequest, MetricsReply};
use crate::sink::MetricsSink;
use crate::sink::metricssendqueue::MetricsSendQueue;

#[derive(Debug)]
pub(crate) struct GoodMetricsServer {
    pub metrics_sink: MetricsSendQueue,
}

#[tonic::async_trait]
impl Metrics for GoodMetricsServer {
    async fn send_metrics(
        &self,
        request: tonic::Request<MetricsRequest>,
    ) -> Result<tonic::Response<MetricsReply>, tonic::Status> {
        log::trace!("request: {:?}", request);

        let queue_result = self.metrics_sink.drain(request.into_inner().metrics);

        match queue_result {
            Ok(result) => {
                log::debug!("result: {:?}", result);

                Ok(Response::new(MetricsReply {}))
            },
            Err(e) => {
                match e {
                    crate::sink::Error::QueueFull => {
                        Err(tonic::Status::resource_exhausted("No space left in the send buffer"))
                    },
                }
            },
        }
    }
}
