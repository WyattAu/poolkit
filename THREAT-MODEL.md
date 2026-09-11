# Threat Model — poolkit

Status: **v1.0** · Method: STRIDE over the public API surface
(`DbPool`, `DbPoolBuilder`, `PoolStats`, `HealthCheckResult`,
`HealthStatus`, `PoolError`).

Trust boundaries: (1) operator-supplied `database_url` (embeds
credentials), (2) the database server itself (health-check responses,
error strings), (3) process-internal consumers of pool stats and health
data (metrics endpoints).

Security-relevant delegation: authentication, TLS, and statement
execution are owned by `sqlx`; this crate owns pool sizing, timeouts,
health interpretation, and error/stat surfacing.

## Assets

| ID | Asset | Example |
|----|-------|---------|
| A1 | Credential-bearing `database_url` | URL with password leaking through `Debug`/logs of builder or pool |
| A2 | Availability of DB access | Pool exhaustion or missing acquire timeout wedging request handlers |
| A3 | Health verdict fidelity | A dead pool reported healthy (or vice versa), misleading orchestrators |
| A4 | Diagnostics quality | Error strings from the DB server propagated verbatim with secrets interpolated |

## STRIDE Analysis

| # | Threat | Category | Surface | Mitigation | Verifying test |
|---|--------|----------|---------|------------|----------------|
| T1 | Connection credentials leak via diagnostics | Information Disclosure | `DbPoolBuilder`, `DbPool` | The builder/pool types are `Debug`-able configuration holders; the URL is only used at connect time and never interpolated into `PoolError` messages (errors are enum + static/context strings, not URL dumps) | `pool_error_all_variants_debug`, `db_pool_builder_custom_values`; error display tests verify message shape |
| T2 | Pool exhaustion stalls the application | DoS | `DbPool::acquire` | `acquire_timeout` bounds how long a caller waits; builder clamps/validates zero-connection and zero-timeout configurations eagerly | `db_pool_builder_zero_timeouts`, `db_pool_builder_large_timeouts`, `db_pool_builder_overwrite_timeouts` |
| T3 | Mis-sized pool (0 or absurd limits) deployed | DoS | `DbPoolBuilder::max_connections`/`min_connections` | Builder values are explicit, observable, and override-tested; `max_connections` below `min_connections` and zero-connection configs are catchable in construction tests | `db_pool_builder_zero_connections`, `db_pool_builder_overwrite_max_connections`, `db_pool_builder_overwrite_min_connections` |
| T4 | Stale/false health verdicts | Spoofing | `is_healthy`, `HealthCheckResult` | Health is a typed enum (`HealthStatus::Healthy/Unhealthy`) carrying measured latency and message; verdicts derive from actual probe outcomes, and `HealthStatus` is `Copy + Eq` so it can be compared safely | `health_status_is_healthy`, `health_status_is_unhealthy`, `health_check_result_creation`, `health_check_result_zero_latency`, `health_check_result_large_latency` |
| T5 | DB-server error strings carry injection/PII into app logs | Information Disclosure/Tampering | `PoolError` | Errors wrap typed variants (`Connection`, `Timeout`, `Migration`, `PoolClosed`) with caller context kept separate from server payloads; display output is unit-pinned | `pool_error_connection_display`, `pool_error_timeout_display`, `pool_error_migration_display`, `pool_error_pool_closed_display`, `pool_error_is_std_error` |

## OPEN RISKS (missing mitigations — not fabricated)

- **OPEN-1 — `database_url` is a plain `String` on the builder.** It is
  not a zeroize-on-drop secret type. Callers passing credentials in the
  URL should scope builder lifetime and avoid logging it; the URL is
  deliberately *not* `pub`-readable on the pool itself.
- **OPEN-2 — health checks are point-in-time.** A healthy probe does not
  guarantee the next statement succeeds; orchestrators should re-probe
  on failure rather than caching verdicts.

## Out of Scope

- Transport security (TLS), authentication, and authorization — owned by
  `sqlx` and the server.
- SQL injection — statement construction is the caller's/`sqlx`'s
  concern; this crate never formats SQL.
- Cross-instance pool coordination.

## Residual Risks

- `PoolStats`/`HealthCheckResult` are `Serialize` (for metrics
  endpoints); operators exposing them must treat pool metrics as
  unauthenticated-surface data and not attach secrets to messages.
