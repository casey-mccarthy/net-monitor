# Tests

Integration tests live here, one file per module. Unit tests live next to the code they test in `src/` (`monitor.rs`, `monitoring_engine.rs`, and `tui.rs` have their own `#[cfg(test)]` modules).

| File | Covers |
|---|---|
| `models_tests.rs` | `Node`, `MonitorDetail`, `NodeStatus`, `StatusChange`, JSON serialisation |
| `database_tests.rs` | Schema creation and migration, node CRUD, monitoring results, status changes, uptime queries |
| `monitoring_tests.rs` | HTTP, TCP, and ping checks, plus the soft/hard state transitions |
| `import_export_tests.rs` | The `NodeImport` JSON format, including `sample_nodes.json` |
| `connection_tests.rs` | SSH target parsing and command construction |
| `config_tests.rs` | `AppConfig` serialisation |
| `tui_tests.rs` | TUI state helpers that can be exercised without a terminal |
| `common/mod.rs` | Shared fixtures: `TestDatabase`, `NodeBuilder`, sample nodes, assertions |

## Running

```bash
RUSTFLAGS="-A dead_code" cargo test                          # everything CI runs
RUSTFLAGS="-A dead_code" cargo test --test database_tests    # one file
RUSTFLAGS="-A dead_code" cargo test test_check_node_http     # tests matching a name
RUSTFLAGS="-A dead_code" cargo test -- --nocapture           # show stdout
RUSTFLAGS="-A dead_code" cargo test -- --test-threads=1      # run serially
```

`RUSTFLAGS="-A dead_code"` matches CI; the project keeps some unused code for planned features and treats every other warning as an error.

## Network tests

Tests that reach real hosts (httpbin.org, public DNS, ICMP to loopback) are gated behind the `network-tests` Cargo feature and never run in CI:

```bash
RUSTFLAGS="-A dead_code" cargo test --features network-tests
```

Ping tests need ICMP access. On Linux that means unprivileged ICMP sockets (`net.ipv4.ping_group_range`) or `CAP_NET_RAW`; on Windows it means a raw socket, so an elevated shell.

## Writing tests

- Use `TestDatabase::new()` from `tests/common/mod.rs` for anything that touches SQLite. It creates a temporary file and removes it on drop. Never point a test at the real data directory.
- Build nodes with `NodeBuilder` or the `fixtures` module rather than constructing `Node` by hand.
- Async tests use `#[tokio::test]`.
- Anything that needs the network goes behind `#[cfg(feature = "network-tests")]`.
- Prefer testing behaviour through the public API in `src/lib.rs`; private helpers get unit tests inside their own module.

## Coverage

```bash
cargo install cargo-llvm-cov
cargo llvm-cov --html --open
```

CI uploads an lcov report to Codecov on every push. `codecov.yml` asks for 70% on new code and tolerates a 1% drop overall.
