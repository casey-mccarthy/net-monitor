---
description: Sync your feature branch with the latest changes from main
---

I'll help you sync your feature branch with the latest changes from main to avoid conflicts and stay up to date.

## What I'll do:

1. **Save current work**
   - Check for uncommitted changes
   - Stash if necessary or prompt to commit

2. **Update main branch**
   - Fetch latest changes from origin
   - Update local main branch

3. **Merge or rebase**
   - Merge main into current feature branch (preserves commit history)
   - OR rebase feature branch on main (linear history)
   - Handle any conflicts interactively

4. **Verify sync**
   - Check branch status
   - Run tests to ensure nothing broke
   - Update remote feature branch

## Sync Strategies:

### Merge (Default - Recommended)
- Preserves all commit timestamps
- Shows clear merge history
- Better for collaborative branches

### Rebase (Optional)
- Creates linear history
- Cleaner commit log
- Better for solo work before PR

## Conflict Resolution:

If conflicts occur, I'll:
1. Show you the conflicting files
2. Help you understand the conflicts
3. Guide you through resolution
4. Verify the resolution works

## Usage:

Just say:
- "Sync with main" (uses merge)
- "Rebase on main" (uses rebase)
- "Update my branch from main"

## Best Practices:

- Sync regularly to avoid large conflicts
- Always sync before creating a PR
- Commit or stash your work before syncing
- Run tests after syncing to verify everything works