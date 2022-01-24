use std::sync::{atomic::AtomicBool, Arc};

use tokio::task::JoinHandle;
use tokio_postgres::{Client, Transaction, NoTls, Connection, Socket, tls::NoTlsStream};

use crate::sink::postgres_sink::{SinkError, DescribedError};


pub struct PostgresConnector {
    connection_string: String,
    client: Client,
    connection_handle: JoinHandle<()>,
    connection_is_alive: Arc<AtomicBool>,
}

impl PostgresConnector {
    pub async fn new(connection_string: String) -> Result<PostgresConnector, SinkError> {
        let connection_is_alive = Arc::new(AtomicBool::new(false));
        let (client, connection_handle) = new_connection(&connection_string, connection_is_alive.clone()).await?;
        Ok(PostgresConnector { connection_string, client, connection_handle, connection_is_alive })
    }

    pub async fn use_connection<'connection_use>(&'connection_use mut self) -> Result<Transaction<'connection_use>, SinkError> {
        // need to get the connection via the method that ensures it's connected
        let client = self.get_connection().await?;
        Ok(client.transaction().await?)
    }

    async fn get_connection<'a>(&'a mut self) -> Result<&'a mut Client, SinkError> {
        match self.connection_is_alive.load(std::sync::atomic::Ordering::Relaxed) {
            true => {
                Ok(&mut self.client)
            },
            false => {
                let (client, connection_handle) = new_connection(&self.connection_string, self.connection_is_alive.clone()).await?;
                self.client = client;
                self.connection_handle = connection_handle;
                Ok(&mut self.client)
            },
        }
    }
}

async fn run_connection(connection: Connection<Socket, NoTlsStream>, still_running: Arc<AtomicBool>) {
    log::info!("spawning connection routine");
    match connection.await {
        Ok(_) => log::info!("connection routine ended"),
        Err(e) => log::info!("connection routine errored: {:?}", e),
    }
    still_running.store(false, std::sync::atomic::Ordering::Relaxed);
}

async fn new_connection(connection_string: &str, connection_is_alive: Arc<AtomicBool>) -> Result<(Client, JoinHandle<()>), SinkError> {
    log::debug!("new_connection: {:?}", connection_string);
    let connection_future = tokio_postgres::connect(connection_string, NoTls);
    match connection_future.await {
        Ok(connection_pair) => {
            let (client, connection) = connection_pair;
            let connection_handle = tokio::spawn(async move {
                run_connection(connection, connection_is_alive).await
            });

            Ok((client, connection_handle))
        },
        Err(err) => {
            Err(SinkError::DescribedError(DescribedError { message: "could not connect".to_string(), inner: err }))
        },
    }
}
