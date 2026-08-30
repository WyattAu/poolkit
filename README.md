# poolkit

Database connection pooling for Rust — SQLx wrapper with health checks, lazy connections, test containers, and metrics.

## Purpose

`poolkit` wraps SQLx's connection pool to provide a batteries-included experience for database-backed Rust services:

- **Health checks** — built-in `SELECT 1` probes with latency reporting.
- **Lazy connections** — connections are only opened on first use.
- **Test containers** — optional integration with `testcontainers` for ephemeral test databases.
- **Metrics** — pool stats (active, idle, max, min) at your fingertips.

## Usage

```rust
use poolkit::DbPool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = DbPool::builder("postgres://localhost/mydb")
        .max_connections(20)
        .min_connections(2)
        .build()
        .await?;

    let health = pool.health_check().await;
    println!("Health: {:?} ({}ms)", health.status, health.latency_ms);

    let stats = pool.stats();
    println!("Pool size: {}, active: {}, idle: {}", stats.size, stats.active, stats.idle);

    Ok(())
}
```

## Comparison with raw sqlx

| Feature | `sqlx::Pool` | `poolkit::DbPool` |
|---|---|---|
| Connection pooling | Yes | Yes (wraps sqlx) |
| Health checks | Manual | Built-in |
| Pool stats | `Pool::size()` only | Full stats struct |
| Config builder | Chained options | Dedicated builder |
| Test containers | DIY | Optional feature |
| Forbidden unsafe | No | Yes |

## Features

- `default` — enables `postgres`
- `postgres` — PostgreSQL support via `sqlx-postgres`
- `sqlite` — SQLite support via `sqlx-sqlite`
- `testcontainers` — ephemeral test database containers

## MSRV

Rust **1.85** (edition 2024).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
