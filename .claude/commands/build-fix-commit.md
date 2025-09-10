---
description: Build project, fix errors automatically, and create conventional commit
---

I'll help you build your Rust project, automatically fix any errors and warnings found, and create a conventional commit. Here's what I'll do:

1. **Build and Check Phase**:
   - Run `cargo check` to identify compilation errors
   - Run `cargo build` to verify full build process
   - Run `cargo clippy` for additional linting suggestions
   - Run `cargo fmt` to ensure consistent formatting

2. **Error and Warning Fixing Phase**:
   - Analyze any compilation errors found
   - Automatically fix syntax errors, type mismatches, and missing imports
   - Fix dead code warnings by adding `#[allow(dead_code)]` annotations or removing unused code
   - Apply clippy suggestions for better code quality
   - Re-run checks after each fix to ensure resolution

3. **Commit Phase**:
   - Determine appropriate conventional commit type based on changes:
     - `feat:` for new features
     - `fix:` for bug fixes
     - `refactor:` for code improvements
     - `style:` for formatting changes
     - `chore:` for maintenance tasks
   - Create descriptive commit message with details of fixes applied
   - Stage all changes and create the commit

4. **Final Verification**:
   - Ensure build passes completely before committing
   - Show git diff for review
   - Only commit if all checks pass

Let's start the build, fix, and commit process for your Rust project.