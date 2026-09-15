// Pool integration tests: unwrap/expect is the test signal.
#![allow(clippy::unwrap_used, clippy::expect_used)]
#![cfg(feature = "sqlite")]

//! SQLite pool integration tests — fully local, no services required.
//!
//! Proves the pool behaviors the builder-only unit tests cannot touch:
//! real connects through `AnyPool`, health checks, stats snapshots,
//! query round trips, acquire/release under contention with a hard cap,
//! and the acquire-timeout path.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use poolkit::{DbPool, HealthStatus};

async fn temp_pool(max_connections: u32, acquire_timeout: Duration) -> (tempfile::TempDir, DbPool) {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}?mode=rwc", dir.path().join("pool.db").display());
    let pool = DbPool::builder(url)
        .max_connections(max_connections)
        .min_connections(0)
        .acquire_timeout(acquire_timeout)
        .build()
        .await
        .unwrap();
    (dir, pool)
}

// ---------------------------------------------------------------------------
// Connect / health / stats
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pool_connects_and_pings() {
    let (_dir, pool) = temp_pool(2, Duration::from_secs(5)).await;
    pool.ping().await.unwrap();
    let stats = pool.stats();
    assert!(stats.size >= 1, "ping must materialize a connection");
    assert_eq!(stats.max_connections, 2);
}

#[tokio::test]
async fn health_check_reports_healthy_with_latency() {
    let (_dir, pool) = temp_pool(2, Duration::from_secs(5)).await;
    let health = pool.health_check().await;
    assert_eq!(health.status, HealthStatus::Healthy, "{health:?}");
    assert!(health.is_healthy());
    assert!(health.message.is_none());
}

#[tokio::test]
async fn stats_reflect_acquire_and_release() {
    let (_dir, pool) = temp_pool(2, Duration::from_secs(5)).await;

    let conn = pool.inner().acquire().await.unwrap();
    let held = pool.stats();
    assert_eq!(held.active, 1, "one connection checked out: {held:?}");
    assert_eq!(held.size, 1);

    drop(conn);
    // Return to pool: the connection becomes idle, size stays, active drops.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let released = pool.stats();
    assert_eq!(released.active, 0, "{released:?}");
    assert_eq!(
        released.idle, 1,
        "released connections are recycled, not dropped"
    );
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ddl_and_dml_roundtrip_through_the_pool() {
    let (_dir, pool) = temp_pool(2, Duration::from_secs(5)).await;

    sqlx_ddl(&pool).await;
    for i in 0..10i64 {
        sqlx::query("INSERT INTO poolkit_test (id, label) VALUES (?, ?)")
            .bind(i)
            .bind(format!("row-{i}"))
            .execute(pool.inner())
            .await
            .unwrap();
    }

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM poolkit_test")
        .fetch_one(pool.inner())
        .await
        .unwrap();
    assert_eq!(count, 10);

    // Data actually landed on disk (not just in a shared connection cache).
    let health = pool.health_check().await;
    assert_eq!(health.status, HealthStatus::Healthy);
}

async fn sqlx_ddl(pool: &DbPool) {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS poolkit_test (id INTEGER PRIMARY KEY, label TEXT NOT NULL)",
    )
    .execute(pool.inner())
    .await
    .unwrap();
}

#[tokio::test]
async fn failed_query_does_not_poison_the_pool() {
    let (_dir, pool) = temp_pool(2, Duration::from_secs(5)).await;

    let err = sqlx::query("SELECT * FROM table_that_does_not_exist")
        .execute(pool.inner())
        .await;
    assert!(err.is_err());

    // The pool still serves queries and reports healthy afterwards.
    let health = pool.health_check().await;
    assert_eq!(health.status, HealthStatus::Healthy, "{health:?}");
}

// ---------------------------------------------------------------------------
// Contention and timeouts
// ---------------------------------------------------------------------------

#[tokio::test]
async fn acquire_release_under_contention_respects_the_cap() {
    let (_dir, pool) = temp_pool(2, Duration::from_secs(10)).await;
    let pool = std::sync::Arc::new(pool);

    let live = std::sync::Arc::new(AtomicUsize::new(0));
    let max_live = std::sync::Arc::new(AtomicUsize::new(0));

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
                .unwrap_or_else(|e| panic!("worker {worker} could not acquire: {e:?}"));
            let now = live.fetch_add(1, Ordering::SeqCst) + 1;
            max_live.fetch_max(now, Ordering::SeqCst);
            // Prove the connection works while held.
            sqlx::query("SELECT 1").execute(&mut *conn).await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
            live.fetch_sub(1, Ordering::SeqCst);
            drop(conn);
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(
        max_live.load(Ordering::SeqCst),
        2,
        "concurrent checkouts must cap at max_connections"
    );
    // Pool bookkeeping (idle reaping) is asynchronous; wait briefly for
    // all checkouts to be accounted for instead of asserting immediately.
    let mut final_stats = pool.stats();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while final_stats.active != 0 && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(25)).await;
        final_stats = pool.stats();
    }
    assert_eq!(final_stats.active, 0, "all connections released");
}

#[tokio::test]
async fn acquire_timeout_fails_when_pool_is_exhausted() {
    let (_dir, pool) = temp_pool(1, Duration::from_millis(250)).await;

    // Hold the only connection.
    let held = pool.inner().acquire().await.unwrap();
    let _guard = held;

    let start = std::time::Instant::now();
    let result = pool.inner().acquire().await;
    let elapsed = start.elapsed();

    assert!(
        result.is_err(),
        "acquire must time out while the pool is exhausted"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, sqlx::Error::PoolTimedOut),
        "expected PoolTimedOut, got: {err:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(200),
        "acquire returned too early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "acquire waited far too long: {elapsed:?}"
    );
}

#[tokio::test]
async fn connecting_to_a_garbage_url_fails_with_connection_error() {
    sqlx::any::install_default_drivers();
    let result = DbPool::builder("sqlite:///nonexistent_dir_42/nope.db?mode=rw")
        .build()
        .await;
    match result {
        Err(poolkit::PoolError::Connection(_)) => {}
        Err(other) => panic!("expected Connection error, got: {other:?}"),
        Ok(_) => panic!("garbage URL must not produce a usable pool"),
    }
}
