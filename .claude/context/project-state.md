# Project State and Context

This document maintains the current state of the net-monitor project for Claude session recovery.

## Project Overview

**Name**: net-monitor  
**Type**: Desktop Application  
**Language**: Rust  
**Current Version**: 0.2.1  
**Repository**: https://github.com/casey-mccarthy/net-monitor

## Current Branch Status

**Active Branch**: `add-ssh-button` (as of last update)  
**Main Branch**: `main`  
**Last Sync**: Check `git log` for latest

## Recent Development

### Last Session Work
- Implemented workflow improvements
- Created conventional commit structure
- Set up Claude commands for development
- Updated documentation to current state

### Active Features
- None currently in active development
- See `.claude/features/in-progress.md` for tracking

### Known Issues
- No critical issues pending
- Check GitHub Issues for latest

## Technology Stack

### Core Technologies
- **Rust**: 2021 Edition
- **GUI**: egui/eframe
- **Database**: SQLite via rusqlite
- **Async**: Tokio runtime
- **HTTP**: reqwest client
- **SSH**: ssh2 library
- **Crypto**: ring for encryption

### Build System
- **Cargo**: Standard Rust toolchain
- **CI/CD**: GitHub Actions
- **Release**: Automated via workflow

## Development Workflow

### Branch Strategy
- `main`: Protected, stable releases only
- Feature branches: `feature/`, `fix/`, `chore/`
- All changes via PR with review

### Commit Convention
- Conventional commits enforced
- Types: feat, fix, docs, style, refactor, perf, test, chore
- Format: `type(scope): description`

### Release Process
- Semantic versioning (MAJOR.MINOR.PATCH)
- Automated based on commit types
- Changelog generated from commits

## File Structure

```
/
├── src/                 # Source code
├── .claude/            # Claude-specific docs
│   ├── commands/       # Claude commands
│   ├── features/       # Feature tracking
│   ├── architecture/   # Technical docs
│   ├── context/        # This file
│   └── workflows/      # Process docs
├── .github/            # GitHub config
│   └── workflows/      # CI/CD
├── docs/               # User documentation
└── tests/              # Test files
```

## Database Schema

**Current Version**: 3  
**Tables**: nodes, monitoring_results, credentials, migrations

See `.claude/architecture/database-schema.md` for details.

## Testing Strategy

- Unit tests in source files
- Integration tests in `tests/`
- Manual testing for GUI
- CI runs tests on PR

## Deployment

### Platforms
- Windows x64
- macOS x64 & ARM64
- Linux x64

### Distribution
- GitHub Releases with binaries
- Automated build on version tag
- Checksums provided

## Environment Variables

```bash
RUST_LOG=debug  # Enable debug logging
```

## Common Commands

```bash
# Development
cargo build
cargo test
cargo run

# Release
cargo build --release

# With logging
RUST_LOG=debug cargo run
```

## Session Recovery Checklist

When starting a new Claude session:

1. [ ] Check current branch: `git status`
2. [ ] Review recent commits: `git log --oneline -10`
3. [ ] Check for uncommitted changes: `git diff`
4. [ ] Review active issues on GitHub
5. [ ] Check `.claude/features/in-progress.md`
6. [ ] Run tests: `cargo test`
7. [ ] Build project: `cargo build`

## Important Files to Review

- `Cargo.toml` - Dependencies and version
- `src/main.rs` - Entry point
- `src/gui.rs` - UI implementation
- `src/models.rs` - Data structures
- `.github/workflows/release.yml` - CI/CD

## Notes for Next Session

- Branch protection needs manual GitHub configuration
- Commitlint npm package not installed (optional)
- Consider implementing email notifications next
- Review and update this file regularly

---

*Last Updated*: During session setting up development workflow improvements