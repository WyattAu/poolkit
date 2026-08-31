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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn db_pool_builder_default_values() {
        let builder = DbPoolBuilder::new("sqlite::memory:");
        assert_eq!(builder.max_connections, 10);
        assert_eq!(builder.min_connections, 1);
        assert_eq!(builder.idle_timeout, Duration::from_secs(600));
        assert_eq!(builder.acquire_timeout, Duration::from_secs(30));
        assert_eq!(builder.database_url, "sqlite::memory:");
    }

    #[test]
    fn db_pool_builder_custom_values() {
        let builder = DbPoolBuilder::new("postgres://localhost/test")
            .max_connections(20)
            .min_connections(5)
            .idle_timeout(Duration::from_secs(120))
            .acquire_timeout(Duration::from_secs(10));
        assert_eq!(builder.max_connections, 20);
        assert_eq!(builder.min_connections, 5);
        assert_eq!(builder.idle_timeout, Duration::from_secs(120));
        assert_eq!(builder.acquire_timeout, Duration::from_secs(10));
        assert_eq!(builder.database_url, "postgres://localhost/test");
    }

    #[test]
    fn db_pool_builder_chaining() {
        let builder = DbPoolBuilder::new("sqlite::memory:")
            .max_connections(50)
            .min_connections(2);
        assert_eq!(builder.max_connections, 50);
        assert_eq!(builder.min_connections, 2);
    }

    #[test]
    fn db_pool_builder_default_factory() {
        let builder = DbPool::builder("sqlite::memory:");
        assert_eq!(builder.max_connections, 10);
        assert_eq!(builder.database_url, "sqlite::memory:");
    }

    #[test]
    fn pool_error_connection_display() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let err = PoolError::Connection(sqlx_err);
        let msg = err.to_string();
        assert!(msg.contains("connection error"));
    }

    #[test]
    fn pool_error_timeout_display() {
        let err = PoolError::Timeout("acquire timed out".into());
        let msg = err.to_string();
        assert!(msg.contains("acquire timed out"));
    }

    #[test]
    fn pool_error_pool_closed_display() {
        let err = PoolError::PoolClosed;
        let msg = err.to_string();
        assert!(msg.contains("pool closed"));
    }

    #[test]
    fn pool_error_migration_display() {
        let err = PoolError::Migration("failed to run migration".into());
        let msg = err.to_string();
        assert!(msg.contains("failed to run migration"));
    }

    #[test]
    fn health_status_is_healthy() {
        let result = HealthCheckResult {
            status: HealthStatus::Healthy,
            latency_ms: 5,
            message: None,
        };
        assert!(result.is_healthy());
    }

    #[test]
    fn health_status_is_unhealthy() {
        let result = HealthCheckResult {
            status: HealthStatus::Unhealthy,
            latency_ms: 100,
            message: Some("connection refused".into()),
        };
        assert!(!result.is_healthy());
    }

    #[test]
    fn health_check_result_creation() {
        let result = HealthCheckResult {
            status: HealthStatus::Healthy,
            latency_ms: 42,
            message: None,
        };
        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(result.latency_ms, 42);
        assert!(result.message.is_none());
    }

    #[test]
    fn health_check_result_with_message() {
        let result = HealthCheckResult {
            status: HealthStatus::Unhealthy,
            latency_ms: 200,
            message: Some("timeout".into()),
        };
        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert_eq!(result.message.as_deref(), Some("timeout"));
    }

    #[test]
    fn health_status_equality() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_eq!(HealthStatus::Unhealthy, HealthStatus::Unhealthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
    }
}
