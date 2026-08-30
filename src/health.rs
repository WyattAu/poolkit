/// Health status of a database connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum HealthStatus {
    /// The connection is healthy and responding.
    Healthy,
    /// The connection is degraded or unreachable.
    Unhealthy,
}

/// Result of a health check against the database.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthCheckResult {
    /// Current health status.
    pub status: HealthStatus,
    /// Latency of the health check in milliseconds.
    pub latency_ms: u64,
    /// Optional error message if the check failed.
    pub message: Option<String>,
}

impl HealthCheckResult {
    /// Returns `true` if the status is [`HealthStatus::Healthy`].
    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }
}
