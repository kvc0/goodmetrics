use std::{collections::{HashMap, HashSet, BTreeMap}, any::Any, rc::Rc, future::Future};

use cached::{proc_macro::cached, Cached};
use futures::pin_mut;
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use serde::Serialize;
use serde_json::json;
use thiserror::Error;
use tokio_postgres::{NoTls, tls::NoTlsStream, Socket, Connection, GenericClient, types::{Type, ToSql}, CopyInSink, binary_copy::BinaryCopyInWriter};

use crate::proto::metrics::pb::{Datum, Dimension, dimension::Value, measurement::Measurement, self, StatisticSet};

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

    pub async fn consume_stuff(&mut self) -> Result<u32, SinkError> {
        log::info!("started consumer");

        while let Some(batch) = self.rx.recv().await {
            let grouped_metrics = group_metrics(batch);
            // OOPS!!! I made all this stuff work on the grouped metrics instead of per-group.
            // Each group is a table :facepalm: I'm tired.

            let dimension_types = get_dimension_type_map(&grouped_metrics);
            let measurement_types = get_measurement_type_map(&grouped_metrics);

            let all_column_types = get_all_column_types(&dimension_types, &measurement_types);

            let tx = self.client.transaction().await?;
            let sink = tx.copy_in(&format!("copy mm (time, ss) from stdin with format binary")).await?;
            let writer = BinaryCopyInWriter::new(sink, &all_column_types);
            let num_written = write(writer, &dimension_types, &measurement_types, datums).await?;
            tx.commit().await?;
        }
        log::info!("ended consumer");
        Ok(1)
        // {
        //     let grouped_rows = grouped_metrics.into_iter().map( |(metric, datums_iter)| {
        //         log::info!("metric: {:?}", metric);
        //         let mut schema: HashMap<String, &str> = HashMap::new();

        //         let mut col_types = vec![Type::TIMESTAMP, Type::OID];

        //         let mut schema_map: BTreeMap<String, Type> = BTreeMap::new();
        //         let rows = datums_iter.into_iter().map(|datum| {
        //             for (dimension_name, value) in datum.dimensions.iter() {
        //                 if let Some(sql_type) = value.sql_type() {
        //                     schema_map.insert(dimension_name.clone(), sql_type);
        //                 }
        //             }

        //             for (measurement_name, measurement_value) in datum.measurements.iter() {

        //             }
        //         });

        //         let rows = datums_iter.into_iter().map(|datum| {
        //             // ------ I don't think this stuff is needed
        //             // for (dimension_name, value) in datum.dimensions {
        //             //     match column_exists(metric.clone(), dimension_name.clone()) {
        //             //         Some(it_does) => {
        //             //             if !it_does {
        //             //                 let a = self.add_column(metric, &dimension_name, &"text".to_string());
        //             //                 ddl_futures.push(a);
        //             //             }
        //             //         },
        //             //         None => {
        //             //             let a = self.add_column(metric, &dimension_name, &"text".to_string());
        //             //             ddl_futures.push(a);
        //             //         },
        //             //     }
        //             // }
        //             // for (measurement_name, measurement) in datum.measurements {
        //             //     match column_exists(metric.clone(), measurement_name.clone()) {
        //             //         Some(it_does) => {
        //             //             if !it_does {
        //             //                 let a = self.add_column(metric, &measurement_name, sql_data_type(&measurement));
        //             //                 ddl_futures.push(a);
        //             //             }
        //             //         },
        //             //         None => {
        //             //             let a = self.add_column(metric, &measurement_name, &"text".to_string());
        //             //             ddl_futures.push(a);
        //             //         },
        //             //     }
        //             // }
        //             // ------ I don't think this stuff is needed

        //             let mut m = serde_json::map::Map::new();

        //             m["time"] = json!(datum.unix_nanos);
        //             schema.insert("time".to_string(), "timestamptz");
        //             datum.dimensions.iter().for_each(|(name, dim)| {
        //                 match &dim.value {
        //                     Some(v) => {
        //                         m[name] = match v {
        //                             Value::String(s) => {
        //                                 schema.insert(name.clone(), "text");
        //                                 json!(s)
        //                             },
        //                             Value::Number(n) => {
        //                                 schema.insert(name.clone(), "bigint"); // 64 bit integer
        //                                 json!(n)
        //                             },
        //                             Value::Boolean(b) => {
        //                                 schema.insert(name.clone(), "boolean");
        //                                 json!(b)
        //                             },
        //                         }
        //                     },
        //                     None => {},
        //                 }
        //             });

        //             datum.measurements.iter().for_each(|(name, measurement)| {
        //                 match &measurement.measurement {
        //                     Some(measurement_kind) => {
        //                         m[name] = match measurement_kind {
        //                             Measurement::Gauge(gauge) => json!(gauge),
        //                             Measurement::StatisticSet(statistic_set) => json!(statistic_set),
        //                             Measurement::Histogram(histogram) => json!(histogram),
        //                         }
        //                     },
        //                     None => {},
        //                 }
        //             });
        //             m
        //         });
        //         // FIXME: Why is this collecting into a map of groups of insanity?
        //         let rows_vector = rows.collect_vec();
        //         (metric, rows_vector, schema)
        //     });

        //     // why does rust make me define stupid temporary mutable variables just
        //     // to tell it the types, when it KNOWS the types and will freak out when I do this
        //     // incorrecty.
        //     let mut a: (&String, Vec<serde_json::map::Map<String, serde_json::Value>>, HashMap<String, &str>);
        //     for b in grouped_rows {
        //         a = b;
        //         let metric = a.0;
        //         let rows = a.1;
        //         let schema = a.2;

        //         // with data as (select * from json_to_recordset('[{"i": 1, "s": "s1", "a": "a1"},{"time":1600000000,"s":"s2","a":"a2","i":2,"k":"k2"}]') as (a text, i integer, s text, time int))
        //         //   insert into mm (a, i, s, time) select a, i, s, to_timestamp(time) as time from data;
        //         let sql_insert = format!("with data as (
        //                 select * from json_to_recordset($1) as ({schema_list})
        //             )
        //             insert into {table} ({name_list}) select to_timestamp(time), {name_list_without_time} as time
        //             from data;",
        //             table = clean_id(metric),
        //             schema_list = "a int,b text,time timestamptz",
        //             name_list = "a,b,time",
        //             name_list_without_time = "a,b"
        //         );
        //     }

        //     // batch.iter().map(|d| {
        //     //     d.metric
        //     // });
        //     // batch.iter().flat_map(|d| {
        //     //     d.dimensions.keys()
        //     // });

        //     // self.client.query("select (ARRAY[3, 2, 1]);", &[]).await
        //     // let query_future = self.client.query("SELECT $1::TEXT", &[&"hello world"]);
        //     let query_future = self.client.query("SELECT '123'", &[]);
        //     log::debug!("got future");
        //     let query_result = query_future.await;
        //     log::debug!("got result: {:?}", query_result);

        //     match query_result {
        //         Ok(number) => log::info!("well.. sql worked {:?}", number),
        //         Err(e) => log::error!("sql failed: {:?}", e),
        //     };
        // }
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

    // async fn execute(&self, data: Vec<Measurement>) -> Result<usize> {
    //     let tx = self.client.transaction().await?;
    //     let sink = tx.copy_in(self.copy_stm.as_str()).await?;
    //     let writer = BinaryCopyInWriter::new(sink, &self.col_types);
    //     let num_written = write(writer, &data).await?;
    //     tx.commit().await?;
    //     Ok(num_written)
    // }
}

async fn write(writer: BinaryCopyInWriter, dimensions: &BTreeMap<String, Type>, measurements: &BTreeMap<String, Type>, data: &Vec<Datum>) -> Result<usize, SinkError> {
    pin_mut!(writer);

    let mut row: Vec<Box<(dyn ToSql + Sync)>> = Vec::new();
    for datum in data {
        row.clear();
        row.push(Box::new(datum.unix_nanos as i64));
        for dimension_name in dimensions.keys() {
            let dimension = &datum.dimensions[dimension_name];
            if let Some(value) = dimension.value.as_ref() {
                row.push(
                    match value {
                        Value::String(s) => Box::new(s),
                        Value::Number(n) => Box::new(*n as i64),
                        Value::Boolean(b) => Box::new(b),
                    }
                )
            } else {
                row.push(Box::new(Option::<String>::None))
            }
        }
        for measurement_name in measurements.keys() {
            let measurement = &datum.measurements[measurement_name];
            if let Some(value) = measurement.measurement.as_ref() {
                row.push(
                    match value {
                        Measurement::Gauge(g) => Box::new(g),
                        Measurement::StatisticSet(s) => Box::new(vec![s.minimum, s.maximum, s.samplesum, s.samplecount]),
                        Measurement::Histogram(h) => todo!(),
                    }
                )
            } else {
                row.push(Box::new(Option::<f64>::None))
            }
        }

        let vec_of_raw_refs = row.iter().map(|c| { c.as_ref() }).collect_vec();
        writer.as_mut().write(&vec_of_raw_refs).await?;
    }

    writer.finish().await?;

    Ok(data.len())
}

// time, dimensions[], measurements[]
fn get_all_column_types(dimension_types: &BTreeMap<String, Type>, measurement_types: &BTreeMap<String, Type>) -> Vec<Type> {
    let mut all_column_types: Vec<Type> = vec![Type::TIMESTAMPTZ];
    all_column_types.extend(dimension_types.values().cloned());
    all_column_types.extend(measurement_types.values().cloned());
    all_column_types
}

fn group_metrics<'a>(batch: Vec<Datum>) -> BTreeMap<&'a String, Vec<&'a Datum>> {
    let grouped_metrics: BTreeMap<&String, Vec<&Datum>> = batch.iter()
        .sorted_by_key(|d| {&d.metric})
        .group_by(|d| {&d.metric})
        .into_iter()
        .map(|(metric, datums_iterable)| {
            (metric, datums_iterable.collect_vec())
        })
        .collect();
    grouped_metrics
}

fn get_dimension_type_map(grouped_metrics: &BTreeMap<&String, Vec<&Datum>>) -> BTreeMap<String, Type> {
    grouped_metrics
        .iter()
        .flat_map(|(metric, datums)| {
            datums
                .iter()
                .map(|d| {
                    d.dimensions.iter()
                })
                .flatten()
        }).filter_map(|(dimension_name, dimension_value)| {
            if let Some(sql_type) = dimension_value.sql_type() {
                Some(
                    (dimension_name.clone(), sql_type)
                )
            } else {
                None
            }
        })
        .collect()
}

fn get_measurement_type_map(grouped_metrics: &BTreeMap<&String, Vec<&Datum>>) -> BTreeMap<String, Type> {
    grouped_metrics
        .iter()
        .flat_map(|(metric, datums)| {
            datums
                .iter()
                .map(|d| {
                    d.measurements.iter()
                })
                .flatten()
        }).filter_map(|(measurement_name, measurement_value)| {
            if let Some(sql_type) = measurement_value.sql_type() {
                Some(
                    (measurement_name.clone(), sql_type)
                )
            } else {
                None
            }
        })
        .collect()
}

trait ToSqlType {
    fn sql_type(&self) -> Option<Type>;
}

impl ToSqlType for Dimension {
    fn sql_type(&self) -> Option<Type> {
        match self.value.as_ref() {
            Some(v) => {
                Some(match v {
                    Value::String(_) => Type::TEXT,
                    Value::Number(_) => Type::INT8,
                    Value::Boolean(_) => Type::BOOL,
                })
            },
            None => None,
        }
    }
}

impl ToSqlType for pb::Measurement {
    fn sql_type(&self) -> Option<Type> {
        match self.measurement.as_ref() {
            Some(v) => {
                Some(match v {
                    Measurement::Gauge(_) => Type::FLOAT8,
                    // Composite type thing
                    Measurement::StatisticSet(_) => Type::RECORD,
                    Measurement::Histogram(_) => Type::RECORD,
                })
            },
            None => None,
        }
    }
}

#[derive(Error, Debug)]
pub enum SinkError {
    #[error("unhandled postgres error")]
    Postgres(#[from] tokio_postgres::Error),
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
