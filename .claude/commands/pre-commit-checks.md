---
description: Run all linters, format checks, and tests before committing
---

I'll run a comprehensive suite of quality checks including formatting, linting, testing, and security audits to ensure your code meets all standards before committing or pushing.

## What I'll do:

1. **Code Formatting**
   - Apply Rust formatting with `cargo fmt`
   - Verify formatting compliance with format check
   - Ensure consistent code style

2. **Linting**
   - Run Clippy with strict warnings as errors
   - Check for common mistakes and anti-patterns
   - Enforce best practices

3. **Testing**
   - Run all unit tests
   - Run integration tests
   - Verify test coverage and functionality

4. **Security Audit**
   - Check for known vulnerabilities in dependencies
   - Use cargo-audit for security scanning

5. **Build Verification**
   - Verify code compiles successfully
   - Check for build warnings

6. **Summary Report**
   - Provide clear pass/fail status for each check
   - Highlight any issues found
   - Suggest fixes if problems are detected

## Checks Performed:

### 1. Format Check
```bash
cargo fmt -- --check
```
Verifies code follows Rust formatting standards.

### 2. Apply Formatting
```bash
cargo fmt
```
Automatically fixes formatting issues.

### 3. Clippy Lint
```bash
RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings
```
Catches common mistakes, anti-patterns, and potential bugs.

### 4. Unit Tests
```bash
RUSTFLAGS="-A dead_code" cargo test --all-features
```
Runs all unit tests to verify functionality.

### 5. Integration Tests
```bash
RUSTFLAGS="-A dead_code" cargo test --test integration_tests
```
Runs integration tests to verify end-to-end workflows.

### 6. Security Audit
```bash
cargo audit
```
Scans dependencies for known security vulnerabilities.

### 7. Build Check
```bash
RUSTFLAGS="-A dead_code" cargo build --release
```
Verifies the project builds successfully in release mode.

## Usage:

```bash
# Run all checks
/pre-commit-checks

# Or just say:
"Run all checks"
"Run pre-commit checks"
"Verify everything passes"
```

## Example Output:

```
Running comprehensive pre-commit checks...

✓ Code formatting applied
✓ Format check passed
✓ Clippy lint passed (0 warnings)
✓ Unit tests passed (45 tests)
✓ Integration tests passed (5 tests)
✓ Security audit passed (0 vulnerabilities)
✓ Build successful

All checks passed! ✨
Your code is ready to commit.
```

## If Checks Fail:

When a check fails, I'll:
1. Show you the specific error or warning
2. Explain what went wrong
3. Suggest how to fix it
4. Offer to help make the fix

Example failure output:
```
Running comprehensive pre-commit checks...

✓ Code formatting applied
✓ Format check passed
✗ Clippy lint failed

Found 3 warnings:
  warning: unused variable `x`
   --> src/main.rs:42:9

I can help fix these issues. Would you like me to:
1. Explain each warning
2. Automatically fix what I can
3. Show you how to fix them manually
```

## When to Use:

- **Before committing**: Catch issues early
- **Before pushing**: Ensure CI will pass
- **After making changes**: Verify nothing broke
- **Before creating a PR**: Make sure everything is clean
- **After resolving conflicts**: Ensure merge didn't break anything

## Benefits:

- **Catch issues early**: Find problems before CI does
- **Save time**: Don't wait for CI to fail
- **Confidence**: Know your code meets standards
- **Clean history**: No "fix lint" commits later
- **Best practices**: Enforces project standards

## Comparison with CI:

This command runs the same checks as GitHub Actions CI:

| Check | Local Command | CI Workflow |
|-------|--------------|-------------|
| Format | `cargo fmt -- --check` | `ci.yml` fmt job |
| Clippy | `cargo clippy` | `ci.yml` clippy job |
| Tests | `cargo test` | `ci.yml` test job |
| Build | `cargo build` | `ci.yml` build job |
| Security | `cargo audit` | `ci.yml` security-audit job |

Running locally ensures CI will pass without waiting.

## Time Estimates:

Typical execution times:
- Format check: < 1 second
- Apply formatting: < 1 second
- Clippy: 5-30 seconds
- Unit tests: 10-60 seconds
- Integration tests: 5-30 seconds
- Security audit: 2-5 seconds
- Build: 30-120 seconds

**Total: 1-4 minutes** (with caching)

## Configuration:

All checks use project-specific configurations:
- `.cargo/audit.toml` - Security audit ignores
- `Cargo.toml` - Project settings
- `RUSTFLAGS="-A dead_code"` - Allow unused code

## Skip Options:

If you need to skip certain checks (not recommended):

```
"Run all checks except security audit"
"Run lint and format only"
"Skip integration tests"
```

## Exit Codes:

- **0**: All checks passed ✓
- **Non-zero**: One or more checks failed ✗

Perfect for use in git hooks or automation scripts.

## Related Commands:

- `/quick-pr` - Includes these checks plus PR creation
- `/build-fix-commit` - Run build, fix errors, and commit
- `/commit-feature` - Commit with conventional format

## Notes:

- Uses `RUSTFLAGS="-A dead_code"` to allow intentional unused code
- Security audit uses `.cargo/audit.toml` for known exceptions
- All checks must pass for the command to succeed
- Formatting is applied before checking to fix auto-fixable issues
