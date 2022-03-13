use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use tokio_postgres::NoTls;

use crate::server::sink::sink_error::{SinkError, StringError};

pub struct PostgresConnector {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

impl PostgresConnector {
    pub async fn new(
        connection_string: String,
        max_conns: usize,
    ) -> Result<PostgresConnector, SinkError> {
        let pg_manager = PostgresConnectionManager::new_from_stringlike(
            connection_string,
            tokio_postgres::NoTls,
        )
        .unwrap();
        let pool = match Pool::builder()
            .max_size(max_conns as u32)
            .build(pg_manager)
            .await
        {
            Ok(pool) => pool,
            Err(e) => panic!("bb8 error {}", e),
        };

        Ok(PostgresConnector { pool })
    }

    pub async fn use_connection(
        &self,
    ) -> Result<bb8::PooledConnection<'_, PostgresConnectionManager<NoTls>>, SinkError> {
        // need to get the connection via the method that ensures it's connected
        let poolconn = match self.pool.get().await {
            Ok(client) => client,
            Err(err) => {
                return Err(SinkError::StringError(StringError {
                    message: format!("failed to get connection: {:?}", err),
                }));
            }
        };
        Ok(poolconn)
    }
}
