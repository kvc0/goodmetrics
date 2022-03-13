use std::fmt::Display;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SinkError {
    #[error("unhandled postgres error")]
    Postgres(#[from] tokio_postgres::Error),

    #[error("Some postgres error with a description")]
    DescribedError(#[from] DescribedError),

    #[error("unhandled error")]
    StringError(#[from] StringError),

    #[error("i gotta have more column")]
    MissingColumn(#[from] MissingColumn),

    #[error("i gotta have more table")]
    MissingTable(#[from] MissingTable),
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

#[derive(Debug, Error)]
pub struct StringError {
    pub message: String,
}

impl Display for StringError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("StringError")
            .field("message", &self.message)
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
