use tokio_postgres::{NoTls, tls::NoTlsStream, Socket};

use crate::proto::metrics::pb::WorkflowMetric;

use super::MetricsSink;

pub struct PostgresSink {
    pub client: tokio_postgres::Client,
    pub connection: tokio_postgres::Connection<Socket, NoTlsStream>,
}

impl MetricsSink for PostgresSink {
    fn drain(&self, metrics: Vec<WorkflowMetric>) -> Result<String, super::Error> {
        for m in metrics {
            log::info!("sinking metrics {:?}", m);
        }

        return Ok("done".to_string());
    }
}

#[derive(Debug)]
pub struct Error {
    message: String,
    inner: tokio_postgres::Error,
}

impl PostgresSink {
    pub async fn new_connection(connection_string: &String) -> Result<PostgresSink, Error> {
        log::debug!("new_connection: {:?}", connection_string);
        let connection_future = tokio_postgres::connect(&connection_string, NoTls);
        match connection_future.await {
            Ok(connection_pair) => {
                let (client, connection) = connection_pair;
                Ok(PostgresSink {
                    client,
                    connection,
                })
            },
            Err(err) => {
                Err(Error { message: "could not connect".to_string(), inner: err })
            },
        }
    }
}
