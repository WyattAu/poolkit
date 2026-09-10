# Changelog

All notable changes to this project are documented here. Format: [Keep a
Changelog](https://keepachangelog.com/) — versions follow [semver](https://semver.org).

## [Unreleased]

### Changed
- Removed the unused direct `tokio` dependency (runtime is supplied by
  `sqlx`'s `runtime-tokio` feature and downstream consumers).

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
