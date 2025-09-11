---
description: Prepare and create a pull request with automated changelog generation
---

I'll help you prepare and create a pull request with a properly formatted changelog based on your branch commits.

## What I'll do:

1. **Pre-flight checks**
   - Ensure all changes are committed
   - Run tests and build verification
   - Check for lint issues
   - Verify branch is up to date with main

2. **Generate changelog from commits**
   - Analyze all commits in the current branch
   - Group by type (features, fixes, etc.)
   - Create formatted changelog entry
   - Include breaking changes if any

3. **Create pull request**
   - Generate PR title from branch name and commits
   - Create comprehensive PR description including:
     - Summary of changes
     - Detailed changelog
     - Testing checklist
     - Related issues
   - Use GitHub CLI to create the PR

4. **Post-creation tasks**
   - Provide PR URL for review
   - Update project tracking
   - Suggest next steps

## Changelog Format:

The PR will include a changelog section like:

```markdown
## Changes in this PR

### Features
- feat: Add email notification system (#123)
- feat: Implement user preferences panel

### Bug Fixes
- fix: Resolve connection timeout issue
- fix: Correct data validation in forms

### Other Changes
- chore: Update dependencies
- docs: Improve API documentation
```

## Requirements:

- You must be on a feature branch (not main)
- All changes must be committed
- GitHub CLI (gh) must be configured

## Usage:

Just say: "Prepare a PR" or "Create a pull request"

I'll handle the rest, including generating the changelog from your commits and creating a well-formatted PR ready for review.