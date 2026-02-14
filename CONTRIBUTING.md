# Contributing to Net-Monitor

Thank you for your interest in contributing to Net-Monitor! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

- Be respectful and inclusive
- Welcome newcomers and help them get started
- Focus on constructive criticism
- Respect differing opinions and experiences

## How to Contribute

### Reporting Issues

1. Check existing issues to avoid duplicates
2. Use issue templates when available
3. Provide detailed reproduction steps
4. Include system information (OS, Rust version)
5. Add screenshots for UI issues

### Suggesting Features

1. Check the roadmap in `.claude/features/planned.md`
2. Open an issue with the "enhancement" label
3. Describe the use case and expected behavior
4. Discuss implementation approach if you have ideas

### Submitting Code

1. Fork the repository
2. Create a feature branch (see naming conventions below)
3. Make your changes following our guidelines
4. Submit a pull request

## Development Setup

### Prerequisites

- Rust 1.70+ (stable)
- Git
- Platform-specific dependencies (see README)

### Getting Started

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/net-monitor.git
cd net-monitor

# Add upstream remote
git remote add upstream https://github.com/casey-mccarthy/net-monitor.git

# Install dependencies and build
cargo build

# Run tests
cargo test

# Run the application
cargo run
```

## Development Workflow

See `.claude/workflows/development-flow.md` for detailed workflow documentation.

### Quick Reference

1. **Branch Naming**: `feature/`, `fix/`, `docs/`, `chore/`, `refactor/`
2. **Commit Format**: `type(scope): description`
3. **PR Process**: Feature branch → PR → Review → Merge

## Coding Standards

### Rust Guidelines

- Follow Rust naming conventions
- Use `cargo fmt` for formatting
- Run `cargo clippy` and fix warnings
- Write idiomatic Rust code
- Add documentation comments for public APIs

### Code Style

```rust
// Good example
/// Monitors a node and returns its status
pub async fn monitor_node(node: &Node) -> Result<Status> {
    // Implementation
}

// Use descriptive variable names
let connection_timeout = Duration::from_secs(30);

// Prefer explicit error handling
let result = operation().context("Failed to perform operation")?;
```

### Testing

- Write unit tests for new functions
- Add integration tests for features
- Test edge cases and error conditions
- Maintain test coverage above 70%

Example test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let node = Node::new("test", "192.168.1.1");
        assert_eq!(node.name, "test");
    }
}
```

## Commit Messages

We use conventional commits for automatic versioning and changelog generation.

### Format

```
type(scope): subject

body (optional)

footer (optional)
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Formatting, missing semicolons, etc
- `refactor`: Code restructuring
- `perf`: Performance improvements
- `test`: Adding tests
- `chore`: Maintenance tasks

### Examples

```
feat(monitor): add email notification support

Implements email alerts when nodes go down or come back up.
Configurable SMTP settings and alert thresholds.

Closes #45
```

```
fix(database): resolve connection pool leak

The connection pool was not properly releasing connections
after errors, causing pool exhaustion.
```

## Pull Request Process

1. **Before Submitting**:
   - Ensure all tests pass
   - Update documentation
   - Add yourself to CONTRIBUTORS (if first contribution)
   - Fill out PR template completely

2. **PR Guidelines**:
   - Keep PRs focused on a single feature/fix
   - Include tests for new functionality
   - Update relevant documentation
   - Reference related issues

3. **Review Process**:
   - Address reviewer feedback promptly
   - Make requested changes
   - Re-request review when ready
   - Be patient - reviews take time

## Project Structure

```
net-monitor/
├── src/              # Source code
│   ├── main.rs      # Entry point
│   ├── tui.rs       # TUI implementation
│   ├── models.rs    # Data structures
│   ├── database.rs  # Database layer
│   ├── monitor.rs   # Monitoring logic
│   └── ...
├── tests/           # Integration tests
├── .claude/         # Claude documentation
├── .github/         # GitHub configuration
└── docs/            # User documentation
```

## Documentation

### Code Documentation

- Add doc comments to public functions
- Include examples in doc comments
- Document complex algorithms
- Keep comments up to date

### User Documentation

- Update README for user-facing changes
- Add entries to appropriate `.claude/` docs
- Include screenshots for UI changes

## Testing Guidelines

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo bench
```

### Writing Tests

- Test public APIs thoroughly
- Include positive and negative cases
- Test error conditions
- Use descriptive test names

## Performance Considerations

- Profile before optimizing
- Document performance-critical code
- Consider memory usage
- Test with realistic data volumes

## Security

- Never commit secrets or credentials
- Validate all user input
- Use secure defaults
- Document security considerations
- Report security issues privately

## Release Process

Releases are automated based on conventional commits:

- Breaking changes → Major version
- Features → Minor version
- Fixes → Patch version

See `.github/workflows/release.yml` for details.

## Getting Help

- Check documentation in `.claude/`
- Review existing issues and PRs
- Ask questions in issue comments
- Use Claude commands for automation

## Claude Integration

This project includes Claude commands for common tasks:

- Creating feature branches
- Making conventional commits
- Preparing pull requests
- Syncing with main
- Triggering releases

See `.claude/commands/` for documentation.

## Recognition

Contributors are recognized in:
- Git history
- GitHub contributors page
- Release notes (for significant contributions)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

## Questions?

If you have questions about contributing, please open an issue with the "question" label.