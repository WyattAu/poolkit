#![forbid(unsafe_code)]

mod error;
pub mod health;

pub use error::PoolError;
pub use health::{HealthCheckResult, HealthStatus};

use std::time::Duration;

/// A managed database connection pool backed by SQLx.
pub struct DbPool {
    pool: sqlx::Pool<sqlx::Any>,
    max_connections: u32,
    min_connections: u32,
    idle_timeout: Duration,
    acquire_timeout: Duration,
}

/// Builder for constructing a [`DbPool`] with custom configuration.
pub struct DbPoolBuilder {
    max_connections: u32,
    min_connections: u32,
    idle_timeout: Duration,
    acquire_timeout: Duration,
    database_url: String,
}

/// Pool statistics snapshot.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PoolStats {
    pub max_connections: u32,
    pub min_connections: u32,
    pub idle_timeout_secs: u64,
    pub acquire_timeout_secs: u64,
    pub size: u32,
    pub idle: u32,
    pub active: u32,
}

impl DbPoolBuilder {
    /// Create a new builder with the given database URL and sensible defaults.
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            max_connections: 10,
            min_connections: 1,
            idle_timeout: Duration::from_secs(600),
            acquire_timeout: Duration::from_secs(30),
        }
    }

    /// Set the maximum number of connections in the pool.
    pub fn max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }

    /// Set the minimum number of idle connections to maintain.
    pub fn min_connections(mut self, min: u32) -> Self {
        self.min_connections = min;
        self
    }

    /// Set the idle timeout before a connection is closed.
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Set the maximum time to wait when acquiring a connection.
    pub fn acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = timeout;
        self
    }

    /// Build the pool, returning a [`PoolError`] on failure.
    pub async fn build(self) -> Result<DbPool, PoolError> {
        let options = sqlx::any::AnyPoolOptions::new()
            .max_connections(self.max_connections)
            .min_connections(self.min_connections)
            .idle_timeout(self.idle_timeout)
            .acquire_timeout(self.acquire_timeout);

        let pool = options
            .connect(&self.database_url)
            .await
            .map_err(PoolError::Connection)?;

        Ok(DbPool {
            pool,
            max_connections: self.max_connections,
            min_connections: self.min_connections,
            idle_timeout: self.idle_timeout,
            acquire_timeout: self.acquire_timeout,
        })
    }
}

impl DbPool {
    /// Create a new builder for this pool.
    pub fn builder(database_url: impl Into<String>) -> DbPoolBuilder {
        DbPoolBuilder::new(database_url)
    }

    /// Run a health check against the underlying database.
    pub async fn health_check(&self) -> HealthCheckResult {
        let start = std::time::Instant::now();
        match sqlx::query("SELECT 1").execute(&self.pool).await {
            Ok(_) => HealthCheckResult {
                status: HealthStatus::Healthy,
                latency_ms: start.elapsed().as_millis() as u64,
                message: None,
            },
            Err(e) => HealthCheckResult {
                status: HealthStatus::Unhealthy,
                latency_ms: start.elapsed().as_millis() as u64,
                message: Some(e.to_string()),
            },
        }
    }

    /// Execute a simple ping (SELECT 1) and return `Ok(())` or a [`PoolError`].
    pub async fn ping(&self) -> Result<(), PoolError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(PoolError::Connection)?;
        Ok(())
    }

    /// Return a snapshot of the current pool statistics.
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            max_connections: self.max_connections,
            min_connections: self.min_connections,
            idle_timeout_secs: self.idle_timeout.as_secs(),
            acquire_timeout_secs: self.acquire_timeout.as_secs(),
            size: self.pool.size(),
            idle: self.pool.num_idle() as u32,
            active: self.pool.size() - self.pool.num_idle() as u32,
        }
    }

    /// Get a reference to the underlying SQLx pool.
    pub fn inner(&self) -> &sqlx::Pool<sqlx::Any> {
        &self.pool
    }
}
