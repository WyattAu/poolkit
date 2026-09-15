# Changelog

All notable changes to this project are documented here. Format: [Keep a
Changelog](https://keepachangelog.com/) — versions follow [semver](https://semver.org).

## [Unreleased]

### Changed
- Removed the unused direct `tokio` dependency (runtime is supplied by
  `sqlx`'s `runtime-tokio` feature and downstream consumers).

## [0.1.2] - 2026-09-12

### Fixed

- **`DbPool` could never connect — `sqlx::AnyPool` had no drivers
  registered.** The `postgres`/`sqlite` features enabled the standalone
  `sqlx-postgres`/`sqlx-sqlite` crates, but `sqlx::any` only registers
  drivers compiled into the `sqlx` facade. Features now enable
  `sqlx/postgres` and `sqlx/sqlite` directly (which also restores bundled
  SQLite), and `DbPoolBuilder::build` calls the idempotent
  `sqlx::any::install_default_drivers()`. Caught by the new integration
  suite: every connect previously failed at runtime.

### Added

- SQLite pool integration suite (`tests/sqlite_pool.rs`, 8 tests, fully
  local in a tempdir): connect + ping, health checks with latency, stats
  reflecting acquire/release/recycle, DDL/DML round trips through the
  pool, failed queries not poisoning the pool, acquire/release under
  contention with a hard checkout cap, the acquire-timeout path
  (`PoolTimedOut`), and connection errors on garbage URLs.
- Postgres pool integration suite (`tests/postgres_pool.rs`, 5 tests)
  against a real Postgres spun up per run with testcontainers: connect/
  ping/health over TCP, DDL/DML round trips, contention capping, resilience
  across failed queries and connection churn, and refusal on unreachable
  servers.

### CI

- New `integration` job running the sqlite suite (no services) and the
  postgres suite (testcontainers).

## [0.1.1] - 2026-09-09

### Fixed
- Bundle `sqlx-sqlite` so the `libsqlite3-sys` feature set matches its
  requirements (the `sqlite` feature now builds hermetically).
- Satisfy the `clippy -D warnings` gate; normalize repo metadata.
- Refresh dependency set.


## [0.1.0] - 2026-09-01

### Added
- Database connection pooling — SQLx wrapper with health checks.
- Published to crates.io (2026-09-01).
