## Summary

<!-- Provide a clear and concise description of what this PR accomplishes and why it's needed -->

## Related Issues

Closes #
<!-- Use "Closes #XX" for issues this PR fully resolves -->
<!-- Use "Relates to #XX" for partial progress or related work -->

## Type of Change

<!-- Mark the applicable option with an [x] -->
- [ ] **Hotfix** - Critical production issue (can merge immediately after CI passes)
- [ ] **Bug fix** - Non-breaking change fixing an issue (wait 24h minimum before merge)
- [ ] **New feature** - Non-breaking change adding functionality (wait 24h minimum before merge)
- [ ] **Breaking change** - Fix or feature causing existing functionality to break (wait 72h, notify maintainers)
- [ ] **Documentation** - Documentation updates only
- [ ] **Performance improvement** - Code changes that improve performance
- [ ] **Refactor** - Code restructuring without behavior changes
- [ ] **Chore** - Maintenance tasks, dependency updates, CI changes

## Changes Made

<!-- List the specific changes in this PR -->
-
-
-

<!-- Include any architectural decisions, rationale, or trade-offs -->
<!-- Mention any dependencies added, removed, or updated -->

## Test Plan

### Testing Performed
- [ ] Unit tests added/updated
- [ ] All tests pass (`cargo test --all-features`)
- [ ] Clippy shows no warnings (`cargo clippy -- -D warnings`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Integration tests pass (if applicable)
- [ ] Manual testing completed
- [ ] Tested on multiple platforms (if UI changes)

### Test Commands Used
```bash
# Commands you ran to verify this PR
cargo fmt && cargo fmt -- --check
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

### Test Results
<!-- Paste relevant test output or describe results -->
```
# Test output or description here
```

## Screenshots/Demo

<!-- If this PR includes UI changes (GUI or TUI), add screenshots, GIFs, or videos -->
<!-- Delete this section if not applicable -->

### Before
<!-- Screenshot/description of old behavior -->

### After
<!-- Screenshot/description of new behavior -->

## Breaking Changes

<!-- If this is a breaking change, provide a migration guide for users -->
<!-- Delete this section if not applicable -->

### What breaks?
<!-- Describe what existing functionality changes or becomes incompatible -->

### Migration Guide
<!-- Provide step-by-step instructions for users to adapt to the changes -->
```bash
# Example migration steps
```

## Checklist

<!-- Ensure all items are completed before requesting review -->
- [ ] My code follows the project's style guidelines (`cargo fmt`)
- [ ] I have performed a self-review of my code
- [ ] I have commented complex or unclear code sections
- [ ] I have updated relevant documentation (README.md, CLAUDE.md, CONTRIBUTING.md)
- [ ] My changes generate no new warnings (`cargo clippy`)
- [ ] I have added tests that prove my fix/feature works
- [ ] All new and existing tests pass (`cargo test --all-features`)
- [ ] I have run the pre-push checks from CLAUDE.md
- [ ] I have rebased my branch on the latest main
- [ ] My commit messages follow [conventional commits](https://www.conventionalcommits.org/) format
- [ ] This PR references an issue with "Closes #XX" or "Relates to #XX"
- [ ] I have waited the minimum review time (unless this is a hotfix)

## Additional Context

<!-- Add any other context, design decisions, trade-offs, or notes for reviewers -->
<!-- Link to relevant discussions, RFCs, or external resources -->

## Reviewer Notes

<!-- Specific areas you'd like reviewers to focus on or pay attention to -->
<!-- Example: "Please pay special attention to the error handling in lines 50-75" -->
<!-- Delete this section if not applicable -->

---

### Review Timing Guidelines
⏱️ **Hotfix**: Can merge immediately after CI passes and approval
⏱️ **Regular PR**: Wait minimum **24 hours** after creation before merge
⏱️ **Breaking Change**: Wait minimum **72 hours** and notify maintainers

> **Note**: Conventional commit format in your PR title is required for automatic semantic versioning.
> Examples: `feat: add X`, `fix: resolve Y`, `docs: update Z`