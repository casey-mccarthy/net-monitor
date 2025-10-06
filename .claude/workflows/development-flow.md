# Development Workflow

This document describes the standard development workflow for net-monitor.

**This project uses REBASING to maintain clean, linear commit history.**

## Workflow Overview

```mermaid
graph LR
    A[Main Branch] --> B{Simple or Complex?}
    B -->|Simple Fix| C[Commit to Main]
    C --> J[Push to Main]
    J --> I[Automatic Release]
    B -->|Complex/Feature| D[Create Feature Branch]
    D --> E[Development]
    E --> F[Commit Changes]
    F --> G[Rebase on Main]
    G --> H[Create PR]
    H --> K[Code Review]
    K --> L[Merge to Main]
    L --> I
    I --> A
```

## Two Workflows

### 1. Direct to Main (Simple Changes)
For quick fixes, doc updates, or small standalone changes:
- Work directly on main branch
- Commit with conventional commit message
- Push to main
- Automatic release triggered

### 2. Feature Branch + Rebase (Complex Changes)
For features, refactors, or changes requiring review:
- Create feature branch
- Make changes and commits
- **Rebase on main** (not merge!)
- Create pull request
- Code review
- Merge to main
- Automatic release triggered

## Step-by-Step Process

### 1. Start New Feature

```bash
# Ensure you're on main and up to date
git checkout main
git pull origin main

# Create feature branch
git checkout -b feature/your-feature-name
# OR use Claude command: "Create a feature branch for [description]"
```

### 2. Development

Write your code following project conventions:
- Follow Rust idioms and best practices
- Maintain consistent code style
- Add tests for new functionality
- Update documentation as needed

### 3. Commit Changes

Use conventional commits for all changes:

```bash
# Stage changes
git add .

# Create conventional commit
git commit -m "feat: add new monitoring metric"
# OR use Claude command: "Commit my changes"
```

Commit types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Formatting, no code change
- `refactor`: Code restructuring
- `perf`: Performance improvement
- `test`: Adding tests
- `chore`: Maintenance

### 4. Keep Branch Updated

Regularly sync with main to avoid conflicts. **Use rebase for clean history:**

```bash
# Fetch latest changes
git fetch origin main

# Rebase your branch on main (preferred)
git rebase origin/main

# Force push with safety flag
git push origin feature/your-feature-name --force-with-lease

# OR use Claude command: "Sync with main"
```

### 5. Pre-PR Checklist

Before creating a PR, ensure:

- [ ] All tests pass: `cargo test`
- [ ] Code compiles without warnings: `cargo build`
- [ ] Code is formatted: `cargo fmt`
- [ ] Linting passes: `cargo clippy`
- [ ] Documentation is updated
- [ ] Commit messages follow convention

### 6. Create Pull Request

```bash
# Rebase on main first!
git fetch origin main && git rebase origin/main

# Push your branch (or force push if already exists)
git push -u origin feature/your-feature-name --force-with-lease

# Create PR via GitHub CLI
gh pr create --title "feat: your feature" --body "Description"

# OR use Claude command for all steps: "/quick-pr feature-name"
# OR just create PR: "/prepare-pr"
```

PR should include:
- Clear title following commit convention
- Description of changes
- Testing performed
- Screenshots if UI changes
- Related issue references
- **Clean, rebased commit history**

### 7. Code Review Process

#### For Authors:
- Respond to all feedback
- Make requested changes
- Re-request review when ready
- Keep PR updated with main

#### For Reviewers:
- Check code quality and style
- Verify tests are adequate
- Ensure documentation is updated
- Test functionality locally if needed

### 8. Merging

Once approved:
1. **Rebase on main one final time** to ensure linear history
2. All CI checks pass
3. Merge to main (can use squash if commits need cleanup)
4. Delete feature branch after merge
5. Automatic release process begins

```bash
# Before merging, final rebase
git fetch origin main
git rebase origin/main
git push origin feature/your-feature-name --force-with-lease

# Then merge via GitHub UI or:
gh pr merge --merge  # or --squash if needed
```

### 9. Release Process

Releases are automated based on commits:

- Breaking changes → Major version bump
- Features → Minor version bump  
- Fixes → Patch version bump

The release workflow:
1. Detects version bump needed
2. Updates version in Cargo.toml
3. Creates git tag
4. Builds binaries for all platforms
5. Creates GitHub release with changelog

## Branch Naming Conventions

- `feature/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation updates
- `chore/` - Maintenance tasks
- `refactor/` - Code refactoring

Examples:
- `feature/email-notifications`
- `fix/connection-timeout`
- `docs/update-readme`
- `chore/update-dependencies`

## Commit Message Examples

### Good Examples

```
feat(monitor): add retry logic for failed connections

Implements exponential backoff for connection retries
to improve reliability during network issues.

Closes #123
```

```
fix(database): prevent connection pool exhaustion

Ensures connections are properly released even when
errors occur during query execution.
```

```
docs: update installation instructions for macOS
```

### Bad Examples

```
Fixed bug          # Too vague
Update code        # No context
WIP               # Don't commit WIP
various changes   # Be specific
```

## Emergency Hotfix Process

For critical production issues:

1. Create branch from main: `fix/critical-issue`
2. Make minimal fix
3. Test thoroughly
4. Create PR with `[HOTFIX]` prefix
5. Get expedited review
6. Merge immediately
7. Verify fix in production

## Tips for Success

### Do:
- Commit early and often
- Write descriptive commit messages
- Keep PRs focused and small
- Test your changes thoroughly
- Document complex logic
- Ask for help when stuck

### Don't:
- Commit directly to main
- Mix unrelated changes in one PR
- Leave TODO comments without tickets
- Ignore CI failures
- Skip documentation updates
- Merge without review

## Useful Git Commands

```bash
# View recent commits
git log --oneline -10

# Check what changed
git diff

# Undo last commit (keep changes)
git reset --soft HEAD~1

# Update commit message
git commit --amend

# Interactive rebase (clean history before PR)
git rebase -i HEAD~3

# Rebase on main
git fetch origin main && git rebase origin/main

# Abort rebase if conflicts are too complex
git rebase --abort

# Continue rebase after resolving conflicts
git add .
git rebase --continue

# Force push safely after rebase
git push origin branch-name --force-with-lease

# Stash changes temporarily
git stash
git stash pop

# View commit history graph
git log --graph --oneline --all
```

## Claude Commands Available

- **/create-feature-branch**: Start new feature development on a branch
- **/commit-feature**: Create conventional commit
- **/sync-main**: Rebase branch on latest main (uses rebase by default)
- **/prepare-pr**: Generate PR with changelog from current branch
- **/quick-pr**: Complete workflow - create branch, work, rebase, create PR (all-in-one)
- **/release**: Trigger new version release

See `.claude/commands/` for detailed documentation.

## Quick Start Examples

### Simple fix workflow:
```bash
# Work directly on main
git checkout main
git pull origin main
# make changes
git add .
git commit -m "fix: description"
git push origin main
# Automatic release triggers!
```

### Feature workflow:
```bash
# Use quick-pr command for guided workflow
/quick-pr email-notifications

# OR manual steps:
git checkout -b feature/email-notifications
# make changes and commits
git fetch origin main && git rebase origin/main
git push -u origin feature/email-notifications --force-with-lease
gh pr create
```

## Getting Help

- Check existing issues on GitHub
- Review this documentation
- Ask in PR comments
- Use Claude commands for automation
- Consult Rust documentation