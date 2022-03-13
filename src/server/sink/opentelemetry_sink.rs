use tonic::transport::Channel;

use crate::proto::channel_connection::get_channel;
use crate::proto::opentelemetry::metrics_service::metrics_service_client::MetricsServiceClient;
use crate::server::sink::sink_error::StringError;

use super::{metricssendqueue::MetricsReceiveQueue, sink_error::SinkError};

pub struct OtelSender {
    rx: MetricsReceiveQueue,
    client: MetricsServiceClient<Channel>,
}

impl OtelSender {
    pub async fn new_connection(
        opentelemetry_endpoint: &str,
        rx: MetricsReceiveQueue,
    ) -> Result<OtelSender, SinkError> {
        let client = match get_channel(opentelemetry_endpoint).await {
            Ok(channel) => MetricsServiceClient::new(channel),
            Err(e) => {
                return Err(SinkError::StringError(StringError {
                    message: format!("Could not get a channel: {:?}", e),
                }))
            }
        };

        Ok(OtelSender {
            rx: rx,
            client: client,
        })
    }

    pub async fn consume_stuff(mut self) -> Result<u32, SinkError> {
        log::info!("started opentelemetry consumer");
        todo!()
    }
}
