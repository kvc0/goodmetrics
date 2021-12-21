use std::{collections::HashMap, any::Any};

use tokio_postgres::{NoTls, tls::NoTlsStream, Socket, Connection};

use crate::proto::metrics::pb::{Datum, Dimension};

use super::metricssendqueue::MetricsReceiveQueue;

pub struct PostgresSender {
    pub client: tokio_postgres::Client,
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
                tokio::spawn(async move {
                    PostgresSender::run_connection(connection).await
                });
                Ok(PostgresSender {
                    client,
                    rx,
                })
            },
            Err(err) => {
                Err(Error { message: "could not connect".to_string(), inner: err })
            },
        }
    }

    async fn run_connection(connection: Connection<Socket, NoTlsStream>) {
        log::info!("spawning connection routine");
        match connection.await {
            Ok(_) => log::info!("connection routine ended"),
            Err(e) => log::info!("connection routine errored: {:?}", e),
        }
    }

    pub async fn consume_stuff(&mut self) {
        log::info!("started consumer");

        while let Some(batch) = self.rx.recv().await {
            log::debug!("consumed: {:?}", batch);

            // self.client.query("select (ARRAY[3, 2, 1]);", &[]).await
            // let query_future = self.client.query("SELECT $1::TEXT", &[&"hello world"]);
            let query_future = self.client.query("SELECT '123'", &[]);
            log::debug!("got future");
            let query_result = query_future.await;
            log::debug!("got result: {:?}", query_result);

            match query_result {
                Ok(number) => log::info!("well.. sql worked {:?}", number),
                Err(e) => log::error!("sql failed: {:?}", e),
            };
        }
        log::info!("ended consumer");
    }
}

struct MetricsBatch {
    tables: Vec<MetricsTable>,
}

struct MetricsTable {
    columns: HashMap<String, MetricsColumn>,
    rows: Vec<Datum>,
}

struct MetricsColumn {
    name: String,
}

impl MetricsColumn {
}

impl MetricsTable {
    fn get_sql_column_names(&self) -> impl Iterator<Item = &String> {
        let a = self.columns.values().map(|column| {
            &column.name
        });
        a
    }
}
