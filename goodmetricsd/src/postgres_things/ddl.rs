use std::time::Duration;

use lazy_static::lazy_static;
use regex::Regex;
use tokio_postgres::Client;

lazy_static! {
    static ref NOT_WHITESPACE: Regex = Regex::new(r"[^\w]+").expect("regex compiles");
}

pub async fn add_column(
    client: &Client,
    table_name: &str,
    column_name: &str,
    data_type: &str,
) -> Result<(), tokio_postgres::Error> {
    client
        .batch_execute(&format!(
            "alter table {table} add column {column} {data_type}",
            table = table_name,
            column = column_name,
            data_type = data_type,
        ))
        .await
}

pub async fn create_table(
    transaction: &Client,
    table_name: &str,
    retention: &Duration,
    compress: bool,
) -> Result<(), tokio_postgres::Error> {
    let chunk = "4h";
    let compression_statement = if compress {
        format!(
            r#"
            ALTER TABLE {table_name} SET (timescaledb.compress, timescaledb.compress_orderby = 'time DESC', timescaledb.compress_chunk_time_interval = '24 hours');
            SELECT add_compression_policy('{table_name}', INTERVAL '{chunk}');
            "#
        )
    } else {
        "".to_string()
    };
    transaction.batch_execute(
    &format!(
            r#"CREATE TABLE {table_name} (time timestamptz);
            SELECT * from create_hypertable('{table_name}', 'time', chunk_time_interval => INTERVAL '{chunk}' );
            SELECT add_retention_policy('{table_name}', INTERVAL '{retention_seconds} seconds');
            {compression_statement}
            "#,
            retention_seconds=retention.as_secs(),
        )
    ).await
}

pub fn clean_id(s: &str) -> String {
    let l = s.to_lowercase();
    let a = NOT_WHITESPACE.replace_all(&l, "_");
    a.to_string()
}
