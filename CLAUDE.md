# Claude Commands for Net Monitor

This file contains common commands that Claude should use when working on this project.

## Build Commands

### Standard Build (Development)
```bash
cargo build
```

### Release Build (No Warnings)
```bash
RUSTFLAGS="-A dead_code" cargo build --release
```

### Clean Build
```bash
cargo clean && cargo build --release
```

## Test Commands

### Run All Tests (No Warnings)
```bash
RUSTFLAGS="-A dead_code" cargo test --all-features
```

### Run Tests with Output
```bash
RUSTFLAGS="-A dead_code" cargo test --all-features -- --nocapture
```

### Run Integration Tests Only
```bash
RUSTFLAGS="-A dead_code" cargo test --test integration_tests
```

## Quality Checks

### Format Check
```bash
cargo fmt -- --check
```

### Apply Formatting
```bash
cargo fmt
```

### Clippy (No Dead Code Warnings)
```bash
RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings
```

### Clippy (Allow Dead Code)
```bash
cargo clippy --all-targets --all-features -- -A dead_code -D warnings
```

### Security Audit
```bash
cargo audit
```

Note: The project uses `.cargo/audit.toml` to ignore known vulnerabilities that have no current fix available.

## Development Workflow

When making changes:
1. Run `cargo fmt` to format code
2. Run `RUSTFLAGS="-A dead_code" cargo test --all-features` to test
3. Run `RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings` for linting
4. Run `RUSTFLAGS="-A dead_code" cargo build --release` for final build

## Notes

- This project intentionally includes unused code for future features (SSH connections, credential management)
- Dead code warnings are suppressed to maintain clean build output
- The `-A dead_code` flag allows unused functions/structs without warnings
- All other warnings are still treated as errors with `-D warnings`