use std::collections::HashMap;

use tonic::metadata::{Ascii, AsciiMetadataValue, MetadataValue};
use tonic::service::Interceptor;

use crate::proto::channel_connection::get_channel;
use crate::proto::goodmetrics::{metrics_client::MetricsClient, Datum, MetricsRequest};

struct AuthInterceptor {
    pub token: MetadataValue<Ascii>,
}
impl Interceptor for AuthInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        request
            .metadata_mut()
            .insert("authorization", self.token.to_owned());
        Ok(request)
    }
}

pub async fn send_metrics(
    metrics: Vec<Datum>,
    endpoint: &str,
    insecure: bool,
    auth_token: Option<String>,
) {
    for metric in &metrics {
        log::debug!("parsed: {}", serde_json::to_string_pretty(&metric).unwrap());
    }

    let mut client = match get_channel(endpoint, insecure).await {
        Ok(channel) => {
            log::debug!("connected: {}", endpoint);

            match auth_token {
                Some(token) => MetricsClient::with_interceptor(
                    channel,
                    AuthInterceptor {
                        token: AsciiMetadataValue::try_from(token)
                            .expect("invalid authorization token"),
                    },
                ),
                None => MetricsClient::with_interceptor(
                    channel,
                    AuthInterceptor {
                        token: AsciiMetadataValue::try_from("none")
                            .expect("invalid authorization token"),
                    },
                ),
            }
        }
        Err(e) => {
            log::error!("failed to connect: {:?}", e);
            std::process::exit(2);
        }
    };

    let result = client
        .send_metrics(MetricsRequest {
            shared_dimensions: HashMap::new(),
            metrics,
        })
        .await;
    match result {
        Ok(r) => {
            log::info!("result: {:?}", r);
        }
        Err(e) => {
            log::error!("error: {:?}", e);
        }
    }
}
