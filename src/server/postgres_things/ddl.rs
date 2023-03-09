use std::time::Duration;

use lazy_static::lazy_static;
use regex::Regex;
use tokio_postgres::Client;

lazy_static! {
    static ref NOT_WHITESPACE: Regex = Regex::new(r"[^\w]+").unwrap();
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
) -> Result<(), tokio_postgres::Error> {
    transaction.batch_execute(
    &format!(
            r#"CREATE TABLE {table} (time timestamptz);
            SELECT * from create_hypertable('{table}', 'time', chunk_time_interval => interval '{chunk}' );
            SELECT add_retention_policy('{table}', INTERVAL '{retention_seconds} seconds');
            "#,
            table=table_name,
            chunk="4h",
            retention_seconds=retention.as_secs(),
        )
    ).await
}

pub fn clean_id(s: &str) -> String {
    let l = s.to_lowercase();
    let a = NOT_WHITESPACE.replace_all(&l, "_");
    a.to_string()
}
