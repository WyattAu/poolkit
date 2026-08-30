use thiserror::Error;

/// Errors that can occur when interacting with the connection pool.
#[derive(Debug, Error)]
pub enum PoolError {
    /// Failed to establish or maintain a database connection.
    #[error("connection error: {0}")]
    Connection(#[source] sqlx::Error),

    /// An operation timed out (e.g. acquiring a connection).
    #[error("timeout: {0}")]
    Timeout(String),

    /// The pool has been closed or is not accepting new connections.
    #[error("pool closed")]
    PoolClosed,

    /// A database migration failed.
    #[error("migration error: {0}")]
    Migration(String),
}
