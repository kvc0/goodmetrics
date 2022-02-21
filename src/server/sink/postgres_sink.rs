use std::{
    collections::BTreeMap,
    error::Error,
    fmt::Display,
    time::{Duration, SystemTime},
};

use crate::{
    postgres_things::{
        ddl::{self, clean_id},
        histogram::get_or_create_histogram_type,
        postgres_connector::PostgresConnector,
        statistic_set::get_or_create_statistic_set_type,
        type_conversion::TypeConverter,
    },
    proto::metrics::pb::{dimension, measurement, Datum, Dimension, Measurement},
};
use futures::pin_mut;
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use thiserror::Error;
use tokio_postgres::{
    binary_copy::BinaryCopyInWriter,
    error::SqlState,
    types::{ToSql, Type, WrongType},
    CopyInSink,
};

use super::metricssendqueue::MetricsReceiveQueue;

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

#[derive(Debug, Error)]
pub struct MissingTable {
    pub table: String,
}

impl Display for MissingTable {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("MissingTable")
            .field("table", &self.table)
            .finish()
    }
}

#[derive(Debug, Error)]
pub struct MissingColumn {
    pub table: String,
    pub column: String,
    pub data_type: String,
}

impl Display for MissingColumn {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("MissingColumn")
            .field("table", &self.table)
            .field("column", &self.column)
            .finish()
    }
}

#[derive(Error, Debug)]
pub enum SinkError {
    #[error("unhandled postgres error")]
    Postgres(#[from] tokio_postgres::Error),

    #[error("Some postgres error with a description")]
    DescribedError(#[from] DescribedError),

    #[error("i gotta have more column")]
    MissingColumn(#[from] MissingColumn),

    #[error("i gotta have more table")]
    MissingTable(#[from] MissingTable),
}

lazy_static! {
    // column "available_messages" of relation "table_name" does not exist
    static ref UNDEFINED_COLUMN: Regex = Regex::new(r#"column "(?P<column>.+)" of relation "(?P<table>.+)" does not exist"#).unwrap();
    static ref UNDEFINED_TABLE: Regex = Regex::new(r#"relation "(?P<table>.+)" does not exist"#).unwrap();
}

pub struct PostgresSender {
    connector: PostgresConnector,
    rx: MetricsReceiveQueue,
    type_converter: TypeConverter,
}

impl PostgresSender {
    pub async fn new_connection(
        connection_string: &str,
        rx: MetricsReceiveQueue,
    ) -> Result<PostgresSender, SinkError> {
        log::debug!("new_connection: {:?}", connection_string);
        let mut connector = PostgresConnector::new(connection_string.to_string()).await?;

        let type_converter = {
            let statistic_set_type = get_or_create_statistic_set_type(&mut connector).await?;
            let histogram_type = get_or_create_histogram_type(&mut connector).await?;
            TypeConverter {
                statistic_set_type,
                histogram_type,
            }
        };

        Ok(PostgresSender {
            connector,
            rx,
            type_converter,
        })
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
                    }
                    Err(e) => match self.handle_error_and_should_it_retry(e).await {
                        Ok(should_retry) => should_retry,
                        Err(retry_failure) => {
                            log::error!("failed to handle error: {:?}", retry_failure);

                            false
                        }
                    },
                }
            }
        }
        log::info!("ended consumer");
        Ok(1)
    }

    async fn run_a_batch(
        &mut self,
        grouped_metrics: &BTreeMap<&String, Vec<&Datum>>,
    ) -> Result<usize, SinkError> {
        let client = self.connector.use_connection().await?;

        let mut rows = 0;
        for (metric, datums) in grouped_metrics.iter() {
            let dimension_types = self.type_converter.get_dimension_type_map(datums);
            let measurement_types = self.type_converter.get_measurement_type_map(datums);

            let all_column_types = get_all_column_types(&dimension_types, &measurement_types);
            let all_column_names = get_all_column_names(&dimension_types, &measurement_types);

            let sink: CopyInSink<bytes::Bytes> = match client
                .copy_in::<String, bytes::Bytes>(&format!(
                    "copy {table_name} ({all_columns}) from stdin with binary",
                    table_name = clean_id(metric),
                    all_columns = all_column_names.join(","),
                ))
                .await
            {
                Ok(sink) => sink,
                Err(postgres_error) => match postgres_error.as_db_error() {
                    Some(dberror) => match *dberror.code() {
                        SqlState::UNDEFINED_COLUMN => {
                            let pair = UNDEFINED_COLUMN.captures(dberror.message()).unwrap();
                            let table = pair.name("table").unwrap().as_str();
                            let column = pair.name("column").unwrap().as_str();
                            log::info!(
                                "missing column: {table}.{column}",
                                table = table,
                                column = column
                            );
                            let the_type = datums
                                .iter()
                                .filter_map(|d| match d.dimensions.get(column) {
                                    Some(dim) => Some(sql_dimension_type_string(dim)),
                                    None => match d.measurements.get(column) {
                                        Some(measurement) => {
                                            Some(sql_data_type_string(measurement))
                                        }
                                        None => None,
                                    },
                                })
                                .next();
                            match the_type {
                                Some(t) => {
                                    return Err(SinkError::MissingColumn(MissingColumn {
                                        table: table.to_string(),
                                        column: column.to_string(),
                                        data_type: t.to_string(),
                                    }))
                                }
                                None => {
                                    return Err(SinkError::DescribedError(DescribedError {
                                        message: "Type not foud, can't add column".to_string(),
                                        inner: postgres_error,
                                    }))
                                }
                            }
                        }
                        SqlState::UNDEFINED_TABLE => {
                            let table_capture =
                                UNDEFINED_TABLE.captures(dberror.message()).unwrap();
                            let table = table_capture.name("table").unwrap().as_str();
                            log::info!("missing table: {table}", table = table);

                            return Err(SinkError::MissingTable(MissingTable {
                                table: table.to_string(),
                            }));
                        }
                        _ => {
                            return Err(SinkError::Postgres(postgres_error));
                        }
                    },
                    None => return Err(SinkError::Postgres(postgres_error)),
                },
            };

            let writer = BinaryCopyInWriter::new(sink, &all_column_types);
            rows += write_and_close(writer, &dimension_types, &measurement_types, datums).await?;
        }

        // client.commit().await?;

        Ok(rows)
    }

    async fn handle_error_and_should_it_retry(&mut self, e: SinkError) -> Result<bool, SinkError> {
        return match e {
            SinkError::Postgres(postgres_error) => match postgres_error.as_db_error() {
                Some(dberror) => match *dberror.code() {
                    SqlState::INSUFFICIENT_PRIVILEGE => {
                        log::error!(
                            "Do you need to grant permissions or reset the table's owner? {:?}",
                            dberror
                        );

                        Ok(false)
                    }
                    _ => {
                        log::error!("unhandled db error: ${err:?}", err = dberror);

                        Ok(false)
                    }
                },
                None => match postgres_error.source() {
                    Some(client_error) => {
                        if client_error.is::<WrongType>() {
                            log::error!("Dropping batch due to mismatch between postgres type and batch type: {:?}", client_error);

                            Ok(false)
                        } else {
                            Ok(false)
                        }
                    }
                    None => {
                        log::error!("postgres without cause: ${err:?}", err = postgres_error);

                        Ok(false)
                    }
                },
            },
            SinkError::MissingColumn(what_column) => {
                log::info!("adding missing column {:?}", what_column);
                let client = self.connector.use_connection().await?;
                ddl::add_column(
                    client,
                    &what_column.table,
                    &what_column.column,
                    &what_column.data_type,
                )
                .await?;

                Ok(true)
            }
            SinkError::MissingTable(what_table) => {
                log::info!("adding missing table {:?}", what_table);
                let client = self.connector.use_connection().await?;
                ddl::create_table(client, &what_table.table).await?;

                Ok(true)
            }
            SinkError::DescribedError(_e) => todo!(),
        };
    }
}

async fn write_and_close(
    writer: BinaryCopyInWriter,
    dimensions: &BTreeMap<String, Type>,
    measurements: &BTreeMap<String, Type>,
    data: &[&Datum],
) -> Result<usize, SinkError> {
    pin_mut!(writer);

    let mut row: Vec<Box<(dyn ToSql + Sync)>> = Vec::new();
    for datum in data {
        row.clear();
        let datum_time = SystemTime::UNIX_EPOCH + Duration::from_nanos(datum.unix_nanos);
        row.push(Box::new(datum_time));
        for dimension_name in dimensions.keys() {
            if !datum.dimensions.contains_key(dimension_name) {
                log::warn!("skipping dimension: {}", dimension_name);
                row.push(Box::new(Option::<String>::None));
                continue;
            }

            let dimension = &datum.dimensions[dimension_name];
            if let Some(value) = dimension.value.as_ref() {
                row.push(match value {
                    dimension::Value::String(s) => Box::new(s),
                    dimension::Value::Number(n) => Box::new(*n as i64),
                    dimension::Value::Boolean(b) => Box::new(b),
                })
            } else {
                row.push(Box::new(Option::<String>::None))
            }
        }
        for measurement_name in measurements.keys() {
            let measurement = &datum.measurements[measurement_name];
            if let Some(value) = measurement.value.as_ref() {
                row.push(match value {
                    measurement::Value::Inumber(i) => Box::new(i),
                    measurement::Value::Fnumber(f) => Box::new(f),
                    // measurement::Value::StatisticSet(s) => Box::new((s.minimum, s.maximum, s.samplesum, s.samplecount)),
                    measurement::Value::StatisticSet(s) => Box::new(s),
                    measurement::Value::Histogram(h) => Box::new(h.to_stupidmap()),
                })
            } else {
                row.push(Box::new(Option::<f64>::None))
            }
        }

        let vec_of_raw_refs = row.iter().map(|c| c.as_ref()).collect_vec();
        writer.as_mut().write(&vec_of_raw_refs).await?;
    }
    writer.finish().await?;
    Ok(data.len())
}

// time, dimensions[], measurements[]
fn get_all_column_types(
    dimension_types: &BTreeMap<String, Type>,
    measurement_types: &BTreeMap<String, Type>,
) -> Vec<Type> {
    let mut all_column_types: Vec<Type> = vec![Type::TIMESTAMPTZ];
    all_column_types.extend(dimension_types.values().cloned());
    all_column_types.extend(measurement_types.values().cloned());
    all_column_types
}

// time, dimensions[], measurements[]
fn get_all_column_names(
    dimension_types: &BTreeMap<String, Type>,
    measurement_types: &BTreeMap<String, Type>,
) -> Vec<String> {
    let mut all_column_types: Vec<String> = vec!["time".to_string()];
    all_column_types.extend(dimension_types.keys().map(|d| clean_id(d)));
    all_column_types.extend(measurement_types.keys().map(|d| clean_id(d)));
    all_column_types
}

fn group_metrics<'a>(batch: &'a [Datum]) -> BTreeMap<&'a String, Vec<&Datum>> {
    let grouped_metrics: BTreeMap<&String, Vec<&Datum>> = batch
        .iter()
        .sorted_by_key(|d| &d.metric)
        .group_by(|d| &d.metric)
        .into_iter()
        .map(|(metric, datums_iterable)| (metric, datums_iterable.collect_vec()))
        .collect();
    grouped_metrics
}

fn sql_data_type_string(measurement: &Measurement) -> &'static str {
    match measurement.value.as_ref().unwrap() {
        measurement::Value::Inumber(_) => "int8",
        measurement::Value::Fnumber(_) => "float8",
        measurement::Value::StatisticSet(_) => "statistic_set",
        measurement::Value::Histogram(_) => "histogram",
    }
}

fn sql_dimension_type_string(dimension: &Dimension) -> &'static str {
    match dimension.value.as_ref().unwrap() {
        dimension::Value::String(_) => "text",
        dimension::Value::Number(_) => "int8",
        dimension::Value::Boolean(_) => "boolean",
    }
}
