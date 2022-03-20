use std::sync::Arc;

use rustls::{ClientConfig, ServerCertVerifier};
use tonic::transport::Channel;

pub async fn get_channel(endpoint: &str) -> Result<Channel, Box<dyn std::error::Error>> {
    let mut channel = Channel::from_shared(endpoint.to_string())?;

    if endpoint.starts_with("https://") {
        let mut config = ClientConfig::new();
        config
            .dangerous()
            .set_certificate_verifier(Arc::new(StupidVerifier {}));
        config.alpn_protocols = vec!["h2".to_string().as_bytes().to_vec()];
        let tls = tonic::transport::ClientTlsConfig::new().rustls_client_config(config);
        channel = channel.tls_config(tls)?;
    }

    Ok(channel.connect().await?)
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
