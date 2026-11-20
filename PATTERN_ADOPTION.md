# PATTERN_ADOPTION: poolkit

Relationship to the WyattAu pattern canon. Statuses follow the closed
vocabulary: adopted, partial, planned, refused, n/a. Evidence paths are
relative to this repo.

| Pattern | Status | Evidence |
|---|---|---|
| P52 | n/a | One shared SQLx pool with global min and max connection counts (src/lib.rs:16-23). No per-tenant partitioning exists, so the bulkhead-isolation pattern does not apply |
| P94 | partial | On-demand health probe: src/lib.rs:124-133 (SELECT 1 with latency reporting). The crate provides the sensing primitive; there is no heartbeat loop and no supervision, so it is not the full watchdog pattern |
