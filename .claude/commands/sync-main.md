---
description: Sync your feature branch with the latest changes from main
---

I'll help you sync your feature branch with the latest changes from main using rebasing to maintain a clean, linear commit history.

## What I'll do:

1. **Save current work**
   - Check for uncommitted changes
   - Stash if necessary or prompt to commit

2. **Update main branch**
   - Fetch latest changes from origin
   - Update local main branch reference

3. **Rebase on main (Default)**
   - Rebase feature branch on top of main (creates linear history)
   - Handle any conflicts interactively
   - OR fall back to merge if requested (preserves exact commit history)

4. **Verify sync**
   - Check branch status
   - Run tests to ensure nothing broke
   - Push updated branch (using --force-with-lease for safety)

## Sync Strategies:

### Rebase (Default - Recommended)
- Creates linear, clean history
- No merge commit clutter
- Better for changelog generation
- Easier to use `git bisect` for debugging
- Better for code review

### Merge (Fallback Option)
- Preserves exact commit timestamps
- Shows explicit merge history
- Use only if rebase conflicts are complex

## Conflict Resolution:

If conflicts occur during rebase, I'll:
1. Show you the conflicting files
2. Help you understand the conflicts
3. Guide you through resolution
4. Stage resolved files with `git add`
5. Continue rebase with `git rebase --continue`
6. Or abort if needed with `git rebase --abort`

## Usage:

Just say:
- "Sync with main" (uses rebase by default)
- "Rebase on main" (explicit rebase)
- "Update my branch from main"
- "Sync with main using merge" (if you specifically want merge)

## What Gets Run:

```bash
# Default rebase workflow
git fetch origin main
git rebase origin/main
git push origin <branch-name> --force-with-lease
```

## Best Practices:

- Sync regularly to avoid large conflicts
- **Always sync before creating a PR** to ensure clean history
- Commit or stash your work before syncing
- Run tests after syncing to verify everything works
- Use `--force-with-lease` instead of `--force` when pushing (safer)
- If rebase gets messy, you can always `git rebase --abort` and try merge instead