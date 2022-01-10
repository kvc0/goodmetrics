use std::{collections::{HashMap, HashSet, BTreeMap}, any::Any, rc::Rc, future::Future};

use cached::{proc_macro::cached, Cached};
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use serde::Serialize;
use serde_json::json;
use tokio_postgres::{NoTls, tls::NoTlsStream, Socket, Connection, GenericClient};

use crate::proto::metrics::pb::{Datum, Dimension, dimension::Value, measurement::Measurement, self};

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
                let mut schema: HashMap<String, &str> = HashMap::new();

                let rows = datums_iter.into_iter().map(|datum| {
                    // ------ I don't think this stuff is needed
                    // for (dimension_name, value) in datum.dimensions {
                    //     match column_exists(metric.clone(), dimension_name.clone()) {
                    //         Some(it_does) => {
                    //             if !it_does {
                    //                 let a = self.add_column(metric, &dimension_name, &"text".to_string());
                    //                 ddl_futures.push(a);
                    //             }
                    //         },
                    //         None => {
                    //             let a = self.add_column(metric, &dimension_name, &"text".to_string());
                    //             ddl_futures.push(a);
                    //         },
                    //     }
                    // }
                    // for (measurement_name, measurement) in datum.measurements {
                    //     match column_exists(metric.clone(), measurement_name.clone()) {
                    //         Some(it_does) => {
                    //             if !it_does {
                    //                 let a = self.add_column(metric, &measurement_name, sql_data_type(&measurement));
                    //                 ddl_futures.push(a);
                    //             }
                    //         },
                    //         None => {
                    //             let a = self.add_column(metric, &measurement_name, &"text".to_string());
                    //             ddl_futures.push(a);
                    //         },
                    //     }
                    // }
                    // ------ I don't think this stuff is needed

                    let mut m = serde_json::map::Map::new();

                    m["time"] = json!(datum.unix_nanos);
                    schema.insert("time".to_string(), "timestamptz");
                    datum.dimensions.iter().for_each(|(name, dim)| {
                        match &dim.value {
                            Some(v) => {
                                m[name] = match v {
                                    Value::String(s) => {
                                        schema.insert(name.clone(), "text");
                                        json!(s)
                                    },
                                    Value::Number(n) => {
                                        schema.insert(name.clone(), "bigint"); // 64 bit integer
                                        json!(n)
                                    },
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
                let rows_vector = rows.collect_vec();
                (metric, rows_vector, schema)
            });

            // why does rust make me define stupid temporary mutable variables just
            // to tell it the types, when it KNOWS the types and will freak out when I do this
            // incorrecty.
            let mut a: (&String, Vec<serde_json::map::Map<String, serde_json::Value>>, HashMap<String, &str>);
            for b in grouped_rows {
                a = b;
                let metric = a.0;
                let rows = a.1;
                let schema = a.2;

                // with data as (select * from json_to_recordset('[{"i": 1, "s": "s1", "a": "a1"},{"time":1600000000,"s":"s2","a":"a2","i":2,"k":"k2"}]') as (a text, i integer, s text, time int))
                //   insert into mm (a, i, s, time) select a, i, s, to_timestamp(time) as time from data;
                let sql_insert = format!("with data as (
                        select * from json_to_recordset($1) as ({schema_list})
                    )
                    insert into {table} ({name_list}) select to_timestamp(time), {name_list_without_time} as time
                    from data;",
                    table = clean_id(metric),
                    schema_list = "a int,b text,time timestamptz",
                    name_list = "a,b,time",
                    name_list_without_time = "a,b"
                );
            }


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

    async fn add_column(&mut self, table_name: &String, column_name: &String, data_type: &str) -> Result<u64, tokio_postgres::Error> {
        let a = self.client.execute(
        &format!(
                "alter table {table} add column {column} {data_type}",
                table=table_name,
                column=column_name,
                data_type=data_type,
            ),
            &[],
        ).await;
        if a.is_ok() {
            COLUMN_EXISTS_CACHE.lock().unwrap().cache_set((table_name.clone(), column_name.clone()), true);
        }
        a
    }
}

#[cached(name = "COLUMN_EXISTS_CACHE", size=8192, time=600, option = true)]
fn column_exists(table: String, column: String) -> Option<bool> {
    None
}

fn sql_data_type(measurement: &pb::Measurement) -> &'static str {
    match measurement.measurement.as_ref().unwrap() {
        Measurement::Gauge(_) => "double precision",
        Measurement::StatisticSet(_) => "statistic_set",
        Measurement::Histogram(h) => "histogram",
    }
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
