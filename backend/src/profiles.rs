//! Per-game profile management for SmartRefresh daemon.
//!
//! Stores and loads game-specific settings based on Steam AppID.

use crate::core_logic::Sensitivity;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, warn};

/// Profile configuration for a specific game
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameProfile {
    /// Steam AppID
    pub app_id: String,
    /// Game name (for display)
    pub name: String,
    /// Minimum refresh rate
    pub min_hz: u32,
    /// Maximum refresh rate
    pub max_hz: u32,
    /// Sensitivity preset
    pub sensitivity: String,
    /// Whether adaptive sensitivity is enabled
    #[serde(default)]
    pub adaptive_sensitivity: bool,
}

impl GameProfile {
    pub fn new(app_id: String, name: String, min_hz: u32, max_hz: u32, sensitivity: String) -> Self {
        Self {
            app_id,
            name,
            min_hz,
            max_hz,
            sensitivity,
            adaptive_sensitivity: false,
        }
    }

    /// Parse sensitivity string to enum
    pub fn get_sensitivity(&self) -> Sensitivity {
        match self.sensitivity.to_lowercase().as_str() {
            "conservative" => Sensitivity::Conservative,
            "aggressive" => Sensitivity::Aggressive,
            _ => Sensitivity::Balanced,
        }
    }
}

/// Profile manager for loading/saving game profiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileManager {
    /// Map of AppID to profile
    profiles: HashMap<String, GameProfile>,
    /// Currently active game AppID
    #[serde(skip)]
    current_app_id: Option<String>,
    /// Global default settings (used when no profile matches)
    pub global_default: GlobalDefault,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalDefault {
    pub min_hz: u32,
    pub max_hz: u32,
    pub sensitivity: String,
    pub adaptive_sensitivity: bool,
}

impl Default for GlobalDefault {
    fn default() -> Self {
        Self {
            min_hz: 40,
            max_hz: 90,
            sensitivity: "balanced".to_string(),
            adaptive_sensitivity: false,
        }
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self {
            profiles: HashMap::new(),
            current_app_id: None,
            global_default: GlobalDefault::default(),
        }
    }
}

impl ProfileManager {
    /// Get the profiles file path
    pub fn profiles_path() -> PathBuf {
        if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home)
                .join(".config")
                .join("smart-refresh")
                .join("profiles.json")
        } else {
            PathBuf::from("/tmp/smart-refresh/profiles.json")
        }
    }

    /// Load profiles from file or return default
    pub fn load_or_default() -> Result<Self, std::io::Error> {
        let path = Self::profiles_path();
        
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            match serde_json::from_str(&contents) {
                Ok(manager) => {
                    info!("Loaded {} game profiles from {:?}", 
                        manager.profiles.len(), path);
                    Ok(manager)
                }
                Err(e) => {
                    warn!("Failed to parse profiles.json: {}, using defaults", e);
                    Ok(Self::default())
                }
            }
        } else {
            info!("No profiles.json found, using defaults");
            Ok(Self::default())
        }
    }

    /// Save profiles to file
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::profiles_path();
        
        // Ensure directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        
        // Atomic write
        let temp_path = path.with_extension("json.tmp");
        std::fs::write(&temp_path, &json)?;
        std::fs::rename(&temp_path, &path)?;
        
        info!("Saved {} profiles to {:?}", self.profiles.len(), path);
        Ok(())
    }

    /// Get profile for a specific AppID
    pub fn get_profile(&self, app_id: &str) -> Option<&GameProfile> {
        self.profiles.get(app_id)
    }

    /// Set or update a profile
    pub fn set_profile(&mut self, profile: GameProfile) {
        info!("Setting profile for {} ({})", profile.name, profile.app_id);
        self.profiles.insert(profile.app_id.clone(), profile);
    }

    /// Remove a profile
    pub fn remove_profile(&mut self, app_id: &str) -> Option<GameProfile> {
        self.profiles.remove(app_id)
    }

    /// Get all profiles
    pub fn get_all_profiles(&self) -> Vec<&GameProfile> {
        self.profiles.values().collect()
    }

    /// Set current active game
    pub fn set_current_game(&mut self, app_id: Option<String>) {
        self.current_app_id = app_id;
    }

    /// Get current active game AppID
    pub fn get_current_game(&self) -> Option<&String> {
        self.current_app_id.as_ref()
    }

    /// Get settings for current game (profile or global default)
    pub fn get_current_settings(&self) -> (u32, u32, Sensitivity, bool) {
        if let Some(app_id) = &self.current_app_id {
            if let Some(profile) = self.profiles.get(app_id) {
                return (
                    profile.min_hz,
                    profile.max_hz,
                    profile.get_sensitivity(),
                    profile.adaptive_sensitivity,
                );
            }
        }
        
        // Return global defaults
        let sensitivity = match self.global_default.sensitivity.to_lowercase().as_str() {
            "conservative" => Sensitivity::Conservative,
            "aggressive" => Sensitivity::Aggressive,
            _ => Sensitivity::Balanced,
        };
        
        (
            self.global_default.min_hz,
            self.global_default.max_hz,
            sensitivity,
            self.global_default.adaptive_sensitivity,
        )
    }

    /// Update global defaults
    pub fn set_global_default(&mut self, min_hz: u32, max_hz: u32, sensitivity: String, adaptive: bool) {
        self.global_default = GlobalDefault {
            min_hz,
            max_hz,
            sensitivity,
            adaptive_sensitivity: adaptive,
        };
    }
}

/// Profile list response for IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileListResponse {
    pub profiles: Vec<GameProfile>,
    pub current_app_id: Option<String>,
    pub global_default: GlobalDefault,
}

impl From<&ProfileManager> for ProfileListResponse {
    fn from(manager: &ProfileManager) -> Self {
        Self {
            profiles: manager.profiles.values().cloned().collect(),
            current_app_id: manager.current_app_id.clone(),
            global_default: manager.global_default.clone(),
        }
    }
}
