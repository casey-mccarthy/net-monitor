---
description: Trigger a new release with automated changelog generation and version bumping
---

I'll help you create a new release with proper versioning and comprehensive changelog generation.

## What I'll do:

1. **Pre-release checks**
   - Ensure we're on main branch
   - Verify all tests pass
   - Check for uncommitted changes
   - Review recent merged PRs

2. **Determine version bump**
   - Analyze commits since last release
   - Determine semantic version bump:
     - MAJOR: Breaking changes (BREAKING CHANGE in commit)
     - MINOR: New features (feat: commits)
     - PATCH: Bug fixes (fix: commits)

3. **Generate comprehensive changelog**
   - Aggregate all commits since last release
   - Group by PR if applicable
   - Organize by type:
     - Breaking Changes
     - Features
     - Bug Fixes
     - Performance Improvements
     - Documentation
     - Other Changes

4. **Create release**
   - Update version in Cargo.toml
   - Generate release notes
   - Create git tag
   - Push to trigger release workflow

## Changelog Format:

```markdown
# Release v0.3.0

## Breaking Changes
- BREAKING: Changed API authentication method

## Features
- feat: Add SSH connection support (#45)
- feat: Implement credential storage system

## Bug Fixes
- fix: Resolve memory leak in monitoring loop
- fix: Correct timezone handling

## Performance
- perf: Optimize database queries

## Documentation
- docs: Update installation guide
```

## Version Determination:

The version bump is automatic based on commits:
- Any BREAKING CHANGE → Major version
- Any feat: commits → Minor version
- Only fix: commits → Patch version

## Release Process:

1. Merge all PRs intended for release
2. Switch to main branch
3. Run this command
4. Review generated changelog
5. Confirm release creation
6. GitHub Actions will build and publish

## Usage:

Just say:
- "Create a new release"
- "Trigger a release"
- "Release the current version"

## Manual Override:

If you need to force a specific version:
- "Create a major release" (x.0.0)
- "Create a minor release" (0.x.0)
- "Create a patch release" (0.0.x)