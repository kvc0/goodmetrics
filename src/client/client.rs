use std::{collections::HashMap, sync::Arc};

use config::cli_config::{get_args, Subcommand};
use metrics::{metrics_client::MetricsClient, MetricsRequest};
use rustls::{ClientConfig, ServerCertVerifier};
use tonic::transport::Channel;

pub mod metrics {
    tonic::include_proto!("goodmetrics");
}
mod config;

#[tokio::main]
async fn main() {
    let args = get_args();

    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(&args.log_level)
            .default_write_style_or("always"),
    )
    .init();

    //    log::info!("{}", serde_json::to_string(&Measurement { name: "asd".to_string(), measurement_type: Some(metrics::measurement::MeasurementType::Gauge(Gauge { value: 42.0 })) }).unwrap());

    match args.command {
        Subcommand::Send { metrics } => {
            for metric in &metrics {
                log::info!("parsed: {}", serde_json::to_string_pretty(&metric).unwrap());
            }

            let mut client = match get_client(&args.goodmetrics_server).await {
                Ok(c) => {
                    log::debug!("connected: {:?}", c);
                    c
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
    }
}

async fn get_client(endpoint: &str) -> Result<MetricsClient<Channel>, Box<dyn std::error::Error>> {
    // FIXME: set up optional no-issuer-validation. Can keep
    //        hostname validation I think though?
    let mut config = ClientConfig::new();
    config
        .dangerous()
        .set_certificate_verifier(Arc::new(StupidVerifier {}));
    config.alpn_protocols = vec!["h2".to_string().as_bytes().to_vec()];

    let tls = tonic::transport::ClientTlsConfig::new().rustls_client_config(config);

    let channel = Channel::from_shared(endpoint.to_string())?
        .tls_config(tls)?
        .connect()
        .await?;
    let client = MetricsClient::new(channel);

    Ok(client)
}

struct StupidVerifier {}

impl ServerCertVerifier for StupidVerifier {
    fn verify_server_cert(
        &self,
        _roots: &rustls::RootCertStore,
        _presented_certs: &[rustls::Certificate],
        _dns_name: webpki::DNSNameRef,
        _ocsp_response: &[u8],
    ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
        // roflmao
        Ok(rustls::ServerCertVerified::assertion())
    }
}
