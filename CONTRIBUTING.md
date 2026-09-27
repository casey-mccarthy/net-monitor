# Contributing to Net Monitor

Thanks for helping. This document covers how to get a change from your editor into a release. The short version: branch, conventional commit, green checks, pull request, rebase, merge. Releases take care of themselves.

## Reporting issues and proposing features

- Search the existing issues first.
- Use the bug report or feature request template; both live in `.github/ISSUE_TEMPLATE/`.
- For bugs, include your OS, how you installed net-monitor, and the relevant lines from `net-monitor.log` (see the README for where it lives).
- For features, describe the problem you are solving, not just the solution. `.claude/features/planned.md` lists what is already on the list.

## Development setup

You need a current stable Rust toolchain. On Linux you also need `libgtk-3-dev`, `libssl-dev`, and `pkg-config`.

```bash
git clone https://github.com/YOUR_USERNAME/net-monitor.git
cd net-monitor
git remote add upstream https://github.com/casey-mccarthy/net-monitor.git

cargo build
RUSTFLAGS="-A dead_code" cargo test
cargo run
```

`RUSTFLAGS="-A dead_code"` is deliberate: the project keeps some unused code around for planned features, and CI treats every other warning as an error.

## Project layout

```
net-monitor/
├── src/
│   ├── main.rs               # Entry point: logging, database path, starts the TUI
│   ├── lib.rs                # Exposes the modules below to the integration tests
│   ├── tui.rs                # The whole terminal UI (ratatui): views, forms, key handling
│   ├── monitoring_engine.rs  # Background monitoring loop and the soft/hard state model
│   ├── monitor.rs            # The checks themselves: HTTP, TCP, ping
│   ├── database.rs           # SQLite persistence and schema migrations
│   ├── models.rs             # Node, MonitorDetail, NodeStatus, StatusChange, NodeImport
│   ├── connection.rs         # What Enter does on a node: open a browser or an SSH terminal
│   └── config.rs             # Config file scaffolding (currently empty, reserved)
├── tests/                    # Integration tests, one file per module (see tests/README.md)
├── scripts/release-notes.sh  # Generates GitHub release notes from conventional commits
├── .github/workflows/        # CI, on-demand build test, and the release pipeline
├── .claude/                  # Project notes, architecture docs, and slash commands
└── docs/                     # Repository setup docs
```

## Workflow

1. **Branch from `main`.** Name it `type/short-description`, where type is one of `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, or `chore`. Include the issue number if there is one: `fix/123-import-crash`.
2. **Commit with conventional commits.** `type(scope): description`, lower-case, no trailing period. The type decides the version bump and which section of the release notes the change lands in, so pick it honestly.
3. **Run the checks before every commit.**

   ```bash
   cargo fmt
   cargo fmt -- --check
   RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings
   RUSTFLAGS="-A dead_code" cargo test
   RUSTFLAGS="-A dead_code" cargo build --release
   ```

4. **Rebase on `main`, never merge it in.** The history is linear.

   ```bash
   git fetch origin main && git rebase origin/main
   git push -u origin your-branch --force-with-lease
   ```

5. **Open a pull request.** Fill in the template, link the issue with `Closes #123`, and give the PR a conventional-commit title.

Never push to `main` directly. Every change, however small, goes through a pull request.

## Commit types

| Type | Use it for | Version bump | Release notes section |
|---|---|---|---|
| `feat` | New user-facing behaviour | minor | Features |
| `fix` | A bug fix | patch | Bug Fixes |
| `perf` | A performance improvement | patch | Performance |
| `docs` | Documentation only | patch | Documentation |
| `refactor`, `style`, `test`, `build`, `ci`, `chore`, `revert` | Everything else | patch | Other Changes |

A `!` after the type or scope (`feat(db)!: ...`) or a `BREAKING CHANGE:` footer makes it a major bump and lists the change under Breaking Changes.

Examples:

```
feat(monitor): add DNS resolution checks

fix(tui): keep the node form open when saving fails

Closes #87
```

## Code

- `cargo fmt` formats it; `cargo clippy` with `-D warnings` must be clean.
- Public functions get a doc comment. Complex logic gets a comment explaining why, not what.
- Prefer `anyhow::Context` on errors that cross a module boundary so the log says what was being attempted.
- Keep TUI state changes in `tui.rs` and monitoring decisions in `monitoring_engine.rs`. `monitor.rs` only knows how to run a single check.

## Tests

- Unit tests live next to the code in `src/`; integration tests live in `tests/`, one file per module, with shared fixtures in `tests/common/mod.rs`.
- Tests that need the network go behind `#[cfg(feature = "network-tests")]`. CI never runs them.
- Database tests use a temporary file, never the real data directory.
- Codecov expects at least 70% coverage on new code (`codecov.yml`).

## Releases

You do not cut releases by hand. When a pull request merges to `main` and CI passes, `.github/workflows/release.yml`:

1. Reads the commits since the last tag and picks the bump: major for breaking changes, minor for `feat`, patch for anything else.
2. Commits the new version to `Cargo.toml` and tags it `vX.Y.Z`.
3. Builds archives for Linux, macOS (Intel and Apple Silicon), and Windows.
4. Publishes a GitHub release whose notes come from `scripts/release-notes.sh`.

To see what the next release notes will say, run the script locally:

```bash
scripts/release-notes.sh 1.5.0 v1.4.10
```

## License

By contributing you agree that your contributions are licensed under the project's terms: MIT OR Apache-2.0.
