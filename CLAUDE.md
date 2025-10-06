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

### Run All Tests (No Warnings, Skip Network Tests)
```bash
RUSTFLAGS="-A dead_code" cargo test --all-features
```

### Run All Tests Including Network Tests (Local Development)
```bash
RUSTFLAGS="-A dead_code" cargo test --all-features --features network-tests
```

### Run Tests with Output
```bash
RUSTFLAGS="-A dead_code" cargo test --all-features -- --nocapture
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

## Development Workflow

When making changes:
1. Run `cargo fmt` to format code
2. Run `RUSTFLAGS="-A dead_code" cargo test --all-features` to test
3. Run `RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings` for linting
4. Run `RUSTFLAGS="-A dead_code" cargo build --release` for final build

## Git Workflow

This project uses **rebasing** to maintain a clean, linear commit history.

### Direct to Main (Simple Fixes)

For simple, standalone fixes or changes:

```bash
# Make changes on main branch
git add .
git commit -m "fix: description of fix"
git push origin main
```

This triggers automatic semantic versioning and release based on commit type.

### Feature Branch Workflow (Complex Changes)

For features or changes requiring review:

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

#### 3. Rebase on Main Before PR
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

#### 4. Create Pull Request
```bash
gh pr create --title "feat: description" --body "Details..."
# OR use slash command: /prepare-pr
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

### When to Use Each Workflow

| Scenario | Workflow | Why |
|----------|----------|-----|
| Quick fix, doc update, small change | Direct to main | Fast, triggers auto-release |
| New feature, refactor, breaking change | Feature branch + rebase + PR | Allows review, testing, discussion |
| Hotfix for production issue | Feature branch + rebase + PR | Document the fix, maintain history |

## Notes

- This project intentionally includes unused code for future features (SSH connections, credential management)
- Dead code warnings are suppressed to maintain clean build output
- The `-A dead_code` flag allows unused functions/structs without warnings
- All other warnings are still treated as errors with `-D warnings`
- **Always use `--force-with-lease`** instead of `--force` when pushing rebased branches to prevent accidentally overwriting others' work