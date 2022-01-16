use std::collections::BTreeMap;

use cached::{proc_macro::cached, Cached};
use futures::pin_mut;
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use thiserror::Error;
use tokio_postgres::{NoTls, tls::NoTlsStream, Socket, Connection, types::{Type, ToSql}, binary_copy::BinaryCopyInWriter, error::SqlState};

use crate::proto::metrics::{pb, pb::{Datum, Dimension, dimension::Value, measurement::Measurement}};

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
    static ref NOT_WHITESPACE: Regex = Regex::new(r"[^\w]+").unwrap();

    // column "available_messages" of relation "table_name" does not exist
    static ref UNDEFINED_COLUMN: Regex = Regex::new(r#"column "(?P<column>.+)" of relation "(?P<table>.+)" does not exist"#).unwrap();
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
            let grouped_metrics = group_metrics(&batch);
            let mut try_again = true;
            while try_again {
                try_again = match self.run_a_batch(&grouped_metrics).await {
                    Ok(rows) => {
                        log::info!("committed ${rows}", rows = rows);

                        false
                    },
                    Err(e) => {
                        log::error!("{:?}", e);

                        match handle_error_and_should_it_retry(e).await {
                            Ok(should_retry) => {
                                should_retry
                            },
                            Err(retry_failure) => {
                                log::error!("failed to handle error: {:?}", retry_failure);

                                false
                            },
                        }
                    },
                }
            }
        }
        log::info!("ended consumer");
        Ok(1)
    }

    async fn run_a_batch(&mut self, grouped_metrics: &BTreeMap<&String, Vec<&Datum>>) -> Result<usize, SinkError> {
        let transaction = self.client.transaction().await?;

        let mut rows = 0;
        for (metric, datums) in grouped_metrics.iter() {
            let dimension_types = get_dimension_type_map(datums);
            let measurement_types = get_measurement_type_map(datums);

            let all_column_types = get_all_column_types(&dimension_types, &measurement_types);
            let all_column_names = get_all_column_names(&dimension_types, &measurement_types);

            let sink = transaction.copy_in(
                &format!(
                    "copy {table_name} ({all_columns}) from stdin with binary",
                    table_name = clean_id(metric),
                    all_columns = all_column_names.join(","),
                )
            ).await?;

            let writer = BinaryCopyInWriter::new(sink, &all_column_types);
            rows += write_and_close(writer, &dimension_types, &measurement_types, &datums).await?;
        }

        transaction.commit().await?;

        Ok(rows)
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

async fn handle_error_and_should_it_retry(e: SinkError) -> Result<bool, SinkError> {
    return match e {
        SinkError::Postgres(postgres_error) => {
            match postgres_error.as_db_error() {
                Some(dberror) => {
                    match dberror.code() {
                        &SqlState::UNDEFINED_COLUMN => {
                            let pair = UNDEFINED_COLUMN.captures(dberror.message()).unwrap();
                            let table = pair.name("table").unwrap().as_str();
                            let column = pair.name("column").unwrap().as_str();
                            log::info!("missing column: {table}.{column}", table = table, column = column);

                            Ok(true)
                        }
                        _ => {
                            log::error!("unhandled db error: ${err:?}", err = dberror);

                            Ok(false)
                        }
                    }
                },
                None => {
                    log::error!("postgres error: ${err:?}", err = postgres_error);

                    Ok(false)
                },
            }
        },
    }
}

async fn write_and_close(writer: BinaryCopyInWriter, dimensions: &BTreeMap<String, Type>, measurements: &BTreeMap<String, Type>, data: &Vec<&Datum>) -> Result<usize, SinkError> {
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

// time, dimensions[], measurements[]
fn get_all_column_names(dimension_types: &BTreeMap<String, Type>, measurement_types: &BTreeMap<String, Type>) -> Vec<String> {
    let mut all_column_types: Vec<String> = vec!["time".to_string()];
    all_column_types.extend(dimension_types.keys().map(|d| { clean_id(d) }));
    all_column_types.extend(measurement_types.keys().map(|m| { clean_id(m) }));
    all_column_types
}

fn group_metrics<'a>(batch: &'a Vec<Datum>) -> BTreeMap<&'a String, Vec<&Datum>> {
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

fn get_dimension_type_map(datums: &Vec<&Datum>) -> BTreeMap<String, Type> {
    datums
        .iter()
        .map(|d| {
            d.dimensions.iter()
        })
        .flatten()
        .filter_map(|(dimension_name, dimension_value)| {
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

fn get_measurement_type_map(datums: &Vec<&Datum>) -> BTreeMap<String, Type> {
    datums
        .iter()
        .map(|d| {
            d.measurements.iter()
        })
        .flatten()
        .filter_map(|(measurement_name, measurement_value)| {
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

fn sql_data_type_string(measurement: &pb::Measurement) -> &'static str {
    match measurement.measurement.as_ref().unwrap() {
        Measurement::Gauge(_) => "double precision",
        Measurement::StatisticSet(_) => "statistic_set",
        Measurement::Histogram(_) => "histogram",
    }
}

fn clean_id(s: &String) -> String {
    let l = s.to_lowercase();
    let a = NOT_WHITESPACE.replace_all(&l, "_");
    a.to_string()
}
