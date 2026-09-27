---
description: Trigger a new release with automated changelog generation and version bumping
---

Releases are automatic. Nothing is tagged or bumped by hand; this command checks that a release will happen and shows what it will say.

## How a release happens

Merging a pull request to `main` runs CI, and when CI passes `.github/workflows/release.yml`:

1. Looks at the commits since the last tag and picks the bump: `BREAKING CHANGE` or `!` → major, `feat` → minor, anything else conventional → patch. No conventional commits → no release.
2. Commits `chore: bump version to X.Y.Z` to `Cargo.toml` on `main` and tags it `vX.Y.Z`.
3. Builds archives for Linux x64, macOS x64, macOS ARM64, and Windows x64, plus `checksums.txt`.
4. Publishes a GitHub release whose notes come from `scripts/release-notes.sh`: the commits since the previous tag grouped into Breaking Changes, Features, Bug Fixes, Performance, Documentation, and Other Changes, followed by the download table and a compare link.

## What I'll do

1. Confirm the working tree is clean and `main` is up to date.
2. List the commits since the last tag and state which bump they will produce.
3. Preview the release notes:

   ```bash
   NEXT=<computed version>
   LAST=$(git describe --tags --abbrev=0)
   scripts/release-notes.sh "$NEXT" "$LAST"
   ```

4. Point out any commit that will read badly in the notes (non-conventional subject, wrong type) so it can be fixed before it merges.

## Forcing a specific bump

Change the commits, not the workflow. A `feat:` commit forces a minor bump; a `!` after the type or a `BREAKING CHANGE:` footer forces a major bump. If a release must be re-run, re-run the failed Release workflow from the Actions tab; do not push tags by hand.
