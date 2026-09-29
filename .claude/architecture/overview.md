# Architecture

Net Monitor is a single binary with four parts. The TUI owns the screen, the monitoring engine owns the background loop, `monitor.rs` knows how to run one check, and the database persists everything.

```
┌──────────────────────────────────────────────────────┐
│ tui.rs                                               │
│ views, forms, key handling, import/export, connect   │
└───────────────┬─────────────────────────┬────────────┘
                │ start / stop / update   │ read & write
                ▼                         ▼
┌──────────────────────────┐   ┌──────────────────────┐
│ monitoring_engine.rs     │   │ database.rs          │
│ per-node tokio tasks,    │──▶│ SQLite: nodes,       │
│ soft/hard state model    │   │ results, changes     │
└───────────┬──────────────┘   └──────────────────────┘
            │ check_node()
            ▼
┌──────────────────────────┐   ┌──────────────────────┐
│ monitor.rs               │   │ connection.rs        │
│ HTTP / TCP / ping checks │   │ browser or ssh on ⏎  │
└──────────────────────────┘   └──────────────────────┘
```

## Components

### `tui.rs`
Renders every view with ratatui and handles all input. Owns the in-memory list of nodes and the `TableState`. Talks to the engine through a `MonitoringHandle` (start, stop, send config updates) and receives status updates over a channel. Import/export, node forms, reorder mode, history, and the about/help overlays all live here. The terminal is restored on panic and around native file dialogs.

### `monitoring_engine.rs`
`start_monitoring` spawns one background thread that owns a tokio runtime and ticks every 250 ms. Each tick drains `NodeConfigUpdate` messages from the TUI (add, update, delete), applies the results of checks that have finished (`evaluate_node_status`, persist runtime state, send the updated node back over a channel), then launches a tokio task running `check_node` for every node that is due. Checks run concurrently, so a slow HTTP timeout never delays the nodes behind it, and a node is never checked twice at once. All per-node state and every database write stay on the engine thread; the tasks only report a result over a channel. Per-node state (previous status, time of the last transition) is seeded from the database on startup so a restart records no duplicate transitions.

The state model:

- **Online** → one failed check → **Degraded** (soft). The node is rechecked every `retry_interval` seconds instead of `monitoring_interval`.
- **Degraded** → `max_check_attempts` consecutive failures → **Offline** (hard).
- Any successful check → **Online**, immediately.

Every transition between the three states writes a `StatusChange` row with the time spent in the previous state, plus the `MonitoringResult` that caused it. Checks that leave the status unchanged are not stored, so the tables grow with transitions, not with checks.

### `monitor.rs`
Pure check functions. `check_http` normalises the URL (default scheme `https://`), accepts invalid certificates, and compares the status code. `check_tcp` resolves the host and tries every address with `connect_timeout`. `check_ping` resolves hostnames, sends up to `count` echo requests, and succeeds on the first reply; it retries with the other ICMP socket type if the platform default is unusable. Failed checks report no latency.

### `database.rs`
One `Connection` per call (no pooling). Creates the three tables on startup and runs idempotent column migrations. See `database-schema.md`.

### `connection.rs`
What Enter does. HTTP nodes open in the default browser via the `open` crate. Ping and TCP nodes spawn the system `ssh` in a new terminal: Terminal.app on macOS, Windows Terminal or `cmd` on Windows, the first of gnome-terminal / konsole / xfce4-terminal / xterm found on Linux. A TCP node's port is passed to `ssh -p`.

### `models.rs`
`Node`, `MonitorDetail` (`Http`, `Ping`, `Tcp`, tagged JSON enum), `NodeStatus`, `MonitoringResult`, `StatusChange`, and `NodeImport` (the import/export shape, which omits runtime state).

## Concurrency

Two long-lived threads plus a task per in-flight check. The main thread renders the TUI and polls for input; the engine thread runs the monitoring loop and owns a multi-threaded tokio runtime on which every check runs as its own task. HTTP and TCP checks are fully async; ping uses the blocking `ping` crate, so it runs on tokio's blocking pool. The TUI and the engine talk over `std::sync::mpsc` channels: config updates and stop signals go in, updated nodes come out. Check tasks report back to the engine thread over another channel and never touch the database or the node list themselves. Both threads open their own SQLite connections per call.

## Data

Everything is local. The database, the log file, and nothing else, in the platform data directory. No secrets are stored: SSH uses the user's own keys, agent, and config.

## Adding a monitor type

1. Add a variant to `MonitorDetail` in `models.rs` and handle it in `get_connection_target` / `get_connection_type`.
2. Implement the check in `monitor.rs` and dispatch to it from `check_node`.
3. Add columns and a migration in `database.rs`, and map them in `add_node`, `update_node`, and the row reader.
4. Add the form fields in `tui.rs` (`NodeForm`, `MonitorTypeForm`, field rendering and validation).
5. Update `sample_nodes.json`, the README table, and the tests.
