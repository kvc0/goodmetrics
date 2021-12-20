use crate::proto::metrics::pb::metrics_server::Metrics;
use crate::proto::metrics::pb::{MetricsRequest, MetricsReply};
use crate::sink::postgres_sink::{PostgresSinkProvider, PostgresSink};

#[derive(Debug)]
pub(crate) struct GoodMetricsServer {
    pub metrics_sink_provider: PostgresSinkProvider,
}

#[tonic::async_trait]
impl Metrics for GoodMetricsServer {
    async fn send_metrics(
        &self,
        request: tonic::Request<MetricsRequest>,
    ) -> Result<tonic::Response<MetricsReply>, tonic::Status> {
        log::debug!("request: {:?}", request);

        let metrics_provider_result = self.metrics_sink_provider.check_out(GoodMetricsServer::use_sink, request).await;
        match metrics_provider_result {
            Ok(result) => {
                Err(tonic::Status::unimplemented(format!("it is not implemented: {}", result)))
            },
            Err(e) => {
                Err(tonic::Status::unimplemented(format!("it is not implemented and it is an error: {:?}", e)))
            },
        }
    }
}

impl GoodMetricsServer {
    async fn use_sink(sink: &PostgresSink, request: tonic::Request<MetricsRequest>) -> Result<String, crate::sink::postgres_sink::Error> {
        Ok("whee".to_string())
    }
}
