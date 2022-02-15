use std::sync::Arc;

use rustls::{ClientConfig, ServerCertVerifier};
use tonic::transport::Channel;

use crate::metrics::metrics_client::MetricsClient;

pub async fn get_client(
    endpoint: &str,
) -> Result<MetricsClient<Channel>, Box<dyn std::error::Error>> {
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
