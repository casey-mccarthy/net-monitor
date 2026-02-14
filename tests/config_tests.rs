// Unit tests for config module

use net_monitor::config::AppConfig;
use tempfile::TempDir;

#[test]
fn test_app_config_default() {
    let config = AppConfig::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("AppConfig"));
}

#[test]
fn test_app_config_clone() {
    let config = AppConfig {};
    let _cloned = config.clone();
}

#[test]
fn test_app_config_debug() {
    let config = AppConfig {};
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("AppConfig"));
}

#[test]
fn test_app_config_serialization() {
    let config = AppConfig {};
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains('{'));
    assert!(json.contains('}'));
}

#[test]
fn test_app_config_deserialization() {
    let json = r#"{}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    let _ = format!("{:?}", config);
}

#[test]
fn test_app_config_save_and_load() {
    let _temp_dir = TempDir::new().unwrap();

    let config = AppConfig {};

    // Test serialization round-trip
    let json = serde_json::to_string_pretty(&config).unwrap();
    let _loaded_config: AppConfig = serde_json::from_str(&json).unwrap();
}

#[test]
fn test_app_config_pretty_serialization() {
    let config = AppConfig {};
    let json = serde_json::to_string_pretty(&config).unwrap();
    assert!(json.contains('{'));
    assert!(json.contains('}'));
}

#[test]
fn test_app_config_load_invalid_json() {
    let invalid_json = "{ this is not valid json }";
    let result: Result<AppConfig, _> = serde_json::from_str(invalid_json);
    assert!(result.is_err());
}

#[test]
fn test_app_config_deserialization_extra_fields() {
    // Extra fields should be ignored
    let json = r#"{"extra_field":"ignored"}"#;
    let _config: AppConfig = serde_json::from_str(json).unwrap();
}

#[test]
fn test_app_config_save_and_load_roundtrip() {
    let original_config = AppConfig {};

    // Serialize (like save)
    let json = serde_json::to_string_pretty(&original_config).unwrap();

    // Deserialize (like load)
    let _loaded_config: AppConfig = serde_json::from_str(&json).unwrap();
}
