# Requirements — poolkit

Numbered, testable requirements. Every requirement maps to at least one named
test; every security-relevant test cites at least one requirement. Threat
IDs reference `THREAT-MODEL.md`.

Scope note: `poolkit` provides database connection pooling for Rust — a
`sqlx` wrapper with builder-configured sizing/timeouts, health checks,
lazy connections, test-container helpers, and pool metrics.

## Functional

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-PK-001 | `DbPool::new(database_url)` and `DbPool::builder(database_url)` construct pool configuration; the URL is preserved verbatim through the builder | MUST |
| REQ-PK-002 | `DbPoolBuilder` exposes `max_connections`, `min_connections`, `acquire_timeout`, and `idle_timeout`, each chainable and overridable | MUST |
| REQ-PK-003 | `DbPool::stats` reports live pool statistics (`PoolStats`) | MUST |
| REQ-PK-004 | `is_healthy`/health-check paths produce `HealthCheckResult` with a `HealthStatus` verdict, latency, and optional message | MUST |
| REQ-PK-005 | `PoolError` covers connection, timeout, migration, and pool-closed failures | MUST |
| REQ-PK-006 | `inner()` grants escape-hatch access to the underlying `sqlx` pool | SHOULD |

## Security

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-PK-100 | Acquire timeouts are configurable so pool contention cannot wedge callers indefinitely (T2) | MUST |
| REQ-PK-101 | Zero-connection and zero-timeout configurations are constructible-and-detectable (not silently coerced into "infinite") so unsafe pool configs fail visibly (T2, T3) | MUST |
| REQ-PK-102 | Error variants render typed, context-separated `Display` messages — database URLs are never interpolated into error output (T1, T5) | MUST |
| REQ-PK-103 | Health verdicts are typed enums derived from probe outcomes, never stringly-taxed booleans (T4) | MUST |

## Robustness

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-PK-200 | All error variants implement `Debug` and `std::error::Error`; `HealthStatus` and `HealthCheckResult` are `Clone`/`Serialize` for metrics pipelines | SHOULD |
| REQ-PK-201 | `PoolStats` serializes for metrics export and formats losslessly via `Debug` | SHOULD |

## Traceability Matrix

| Requirement | Test (fn, file) | Property class |
|-------------|-----------------|----------------|
| REQ-PK-001 | `db_pool_builder_default_factory`, `db_pool_builder_database_url_preserved`, `db_pool_builder_default_values` (`src/lib.rs` tests) | unit |
| REQ-PK-002 | `db_pool_builder_chaining`, `db_pool_builder_custom_values`, `db_pool_builder_overwrite_max_connections`, `db_pool_builder_overwrite_min_connections`, `db_pool_builder_overwrite_timeouts`, `db_pool_builder_large_timeouts` | unit |
| REQ-PK-003 | `pool_stats_debug_format`, `pool_stats_serialization` | unit |
| REQ-PK-004 | `health_check_result_creation`, `health_check_result_with_message`, `health_status_is_healthy`, `health_status_is_unhealthy` (`src/health.rs` tests) | unit |
| REQ-PK-005 | `pool_error_all_variants_debug`, `pool_error_connection_display`, `pool_error_timeout_display`, `pool_error_migration_display`, `pool_error_pool_closed_display` (`src/error.rs` tests) | unit |
| REQ-PK-006 | `inner` accessor (lib) | unit |
| REQ-PK-100 | `db_pool_builder_overwrite_timeouts`, `db_pool_builder_zero_timeouts` | unit |
| REQ-PK-101 | `db_pool_builder_zero_connections`, `db_pool_builder_zero_timeouts` | unit |
| REQ-PK-102 | `pool_error_connection_display`, `pool_error_timeout_display`, `pool_error_is_std_error` | unit |
| REQ-PK-103 | `health_status_is_healthy`, `health_status_is_unhealthy`, `health_check_result_zero_latency`, `health_check_result_large_latency` | unit |
| REQ-PK-200 | `pool_error_is_std_error`, `health_status_clone`, `health_status_copy`, `health_check_result_clone`, `health_status_equality`, `health_status_debug_format`, `health_check_result_debug_format` | unit |
| REQ-PK-201 | `pool_stats_serialization`, `pool_stats_debug_format`, `health_check_result_with_long_message`, `pool_error_timeout_long_message`, `pool_error_timeout_empty_message`, `pool_error_migration_empty_message`, `pool_error_connection_display_different_variant`, `pool_error_connection_display_row_not_found` | unit |

## Test Count

- 37 `#[test]` functions in the unit suite.
- All-features suite passes with 0 failures; no-default-features suite passes.
