use lazy_static::lazy_static;
use regex::Regex;
use tokio_postgres::Client;

lazy_static! {
    static ref NOT_WHITESPACE: Regex = Regex::new(r"[^\w]+").unwrap();
}

pub async fn add_column(client: &Client, table_name: &str, column_name: &str, data_type: &str) -> Result<(), tokio_postgres::Error> {
    client.batch_execute(
    &format!(
            "alter table {table} add column {column} {data_type}",
            table=table_name,
            column=column_name,
            data_type=data_type,
        )
    ).await
}

pub fn clean_id(s: &String) -> String {
    let l = s.to_lowercase();
    let a = NOT_WHITESPACE.replace_all(&l, "_");
    a.to_string()
}
