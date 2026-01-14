//! Display Control module for managing refresh rate via Gamescope.
//!
//! This module handles refresh rate changes through gamescope-cmd execution.

use crate::error::DisplayError;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tokio::process::Command;

/// Minimum allowed refresh rate in Hz.
pub const MIN_ALLOWED_HZ: u32 = 40;

/// Maximum allowed refresh rate in Hz.
pub const MAX_ALLOWED_HZ: u32 = 90;

/// Manages display refresh rate through Gamescope commands.
pub struct DisplayManager {
    /// Current refresh rate in Hz (atomic for thread-safe reads).
    current_hz: AtomicU32,
    /// Minimum allowed refresh rate.
    min_hz: AtomicU32,
    /// Maximum allowed refresh rate.
    max_hz: AtomicU32,
    /// Timestamp of the last refresh rate change.
    last_change: Mutex<Instant>,
}

impl DisplayManager {
    /// Create a new DisplayManager with the specified Hz range.
    ///
    /// # Arguments
    /// * `min_hz` - Minimum refresh rate (clamped to 40-90 range)
    /// * `max_hz` - Maximum refresh rate (clamped to 40-90 range)
    pub fn new(min_hz: u32, max_hz: u32) -> Self {
        let clamped_min = min_hz.clamp(MIN_ALLOWED_HZ, MAX_ALLOWED_HZ);
        let clamped_max = max_hz.clamp(MIN_ALLOWED_HZ, MAX_ALLOWED_HZ);

        // Ensure min <= max
        let (final_min, final_max) = if clamped_min > clamped_max {
            (clamped_max, clamped_min)
        } else {
            (clamped_min, clamped_max)
        };

        Self {
            current_hz: AtomicU32::new(final_max), // Start at max Hz
            min_hz: AtomicU32::new(final_min),
            max_hz: AtomicU32::new(final_max),
            last_change: Mutex::new(Instant::now()),
        }
    }

    /// Clamp a refresh rate value to the configured [min_hz, max_hz] range.
    ///
    /// Values below min_hz become min_hz, values above max_hz become max_hz.
    pub fn clamp_hz(&self, hz: u32) -> u32 {
        let min = self.min_hz.load(Ordering::Relaxed);
        let max = self.max_hz.load(Ordering::Relaxed);
        hz.clamp(min, max)
    }

    /// Set refresh rate via gamescope-cmd.
    ///
    /// Returns Ok(true) if rate was changed, Ok(false) if already at target.
    ///
    /// # Arguments
    /// * `hz` - Target refresh rate (will be clamped to configured range)
    pub async fn set_refresh_rate(&self, hz: u32) -> Result<bool, DisplayError> {
        let clamped_hz = self.clamp_hz(hz);
        let current = self.current_hz.load(Ordering::Relaxed);

        // Skip execution if rate unchanged (Requirement 2.4)
        if clamped_hz == current {
            return Ok(false);
        }

        // Execute gamescope-cmd
        self.execute_gamescope_cmd(clamped_hz).await?;

        // Update current Hz and timestamp
        self.current_hz.store(clamped_hz, Ordering::Relaxed);
        if let Ok(mut last_change) = self.last_change.lock() {
            *last_change = Instant::now();
        }

        Ok(true)
    }

    /// Execute the gamescope-cmd command to change refresh rate.
    async fn execute_gamescope_cmd(&self, hz: u32) -> Result<(), DisplayError> {
        let output = Command::new("gamescope-cmd")
            .arg("-r")
            .arg(hz.to_string())
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    DisplayError::CommandNotFound
                } else {
                    DisplayError::ExecutionFailed(e)
                }
            })?;

        if !output.status.success() {
            return Err(DisplayError::CommandFailed {
                exit_code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    /// Get current refresh rate.
    pub fn get_current_hz(&self) -> u32 {
        self.current_hz.load(Ordering::Relaxed)
    }

    /// Get the minimum configured refresh rate.
    pub fn get_min_hz(&self) -> u32 {
        self.min_hz.load(Ordering::Relaxed)
    }

    /// Get the maximum configured refresh rate.
    pub fn get_max_hz(&self) -> u32 {
        self.max_hz.load(Ordering::Relaxed)
    }

    /// Update min/max range.
    ///
    /// # Arguments
    /// * `min` - New minimum refresh rate
    /// * `max` - New maximum refresh rate
    pub fn set_range(&self, min: u32, max: u32) {
        let clamped_min = min.clamp(MIN_ALLOWED_HZ, MAX_ALLOWED_HZ);
        let clamped_max = max.clamp(MIN_ALLOWED_HZ, MAX_ALLOWED_HZ);

        // Ensure min <= max
        let (final_min, final_max) = if clamped_min > clamped_max {
            (clamped_max, clamped_min)
        } else {
            (clamped_min, clamped_max)
        };

        self.min_hz.store(final_min, Ordering::Relaxed);
        self.max_hz.store(final_max, Ordering::Relaxed);
    }

    /// Get the timestamp of the last refresh rate change.
    pub fn get_last_change(&self) -> Instant {
        self.last_change
            .lock()
            .map(|guard| *guard)
            .unwrap_or_else(|_| Instant::now())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // **Feature: smart-refresh-daemon, Property 3: Refresh Rate Clamping**
    // **Validates: Requirements 2.3**
    proptest! {
        #[test]
        fn test_refresh_rate_clamping(
            requested_hz in 0u32..200u32,
            min_hz in 40u32..=90u32,
            max_hz in 40u32..=90u32
        ) {
            // Ensure min <= max for valid configuration
            let (actual_min, actual_max) = if min_hz > max_hz {
                (max_hz, min_hz)
            } else {
                (min_hz, max_hz)
            };

            let manager = DisplayManager::new(actual_min, actual_max);
            let clamped = manager.clamp_hz(requested_hz);

            // Property: clamped value is always within [min_hz, max_hz]
            prop_assert!(clamped >= actual_min, "Clamped {} should be >= min {}", clamped, actual_min);
            prop_assert!(clamped <= actual_max, "Clamped {} should be <= max {}", clamped, actual_max);

            // Property: values below min become min
            if requested_hz < actual_min {
                prop_assert_eq!(clamped, actual_min);
            }

            // Property: values above max become max
            if requested_hz > actual_max {
                prop_assert_eq!(clamped, actual_max);
            }

            // Property: values within range are unchanged
            if requested_hz >= actual_min && requested_hz <= actual_max {
                prop_assert_eq!(clamped, requested_hz);
            }
        }
    }

    #[test]
    fn test_display_manager_creation() {
        let manager = DisplayManager::new(40, 90);
        assert_eq!(manager.get_min_hz(), 40);
        assert_eq!(manager.get_max_hz(), 90);
        assert_eq!(manager.get_current_hz(), 90); // Starts at max
    }

    #[test]
    fn test_display_manager_swapped_range() {
        // If min > max, they should be swapped
        let manager = DisplayManager::new(90, 40);
        assert_eq!(manager.get_min_hz(), 40);
        assert_eq!(manager.get_max_hz(), 90);
    }

    #[test]
    fn test_display_manager_clamped_range() {
        // Values outside 40-90 should be clamped
        let manager = DisplayManager::new(20, 120);
        assert_eq!(manager.get_min_hz(), 40);
        assert_eq!(manager.get_max_hz(), 90);
    }

    #[test]
    fn test_clamp_hz_below_min() {
        let manager = DisplayManager::new(50, 80);
        assert_eq!(manager.clamp_hz(30), 50);
    }

    #[test]
    fn test_clamp_hz_above_max() {
        let manager = DisplayManager::new(50, 80);
        assert_eq!(manager.clamp_hz(100), 80);
    }

    #[test]
    fn test_clamp_hz_within_range() {
        let manager = DisplayManager::new(50, 80);
        assert_eq!(manager.clamp_hz(60), 60);
    }

    #[test]
    fn test_set_range() {
        let manager = DisplayManager::new(40, 90);
        manager.set_range(50, 70);
        assert_eq!(manager.get_min_hz(), 50);
        assert_eq!(manager.get_max_hz(), 70);
    }

    #[test]
    fn test_set_range_swapped() {
        let manager = DisplayManager::new(40, 90);
        manager.set_range(70, 50);
        assert_eq!(manager.get_min_hz(), 50);
        assert_eq!(manager.get_max_hz(), 70);
    }
}
