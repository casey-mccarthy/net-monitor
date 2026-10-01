# Database schema

SQLite, one file: `network_monitor.db` in the platform data directory. `Database::new` creates the tables if they are missing and then runs the migrations below, every start, idempotently. There is no schema version table; each migration checks `PRAGMA table_info` for the column it adds.

## Tables

### `nodes`

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PRIMARY KEY AUTOINCREMENT | |
| `name` | TEXT NOT NULL | |
| `monitor_type` | TEXT NOT NULL | `Http`, `Ping`, or `Tcp` |
| `status` | TEXT NOT NULL | `Online`, `Degraded`, or `Offline` |
| `last_check` | TEXT | RFC 3339 UTC |
| `response_time` | INTEGER | milliseconds, NULL for failed checks |
| `monitoring_interval` | INTEGER NOT NULL DEFAULT 5 | seconds |
| `http_url` | TEXT | Http only |
| `http_expected_status` | INTEGER | Http only |
| `ping_host` | TEXT | Ping only |
| `ping_count` | INTEGER | Ping only |
| `ping_timeout` | INTEGER | Ping only, seconds |
| `tcp_host` | TEXT | Tcp only |
| `tcp_port` | INTEGER | Tcp only |
| `tcp_timeout` | INTEGER | Tcp only, seconds |
| `display_order` | INTEGER | user-chosen row order |
| `consecutive_failures` | INTEGER NOT NULL DEFAULT 0 | runtime state for the soft/hard model |
| `max_check_attempts` | INTEGER NOT NULL DEFAULT 3 | failures before Offline |
| `retry_interval` | INTEGER NOT NULL DEFAULT 15 | seconds between checks while Degraded |

Only the columns for the node's own `monitor_type` are populated; the rest are NULL.

### `monitoring_results`

One row per status transition, plus the first check a node ever gets. Checks that leave the status unchanged are not stored.

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PRIMARY KEY AUTOINCREMENT | |
| `node_id` | INTEGER NOT NULL | `REFERENCES nodes(id) ON DELETE CASCADE` |
| `timestamp` | TEXT NOT NULL | RFC 3339 UTC |
| `status` | TEXT NOT NULL | the node's evaluated status after the check (`Online`, `Degraded`, or `Offline`) |
| `response_time` | INTEGER | milliseconds, NULL on failure |
| `details` | TEXT | human-readable outcome or error |

### `status_changes`

One row per transition between Online, Degraded, and Offline. A node's first check ever does not write one.

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PRIMARY KEY AUTOINCREMENT | |
| `node_id` | INTEGER NOT NULL | `REFERENCES nodes(id) ON DELETE CASCADE` |
| `from_status` | TEXT NOT NULL | |
| `to_status` | TEXT NOT NULL | |
| `changed_at` | TEXT NOT NULL | RFC 3339 UTC, the timestamp of the check that caused the change |
| `duration_ms` | INTEGER | time spent in `from_status`; NULL for the first record |
| `last_success_at` | TEXT | RFC 3339 UTC, the last successful check as of this change: the check itself for a change into Online, the last check that still succeeded for a change out of it; NULL for rows written before the column existed |

Indexes: `idx_status_changes_node_id`, `idx_status_changes_changed_at`.

The runtime-state update on `nodes`, the `status_changes` row, and the `monitoring_results` row for one check are written in a single transaction (`record_check`).

### `engine_runs`

One row per run of the monitoring engine. The engine inserts it when it starts and updates `last_alive_at` every 5 seconds and on a clean stop. The time between one run's `last_alive_at` and the next run's `started_at` is a monitoring gap.

| Column | Type | Notes |
|---|---|---|
| `id` | INTEGER PRIMARY KEY AUTOINCREMENT | |
| `started_at` | TEXT NOT NULL | RFC 3339 UTC |
| `last_alive_at` | TEXT NOT NULL | RFC 3339 UTC |

## Migrations

Run in this order on every startup, in `Database::init_tables`:

1. `migrate_tcp_columns` adds `tcp_host`, `tcp_port`, `tcp_timeout`.
2. `migrate_unknown_status` rewrites the retired `Unknown` status to `Offline` in all three tables.
3. `migrate_display_order_column` adds `display_order` and backfills it alphabetically.
4. `migrate_retry_columns` adds `consecutive_failures`, `max_check_attempts`, `retry_interval`.
5. `migrate_last_success_column` adds `status_changes.last_success_at`.

`engine_runs` is created by `CREATE TABLE IF NOT EXISTS` like the others and needs no migration.

Databases created by old versions may still carry a `credential_id` column and a `credentials` table from the removed credential store. Nothing reads them.

Adding a column: write a `migrate_*` function that checks `PRAGMA table_info`, `ALTER TABLE ... ADD COLUMN` with a default, and call it from `init_tables`. Keep it idempotent.

## Queries worth knowing

All in `database.rs`:

- `get_all_nodes` orders by `display_order`, then name.
- `get_latest_monitoring_result(node_id)` and `get_first_check_time(node_id)` are the two ends of a node's results.
- `get_status_changes_ascending(node_id)` feeds `history::periods` and `history::outages`; `get_latest_status_change(node_id)` seeds the engine on startup.
- `get_current_status_duration(node_id)` is the Uptime/Downtime column: since the latest change, or since the first check when there has never been one.
- `calculate_uptime_percentage(node_id, from, to)` sums `duration_ms` by `from_status` over a window.
- `record_check(node, change, result)` is the engine's one write per check.
- `start_engine_run`, `touch_engine_run`, `get_engine_runs` back the monitoring gaps.

## Housekeeping

There is no automatic pruning or VACUUM. Both history tables grow with status transitions, not with checks, and `engine_runs` grows by one row per start, so a stable network produces very little data. Deleting a node cascades to its results and status changes.
