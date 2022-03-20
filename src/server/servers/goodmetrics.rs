use tonic::Response;

use crate::proto::goodmetrics::metrics_server::Metrics;
use crate::proto::goodmetrics::{MetricsReply, MetricsRequest};
use crate::server::sink::metricssendqueue::MetricsSendQueue;
use crate::server::sink::MetricsSink;

#[derive(Debug)]
pub struct GoodMetricsServer {
    pub metrics_sink: MetricsSendQueue,
}

#[tonic::async_trait]
impl Metrics for GoodMetricsServer {
    async fn send_metrics(
        &self,
        request: tonic::Request<MetricsRequest>,
    ) -> Result<tonic::Response<MetricsReply>, tonic::Status> {
        log::trace!("request: {:?}", request);

        // We shared the dimensions across the wire, but here we'll keep it simple and just spew it all across each datum
        let mut request = request.into_inner();
        request
            .metrics
            .iter_mut()
            .for_each(|datum| datum.dimensions.extend(request.shared_dimensions.clone()));
        let queue_result = self.metrics_sink.drain(request.metrics);

        match queue_result {
            Ok(result) => {
                log::debug!("result: {:?}", result);

                Ok(Response::new(MetricsReply {}))
            }
            Err(e) => match e {
                crate::server::sink::ErrorCode::QueueFull => Err(
                    tonic::Status::resource_exhausted("No space left in the send buffer"),
                ),
            },
        }
    }
}
