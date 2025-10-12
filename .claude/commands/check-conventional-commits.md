---
description: Check and fix PR/branch to follow conventional commits standards
---

I'll analyze the current branch and PR to ensure they follow conventional commits standards, then make any necessary changes to trigger the correct repository workflows.

**Reference**: [Conventional Commits v1.0.0 Specification](https://www.conventionalcommits.org/en/v1.0.0/)

## What I'll do:

1. **Analyze current state**
   - Check if there's an open PR for the current branch
   - Review PR title and description
   - Examine commit messages on the branch
   - Identify branch name pattern

2. **Check conventional commits compliance**
   - Verify commit message format: `type(scope): description`
   - Validate PR title follows same format
   - Check if commit type matches the actual changes
   - Ensure proper use of semantic versioning triggers

3. **Determine correct commit type**
   Based on the changes in the branch (per official spec):
   - `feat`: New feature (triggers MINOR version bump)
   - `fix`: Bug fix, including security fixes (triggers PATCH version bump)

   Additional types (from Angular convention, commonly used):
   - `build`: Changes affecting build system or dependencies
   - `ci`: Changes to CI configuration files and scripts
   - `docs`: Documentation only changes
   - `perf`: Performance improvement (usually triggers PATCH version bump)
   - `refactor`: Code change that neither fixes a bug nor adds a feature
   - `style`: Changes that don't affect code meaning (formatting, whitespace)
   - `test`: Adding missing tests or correcting existing tests
   - `chore`: Other changes that don't modify src or test files
   - `revert`: Revert previous commit

4. **Make necessary corrections**
   - Update PR title if needed
   - Amend commit message(s) if needed
   - Force push changes with `--force-with-lease`
   - Verify authorship before amending

5. **Verify workflow triggers**
   - Confirm the commit type will trigger intended workflows
   - Check if semantic-release will process correctly
   - Validate that version bumping will work as expected

## Conventional Commits Format:

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Valid Types (Official Spec):
- **feat**: A new feature (triggers MINOR version bump → 0.x.0)
- **fix**: A bug fix, including security fixes (triggers PATCH version bump → 0.0.x)

### Additional Types (Angular Convention - Widely Adopted):
- **build**: Changes affecting build system or dependencies (no version bump)
- **ci**: Changes to CI configuration files and scripts (no version bump)
- **docs**: Documentation only changes (no version bump)
- **perf**: Performance improvement (usually triggers PATCH version bump)
- **refactor**: Code change that neither fixes a bug nor adds a feature (no version bump)
- **style**: Changes that don't affect code meaning - formatting, whitespace (no version bump)
- **test**: Adding missing tests or correcting existing tests (no version bump)
- **chore**: Other changes that don't modify src or test files (no version bump)
- **revert**: Reverts a previous commit (no version bump)

**Important**: The official Conventional Commits specification only defines `feat` and `fix`. All other types are conventions from Angular and are widely adopted but not part of the core spec.

### Breaking Changes:
Add `!` after type or `BREAKING CHANGE:` in footer for MAJOR version bump:
```
feat!: change API authentication method

BREAKING CHANGE: API now requires JWT tokens instead of API keys
```

## Examples:

### Good PR Titles:
- `feat(monitor): add support for SSH connections`
- `fix: resolve memory leak in connection pool`
- `fix: add permissions block to CI workflow` (security fixes use `fix`)
- `docs: update installation instructions`
- `chore: bump dependencies to latest versions`

### Bad PR Titles:
- `Update README` (missing type)
- `Potential fix for issue #42` (missing type)
- `Added new feature for monitoring` (wrong tense, missing type)
- `Fix bug` (too vague, missing specifics)

## What happens after:

1. **PR title updated**: Reflects proper conventional commit format
2. **Commit message amended**: Follows conventional commits standard
3. **Changes force-pushed**: Using `--force-with-lease` for safety
4. **Workflows triggered**: Based on commit type:
   - `feat` → triggers MINOR version bump and release (0.x.0)
   - `fix` → triggers PATCH version bump and release (0.0.x)
   - `feat!` or `fix!` with BREAKING CHANGE → triggers MAJOR version bump (x.0.0)
   - `docs`, `chore`, `ci`, `style`, `refactor`, `test` → no automatic release

## Safety checks:

- Verify commit authorship before amending
- Use `--force-with-lease` to prevent overwriting others' work
- Check that branch is not pushed to main/master
- Confirm PR exists before making changes

## Usage:

Simply run `/check-conventional-commits` and I'll:
1. Analyze your current branch and PR
2. Determine if changes are needed
3. Make corrections to follow conventional commits
4. Ensure proper workflow triggers are set
