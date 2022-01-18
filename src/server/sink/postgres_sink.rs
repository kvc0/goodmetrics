use std::{collections::BTreeMap, error::Error, fmt::Display, time::{SystemTime, Duration}};

use cached::{proc_macro::cached, Cached};
use futures::pin_mut;
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use thiserror::Error;
use tokio_postgres::{NoTls, tls::NoTlsStream, Socket, Connection, types::{Type, ToSql, WrongType}, binary_copy::BinaryCopyInWriter, error::SqlState};
use crate::{proto::metrics::pb::{Datum, dimension, measurement, Dimension, Measurement}, postgres_things::statistic_set::get_or_create_statistic_set_type};

use super::metricssendqueue::MetricsReceiveQueue;

pub struct PostgresSender {
    pub client: tokio_postgres::Client,
    rx: MetricsReceiveQueue,
    statistic_set_type: Type,
}

#[derive(Debug, Error)]
pub struct DescribedError {
    pub message: String,
    pub inner: tokio_postgres::Error,
}

impl Display for DescribedError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("DescribedError")
            .field("message", &self.message)
            .field("cause", &self.inner)
            .finish()
    }
}

#[derive(Error, Debug)]
pub enum SinkError {
    #[error("unhandled postgres error")]
    Postgres(#[from] tokio_postgres::Error),

    #[error("Some postgres error with a description")]
    DescribedError(#[from] DescribedError),
}

lazy_static! {
    static ref NOT_WHITESPACE: Regex = Regex::new(r"[^\w]+").unwrap();

    // column "available_messages" of relation "table_name" does not exist
    static ref UNDEFINED_COLUMN: Regex = Regex::new(r#"column "(?P<column>.+)" of relation "(?P<table>.+)" does not exist"#).unwrap();
}

impl PostgresSender {
    pub async fn new_connection(connection_string: &String, rx: MetricsReceiveQueue) -> Result<PostgresSender, SinkError> {
        log::debug!("new_connection: {:?}", connection_string);
        let connection_future = tokio_postgres::connect(&connection_string, NoTls);
        match connection_future.await {
            Ok(connection_pair) => {
                let (client, connection) = connection_pair;
                tokio::spawn(async move {
                    PostgresSender::run_connection(connection).await
                });

                let statistic_set_type = get_or_create_statistic_set_type(&client).await?;

                Ok(PostgresSender {
                    client,
                    rx,
                    statistic_set_type,
                })
            },
            Err(err) => {
                Err(SinkError::DescribedError(DescribedError { message: "could not connect".to_string(), inner: err }))
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
                        &SqlState::INSUFFICIENT_PRIVILEGE => {
                            log::error!("{}. Do you need to grant permissions or reset the table's owner?", dberror.message());

                            Ok(false)
                        },
                        _ => {
                            log::error!("unhandled db error: ${err:?}", err = dberror);

                            Ok(false)
                        }
                    }
                },
                None => {
                    match postgres_error.source() {
                        Some(client_error) => {
                            if client_error.is::<WrongType>() {
                                log::error!("Dropping batch due to mismatch between postgres type and batch type: {:?}", client_error);

                                Ok(false)
                            } else {
                                Ok(false)
                            }
                        },
                        None => {
                            log::error!("postgres without cause: ${err:?}", err = postgres_error);

                            Ok(false)
                        },
                    }
                },
            }
        },
        SinkError::DescribedError(e) => todo!(),
    }
}

async fn write_and_close(writer: BinaryCopyInWriter, dimensions: &BTreeMap<String, Type>, measurements: &BTreeMap<String, Type>, data: &Vec<&Datum>) -> Result<usize, SinkError> {
    pin_mut!(writer);

    let mut row: Vec<Box<(dyn ToSql + Sync)>> = Vec::new();
    for datum in data {
        row.clear();
        let datum_time = SystemTime::UNIX_EPOCH + Duration::from_nanos(datum.unix_nanos);
        row.push(Box::new(datum_time));
        for dimension_name in dimensions.keys() {
            let dimension = &datum.dimensions[dimension_name];
            if let Some(value) = dimension.value.as_ref() {
                row.push(
                    match value {
                        dimension::Value::String(s) => Box::new(s),
                        dimension::Value::Number(n) => Box::new(*n as i64),
                        dimension::Value::Boolean(b) => Box::new(b),
                    }
                )
            } else {
                row.push(Box::new(Option::<String>::None))
            }
        }
        for measurement_name in measurements.keys() {
            let measurement = &datum.measurements[measurement_name];
            if let Some(value) = measurement.value.as_ref() {
                row.push(
                    match value {
                        measurement::Value::Gauge(g) => Box::new(g),
                        // measurement::Value::StatisticSet(s) => Box::new((s.minimum, s.maximum, s.samplesum, s.samplecount)),
                        measurement::Value::StatisticSet(s) => Box::new(s),
                        measurement::Value::Histogram(h) => todo!(),
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
                    dimension::Value::String(_) => Type::TEXT,
                    dimension::Value::Number(_) => Type::INT8,
                    dimension::Value::Boolean(_) => Type::BOOL,
                })
            },
            None => None,
        }
    }
}

impl ToSqlType for Measurement {
    fn sql_type(&self) -> Option<Type> {
        match self.value.as_ref() {
            Some(v) => {
                Some(match v {
                    measurement::Value::Gauge(_) => Type::FLOAT8,
                    // Composite type thing
                    measurement::Value::StatisticSet(_) => Type::RECORD,
                    measurement::Value::Histogram(_) => Type::RECORD,
                })
            },
            None => None,
        }
    }
}

#[cached(name = "COLUMN_EXISTS_CACHE", size=8192, time=600, option = true)]
fn column_exists(table: String, column: String) -> Option<bool> {
    None
}

fn sql_data_type_string(measurement: &Measurement) -> &'static str {
    match measurement.value.as_ref().unwrap() {
        measurement::Value::Gauge(_) => "double precision",
        measurement::Value::StatisticSet(_) => "statistic_set",
        measurement::Value::Histogram(_) => "histogram",
    }
}

fn clean_id(s: &String) -> String {
    let l = s.to_lowercase();
    let a = NOT_WHITESPACE.replace_all(&l, "_");
    a.to_string()
}
