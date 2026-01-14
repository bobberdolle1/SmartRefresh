//! Multi-monitor detection for SmartRefresh daemon.
//!
//! Detects external displays and pauses SmartRefresh when connected.

use std::path::Path;
use tracing::{debug, warn};

/// DRM connector paths to check
const DRM_PATH: &str = "/sys/class/drm";

/// Connector types that indicate external displays
const EXTERNAL_CONNECTORS: &[&str] = &["HDMI", "DP", "DisplayPort", "DVI", "VGA"];

/// Monitor detector for external display detection
pub struct MonitorDetector {
    /// Cached connector paths
    connector_paths: Vec<String>,
}

impl MonitorDetector {
    pub fn new() -> Self {
        let connector_paths = Self::find_external_connectors();
        debug!("Found {} potential external connector paths", connector_paths.len());
        Self { connector_paths }
    }

    /// Find all external connector paths in /sys/class/drm
    fn find_external_connectors() -> Vec<String> {
        let mut paths = Vec::new();
        
        let drm_path = Path::new(DRM_PATH);
        if !drm_path.exists() {
            warn!("DRM path {} does not exist", DRM_PATH);
            return paths;
        }

        if let Ok(entries) = std::fs::read_dir(drm_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                
                // Check if this is an external connector type
                for connector_type in EXTERNAL_CONNECTORS {
                    if name.contains(connector_type) {
                        let status_path = entry.path().join("status");
                        if status_path.exists() {
                            paths.push(status_path.to_string_lossy().to_string());
                        }
                        break;
                    }
                }
            }
        }

        paths
    }

    /// Check if any external display is connected
    pub async fn has_external_display(&self) -> bool {
        for path in &self.connector_paths {
            if let Ok(status) = tokio::fs::read_to_string(path).await {
                let status = status.trim().to_lowercase();
                if status == "connected" {
                    debug!("External display detected at {}", path);
                    return true;
                }
            }
        }
        false
    }

    /// Synchronous version for non-async contexts
    pub fn has_external_display_sync(&self) -> bool {
        for path in &self.connector_paths {
            if let Ok(status) = std::fs::read_to_string(path) {
                let status = status.trim().to_lowercase();
                if status == "connected" {
                    return true;
                }
            }
        }
        false
    }
}

impl Default for MonitorDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_detector_creation() {
        let detector = MonitorDetector::new();
        // Should not panic even if DRM path doesn't exist
        assert!(detector.connector_paths.len() >= 0);
    }

    #[test]
    fn test_external_connector_types() {
        // Verify our connector list covers common types
        assert!(EXTERNAL_CONNECTORS.contains(&"HDMI"));
        assert!(EXTERNAL_CONNECTORS.contains(&"DP"));
        assert!(EXTERNAL_CONNECTORS.contains(&"DisplayPort"));
    }
}
