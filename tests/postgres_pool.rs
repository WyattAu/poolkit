// Tests talk to a real Postgres in docker; unwrap/expect is the test signal.
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(feature = "postgres")]

//! Postgres pool integration tests against a real Postgres server (docker
//! required, spun up per run with testcontainers).
//!
//! ```sh
//! cargo test --features postgres --test postgres_pool
//! ```
//!
//! Proves pool behavior over TCP against a real server: connect + ping,
//! health checks, DDL/DML round trips, contention capping, and pool
//! resilience across connection churn.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use poolkit::{DbPool, HealthStatus};
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

async fn temp_pool(max_connections: u32) -> (testcontainers::ContainerAsync<Postgres>, DbPool) {
    let container = Postgres::default().start().await.unwrap();
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    let pool = DbPool::builder(url)
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(10))
        .build()
        .await
        .unwrap();
    (container, pool)
}

#[tokio::test]
async fn pool_connects_pings_and_reports_healthy() {
    let (_c, pool) = temp_pool(2).await;
    pool.ping().await.unwrap();
    let health = pool.health_check().await;
    assert_eq!(health.status, HealthStatus::Healthy, "{health:?}");
    assert!(
        health.latency_ms <= 5_000,
        "health latency sanity: {health:?}"
    );
}

#[tokio::test]
async fn ddl_dml_roundtrip_on_postgres() {
    let (_c, pool) = temp_pool(2).await;

    sqlx::query("CREATE TABLE poolkit_pg_test (id BIGINT PRIMARY KEY, label TEXT NOT NULL)")
        .execute(pool.inner())
        .await
        .unwrap();
    for i in 0..10i64 {
        sqlx::query("INSERT INTO poolkit_pg_test (id, label) VALUES ($1, $2)")
            .bind(i)
            .bind(format!("row-{i}"))
            .execute(pool.inner())
            .await
            .unwrap();
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM poolkit_pg_test")
        .fetch_one(pool.inner())
        .await
        .unwrap();
    assert_eq!(count, 10);
    assert_eq!(pool.health_check().await.status, HealthStatus::Healthy);
}

#[tokio::test]
async fn contention_caps_concurrent_checkouts_at_max() {
    let (_c, pool) = temp_pool(2).await;
    let pool = Arc::new(pool);

    let live = Arc::new(AtomicUsize::new(0));
    let max_live = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for worker in 0..8 {
        let pool = pool.clone();
        let live = live.clone();
        let max_live = max_live.clone();
        handles.push(tokio::spawn(async move {
            let mut conn = pool
                .inner()
                .acquire()
                .await
                .unwrap_or_else(|e| panic!("worker {worker} acquire failed: {e:?}"));
            let now = live.fetch_add(1, Ordering::SeqCst) + 1;
            max_live.fetch_max(now, Ordering::SeqCst);
            sqlx::query("SELECT 1").execute(&mut *conn).await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
            live.fetch_sub(1, Ordering::SeqCst);
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(
        max_live.load(Ordering::SeqCst),
        2,
        "checkout cap must hold over TCP"
    );

    // Pool bookkeeping (idle reaping) is asynchronous; poll briefly.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while pool.stats().active != 0 && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(pool.stats().active, 0);
}

#[tokio::test]
async fn pool_survives_failed_queries_and_connection_churn() {
    let (_c, pool) = temp_pool(2).await;

    // A failing query must not poison the pool.
    let failed = sqlx::query("SELECT * FROM missing_table")
        .execute(pool.inner())
        .await;
    assert!(failed.is_err());
    assert_eq!(pool.health_check().await.status, HealthStatus::Healthy);

    // Churn: open and drop many short-lived checkouts.
    for _ in 0..20 {
        let mut conn = pool.inner().acquire().await.unwrap();
        sqlx::query("SELECT 1").execute(&mut *conn).await.unwrap();
    }
    // Pool bookkeeping (idle reaping) is asynchronous; poll briefly.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while pool.stats().active != 0 && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(pool.stats().active, 0);
    assert_eq!(pool.health_check().await.status, HealthStatus::Healthy);
}

#[tokio::test]
async fn refuse_to_connect_when_server_is_unreachable() {
    sqlx::any::install_default_drivers();
    // Port 1 on loopback: nothing listens there.
    let result = DbPool::builder("postgres://postgres:postgres@127.0.0.1:1/nope")
        .acquire_timeout(Duration::from_secs(2))
        .build()
        .await;
    match result {
        Err(poolkit::PoolError::Connection(_)) => {}
        Err(other) => panic!("expected Connection error, got: {other:?}"),
        Ok(_) => panic!("unreachable server must not produce a usable pool"),
    }
}
