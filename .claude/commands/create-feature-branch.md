---
description: Create and switch to a new feature branch following naming conventions
---

I'll help you create a new feature branch for your development work. This ensures all your commits are isolated from main and can be properly tracked for changelog generation.

## What I'll do:

1. **Check current branch status**
   - Ensure working directory is clean
   - Verify we're on main or an appropriate base branch

2. **Create the feature branch**
   - Follow naming convention: `feature/<feature-name>` or `fix/<issue-name>`
   - Branch types:
     - `feature/` - New features
     - `fix/` - Bug fixes
     - `chore/` - Maintenance tasks
     - `docs/` - Documentation updates
     - `refactor/` - Code refactoring

3. **Set up branch tracking**
   - Configure upstream tracking
   - Push branch to remote with tracking

4. **Update context**
   - Document the feature being worked on
   - Update `.claude/context/project-state.md` with current work

## Usage:

Just tell me:
- What feature or fix you're working on
- Any related issue numbers
- Whether this is a feature, fix, chore, docs, or refactor

Example prompts:
- "Create a feature branch for adding email notifications"
- "Create a fix branch for issue #42"
- "Create a chore branch for updating dependencies"

The branch will be created and you'll be switched to it, ready to start making commits that will be included in the changelog when merged.