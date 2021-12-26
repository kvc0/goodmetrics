use std::{collections::{HashMap, HashSet}, any::Any, rc::Rc};

use cached::{proc_macro::cached, Cached};
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use serde::Serialize;
use serde_json::json;
use tokio_postgres::{NoTls, tls::NoTlsStream, Socket, Connection, GenericClient};

use crate::proto::metrics::pb::{Datum, Dimension, dimension::Value, measurement::Measurement};

use super::metricssendqueue::MetricsReceiveQueue;

pub struct PostgresSender {
    pub client: tokio_postgres::Client,
    rx: MetricsReceiveQueue,
}

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub inner: tokio_postgres::Error,
}

lazy_static! {
    static ref not_whitespace: Regex = Regex::new(r"[^\w]+").unwrap();
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
            let grouped_metrics = batch.iter()
                .sorted_by_key(|d| {&d.metric})
                .group_by(|d| {&d.metric});

            let grouped_rows = grouped_metrics.into_iter().map( |(metric, datums_iter)| {
                log::info!("metric: {:?}", metric);
                let datums: Vec<&Datum> = datums_iter.into_iter().collect();

                let rows = datums.iter().map(|datum| {
                    let mut m = serde_json::map::Map::new();

                    m["time"] = json!(datum.unix_nanos);
                    datum.dimensions.iter().for_each(|(name, dim)| {
                        match &dim.value {
                            Some(v) => {
                                m[name] = match v {
                                    Value::String(s) => json!(s),
                                    Value::Number(n) => json!(n),
                                }
                            },
                            None => {},
                        }
                    });

                    datum.measurements.iter().for_each(|(name, measurement)| {
                        match &measurement.measurement {
                            Some(measurement_kind) => {
                                m[name] = match measurement_kind {
                                    Measurement::Gauge(gauge) => json!(gauge),
                                    Measurement::StatisticSet(statistic_set) => json!(statistic_set),
                                    Measurement::Histogram(histogram) => json!(histogram),
                                }
                            },
                            None => {},
                        }
                    });
                    m
                });
                // FIXME: Why is this collecting into a map of groups of insanity?
                rows.collect_vec()
            });

            // COLUMN_EXISTS_CACHE.lock().unwrap().cache_set((metric, "c"), true);
            // let sql_insert_header = format!("INSERT INTO {table} select * from json_populate_recordset(null::mm, $1);\n",
            //     table = clean_id(metric),
            // );

            // batch.iter().map(|d| {
            //     d.metric
            // });
            // batch.iter().flat_map(|d| {
            //     d.dimensions.keys()
            // });

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

#[cached(name = "COLUMN_EXISTS_CACHE", size=8192, time=600, option = true)]
fn column_exists(table: String, column: String) -> Option<bool> {
    None
}

fn clean_id(s: &String) -> String {
    let l = s.to_lowercase();
    let a = not_whitespace.replace_all(&l, "_");
    a.to_string()
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
