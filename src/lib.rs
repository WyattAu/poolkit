#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Database connection pool management for Rust.

mod error;
/// Health check types.
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
    /// Maximum connections in the pool.
    pub max_connections: u32,
    /// Minimum connections in the pool.
    pub min_connections: u32,
    /// Idle timeout in seconds.
    pub idle_timeout_secs: u64,
    /// Acquire timeout in seconds.
    pub acquire_timeout_secs: u64,
    /// Current pool size.
    pub size: u32,
    /// Number of idle connections.
    pub idle: u32,
    /// Number of active connections.
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

    // ---- Additional DbPoolBuilder tests ----

    #[test]
    fn db_pool_builder_zero_connections() {
        let builder = DbPoolBuilder::new("sqlite::memory:")
            .max_connections(0)
            .min_connections(0);
        assert_eq!(builder.max_connections, 0);
        assert_eq!(builder.min_connections, 0);
    }

    #[test]
    fn db_pool_builder_large_timeouts() {
        let builder = DbPoolBuilder::new("sqlite::memory:")
            .idle_timeout(Duration::from_secs(3600))
            .acquire_timeout(Duration::from_secs(600));
        assert_eq!(builder.idle_timeout, Duration::from_secs(3600));
        assert_eq!(builder.acquire_timeout, Duration::from_secs(600));
    }

    #[test]
    fn db_pool_builder_zero_timeouts() {
        let builder = DbPoolBuilder::new("sqlite::memory:")
            .idle_timeout(Duration::from_secs(0))
            .acquire_timeout(Duration::from_millis(0));
        assert_eq!(builder.idle_timeout, Duration::from_secs(0));
        assert_eq!(builder.acquire_timeout, Duration::from_millis(0));
    }

    #[test]
    fn db_pool_builder_database_url_preserved() {
        let urls = vec![
            "sqlite::memory:",
            "postgres://user:pass@localhost:5432/mydb",
            "mysql://root@127.0.0.1/test",
        ];
        for url in urls {
            let builder = DbPoolBuilder::new(url);
            assert_eq!(builder.database_url, url);
        }
    }

    #[test]
    fn db_pool_builder_overwrite_max_connections() {
        let builder = DbPoolBuilder::new("sqlite::memory:")
            .max_connections(50)
            .max_connections(100);
        assert_eq!(builder.max_connections, 100);
    }

    #[test]
    fn db_pool_builder_overwrite_min_connections() {
        let builder = DbPoolBuilder::new("sqlite::memory:")
            .min_connections(5)
            .min_connections(1);
        assert_eq!(builder.min_connections, 1);
    }

    #[test]
    fn db_pool_builder_overwrite_timeouts() {
        let builder = DbPoolBuilder::new("sqlite::memory:")
            .idle_timeout(Duration::from_secs(120))
            .idle_timeout(Duration::from_secs(60));
        assert_eq!(builder.idle_timeout, Duration::from_secs(60));
    }

    // ---- Additional HealthStatus / HealthCheckResult tests ----

    #[test]
    fn health_status_debug_format() {
        assert_eq!(format!("{:?}", HealthStatus::Healthy), "Healthy");
        assert_eq!(format!("{:?}", HealthStatus::Unhealthy), "Unhealthy");
    }

    #[test]
    fn health_status_clone() {
        let status = HealthStatus::Healthy;
        let cloned = status;
        assert_eq!(status, cloned);
    }

    #[test]
    fn health_status_copy() {
        let status = HealthStatus::Unhealthy;
        let copied = status;
        assert_eq!(status, copied);
    }

    #[test]
    fn health_check_result_debug_format() {
        let result = HealthCheckResult {
            status: HealthStatus::Healthy,
            latency_ms: 5,
            message: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Healthy"));
        assert!(debug_str.contains("5"));
    }

    #[test]
    fn health_check_result_clone() {
        let original = HealthCheckResult {
            status: HealthStatus::Unhealthy,
            latency_ms: 100,
            message: Some("timeout".into()),
        };
        let cloned = original.clone();
        assert_eq!(cloned.status, HealthStatus::Unhealthy);
        assert_eq!(cloned.latency_ms, 100);
        assert_eq!(cloned.message.as_deref(), Some("timeout"));
    }

    #[test]
    fn health_check_result_zero_latency() {
        let result = HealthCheckResult {
            status: HealthStatus::Healthy,
            latency_ms: 0,
            message: None,
        };
        assert!(result.is_healthy());
        assert_eq!(result.latency_ms, 0);
    }

    #[test]
    fn health_check_result_large_latency() {
        let result = HealthCheckResult {
            status: HealthStatus::Unhealthy,
            latency_ms: u64::MAX,
            message: Some("extremely slow".into()),
        };
        assert!(!result.is_healthy());
        assert_eq!(result.latency_ms, u64::MAX);
    }

    #[test]
    fn health_check_result_with_long_message() {
        let long_msg = "a".repeat(1000);
        let result = HealthCheckResult {
            status: HealthStatus::Unhealthy,
            latency_ms: 50,
            message: Some(long_msg.clone()),
        };
        assert_eq!(result.message.as_deref(), Some(long_msg.as_str()));
    }

    // ---- Additional PoolError display tests ----

    #[test]
    fn pool_error_connection_display_row_not_found() {
        let err = PoolError::Connection(sqlx::Error::RowNotFound);
        let msg = err.to_string();
        assert!(msg.contains("connection error"));
        assert!(msg.contains("no rows"));
    }

    #[test]
    fn pool_error_connection_display_different_variant() {
        let err = PoolError::Connection(sqlx::Error::PoolClosed);
        let msg = err.to_string();
        assert!(msg.contains("connection error"));
    }

    #[test]
    fn pool_error_timeout_empty_message() {
        let err = PoolError::Timeout("".into());
        let msg = err.to_string();
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn pool_error_timeout_long_message() {
        let err = PoolError::Timeout("acquiring connection from pool timed out after 30s".into());
        let msg = err.to_string();
        assert!(msg.contains("acquiring connection from pool timed out after 30s"));
    }

    #[test]
    fn pool_error_migration_empty_message() {
        let err = PoolError::Migration("".into());
        let msg = err.to_string();
        assert!(msg.contains("migration error"));
    }

    #[test]
    fn pool_error_all_variants_debug() {
        let errors = vec![
            PoolError::Connection(sqlx::Error::RowNotFound),
            PoolError::Timeout("test".into()),
            PoolError::PoolClosed,
            PoolError::Migration("test".into()),
        ];
        for err in errors {
            let debug_str = format!("{:?}", err);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn pool_error_is_std_error() {
        let err = PoolError::Timeout("timeout".into());
        let std_err: &dyn std::error::Error = &err;
        assert!(std_err.to_string().contains("timeout"));
    }

    // ---- PoolStats serialization ----

    #[test]
    fn pool_stats_serialization() {
        let stats = PoolStats {
            max_connections: 10,
            min_connections: 1,
            idle_timeout_secs: 600,
            acquire_timeout_secs: 30,
            size: 5,
            idle: 2,
            active: 3,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"max_connections\":10"));
        assert!(json.contains("\"min_connections\":1"));
        assert!(json.contains("\"idle_timeout_secs\":600"));
        assert!(json.contains("\"acquire_timeout_secs\":30"));
        assert!(json.contains("\"size\":5"));
        assert!(json.contains("\"idle\":2"));
        assert!(json.contains("\"active\":3"));
    }

    #[test]
    fn pool_stats_debug_format() {
        let stats = PoolStats {
            max_connections: 10,
            min_connections: 1,
            idle_timeout_secs: 600,
            acquire_timeout_secs: 30,
            size: 5,
            idle: 2,
            active: 3,
        };
        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("PoolStats"));
        assert!(debug_str.contains("10"));
    }
}
