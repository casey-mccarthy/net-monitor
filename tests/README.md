# Net Monitor Test Suite

This directory contains comprehensive tests for the Net Monitor application, including unit tests, integration tests, and test utilities.

## Test Structure

### Unit Tests
All unit tests have been moved to the `tests/` directory following Rust best practices:
- `tests/models_tests.rs` - Tests for data structures and serialization (11 tests)
- `tests/monitoring_tests.rs` - Tests for monitoring functionality (16 tests)
- `tests/database_tests.rs` - Tests for database operations (21 tests)
- `tests/credentials_tests.rs` - Tests for credentials module (2 tests)

### Integration Tests
Integration tests are also located in the `tests/` directory:
- `tests/database_tests.rs` - Database persistence and CRUD operations
- `tests/monitoring_tests.rs` - End-to-end monitoring workflows
- `tests/import_export_tests.rs` - Import/export functionality (9 tests)
- `tests/common/mod.rs` - Shared test utilities, fixtures, and assertions

## Running Tests

### Basic Test Commands

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_check_node_http_success

# Run tests in a specific module
cargo test models::tests

# Run integration tests only
cargo test --test integration_tests

# Run tests with verbose output
cargo test -- --nocapture --test-threads=1
```

### Test Categories

#### Unit Tests
```bash
# Run unit tests for models
cargo test --test models_tests

# Run unit tests for monitoring
cargo test --test monitoring_tests

# Run unit tests for database
cargo test --test database_tests

# Run unit tests for credentials
cargo test --test credentials_tests
```

#### Integration Tests
```bash
# Run integration tests for import/export
cargo test --test import_export_tests

# Run database integration tests
cargo test --test database_tests

# Run monitoring integration tests
cargo test --test monitoring_tests
```

### Network Tests

Some tests require network access to external services (like httpbin.org). These tests are **disabled by default** to prevent flaky CI failures.

To run network tests locally:
```bash
cargo test --features network-tests
```

**Note:** Network tests are automatically excluded in CI to maintain test reliability and avoid issues with:
- External service unavailability
- Network timeouts
- DNS resolution issues
- Rate limiting

### Test Configuration

The test suite supports different configurations based on environment variables:

#### Environment Variables

- `NET_MONITOR_TEST_ENV` - Set to `ci`, `development`, or `fast`
- `NET_MONITOR_SKIP_NETWORK_TESTS` - Set to `1` or `true` to skip network tests
- `NET_MONITOR_SKIP_SLOW_TESTS` - Set to `1` or `true` to skip slow tests
- `NET_MONITOR_SKIP_INTEGRATION_TESTS` - Set to `1` or `true` to skip integration tests
- `NET_MONITOR_HTTP_TIMEOUT` - HTTP test timeout in seconds
- `NET_MONITOR_PING_TIMEOUT` - Ping test timeout in seconds
- `NET_MONITOR_MAX_CONCURRENT_TESTS` - Maximum concurrent tests

#### Test Environments

**Development (default):**
```bash
# Full test suite with network tests
cargo test
```

**CI Environment:**
```bash
# Skip network and slow tests for CI
NET_MONITOR_TEST_ENV=ci cargo test
```

**Fast Tests Only:**
```bash
# Only unit tests, no network or integration tests
NET_MONITOR_TEST_ENV=fast cargo test
```

**Custom Configuration:**
```bash
# Custom timeouts and skip network tests
NET_MONITOR_HTTP_TIMEOUT=5 \
NET_MONITOR_PING_TIMEOUT=2 \
NET_MONITOR_SKIP_NETWORK_TESTS=1 \
cargo test
```

## Test Coverage

### Models Module Tests (tests/models_tests.rs)
- Data structure creation and validation
- Serialization/deserialization (JSON)
- Display formatting
- Partial equality comparisons
- Enum variant handling
- MonitorDetail and NodeStatus types
- Node and NodeImport creation

### Monitoring Module Tests (tests/monitoring_tests.rs)
- HTTP monitoring (success and failure cases)
- Ping monitoring (localhost and invalid hosts)
- Response time measurement
- Error handling and propagation
- Async/await functionality
- Network tests (with feature flag)
- Concurrent monitoring operations
- Full monitoring workflows

### Database Module Tests (tests/database_tests.rs)
- Database creation and initialization
- CRUD operations (Create, Read, Update, Delete)
- Node storage and retrieval (HTTP and Ping nodes)
- Monitoring result storage
- Database persistence across connections
- Error handling for invalid data
- Node status parsing
- Response time and last check handling

### Credentials Module Tests (tests/credentials_tests.rs)
- SensitiveString creation and usage
- SSH credential types (Default, Password, Key)
- Username extraction from credentials
- Secret requirement checking

### Integration Tests
- Complete monitoring workflows
- Database persistence across sessions
- Concurrent monitoring operations
- Import/export functionality (tests/import_export_tests.rs)
- Error recovery scenarios
- End-to-end application flows

## Test Utilities

### Common Test Functions
Located in `tests/common/mod.rs`:

**Test Database Fixture:**
- `TestDatabase` - RAII test database fixture with automatic cleanup

**Node Builders:**
- `NodeBuilder` - Fluent API for creating test nodes
- Builder methods: `.name()`, `.http()`, `.ping()`, `.monitoring_interval()`

**Fixtures (in `fixtures` module):**
- `http_node()` - Standard HTTP test node (httpbin.org/status/200)
- `http_failure_node()` - HTTP node that returns 404
- `ping_node()` - Standard Ping test node (127.0.0.1)
- `unit_test_http_node()` - HTTP node for unit tests (example.com)
- `unit_test_ping_node()` - Ping node for unit tests

**Assertions (in `assertions` module):**
- `assert_http_node()` - Validates HTTP node properties
- `assert_ping_node()` - Validates ping node properties

## Test Data

### HTTP Test Endpoints
- `https://httpbin.org/status/200` - Returns HTTP 200
- `https://httpbin.org/status/404` - Returns HTTP 404
- `https://httpbin.org/status/500` - Returns HTTP 500

### Ping Test Hosts
- `127.0.0.1` - Localhost (should always be reachable)
- `8.8.8.8` - Google DNS (usually reachable)

## Best Practices

### Writing Tests
1. **Use descriptive test names** that explain what is being tested
2. **Test both success and failure cases** for each function
3. **Use temporary databases** for database tests
4. **Clean up resources** after tests complete
5. **Handle async operations** properly with `#[tokio::test]`
6. **Use test utilities** from `common/mod.rs` for consistency

### Test Organization
1. **Unit tests** are now in the `tests/` directory (following Rust best practices)
2. **Integration tests** are also in the `tests/` directory
3. **Shared utilities** go in `tests/common/mod.rs`
4. **Test fixtures and assertions** are organized in submodules within `common/mod.rs`

### Performance Considerations
1. **Use temporary files** for database tests
2. **Limit network calls** in unit tests
3. **Use appropriate timeouts** for network tests
4. **Run slow tests separately** from fast tests
5. **Use concurrent test execution** when appropriate

## Troubleshooting

### Common Issues

**Tests failing due to network issues:**
```bash
# Skip network tests
NET_MONITOR_SKIP_NETWORK_TESTS=1 cargo test
```

**Tests timing out:**
```bash
# Increase timeouts
NET_MONITOR_HTTP_TIMEOUT=30 \
NET_MONITOR_PING_TIMEOUT=15 \
cargo test
```

**Database tests failing:**
```bash
# Ensure SQLite is available
# Check file permissions for temporary files
```

**Concurrent test issues:**
```bash
# Run tests sequentially
cargo test -- --test-threads=1
```

### Debugging Tests
```bash
# Run with verbose output
cargo test -- --nocapture

# Run specific test with output
cargo test test_name -- --nocapture

# Run with debug logging
RUST_LOG=debug cargo test
```

## Continuous Integration

The test suite is designed to work in CI environments:

```yaml
# Example GitHub Actions configuration
- name: Run tests
  env:
    NET_MONITOR_TEST_ENV: ci
  run: cargo test
```

CI configuration:
- Skips network tests by default
- Uses shorter timeouts
- Runs integration tests
- Uses temporary databases
- Cleans up after tests

## Contributing

When adding new tests:

1. **Follow existing patterns** for test organization
2. **Add appropriate test coverage** for new functionality
3. **Update this documentation** if adding new test categories
4. **Use test utilities** from `common/mod.rs`
5. **Consider CI compatibility** for new tests
6. **Add environment variables** for configurable test behavior 