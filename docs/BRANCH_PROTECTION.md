# Branch protection for `main`

Repository settings live at https://github.com/casey-mccarthy/net-monitor/settings/branches. This page records what the workflow in `CLAUDE.md` assumes about `main`.

## Rules for `main`

- **Require a pull request before merging.** Nothing lands on `main` without a PR, including one-line fixes.
- **Require status checks to pass before merging**, with **require branches to be up to date** on. Require the `CI Success` check: it is the fan-in job in `.github/workflows/ci.yml` and fails if any of format, clippy, tests, coverage, the four platform builds, the security audit, or the dependency check fails.
- **Require conversation resolution before merging.**
- **Require linear history.** The project rebases; merge commits are not allowed.
- **Do not lock the branch and do not block pushes from GitHub Actions.** The release workflow pushes the `chore: bump version to X.Y.Z` commit and the `vX.Y.Z` tag to `main` with the Actions token. If the rules apply to everyone with no bypass for `github-actions[bot]`, releases stop working. Either add the Actions app to the bypass list or leave administrators exempt.

## Pull request settings

Under **Settings → General → Pull requests**:

- Allow **rebase merging**. Turn off merge commits. Squash merging is fine as a fallback when a PR's commits need cleanup, but the squashed message must still be a conventional commit or the release workflow will not categorise it.
- **Automatically delete head branches.**

## Why it matters for releases

The release notes (`scripts/release-notes.sh`) are built from the commit subjects between two tags. Linear history plus conventional commits is what makes that list readable. A merge commit or a `Fixed stuff` subject shows up in the release notes exactly as written.

## Verifying

1. A direct push to `main` is rejected.
2. A PR with a failing check cannot be merged.
3. After a PR merges, the Release workflow runs, pushes a version bump, and a new release appears with categorised notes.

## Related

- `CLAUDE.md` for the development workflow
- `CONTRIBUTING.md` for commit conventions
- `.github/pull_request_template.md` for what a PR should contain
