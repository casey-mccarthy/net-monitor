# Branch Protection Setup for net-monitor

This document outlines the branch protection rules needed to ensure code quality and enable proper changelog generation.

## GitHub Settings Configuration

Navigate to your repository settings on GitHub: https://github.com/casey-mccarthy/net-monitor/settings/branches

### Main Branch Protection Rules

1. Go to **Settings** → **Branches**
2. Click **Add rule** or edit existing rule for `main`
3. Configure the following settings:

#### Required Settings

- [x] **Require a pull request before merging**
  - [x] Require approvals: 1 (or adjust based on team size)
  - [x] Dismiss stale pull request approvals when new commits are pushed
  - [x] Require review from CODEOWNERS (optional)

- [x] **Require status checks to pass before merging**
  - [x] Require branches to be up to date before merging
  - Add required status checks:
    - `build` (from CI workflow)
    - `test` (if applicable)
    - `lint` (if applicable)

- [x] **Require conversation resolution before merging**
  - Ensures all PR comments are addressed

- [x] **Include administrators**
  - Applies these rules even to repository administrators
  - Ensures consistent workflow for all contributors

#### Optional but Recommended

- [ ] **Require signed commits**
  - Adds additional security verification

- [x] **Require linear history**
  - Prevents merge commits, keeping history clean
  - Use squash or rebase merging

- [x] **Lock branch**
  - Prevents force pushes and deletions
  - Read-only for everyone

- [ ] **Restrict who can push to matching branches**
  - Limit to specific users or teams if needed

## Pull Request Settings

Navigate to **Settings** → **General** → **Pull Requests**:

1. **Merge button**:
   - [x] Allow squash merging (recommended for clean history)
   - [ ] Allow merge commits (optional)
   - [x] Allow rebase merging (for linear history)

2. **Default commit message**:
   - Use pull request title and description

3. **Automatically delete head branches**:
   - [x] Enable to keep repository clean

## Workflow Benefits

With these protections in place:

1. **All changes go through PR review** - No direct pushes to main
2. **Commit history is preserved** - Each PR's commits contribute to changelog
3. **Quality gates are enforced** - Tests must pass before merging
4. **Consistent process** - Same rules apply to everyone

## Verification

After setting up branch protection:

1. Try to push directly to main - should be rejected
2. Create a test PR - should require review
3. Check that status checks are required
4. Verify administrators cannot bypass rules

## Troubleshooting

If you need to make emergency changes:
1. Create a PR with detailed explanation
2. Request expedited review
3. If absolutely necessary, temporarily disable "Include administrators"
4. Re-enable immediately after emergency fix

## Related Documentation

- See `.github/pull_request_template.md` for PR guidelines
- See `CONTRIBUTING.md` for development workflow
- See `.claude/workflows/development-flow.md` for detailed process