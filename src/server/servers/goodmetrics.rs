use crate::proto::metrics::pb::metrics_server::Metrics;
use crate::proto::metrics::pb::{MetricsRequest, MetricsReply};

#[derive(Debug, Default)]
pub(crate) struct GoodMetricsServer {}

#[tonic::async_trait]
impl Metrics for GoodMetricsServer {
    async fn send_metrics(
        &self,
        request: tonic::Request<MetricsRequest>,
    ) -> Result<tonic::Response<MetricsReply>, tonic::Status> {
        log::debug!("request: {:?}", request);

        return Err(tonic::Status::unimplemented("it is not implemented"));
    }
}
