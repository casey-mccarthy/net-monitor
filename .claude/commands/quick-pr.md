---
description: Create feature branch, rebase on main, and submit PR in one workflow
---

I'll help you create a feature branch, work on your changes, rebase on main, and create a pull request - all in a streamlined workflow.

## What I'll do:

1. **Verify starting point**
   - Ensure we're starting from main branch
   - Check that working directory is clean
   - Fetch latest changes from origin

2. **Create feature branch**
   - Create branch with provided name following conventions
   - Branch types:
     - `feature/` - New features
     - `fix/` - Bug fixes
     - `chore/` - Maintenance tasks
     - `docs/` - Documentation updates
     - `refactor/` - Code refactoring

3. **Guide you through work**
   - You make your changes
   - I help with conventional commits as you work
   - Run tests and quality checks

4. **Pre-push quality checks**
   - Apply code formatting with `cargo fmt`
   - Verify formatting compliance
   - Run all tests to ensure functionality
   - Execute clippy for lint checks
   - Run integration tests

5. **Rebase on main**
   - Fetch latest main branch changes
   - Rebase your commits on top of main (linear history)
   - Handle any conflicts if they occur

6. **Create pull request**
   - Push branch to remote with tracking
   - Generate PR description with changelog from commits
   - Create PR using GitHub CLI
   - Provide PR URL for review

## Usage:

Provide a branch name that describes your work:

```bash
# Quick PR with feature branch
/quick-pr email-notifications

# Quick PR with fix branch
/quick-pr fix-timeout-issue

# Quick PR with chore branch
/quick-pr update-dependencies
```

I'll automatically detect the branch type from your commit messages.

## Branch Naming:

The command accepts short names and will prefix appropriately:

| Your Input | Branch Created | Use When |
|------------|----------------|----------|
| `email-notifications` | `feature/email-notifications` | Adding new feature |
| `fix-timeout` | `fix/fix-timeout` | Fixing a bug |
| `update-deps` | `chore/update-deps` | Maintenance work |
| `api-docs` | `docs/api-docs` | Documentation only |

Or specify the full prefix yourself:

```bash
/quick-pr feature/email-notifications
/quick-pr fix/connection-timeout
```

## Workflow Steps:

### 1. Command Start
```bash
/quick-pr feature-name
```

### 2. I Create Branch
- Create and switch to branch
- Confirm you're ready to work

### 3. You Work
- Make your changes
- Tell me when to commit: "Commit these changes"
- I'll create conventional commits
- Continue until done

### 4. Ready for PR
- Say: "Ready for PR" or "Create pull request"
- I'll run all quality checks:
  - Apply formatting
  - Verify formatting compliance
  - Run all tests
  - Run clippy
  - Run integration tests
- I'll rebase on main
- Handle any conflicts with you
- Push to remote

### 5. PR Created
- Generate changelog from commits
- Create PR with comprehensive description including:
  - Summary of changes
  - Detailed changelog grouped by type
  - Testing checklist
  - Breaking changes if any
- Provide URL for review

## Example Session:

```
You: /quick-pr email-notifications

Claude: Created and switched to branch feature/email-notifications
        Ready to start working. Make your changes and let me know when to commit.

You: [makes changes to notification system]
     Commit these changes

Claude: [analyzes changes]
        Creating commit: "feat(notifications): add email notification system"
        Committed. Continue working or say "ready for pr" when done.

You: [adds tests]
     Commit the tests

Claude: Creating commit: "test(notifications): add email notification tests"
        Committed.

You: Ready for PR

Claude: Running pre-push quality checks...
        ✓ Applied formatting
        ✓ Formatting verified
        ✓ All tests passed
        ✓ Clippy checks passed
        ✓ Integration tests passed

        [rebases on main]
        Rebased successfully on main
        [pushes branch]
        [creates PR]

        PR created: https://github.com/casey-mccarthy/net-monitor/pull/123

        Title: feat: Add email notification system

        ## Changes in this PR

        ### Features
        - feat(notifications): add email notification system

        ### Other Changes
        - test(notifications): add email notification tests
```

## What Gets Run:

```bash
# 1. Setup
git checkout main
git pull origin main
git checkout -b feature/branch-name

# 2. Work phase (repeated as needed)
git add .
git commit -m "conventional commit message"

# 3. Pre-push quality checks
cargo fmt
cargo fmt -- --check
RUSTFLAGS="-A dead_code" cargo test --all-features
RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings
RUSTFLAGS="-A dead_code" cargo test --test integration_tests

# 4. Prepare PR
git fetch origin main
git rebase origin/main
git push -u origin feature/branch-name

# 5. Create PR
gh pr create --title "..." --body "..."
```

## PR Description Format:

The generated PR will include:

```markdown
## Summary

- High-level overview of changes
- Key points from commit messages

## Changes in this PR

### ⚠️ Breaking Changes
- Any breaking changes found in commits

### ✨ Features
- feat: Add email notification system
- feat: Implement user preferences panel

### 🐛 Bug Fixes
- fix: Resolve connection timeout issue
- fix: Correct data validation in forms

### 📚 Documentation
- docs: Improve API documentation

### 🔧 Other Changes
- chore: Update dependencies
- test: Add integration tests

## Test plan

- [ ] Manual testing completed
- [ ] Integration tests pass
- [ ] Unit tests pass
- [ ] No regressions identified

🤖 Generated with [Claude Code](https://claude.com/claude-code)
```

## Benefits:

- **Fast**: Complete workflow in one command
- **Clean history**: Rebasing ensures linear commits
- **Automatic changelog**: Generated from conventional commits
- **Quality assured**: All checks run before pushing
- **No mistakes**: Guided through each step
- **Best practices**: Follows project conventions automatically
- **CI-ready**: All checks that run in CI are verified locally first

## Best Practices:

- Make small, focused commits as you work
- Run tests before saying "ready for pr"
- Use conventional commit format (I'll help!)
- Keep feature branches short-lived
- Rebase regularly if working over multiple days

## Quality Checks:

Before pushing, the command automatically runs:

1. **Formatting**: Ensures code style compliance
2. **Tests**: Validates all functionality works
3. **Clippy**: Catches common mistakes and anti-patterns
4. **Integration tests**: Verifies end-to-end workflows
5. **Format verification**: Confirms no formatting issues remain

This ensures all GitHub Actions CI checks will pass.

## Conflict Resolution:

If rebase conflicts occur:
1. I'll show you the conflicting files
2. You resolve the conflicts
3. Tell me "conflicts resolved"
4. I'll continue the rebase
5. We finish creating the PR

Or you can abort with: "abort the rebase"

## Requirements:

- Clean working directory to start
- GitHub CLI (`gh`) must be configured
- Must be on main branch (or I'll switch you)
- Internet connection for fetching and pushing

## Alternative Workflows:

If you prefer more control, use individual commands:
1. `/create-feature-branch` - Just create branch
2. `/commit-feature` - Just commit changes
3. `/sync-main` - Just rebase on main

The `/quick-pr` command combines all of these for speed!
