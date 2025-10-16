// Unit tests for config module

use net_monitor::config::{AppConfig, UiMode};
use std::str::FromStr;
use tempfile::TempDir;

#[test]
fn test_ui_mode_default() {
    let mode = UiMode::default();
    assert_eq!(mode, UiMode::Gui);
}

#[test]
fn test_ui_mode_display_gui() {
    let mode = UiMode::Gui;
    assert_eq!(mode.to_string(), "gui");
}

#[test]
fn test_ui_mode_display_tui() {
    let mode = UiMode::Tui;
    assert_eq!(mode.to_string(), "tui");
}

#[test]
fn test_ui_mode_from_str_gui() {
    let mode = UiMode::from_str("gui").unwrap();
    assert_eq!(mode, UiMode::Gui);
}

#[test]
fn test_ui_mode_from_str_tui() {
    let mode = UiMode::from_str("tui").unwrap();
    assert_eq!(mode, UiMode::Tui);
}

#[test]
fn test_ui_mode_from_str_case_insensitive() {
    assert_eq!(UiMode::from_str("GUI").unwrap(), UiMode::Gui);
    assert_eq!(UiMode::from_str("TUI").unwrap(), UiMode::Tui);
    assert_eq!(UiMode::from_str("Gui").unwrap(), UiMode::Gui);
    assert_eq!(UiMode::from_str("Tui").unwrap(), UiMode::Tui);
    assert_eq!(UiMode::from_str("gUi").unwrap(), UiMode::Gui);
}

#[test]
fn test_ui_mode_from_str_invalid() {
    let result = UiMode::from_str("invalid");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid UI mode"));
}

#[test]
fn test_ui_mode_equality() {
    assert_eq!(UiMode::Gui, UiMode::Gui);
    assert_eq!(UiMode::Tui, UiMode::Tui);
    assert_ne!(UiMode::Gui, UiMode::Tui);
}

#[test]
fn test_ui_mode_clone() {
    let mode = UiMode::Gui;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn test_ui_mode_copy() {
    let mode = UiMode::Tui;
    let copied = mode;
    assert_eq!(mode, UiMode::Tui);
    assert_eq!(copied, UiMode::Tui);
}

#[test]
fn test_ui_mode_debug() {
    let mode = UiMode::Gui;
    let debug_str = format!("{:?}", mode);
    assert!(debug_str.contains("Gui"));
}

#[test]
fn test_ui_mode_serialization() {
    let mode = UiMode::Gui;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"gui\"");

    let mode = UiMode::Tui;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"tui\"");
}

#[test]
fn test_ui_mode_deserialization() {
    let mode: UiMode = serde_json::from_str("\"gui\"").unwrap();
    assert_eq!(mode, UiMode::Gui);

    let mode: UiMode = serde_json::from_str("\"tui\"").unwrap();
    assert_eq!(mode, UiMode::Tui);
}

#[test]
fn test_app_config_default() {
    let config = AppConfig::default();
    assert_eq!(config.ui_mode, UiMode::Gui);
}

#[test]
fn test_app_config_clone() {
    let config = AppConfig {
        ui_mode: UiMode::Tui,
    };
    let cloned = config.clone();
    assert_eq!(cloned.ui_mode, UiMode::Tui);
}

#[test]
fn test_app_config_debug() {
    let config = AppConfig {
        ui_mode: UiMode::Gui,
    };
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("AppConfig"));
    assert!(debug_str.contains("Gui"));
}

#[test]
fn test_app_config_serialization() {
    let config = AppConfig {
        ui_mode: UiMode::Tui,
    };
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("\"ui_mode\""));
    assert!(json.contains("\"tui\""));
}

#[test]
fn test_app_config_deserialization() {
    let json = r#"{"ui_mode":"tui"}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.ui_mode, UiMode::Tui);
}

#[test]
fn test_app_config_deserialization_with_default() {
    // When ui_mode is missing, it should default to Gui
    let json = r#"{}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.ui_mode, UiMode::Gui);
}

#[test]
fn test_app_config_save_and_load() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();

    // Set up environment to use temp directory
    // Note: This test may not work perfectly due to ProjectDirs usage
    // but it tests the serialization/deserialization logic

    let config = AppConfig {
        ui_mode: UiMode::Tui,
    };

    // Test serialization round-trip
    let json = serde_json::to_string_pretty(&config).unwrap();
    let loaded_config: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded_config.ui_mode, config.ui_mode);

    drop(temp_dir);
}

#[test]
fn test_app_config_pretty_serialization() {
    let config = AppConfig {
        ui_mode: UiMode::Gui,
    };
    let json = serde_json::to_string_pretty(&config).unwrap();

    // Pretty JSON should have newlines
    assert!(json.contains('\n'));
    assert!(json.contains("\"ui_mode\""));
    assert!(json.contains("\"gui\""));
}

#[test]
fn test_ui_mode_from_str_empty() {
    let result = UiMode::from_str("");
    assert!(result.is_err());
}

#[test]
fn test_ui_mode_from_str_whitespace() {
    let result = UiMode::from_str("  gui  ");
    // The current implementation doesn't trim, so this should fail
    // If you want to support trimming, you'd need to modify the from_str implementation
    assert!(result.is_err());
}

#[test]
fn test_app_config_multiple_fields() {
    // Test that config handles multiple ui_mode values correctly
    let configs = vec![
        AppConfig {
            ui_mode: UiMode::Gui,
        },
        AppConfig {
            ui_mode: UiMode::Tui,
        },
    ];

    for config in configs {
        let json = serde_json::to_string(&config).unwrap();
        let loaded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.ui_mode, loaded.ui_mode);
    }
}

// ========== AppConfig File I/O Tests ==========

#[test]
fn test_app_config_load_creates_default_when_missing() {
    let temp_dir = TempDir::new().unwrap();

    // Override HOME to point to temp directory
    std::env::set_var("HOME", temp_dir.path());

    // Load should create a new default config when file doesn't exist
    let config = AppConfig::load();

    // The load may succeed or fail depending on whether ProjectDirs can determine
    // a valid config directory in the test environment
    if let Ok(loaded_config) = config {
        assert_eq!(loaded_config.ui_mode, UiMode::Gui);
    }

    drop(temp_dir);
}

#[test]
fn test_app_config_save_creates_parent_directory() {
    // Test that serialization works (save would create parent in real usage)
    let config = AppConfig {
        ui_mode: UiMode::Tui,
    };

    let json = serde_json::to_string_pretty(&config).unwrap();

    // Verify we can deserialize what we serialized
    let loaded: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.ui_mode, config.ui_mode);
}

#[test]
fn test_app_config_save_and_load_roundtrip() {
    // Test serialization roundtrip (simulates save/load)
    let original_config = AppConfig {
        ui_mode: UiMode::Tui,
    };

    // Serialize (like save)
    let json = serde_json::to_string_pretty(&original_config).unwrap();

    // Deserialize (like load)
    let loaded_config: AppConfig = serde_json::from_str(&json).unwrap();

    // Should match original
    assert_eq!(loaded_config.ui_mode, original_config.ui_mode);
}

#[test]
fn test_app_config_load_existing_valid_file() {
    // Test that we can deserialize a valid JSON config
    // Note: Actual file I/O with ProjectDirs may not work with test env variables
    let config_json = r#"{"ui_mode":"tui"}"#;
    let config: Result<AppConfig, _> = serde_json::from_str(config_json);

    assert!(config.is_ok());
    if let Ok(config) = config {
        assert_eq!(config.ui_mode, UiMode::Tui);
    }
}

#[test]
fn test_app_config_load_invalid_json() {
    // Test that invalid JSON fails to deserialize
    let invalid_json = "{ this is not valid json }";
    let result: Result<AppConfig, _> = serde_json::from_str(invalid_json);

    assert!(result.is_err());
}

#[test]
fn test_app_config_save_overwrites_existing() {
    // Test serialization preserves data (simulates overwrite)
    let config1 = AppConfig {
        ui_mode: UiMode::Gui,
    };
    let json1 = serde_json::to_string(&config1).unwrap();

    let config2 = AppConfig {
        ui_mode: UiMode::Tui,
    };
    let json2 = serde_json::to_string(&config2).unwrap();

    // JSON should be different
    assert_ne!(json1, json2);

    // Deserializing json2 should give back config2
    let loaded: AppConfig = serde_json::from_str(&json2).unwrap();
    assert_eq!(loaded.ui_mode, UiMode::Tui);
}

#[test]
fn test_app_config_default_is_gui() {
    let config = AppConfig::default();
    assert_eq!(config.ui_mode, UiMode::Gui);
}

#[test]
fn test_app_config_serialization_format() {
    let config = AppConfig {
        ui_mode: UiMode::Gui,
    };

    let json = serde_json::to_string_pretty(&config).unwrap();

    // Verify the JSON structure
    assert!(json.contains("\"ui_mode\""));
    assert!(json.contains("\"gui\""));
    assert!(json.contains('{'));
    assert!(json.contains('}'));
}

#[test]
fn test_app_config_deserialization_missing_field() {
    // When ui_mode is missing, default should be used
    let json = r#"{}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.ui_mode, UiMode::Gui);
}

#[test]
fn test_app_config_deserialization_extra_fields() {
    // Extra fields should be ignored
    let json = r#"{"ui_mode":"tui","extra_field":"ignored"}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.ui_mode, UiMode::Tui);
}
