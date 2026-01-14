//! Configuration module for persistent settings.
//!
//! This module handles loading, saving, and validating daemon configuration.

use crate::core_logic::Sensitivity;
use crate::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Daemon configuration.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Config {
    pub min_hz: u32,
    pub max_hz: u32,
    pub sensitivity: Sensitivity,
    pub enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_hz: 40,
            max_hz: 90,
            sensitivity: Sensitivity::Balanced,
            enabled: true,
        }
    }
}

impl Config {
    /// Validate configuration values.
    /// Returns Ok(()) if valid, Err with descriptive message if invalid.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.min_hz > self.max_hz {
            return Err(ConfigError::ValidationError(format!(
                "min_hz ({}) cannot be greater than max_hz ({})",
                self.min_hz, self.max_hz
            )));
        }

        if self.min_hz < 40 {
            return Err(ConfigError::ValidationError(format!(
                "min_hz ({}) must be at least 40Hz",
                self.min_hz
            )));
        }

        if self.max_hz > 90 {
            return Err(ConfigError::ValidationError(format!(
                "max_hz ({}) must not exceed 90Hz",
                self.max_hz
            )));
        }

        Ok(())
    }
}

/// Configuration manager with file I/O.
pub struct ConfigManager {
    config: RwLock<Config>,
    path: PathBuf,
}

impl ConfigManager {
    /// Load configuration from file or use defaults.
    /// If the file doesn't exist, returns a manager with default config.
    pub fn load_or_default(path: &Path) -> Result<Self, ConfigError> {
        let config = if path.exists() {
            let contents = fs::read_to_string(path).map_err(|e| {
                ConfigError::ParseError(format!("Failed to read config file: {}", e))
            })?;

            let config: Config = serde_json::from_str(&contents)
                .map_err(|e| ConfigError::ParseError(format!("Invalid JSON: {}", e)))?;

            // Validate loaded config
            config.validate()?;
            config
        } else {
            Config::default()
        };

        Ok(Self {
            config: RwLock::new(config),
            path: path.to_path_buf(),
        })
    }

    /// Save configuration to file using atomic write.
    pub fn save(&self) -> Result<(), ConfigError> {
        let config = self.config.read().map_err(|_| {
            ConfigError::ValidationError("Failed to acquire read lock".to_string())
        })?;

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Atomic write: write to temp file, then rename
        let temp_path = self.path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(&*config)
            .map_err(|e| ConfigError::ParseError(format!("Failed to serialize config: {}", e)))?;

        {
            let mut file = fs::File::create(&temp_path)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
        }

        fs::rename(&temp_path, &self.path)?;

        Ok(())
    }

    /// Get current configuration.
    pub fn get(&self) -> Config {
        self.config
            .read()
            .map(|c| c.clone())
            .unwrap_or_else(|_| Config::default())
    }

    /// Update configuration with validation.
    pub fn update(&self, config: Config) -> Result<(), ConfigError> {
        // Validate before updating
        config.validate()?;

        let mut current = self.config.write().map_err(|_| {
            ConfigError::ValidationError("Failed to acquire write lock".to_string())
        })?;

        *current = config;

        // Release lock before saving
        drop(current);

        // Persist to file
        self.save()
    }

    /// Get the config file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the default config path (~/.config/smart-refresh/config.json).
    pub fn default_path() -> PathBuf {
        dirs_config_path().join("config.json")
    }
}

/// Get the config directory path.
fn dirs_config_path() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("smart-refresh")
    } else {
        PathBuf::from("/tmp/smart-refresh")
    }
}

// Custom serialization for Sensitivity enum
impl Serialize for Sensitivity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            Sensitivity::Conservative => "conservative",
            Sensitivity::Balanced => "balanced",
            Sensitivity::Aggressive => "aggressive",
        };
        serializer.serialize_str(s)
    }
}

impl<'de> Deserialize<'de> for Sensitivity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "conservative" => Ok(Sensitivity::Conservative),
            "balanced" => Ok(Sensitivity::Balanced),
            "aggressive" => Ok(Sensitivity::Aggressive),
            _ => Err(serde::de::Error::custom(format!(
                "invalid sensitivity: {}, expected one of: conservative, balanced, aggressive",
                s
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.min_hz, 40);
        assert_eq!(config.max_hz, 90);
        assert_eq!(config.sensitivity, Sensitivity::Balanced);
        assert!(config.enabled);
    }

    #[test]
    fn test_config_manager_load_nonexistent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        
        let manager = ConfigManager::load_or_default(&path).unwrap();
        let config = manager.get();
        
        // Should use defaults when file doesn't exist
        assert_eq!(config, Config::default());
    }

    #[test]
    fn test_config_manager_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        
        // Create and save config
        let manager = ConfigManager::load_or_default(&path).unwrap();
        let mut config = manager.get();
        config.min_hz = 50;
        config.max_hz = 80;
        config.sensitivity = Sensitivity::Aggressive;
        manager.update(config.clone()).unwrap();
        
        // Load again and verify
        let manager2 = ConfigManager::load_or_default(&path).unwrap();
        let loaded = manager2.get();
        
        assert_eq!(loaded.min_hz, 50);
        assert_eq!(loaded.max_hz, 80);
        assert_eq!(loaded.sensitivity, Sensitivity::Aggressive);
    }

    #[test]
    fn test_config_validation_min_greater_than_max() {
        let config = Config {
            min_hz: 80,
            max_hz: 60,
            sensitivity: Sensitivity::Balanced,
            enabled: true,
        };
        
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::ValidationError(_)));
    }

    #[test]
    fn test_config_validation_min_too_low() {
        let config = Config {
            min_hz: 30,
            max_hz: 90,
            sensitivity: Sensitivity::Balanced,
            enabled: true,
        };
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_max_too_high() {
        let config = Config {
            min_hz: 40,
            max_hz: 120,
            sensitivity: Sensitivity::Balanced,
            enabled: true,
        };
        
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_sensitivity_serialization() {
        let config = Config {
            min_hz: 40,
            max_hz: 90,
            sensitivity: Sensitivity::Conservative,
            enabled: true,
        };
        
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"conservative\""));
        
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.sensitivity, Sensitivity::Conservative);
    }

    #[test]
    fn test_invalid_sensitivity_deserialization() {
        let json = r#"{"min_hz":40,"max_hz":90,"sensitivity":"invalid","enabled":true}"#;
        let result: Result<Config, _> = serde_json::from_str(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid sensitivity"));
    }

    // Strategy to generate valid Sensitivity values
    fn sensitivity_strategy() -> impl Strategy<Value = Sensitivity> {
        prop_oneof![
            Just(Sensitivity::Conservative),
            Just(Sensitivity::Balanced),
            Just(Sensitivity::Aggressive),
        ]
    }

    // Strategy to generate valid Config values (within valid range)
    fn valid_config_strategy() -> impl Strategy<Value = Config> {
        (40u32..=90u32, 40u32..=90u32, sensitivity_strategy(), any::<bool>())
            .prop_filter_map("min must be <= max", |(min, max, sens, enabled)| {
                if min <= max {
                    Some(Config {
                        min_hz: min,
                        max_hz: max,
                        sensitivity: sens,
                        enabled,
                    })
                } else {
                    None
                }
            })
    }

    // **Feature: smart-refresh-daemon, Property 6: Configuration Round-Trip**
    // **Validates: Requirements 5.4, 6.3**
    //
    // For any valid configuration (min_hz, max_hz, sensitivity), serializing to JSON,
    // writing to file, reading back, and deserializing should produce an equivalent
    // configuration object.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_config_json_round_trip(config in valid_config_strategy()) {
            // Serialize to JSON
            let json = serde_json::to_string(&config).unwrap();
            
            // Deserialize back
            let parsed: Config = serde_json::from_str(&json).unwrap();
            
            // Should be equivalent
            prop_assert_eq!(config, parsed);
        }

        #[test]
        fn prop_config_file_round_trip(config in valid_config_strategy()) {
            let dir = tempdir().unwrap();
            let path = dir.path().join("config.json");
            
            // Create manager and update with config
            let manager = ConfigManager::load_or_default(&path).unwrap();
            manager.update(config.clone()).unwrap();
            
            // Load from file
            let manager2 = ConfigManager::load_or_default(&path).unwrap();
            let loaded = manager2.get();
            
            // Should be equivalent
            prop_assert_eq!(config, loaded);
        }
    }

    // **Feature: smart-refresh-daemon, Property 7: Configuration Validation**
    // **Validates: Requirements 6.4**
    //
    // For any configuration where min_hz > max_hz, or min_hz < 40, or max_hz > 90,
    // or sensitivity is not one of [conservative, balanced, aggressive], the
    // validation function should return an error.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_config_validation_rejects_min_greater_than_max(
            min_hz in 41u32..=100u32,
            max_hz in 40u32..=99u32,
            sensitivity in sensitivity_strategy(),
            enabled in any::<bool>(),
        ) {
            // Only test when min > max
            prop_assume!(min_hz > max_hz);
            
            let config = Config {
                min_hz,
                max_hz,
                sensitivity,
                enabled,
            };
            
            let result = config.validate();
            prop_assert!(result.is_err(), "Should reject config where min_hz > max_hz");
            
            if let Err(ConfigError::ValidationError(msg)) = result {
                prop_assert!(msg.contains("cannot be greater than"), "Error message should mention min > max");
            } else {
                prop_assert!(false, "Should be ValidationError");
            }
        }

        #[test]
        fn prop_config_validation_rejects_min_below_40(
            min_hz in 0u32..40u32,
            max_hz in 40u32..=90u32,
            sensitivity in sensitivity_strategy(),
            enabled in any::<bool>(),
        ) {
            let config = Config {
                min_hz,
                max_hz,
                sensitivity,
                enabled,
            };
            
            let result = config.validate();
            prop_assert!(result.is_err(), "Should reject config where min_hz < 40");
            
            if let Err(ConfigError::ValidationError(msg)) = result {
                prop_assert!(msg.contains("at least 40Hz"), "Error message should mention minimum 40Hz");
            } else {
                prop_assert!(false, "Should be ValidationError");
            }
        }

        #[test]
        fn prop_config_validation_rejects_max_above_90(
            min_hz in 40u32..=90u32,
            max_hz in 91u32..=200u32,
            sensitivity in sensitivity_strategy(),
            enabled in any::<bool>(),
        ) {
            // Ensure min <= max for this test (we're testing max > 90, not min > max)
            prop_assume!(min_hz <= max_hz);
            
            let config = Config {
                min_hz,
                max_hz,
                sensitivity,
                enabled,
            };
            
            let result = config.validate();
            prop_assert!(result.is_err(), "Should reject config where max_hz > 90");
            
            if let Err(ConfigError::ValidationError(msg)) = result {
                prop_assert!(msg.contains("not exceed 90Hz"), "Error message should mention maximum 90Hz");
            } else {
                prop_assert!(false, "Should be ValidationError");
            }
        }

        #[test]
        fn prop_config_validation_accepts_valid_configs(config in valid_config_strategy()) {
            // Valid configs should pass validation
            let result = config.validate();
            prop_assert!(result.is_ok(), "Valid config should pass validation: {:?}", config);
        }
    }
}
