use std::env;

/// Test environment configuration
#[derive(Debug, Clone)]
pub struct TestEnvironment {
    /// Whether to run network-dependent tests
    pub run_network_tests: bool,
    /// Whether to run slow tests
    pub run_slow_tests: bool,
    /// Whether to run integration tests
    pub run_integration_tests: bool,
    /// Timeout for HTTP tests in seconds
    pub http_timeout: u64,
    /// Timeout for ping tests in seconds
    pub ping_timeout: u64,
    /// Base URL for HTTP testing
    pub http_test_url: String,
    /// Test host for ping testing
    pub ping_test_host: String,
}

impl Default for TestEnvironment {
    fn default() -> Self {
        Self {
            run_network_tests: true,
            run_slow_tests: true,
            run_integration_tests: true,
            http_timeout: 10,
            ping_timeout: 5,
            http_test_url: "https://httpbin.org".to_string(),
            ping_test_host: "127.0.0.1".to_string(),
        }
    }
}

impl TestEnvironment {
    /// Creates a test environment from environment variables
    pub fn from_env() -> Self {
        let mut env = Self::default();

        // Check for environment variables to disable certain test types
        if let Ok(val) = env::var("NET_MONITOR_SKIP_NETWORK_TESTS") {
            if val == "1" || val.to_lowercase() == "true" {
                env.run_network_tests = false;
            }
        }

        if let Ok(val) = env::var("NET_MONITOR_SKIP_SLOW_TESTS") {
            if val == "1" || val.to_lowercase() == "true" {
                env.run_slow_tests = false;
            }
        }

        if let Ok(val) = env::var("NET_MONITOR_SKIP_INTEGRATION_TESTS") {
            if val == "1" || val.to_lowercase() == "true" {
                env.run_integration_tests = false;
            }
        }

        // Override timeouts if specified
        if let Ok(val) = env::var("NET_MONITOR_HTTP_TIMEOUT") {
            if let Ok(timeout) = val.parse() {
                env.http_timeout = timeout;
            }
        }

        if let Ok(val) = env::var("NET_MONITOR_PING_TIMEOUT") {
            if let Ok(timeout) = val.parse() {
                env.ping_timeout = timeout;
            }
        }

        // Override test URLs/hosts if specified
        if let Ok(url) = env::var("NET_MONITOR_HTTP_TEST_URL") {
            env.http_test_url = url;
        }

        if let Ok(host) = env::var("NET_MONITOR_PING_TEST_HOST") {
            env.ping_test_host = host;
        }

        env
    }

    /// Returns true if network tests should be run
    pub fn should_run_network_tests(&self) -> bool {
        self.run_network_tests
    }

    /// Returns true if slow tests should be run
    pub fn should_run_slow_tests(&self) -> bool {
        self.run_slow_tests
    }

    /// Returns true if integration tests should be run
    pub fn should_run_integration_tests(&self) -> bool {
        self.run_integration_tests
    }

    /// Creates a CI-friendly test environment
    pub fn ci() -> Self {
        Self {
            run_network_tests: false, // Skip network tests in CI
            run_slow_tests: false,    // Skip slow tests in CI
            run_integration_tests: true,
            http_timeout: 5,
            ping_timeout: 2,
            http_test_url: "https://httpbin.org".to_string(),
            ping_test_host: "127.0.0.1".to_string(),
        }
    }

    /// Creates a development test environment
    pub fn development() -> Self {
        Self {
            run_network_tests: true,
            run_slow_tests: true,
            run_integration_tests: true,
            http_timeout: 15,
            ping_timeout: 10,
            http_test_url: "https://httpbin.org".to_string(),
            ping_test_host: "127.0.0.1".to_string(),
        }
    }
}

/// Test configuration for different scenarios
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// The test environment
    pub environment: TestEnvironment,
    /// Whether to use temporary databases
    pub use_temp_db: bool,
    /// Whether to clean up after tests
    pub cleanup_after_tests: bool,
    /// Maximum number of concurrent tests
    pub max_concurrent_tests: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            environment: TestEnvironment::from_env(),
            use_temp_db: true,
            cleanup_after_tests: true,
            max_concurrent_tests: 4,
        }
    }
}

impl TestConfig {
    /// Creates a test configuration for CI
    pub fn ci() -> Self {
        Self {
            environment: TestEnvironment::ci(),
            use_temp_db: true,
            cleanup_after_tests: true,
            max_concurrent_tests: 2,
        }
    }

    /// Creates a test configuration for development
    pub fn development() -> Self {
        Self {
            environment: TestEnvironment::development(),
            use_temp_db: true,
            cleanup_after_tests: true,
            max_concurrent_tests: 8,
        }
    }

    /// Creates a test configuration for fast tests only
    pub fn fast() -> Self {
        Self {
            environment: TestEnvironment {
                run_network_tests: false,
                run_slow_tests: false,
                run_integration_tests: false,
                ..Default::default()
            },
            use_temp_db: true,
            cleanup_after_tests: true,
            max_concurrent_tests: 4,
        }
    }
}

// Global test configuration instance
lazy_static::lazy_static! {
    pub static ref TEST_CONFIG: TestConfig = TestConfig::from_env();
}

impl TestConfig {
    /// Creates a test configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Override based on environment
        if let Ok(env) = env::var("NET_MONITOR_TEST_ENV") {
            match env.to_lowercase().as_str() {
                "ci" => config = Self::ci(),
                "development" => config = Self::development(),
                "fast" => config = Self::fast(),
                _ => {}
            }
        }

        // Override concurrent test limit
        if let Ok(val) = env::var("NET_MONITOR_MAX_CONCURRENT_TESTS") {
            if let Ok(limit) = val.parse() {
                config.max_concurrent_tests = limit;
            }
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_environment_default() {
        let env = TestEnvironment::default();
        assert!(env.run_network_tests);
        assert!(env.run_slow_tests);
        assert!(env.run_integration_tests);
        assert_eq!(env.http_timeout, 10);
        assert_eq!(env.ping_timeout, 5);
    }

    #[test]
    fn test_test_environment_ci() {
        let env = TestEnvironment::ci();
        assert!(!env.run_network_tests);
        assert!(!env.run_slow_tests);
        assert!(env.run_integration_tests);
        assert_eq!(env.http_timeout, 5);
        assert_eq!(env.ping_timeout, 2);
    }

    #[test]
    fn test_test_config_default() {
        let config = TestConfig::default();
        assert!(config.use_temp_db);
        assert!(config.cleanup_after_tests);
        assert_eq!(config.max_concurrent_tests, 4);
    }

    #[test]
    fn test_test_config_ci() {
        let config = TestConfig::ci();
        assert!(!config.environment.run_network_tests);
        assert!(!config.environment.run_slow_tests);
        assert_eq!(config.max_concurrent_tests, 2);
    }

    #[test]
    fn test_test_config_fast() {
        let config = TestConfig::fast();
        assert!(!config.environment.run_network_tests);
        assert!(!config.environment.run_slow_tests);
        assert!(!config.environment.run_integration_tests);
    }
}
