---
description: Run all linters, format checks, and tests before committing
---

I'll run a comprehensive suite of quality checks including formatting, linting, testing, and security audits to ensure your code meets all standards before committing or pushing.

## Modes:

### 1. Check Only (Default)
Runs all checks and reports issues without making changes.

### 2. Auto-Fix Mode (`--fix`)
Runs checks and automatically fixes what can be fixed (formatting, clippy suggestions).

### 3. Fix and Commit Mode (`--fix-commit`)
Runs checks, auto-fixes issues, and creates a conventional commit with all changes.

## What I'll do:

1. **Code Formatting**
   - Apply Rust formatting with `cargo fmt` (if fix mode)
   - Verify formatting compliance with format check
   - Ensure consistent code style

2. **Build and Check**
   - Run `cargo check` to identify compilation errors
   - Fix syntax errors, type mismatches, and missing imports (if fix mode)
   - Verify code compiles successfully

3. **Linting**
   - Run Clippy with strict warnings as errors
   - Check for common mistakes and anti-patterns
   - Apply clippy suggestions automatically (if fix mode)
   - Enforce best practices

4. **Testing**
   - Run all unit tests
   - Run integration tests
   - Verify test coverage and functionality

5. **Security Audit**
   - Check for known vulnerabilities in dependencies
   - Use cargo-audit for security scanning

6. **Build Verification**
   - Verify code compiles successfully in release mode
   - Check for build warnings

7. **Commit (if --fix-commit mode)**
   - Determine appropriate conventional commit type based on changes:
     - `feat:` for new features
     - `fix:` for bug fixes
     - `refactor:` for code improvements
     - `style:` for formatting changes
     - `chore:` for maintenance tasks
   - Create descriptive commit message with details of fixes applied
   - Stage all changes and create the commit
   - Only commit if all checks pass

8. **Summary Report**
   - Provide clear pass/fail status for each check
   - Highlight any issues found
   - List fixes applied (if fix mode)
   - Show commit details (if commit mode)

## Checks Performed:

### 1. Build Check
```bash
cargo check
```
Identifies compilation errors early.

### 2. Apply Formatting
```bash
cargo fmt
```
Automatically fixes formatting issues.

### 3. Format Check
```bash
cargo fmt -- --check
```
Verifies code follows Rust formatting standards.

### 4. Clippy Lint
```bash
RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings
```
Catches common mistakes, anti-patterns, and potential bugs.

### 5. Unit Tests
```bash
RUSTFLAGS="-A dead_code" cargo test --all-features
```
Runs all unit tests to verify functionality.

### 6. Integration Tests
```bash
RUSTFLAGS="-A dead_code" cargo test --test integration_tests
```
Runs integration tests to verify end-to-end workflows.

### 7. Security Audit
```bash
cargo audit
```
Scans dependencies for known security vulnerabilities.

### 8. Build Verification
```bash
RUSTFLAGS="-A dead_code" cargo build --release
```
Verifies the project builds successfully in release mode.

## Usage:

```bash
# Check only (default)
/pre-commit-checks

# Auto-fix mode
/pre-commit-checks --fix

# Fix and commit mode
/pre-commit-checks --fix-commit

# Or just say:
"Run all checks"
"Run checks and fix issues"
"Run checks, fix, and commit"
"Build, fix, and commit"
```

## Example Output:

### Check Only Mode:
```
Running comprehensive pre-commit checks...

✓ Build check passed
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

### Fix Mode:
```
Running comprehensive pre-commit checks with auto-fix...

✓ Build check passed
✓ Applied formatting fixes (3 files changed)
✓ Format check passed
✓ Applied clippy suggestions (2 fixes)
✓ Clippy lint passed
✓ Unit tests passed (45 tests)
✓ Integration tests passed (5 tests)
✓ Security audit passed
✓ Build successful

Fixes applied:
- Formatted src/main.rs, src/lib.rs, src/monitor.rs
- Removed unused variable in src/connection.rs
- Simplified boolean expression in src/database.rs

All checks passed! ✨
Changes have been made but not committed.
```

### Fix and Commit Mode:
```
Running comprehensive pre-commit checks with auto-fix and commit...

✓ Build check passed
✓ Applied formatting fixes (3 files changed)
✓ Format check passed
✓ Applied clippy suggestions (2 fixes)
✓ Clippy lint passed
✓ Unit tests passed (45 tests)
✓ Integration tests passed (5 tests)
✓ Security audit passed
✓ Build successful

Fixes applied:
- Formatted src/main.rs, src/lib.rs, src/monitor.rs
- Removed unused variable in src/connection.rs
- Simplified boolean expression in src/database.rs

Creating commit...
✓ Committed: style: apply formatting and clippy fixes

All checks passed and changes committed! ✨
```

## If Checks Fail:

When a check fails, I'll:
1. Show you the specific error or warning
2. Explain what went wrong
3. Suggest how to fix it (or auto-fix in fix mode)
4. Re-run checks after fixes

Example failure output:
```
Running comprehensive pre-commit checks...

✓ Build check passed
✓ Code formatting applied
✓ Format check passed
✗ Clippy lint failed

Found 3 warnings:
  warning: unused variable `x`
   --> src/main.rs:42:9

In fix mode, I would automatically:
1. Remove the unused variable
2. Re-run clippy to verify fix
3. Continue with remaining checks

Would you like me to run in --fix mode?
```

## When to Use:

- **Before committing**: Catch issues early (check only)
- **Before pushing**: Ensure CI will pass (check only)
- **After making changes**: Verify and fix issues (fix mode)
- **Before creating a PR**: Make sure everything is clean (fix mode)
- **After resolving conflicts**: Ensure merge didn't break anything (check only)
- **Quick cleanup**: Fix formatting and lint issues automatically (fix-commit mode)

## Benefits:

- **Catch issues early**: Find problems before CI does
- **Save time**: Don't wait for CI to fail
- **Confidence**: Know your code meets standards
- **Auto-fix**: Let tools fix what they can
- **Clean history**: No "fix lint" commits later (with fix-commit mode)
- **Best practices**: Enforces project standards

## Comparison with CI:

This command runs the same checks as GitHub Actions CI:

| Check | Local Command | CI Workflow |
|-------|--------------|-------------|
| Build | `cargo check` | `ci.yml` build job |
| Format | `cargo fmt -- --check` | `ci.yml` fmt job |
| Clippy | `cargo clippy` | `ci.yml` clippy job |
| Tests | `cargo test` | `ci.yml` test job |
| Build | `cargo build` | `ci.yml` build job |
| Security | `cargo audit` | `ci.yml` security-audit job |

Running locally ensures CI will pass without waiting.

## Time Estimates:

Typical execution times:
- Build check: 5-20 seconds
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
- `/commit-feature` - Commit with conventional format (manual)

## Notes:

- Uses `RUSTFLAGS="-A dead_code"` to allow intentional unused code
- Security audit uses `.cargo/audit.toml` for known exceptions
- All checks must pass for the command to succeed
- Fix mode only applies automatic fixes (formatting, simple clippy suggestions)
- Complex issues may require manual intervention
- In fix-commit mode, the commit message reflects the fixes applied
