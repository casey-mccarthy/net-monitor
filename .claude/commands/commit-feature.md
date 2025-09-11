---
description: Create a properly formatted conventional commit for your changes
---

I'll help you create conventional commits that follow the project standards and contribute to automatic changelog generation.

## What I'll do:

1. **Analyze changes**
   - Review modified files
   - Understand the nature of changes
   - Identify the appropriate commit type

2. **Determine commit type**
   - `feat`: New feature added
   - `fix`: Bug fix
   - `docs`: Documentation changes
   - `style`: Code style changes (formatting, semicolons, etc.)
   - `refactor`: Code refactoring (no functional changes)
   - `perf`: Performance improvements
   - `test`: Test additions or corrections
   - `chore`: Maintenance tasks
   - `build`: Build system changes
   - `ci`: CI/CD changes

3. **Create commit message**
   - Format: `type(scope): description`
   - Add body for detailed explanation
   - Include footer for breaking changes or issue references

4. **Stage and commit**
   - Stage appropriate files
   - Create commit with formatted message
   - Verify commit follows standards

## Commit Message Structure:

```
type(scope): short description (50 chars or less)

Longer explanation of the change if needed. Wrap at 72 characters.
Explain the problem this commit solves and why this change was made.

Fixes #123
BREAKING CHANGE: Description of breaking change if applicable
```

## Examples:

### Simple feature:
```
feat(monitor): add retry logic for failed connections
```

### Bug fix with details:
```
fix(database): resolve connection pool exhaustion

The connection pool was not properly releasing connections after
errors, leading to pool exhaustion. This commit ensures connections
are always returned to the pool.

Fixes #42
```

### Breaking change:
```
feat(api)!: change authentication to use JWT tokens

BREAKING CHANGE: API now requires JWT tokens instead of API keys.
Users must update their client configuration.
```

## Best Practices:

1. **One logical change per commit**
2. **Write in imperative mood** ("add" not "added")
3. **Reference issues** when applicable
4. **Explain why**, not just what
5. **Keep subject line under 50 characters**

## Usage:

Just tell me what you changed, and I'll:
- Suggest the appropriate commit type
- Format the message properly
- Create the commit

Examples:
- "Commit my changes to the login system"
- "I fixed the timeout bug"
- "Commit the new SSH feature"