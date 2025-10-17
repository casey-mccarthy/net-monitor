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

### Run All Tests (Excludes Network Tests)
```bash
RUSTFLAGS="-A dead_code" cargo test
```

**Note:** Network tests are excluded by default to avoid flaky CI failures due to external service dependencies.

### Run All Tests Including Network Tests (Local Development Only)
```bash
RUSTFLAGS="-A dead_code" cargo test --features network-tests
```

**Important:** Only run network tests locally. They are automatically excluded in CI.

### Run Tests with Output
```bash
RUSTFLAGS="-A dead_code" cargo test -- --nocapture
```

### Run Integration Tests Only
```bash
RUSTFLAGS="-A dead_code" cargo test --test integration_tests
```

### Run Integration Tests with Network Tests
```bash
RUSTFLAGS="-A dead_code" cargo test --test integration_tests --features network-tests
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

## Code Coverage

This project uses `cargo-llvm-cov` for code coverage analysis.

### Install cargo-llvm-cov
```bash
cargo install cargo-llvm-cov
```

### Generate Coverage Report (HTML)
```bash
cargo llvm-cov --html --open
```

This generates an HTML report in `target/llvm-cov/html/` and opens it in your browser.

### Generate Coverage Report (Terminal)
```bash
cargo llvm-cov
```

### Generate Coverage with All Features (Excludes Network Tests)
```bash
cargo llvm-cov
```

**Note:** By default, coverage excludes network tests (same as regular test runs).

### Generate Coverage with Network Tests (Local Development Only)
```bash
cargo llvm-cov --features network-tests
```

**Important:** Only run with network tests locally, not in CI.

### Generate Coverage for CI (LCOV Format)
```bash
cargo llvm-cov --lcov --output-path coverage.lcov
```

This generates a `coverage.lcov` file suitable for upload to coverage services like Codecov.

### Generate Coverage with No Dead Code Warnings
```bash
RUSTFLAGS="-A dead_code" cargo llvm-cov --html --open
```

### Clean Coverage Data
```bash
cargo llvm-cov clean
```

**Coverage Tips:**
- Coverage reports are excluded from git via `.gitignore`
- The CI workflow automatically generates coverage reports on every push
- Focus on covering critical paths: credential management, node operations, connection logic
- Some GUI/TUI code may be difficult to cover with unit tests - that's okay

## Development Workflow

When making changes:
1. Run `cargo fmt` to format code
2. **IMPORTANT:** Commit formatting changes if any were made: `git add -A && git commit -m "style: apply formatting"`
3. Run `cargo fmt -- --check` to verify formatting
4. Run `RUSTFLAGS="-A dead_code" cargo test --all-features` to test
5. Run `RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings` for linting
6. Run `RUSTFLAGS="-A dead_code" cargo build --release` for final build

**⚠️ KEY POINT:** Always commit formatting changes immediately after running `cargo fmt`. Never push code with uncommitted formatting changes, as this will cause CI failures.

## Git Workflow

This project uses **rebasing** to maintain a clean, linear commit history.

**🚨 CRITICAL: Never push directly to main.** All changes must go through a pull request, regardless of size or complexity.

### Standard Workflow (All Changes)

All changes, from simple fixes to complex features, follow this workflow:

#### 1. Create Feature Branch
```bash
git checkout -b feature/branch-name
# OR use slash command: /create-feature-branch
```

#### 2. Make Changes and Commit
```bash
git add .
git commit -m "feat: description"
# OR use slash command: /commit-feature
```

#### 3. Pre-Push Checks (Before Creating PR)

Before pushing to remote and creating a PR, run these checks locally:

```bash
# 1. Apply formatting
cargo fmt

# 2. Check if formatting made any changes and commit them
if ! git diff --quiet; then
    git add -A
    git commit -m "style: apply cargo fmt formatting fixes"
fi

# 3. Verify formatting is correct (should pass now)
cargo fmt -- --check

# 4. Run tests
RUSTFLAGS="-A dead_code" cargo test --all-features

# 5. Run clippy
RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings

# 6. Run integration tests
RUSTFLAGS="-A dead_code" cargo test --test integration_tests
```

**All-in-One Pre-Push Check:**
```bash
cargo fmt && \
(git diff --quiet || (git add -A && git commit -m "style: apply cargo fmt formatting fixes")) && \
cargo fmt -- --check && \
RUSTFLAGS="-A dead_code" cargo test --all-features && \
RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings && \
RUSTFLAGS="-A dead_code" cargo test --test integration_tests
```

**⚠️ CRITICAL: Always commit formatting changes before pushing!**

If `cargo fmt` makes any changes to your code, those changes MUST be committed before pushing and creating a PR. Otherwise, CI formatting checks will fail. The commands above automatically handle this.

#### 4. Rebase on Main Before PR
```bash
# Fetch latest main
git fetch origin main

# Rebase your branch on main (creates linear history)
git rebase origin/main

# If conflicts occur, resolve them, then:
git add .
git rebase --continue

# Force push your rebased branch
git push origin feature/branch-name --force-with-lease
```

#### 5. Create Pull Request
```bash
gh pr create --title "feat: description" --body "Details..."
```

### Quick PR Workflow (All-in-One)

Use the `/quick-pr` command to automate the entire workflow:

```bash
# Creates branch, lets you work, rebases, and creates PR
/quick-pr feature-name
```

### Rebasing Commands

```bash
# Rebase current branch on main
git fetch origin main && git rebase origin/main

# Interactive rebase to clean up commits
git rebase -i HEAD~3

# Abort rebase if things go wrong
git rebase --abort

# Continue after resolving conflicts
git rebase --continue
```

### Sync Feature Branch with Main

```bash
# Use rebase (preferred - maintains linear history)
git fetch origin main && git rebase origin/main
git push origin feature/branch-name --force-with-lease

# OR use slash command: /sync-main
```

### Why Rebasing?

- **Linear history**: Easy to follow the project timeline
- **Clean commits**: No merge commit noise
- **Better for releases**: Changelog generation is clearer
- **Easier debugging**: `git bisect` works better with linear history

### Why This Workflow?

**All changes require PRs because:**
- ✅ **Code Review**: Even small changes benefit from a second pair of eyes
- ✅ **CI Validation**: Ensures all checks pass before merging
- ✅ **Documentation**: PRs create a searchable history of why changes were made
- ✅ **Reversibility**: Easy to revert a PR if issues are discovered
- ✅ **Consistency**: One workflow for everything, no exceptions

**Branch naming conventions:**

| Change Type | Branch Name Pattern | Example |
|-------------|-------------------|---------|
| New feature | `feat/description` or `feat/123-description` | `feat/add-ssh-support` |
| Bug fix | `fix/description` or `fix/456-description` | `fix/credential-validation` |
| Documentation | `docs/description` | `docs/update-readme` |
| Refactoring | `refactor/description` | `refactor/database-layer` |
| Performance | `perf/description` | `perf/optimize-monitoring` |
| Tests | `test/description` | `test/add-coverage` |
| Chore | `chore/description` | `chore/update-dependencies` |

## Working with GitHub Issues

When working on GitHub issues, follow this workflow to keep issue status up-to-date:

### 1. Check Issue Status Before Starting

Before starting work on an issue, check its current status:

```bash
# View issue details
gh issue view <issue-number>

# List all open issues
gh issue list
```

### 2. Update Issue to "In Progress" When Starting Work

**IMPORTANT:** As soon as you start working on an issue, update its status to "In Progress":

```bash
# Mark issue as in progress
gh issue edit <issue-number> --add-label "in progress"
```

Or if your repository uses GitHub Projects:

```bash
# Update issue status in project (if applicable)
gh issue edit <issue-number> --add-field "Status=In Progress"
```

### 3. Reference Issue in Branch Name and Commits

Create a branch that references the issue number:

```bash
# Good branch names
git checkout -b feat/123-add-feature
git checkout -b fix/456-bug-description
```

Reference the issue in your commit messages:

```bash
git commit -m "feat: add new feature

Implements the feature requested in #123

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

### 4. Link PR to Issue

When creating the pull request, link it to the issue using keywords in the PR description:

```bash
gh pr create --title "feat: add new feature" --body "Closes #123

## Summary
Implementation of new feature as requested.

..."
```

**Linking Keywords:**
- `Closes #123` - Links and will auto-close issue when PR merges
- `Fixes #123` - Same as Closes
- `Resolves #123` - Same as Closes
- `Relates to #123` - Links but doesn't auto-close

### 5. Issue Closure

**Do NOT manually close issues.** GitHub Actions will automatically close issues when the linked PR is merged to main.

### Complete Workflow Example

```bash
# 1. Start working on issue #123
gh issue view 123
gh issue edit 123 --add-label "in progress"

# 2. Create feature branch
git checkout -b feat/123-awesome-feature

# 3. Make changes and commit
git add .
git commit -m "feat: implement awesome feature

Implements #123

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>"

# 4. Run pre-push checks
cargo fmt && \
(git diff --quiet || (git add -A && git commit -m "style: apply cargo fmt formatting fixes")) && \
cargo fmt -- --check && \
RUSTFLAGS="-A dead_code" cargo test && \
RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings

# 5. Rebase on main
git fetch origin main && git rebase origin/main

# 6. Push to remote
git push origin feat/123-awesome-feature --force-with-lease

# 7. Create PR with issue link
gh pr create --title "feat: implement awesome feature" --body "Closes #123

## Summary
- Implemented the awesome feature
- Added tests
- Updated documentation

🤖 Generated with [Claude Code](https://claude.com/claude-code)"

# Issue #123 will automatically be closed when PR is merged
```

### Quick Reference

| Action | Command |
|--------|---------|
| View issue | `gh issue view <number>` |
| List open issues | `gh issue list` |
| Mark in progress | `gh issue edit <number> --add-label "in progress"` |
| Link PR to issue | Use `Closes #<number>` in PR body |
| Check issue status | `gh issue view <number>` |

**Remember:**
- ✅ **DO** update issue to "In Progress" when you start work
- ✅ **DO** link PRs to issues with "Closes #X" in PR description
- ✅ **DO** reference issues in commits with "#X"
- ❌ **DON'T** manually close issues - let GitHub Actions handle it

## Notes

- **🚨 NEVER push directly to main** - All changes must go through a pull request, no exceptions
- PRs trigger automatic semantic versioning and releases when merged to main
- This project intentionally includes unused code for future features (SSH connections, credential management)
- Dead code warnings are suppressed to maintain clean build output
- The `-A dead_code` flag allows unused functions/structs without warnings
- All other warnings are still treated as errors with `-D warnings`
- **Always use `--force-with-lease`** instead of `--force` when pushing rebased branches to prevent accidentally overwriting others' work