use tokio_postgres::{NoTls, tls::NoTlsStream, Socket};

use crate::proto::metrics::pb::WorkflowMetric;

use super::metricssendqueue::MetricsReceiveQueue;

pub struct PostgresSender {
    pub client: tokio_postgres::Client,
    pub connection: tokio_postgres::Connection<Socket, NoTlsStream>,
    rx: MetricsReceiveQueue,
}

#[derive(Debug)]
pub struct Error {
    message: String,
    inner: tokio_postgres::Error,
}

impl PostgresSender {
    pub async fn new_connection(connection_string: &String, rx: MetricsReceiveQueue) -> Result<PostgresSender, Error> {
        log::debug!("new_connection: {:?}", connection_string);
        let connection_future = tokio_postgres::connect(&connection_string, NoTls);
        match connection_future.await {
            Ok(connection_pair) => {
                let (client, connection) = connection_pair;
                Ok(PostgresSender {
                    client,
                    connection,
                    rx,
                })
            },
            Err(err) => {
                Err(Error { message: "could not connect".to_string(), inner: err })
            },
        }
    }

    pub async fn consume_stuff(&mut self) {
        log::info!("started consumer");
        for batch in self.rx.rx.recv().await {
            log::debug!("consumed: {:?}", batch)
        }
        log::info!("ended consumer");
    }
}
