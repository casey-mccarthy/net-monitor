# Development workflow

Branch, commit, check, rebase, pull request. `main` is protected and every change goes through a PR, however small. Merging to `main` releases automatically.

```mermaid
graph LR
    A[main] --> B[branch: type/description]
    B --> C[commit: type(scope): subject]
    C --> D[fmt, clippy, test, build]
    D --> E[rebase on main]
    E --> F[pull request]
    F --> G[CI + review]
    G --> H[merge]
    H --> I[release workflow: bump, tag, build, publish]
    I --> A
```

## 1. Branch

```bash
git checkout main && git pull origin main
git checkout -b fix/123-short-description
```

Prefix with `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, or `chore`. Add the issue number when there is one. `/create-feature-branch` does this for you.

## 2. Commit

Conventional commits, enforced by `.commitlintrc.json`:

```
type(scope): subject

Optional body explaining why.

Closes #123
```

The type sets the version bump (`feat` minor, everything else patch, `!` or `BREAKING CHANGE:` major) and the release notes section. `/commit-feature` writes one for you.

## 3. Check before every commit

```bash
cargo fmt
cargo fmt -- --check
RUSTFLAGS="-A dead_code" cargo clippy --all-targets --all-features -- -D warnings
RUSTFLAGS="-A dead_code" cargo test
RUSTFLAGS="-A dead_code" cargo build --release
```

If `cargo fmt` changed anything, commit it before you push. `/pre-commit-checks` runs the lot.

## 4. Rebase, never merge

```bash
git fetch origin main && git rebase origin/main
git push -u origin fix/123-short-description --force-with-lease
```

Always `--force-with-lease`, never `--force`. `/sync-main` does this.

## 5. Pull request

Use the template. Conventional-commit title, `Closes #123` in the body, and say how you tested it. `/quick-pr` does steps 4 and 5 together.

Reviewers check that the change is small, tested, documented, and that the commit types are honest, because they become the changelog.

## 6. Merge

Rebase-merge (or squash when the commits need cleanup, keeping a conventional subject). Delete the branch. The Release workflow now:

1. Works out the bump from the commits since the last tag.
2. Commits `chore: bump version to X.Y.Z` and tags `vX.Y.Z` on `main`.
3. Builds Linux, macOS Intel, macOS ARM, and Windows archives.
4. Publishes the release with notes from `scripts/release-notes.sh`.

That bump commit is the one commit that lands on `main` without a PR. It is skipped by the workflow and by the release notes.

## Hotfixes

Same flow. The only difference is that the reviewer looks at it now instead of tomorrow. There is no path around the PR.

## Git reminders

```bash
git log --oneline -10                      # recent history
git rebase -i HEAD~3                       # tidy commits before opening the PR
git rebase --abort                         # bail out of a bad rebase
git add . && git rebase --continue         # after fixing conflicts
git stash && git stash pop                 # park changes
scripts/release-notes.sh 1.5.0 v1.4.10     # preview the next release notes
```

## Slash commands

- `/create-feature-branch`: new branch with the right name
- `/commit-feature`: conventional commit for the staged changes
- `/pre-commit-checks`: fmt, clippy, tests, build
- `/check-conventional-commits`: audit the branch's commit messages
- `/sync-main`: rebase on `main` and force-with-lease push
- `/quick-pr`: rebase and open the PR
- `/release`: explains and previews the automatic release
