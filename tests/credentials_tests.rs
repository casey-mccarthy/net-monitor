// Unit tests for credentials module
// Moved from src/credentials.rs to follow Rust best practices

use net_monitor::credentials::{SensitiveString, SshCredential};

#[test]
fn test_sensitive_string() {
    let sensitive = SensitiveString::new("secret".to_string());
    assert_eq!(sensitive.as_str(), "secret");

    // Test zeroization
    drop(sensitive);
    // In a real test, we'd verify memory was zeroed
}

#[test]
fn test_ssh_credential_types() {
    let default_cred = SshCredential::Default;
    assert_eq!(default_cred.username(), None);
    assert!(!default_cred.requires_secret());

    let password_cred = SshCredential::Password {
        username: "user".to_string(),
        password: "pass".into(),
    };
    assert_eq!(password_cred.username(), Some("user"));
    assert!(password_cred.requires_secret());

    let key_cred = SshCredential::Key {
        username: "user".to_string(),
        private_key_path: "/path/to/key".into(),
        passphrase: None,
    };
    assert_eq!(key_cred.username(), Some("user"));
    assert!(!key_cred.requires_secret());
}

// Note: test_file_credential_store was removed because it requires access to
// private fields of FileCredentialStore. This test would be better suited as
// an integration test if FileCredentialStore gets a public constructor.
