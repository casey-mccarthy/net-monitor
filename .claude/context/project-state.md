# Project State

Orientation for a new session. Facts here are checked against the code; if they drift, fix them.

## What it is

- **net-monitor**: a Rust terminal application (ratatui + crossterm) that monitors HTTP endpoints, TCP ports, and ping targets and stores results in SQLite.
- **Repository**: https://github.com/casey-mccarthy/net-monitor
- **Version**: whatever `Cargo.toml` says. The release workflow bumps it; never bump it by hand.
- **Default branch**: `main`, protected. All work goes through a pull request and a rebase.

## Stack

| Concern | Crate |
|---|---|
| TUI | `ratatui`, `crossterm` |
| Async runtime | `tokio` |
| HTTP checks | `reqwest` (accepts self-signed certificates) |
| Ping | `ping` (unprivileged datagram sockets where available) |
| TCP | `std::net::TcpStream::connect_timeout` |
| Storage | `rusqlite` with bundled SQLite |
| File dialogs | `rfd` (needs GTK 3 on Linux) |
| Open browser | `open` |
| Logging | `tracing` to `net-monitor.log` in the data directory |

There is no SSH library and no crypto: connecting to a node shells out to the system `ssh` in a new terminal window. The encrypted credential store that used to exist was removed in #98.

## Layout

```
src/main.rs               entry point, logging, database path
src/tui.rs                all views, forms, and key handling (largest file)
src/monitoring_engine.rs  background loop, soft/hard state model
src/monitor.rs            single-check implementations: HTTP, TCP, ping
src/database.rs           SQLite schema, migrations, queries
src/models.rs             data types shared by everything
src/connection.rs         browser / SSH launch on Enter
src/config.rs             empty AppConfig scaffold, unused
tests/                    integration tests, one file per module
scripts/release-notes.sh  release notes generator used by the release workflow
```

## Workflow reminders

- Run `cargo fmt`, clippy with `-D warnings`, and the tests before every commit. `CLAUDE.md` has the exact commands.
- Commit messages are conventional commits. They drive the version bump and the release notes.
- Releases are automatic on merge to `main`. See `.claude/commands/release.md`.
- `sample_nodes.json` must stay importable; `tests/import_export_tests.rs` checks it.

## Session checklist

1. `git status` and `git log --oneline -10`
2. Skim open issues and PRs on GitHub
3. `RUSTFLAGS="-A dead_code" cargo test`
4. Check `.claude/features/in-progress.md` for anything mid-flight
