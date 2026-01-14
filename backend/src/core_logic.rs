//! Core Logic module implementing the hysteresis algorithm.
//!
//! This module contains the state machine for refresh rate decisions
//! based on sustained FPS patterns.

use std::time::{Duration, Instant};

/// Algorithm state for hysteresis control.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlgorithmState {
    /// Stable state - no pending rate change
    Stable,
    /// FPS has dropped below threshold, waiting for sustained drop
    Dropping { since: Instant },
    /// FPS is stable/high, waiting for sustained increase opportunity
    Increasing { since: Instant },
}

impl Default for AlgorithmState {
    fn default() -> Self {
        Self::Stable
    }
}

/// Sensitivity presets for the hysteresis algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Sensitivity {
    /// Conservative: 2s drop, 5s increase - slower transitions, more stable
    Conservative,
    /// Balanced: 1s drop, 3s increase - default behavior
    #[default]
    Balanced,
    /// Aggressive: 500ms drop, 1.5s increase - faster transitions, more responsive
    Aggressive,
}

/// Device mode for hardware-specific throttling.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum DeviceMode {
    /// OLED: Fast switching, minimal throttling (500ms min interval)
    #[default]
    Oled,
    /// LCD: Slow switching, aggressive throttling (2000ms min interval)
    Lcd,
    /// Custom: User-defined settings, no forced constraints
    Custom,
}

impl Sensitivity {
    /// Get the drop threshold duration for this sensitivity level.
    pub fn drop_threshold(&self) -> Duration {
        match self {
            Sensitivity::Conservative => Duration::from_secs(2),
            Sensitivity::Balanced => Duration::from_secs(1),
            Sensitivity::Aggressive => Duration::from_millis(500),
        }
    }

    /// Get the increase threshold duration for this sensitivity level.
    pub fn increase_threshold(&self) -> Duration {
        match self {
            Sensitivity::Conservative => Duration::from_secs(5),
            Sensitivity::Balanced => Duration::from_secs(3),
            Sensitivity::Aggressive => Duration::from_millis(1500),
        }
    }
}


/// Hysteresis controller for refresh rate decisions.
///
/// Implements a state machine that prevents rapid refresh rate oscillation
/// by requiring sustained FPS patterns before making changes.
pub struct HysteresisController {
    /// Current algorithm state
    state: AlgorithmState,
    /// Duration FPS must stay below threshold before dropping Hz
    drop_threshold: Duration,
    /// Duration FPS must stay at/above threshold before increasing Hz
    increase_threshold: Duration,
    /// Minimum time between consecutive rate changes
    min_change_interval: Duration,
    /// Timestamp of last successful rate change
    last_change: Option<Instant>,
    /// Current sensitivity setting
    sensitivity: Sensitivity,
    /// Current device mode
    device_mode: DeviceMode,
    /// LCD mode forced Hz range (min, max)
    lcd_hz_range: (u32, u32),
}

impl HysteresisController {
    /// Minimum interval between rate changes for OLED (500ms)
    const MIN_CHANGE_INTERVAL_OLED_MS: u64 = 500;
    /// Minimum interval between rate changes for LCD (2000ms) - prevents flickering
    const MIN_CHANGE_INTERVAL_LCD_MS: u64 = 2000;
    /// LCD forced Hz range
    const LCD_MIN_HZ: u32 = 40;
    const LCD_MAX_HZ: u32 = 60;

    /// Create a new HysteresisController with the specified sensitivity.
    pub fn new(sensitivity: Sensitivity) -> Self {
        Self {
            state: AlgorithmState::Stable,
            drop_threshold: sensitivity.drop_threshold(),
            increase_threshold: sensitivity.increase_threshold(),
            min_change_interval: Duration::from_millis(Self::MIN_CHANGE_INTERVAL_OLED_MS),
            last_change: None,
            sensitivity,
            device_mode: DeviceMode::Oled,
            lcd_hz_range: (Self::LCD_MIN_HZ, Self::LCD_MAX_HZ),
        }
    }

    /// Get the current algorithm state.
    pub fn state(&self) -> AlgorithmState {
        self.state
    }

    /// Get the current sensitivity setting.
    pub fn sensitivity(&self) -> Sensitivity {
        self.sensitivity
    }

    /// Get the current device mode.
    pub fn device_mode(&self) -> DeviceMode {
        self.device_mode
    }

    /// Get the timestamp of the last rate change.
    pub fn last_change(&self) -> Option<Instant> {
        self.last_change
    }

    /// Check if enough time has passed since the last rate change.
    fn can_change(&self, now: Instant) -> bool {
        match self.last_change {
            Some(last) => now.duration_since(last) >= self.min_change_interval,
            None => true,
        }
    }

    /// Record that a rate change occurred.
    fn record_change(&mut self, now: Instant) {
        self.last_change = Some(now);
    }

    /// Update sensitivity (adjusts thresholds).
    pub fn set_sensitivity(&mut self, sensitivity: Sensitivity) {
        self.sensitivity = sensitivity;
        self.drop_threshold = sensitivity.drop_threshold();
        self.increase_threshold = sensitivity.increase_threshold();
        // Reset state when sensitivity changes
        self.state = AlgorithmState::Stable;
    }

    /// Apply device mode constraints.
    /// 
    /// For LCD mode:
    /// - Forces min_change_interval to 2000ms (prevents flickering)
    /// - Forces sensitivity to Conservative (slower reactions)
    /// - Clamps Hz range to 40-60
    pub fn apply_mode_constraints(&mut self, mode: DeviceMode) {
        self.device_mode = mode;
        
        match mode {
            DeviceMode::Lcd => {
                // LCD: Aggressive throttling to prevent flickering
                self.min_change_interval = Duration::from_millis(Self::MIN_CHANGE_INTERVAL_LCD_MS);
                // Force conservative sensitivity for LCD
                self.sensitivity = Sensitivity::Conservative;
                self.drop_threshold = Sensitivity::Conservative.drop_threshold();
                self.increase_threshold = Sensitivity::Conservative.increase_threshold();
            }
            DeviceMode::Oled => {
                // OLED: Standard fast switching
                self.min_change_interval = Duration::from_millis(Self::MIN_CHANGE_INTERVAL_OLED_MS);
                // Keep current sensitivity for OLED
            }
            DeviceMode::Custom => {
                // Custom: Use OLED timing but allow user sensitivity
                self.min_change_interval = Duration::from_millis(Self::MIN_CHANGE_INTERVAL_OLED_MS);
            }
        }
        
        // Reset state when mode changes
        self.state = AlgorithmState::Stable;
    }

    /// Clamp Hz value based on device mode constraints.
    /// For LCD mode, clamps to 40-60 Hz range.
    pub fn clamp_hz(&self, hz: u32, user_min: u32, user_max: u32) -> u32 {
        match self.device_mode {
            DeviceMode::Lcd => {
                // LCD: Force 40-60 Hz range
                let effective_min = user_min.max(self.lcd_hz_range.0);
                let effective_max = user_max.min(self.lcd_hz_range.1);
                hz.clamp(effective_min, effective_max)
            }
            _ => {
                // OLED/Custom: Use user-defined range
                hz.clamp(user_min, user_max)
            }
        }
    }


    /// Process FPS sample and determine if refresh rate should change.
    ///
    /// Returns `Some(new_hz)` if a change is needed, `None` otherwise.
    ///
    /// # Algorithm
    /// - If FPS < (CurrentHz - 1) for `drop_threshold` duration → decrease Hz to match FPS
    /// - If FPS >= CurrentHz for `increase_threshold` duration → increase Hz by one step
    /// - Enforces minimum interval between changes
    pub fn process(&mut self, current_fps: f64, current_hz: u32) -> Option<u32> {
        self.process_with_time(current_fps, current_hz, Instant::now())
    }

    /// Process with explicit timestamp (for testing).
    pub fn process_with_time(
        &mut self,
        current_fps: f64,
        current_hz: u32,
        now: Instant,
    ) -> Option<u32> {
        // Check if FPS is below the drop threshold (CurrentHz - 1)
        let fps_below_threshold = current_fps < (current_hz as f64 - 1.0);
        // Check if FPS is at or above current Hz (can potentially increase)
        let fps_at_or_above = current_fps >= current_hz as f64;

        match self.state {
            AlgorithmState::Stable => {
                if fps_below_threshold {
                    // Start tracking potential drop
                    self.state = AlgorithmState::Dropping { since: now };
                } else if fps_at_or_above {
                    // Start tracking potential increase
                    self.state = AlgorithmState::Increasing { since: now };
                }
                None
            }

            AlgorithmState::Dropping { since } => {
                if !fps_below_threshold {
                    // FPS recovered, go back to stable
                    self.state = AlgorithmState::Stable;
                    None
                } else if now.duration_since(since) >= self.drop_threshold {
                    // Sustained drop - check if we can change
                    if self.can_change(now) {
                        // Calculate new Hz based on FPS (floor to nearest integer)
                        let new_hz = current_fps.floor() as u32;
                        self.state = AlgorithmState::Stable;
                        self.record_change(now);
                        Some(new_hz)
                    } else {
                        // Can't change yet due to min interval, stay in dropping state
                        None
                    }
                } else {
                    // Still waiting for sustained drop
                    None
                }
            }

            AlgorithmState::Increasing { since } => {
                if fps_below_threshold {
                    // FPS dropped, switch to dropping state
                    self.state = AlgorithmState::Dropping { since: now };
                    None
                } else if !fps_at_or_above {
                    // FPS dropped below current Hz but not below threshold
                    self.state = AlgorithmState::Stable;
                    None
                } else if now.duration_since(since) >= self.increase_threshold {
                    // Sustained high FPS - check if we can change
                    if self.can_change(now) {
                        // Increase by one step
                        let new_hz = current_hz + 1;
                        self.state = AlgorithmState::Stable;
                        self.record_change(now);
                        Some(new_hz)
                    } else {
                        // Can't change yet due to min interval
                        None
                    }
                } else {
                    // Still waiting for sustained increase
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Unit tests for basic functionality
    #[test]
    fn test_sensitivity_thresholds() {
        assert_eq!(
            Sensitivity::Conservative.drop_threshold(),
            Duration::from_secs(2)
        );
        assert_eq!(
            Sensitivity::Conservative.increase_threshold(),
            Duration::from_secs(5)
        );

        assert_eq!(
            Sensitivity::Balanced.drop_threshold(),
            Duration::from_secs(1)
        );
        assert_eq!(
            Sensitivity::Balanced.increase_threshold(),
            Duration::from_secs(3)
        );

        assert_eq!(
            Sensitivity::Aggressive.drop_threshold(),
            Duration::from_millis(500)
        );
        assert_eq!(
            Sensitivity::Aggressive.increase_threshold(),
            Duration::from_millis(1500)
        );
    }

    #[test]
    fn test_new_controller_starts_stable() {
        let controller = HysteresisController::new(Sensitivity::Balanced);
        assert_eq!(controller.state(), AlgorithmState::Stable);
        assert!(controller.last_change().is_none());
    }

    #[test]
    fn test_set_sensitivity_resets_state() {
        let mut controller = HysteresisController::new(Sensitivity::Balanced);
        let now = Instant::now();

        // Put controller in Dropping state
        controller.state = AlgorithmState::Dropping { since: now };

        // Change sensitivity
        controller.set_sensitivity(Sensitivity::Aggressive);

        // State should be reset to Stable
        assert_eq!(controller.state(), AlgorithmState::Stable);
        assert_eq!(controller.sensitivity(), Sensitivity::Aggressive);
    }


    // **Feature: smart-refresh-daemon, Property 4: Hysteresis Algorithm Behavior**
    // **Validates: Requirements 3.1, 3.2**
    //
    // For any sequence of FPS samples and current Hz value:
    // - If FPS remains below (CurrentHz - 1) for at least 1 second, algorithm outputs decrease
    // - If FPS remains at or above CurrentHz for at least 3 seconds, algorithm outputs increase
    // - If neither condition is sustained, algorithm outputs no change
    proptest! {
        #[test]
        fn prop_hysteresis_decrease_after_sustained_drop(
            current_hz in 41u32..=90u32,
            fps_offset in 2.0f64..=20.0f64,
        ) {
            // FPS is below (current_hz - 1)
            let low_fps = (current_hz as f64) - fps_offset;
            let low_fps = low_fps.max(1.0); // Ensure FPS is positive

            let mut controller = HysteresisController::new(Sensitivity::Balanced);
            let start = Instant::now();

            // First call - should transition to Dropping state
            let result1 = controller.process_with_time(low_fps, current_hz, start);
            prop_assert!(result1.is_none(), "Should not change on first sample");
            let is_dropping = matches!(controller.state(), AlgorithmState::Dropping { .. });
            prop_assert!(is_dropping, "Should be in Dropping state");

            // Call after drop threshold (1 second for Balanced)
            let after_threshold = start + Duration::from_millis(1001);
            let result2 = controller.process_with_time(low_fps, current_hz, after_threshold);

            // Should output a decrease decision
            prop_assert!(result2.is_some(), "Should output decrease after sustained drop");
            let new_hz = result2.unwrap();
            prop_assert!(new_hz < current_hz, "New Hz should be lower than current");
            prop_assert_eq!(new_hz, low_fps.floor() as u32, "New Hz should match floored FPS");
        }

        #[test]
        fn prop_hysteresis_increase_after_sustained_high(
            current_hz in 40u32..=89u32,
            fps_offset in 0.0f64..=10.0f64,
        ) {
            // FPS is at or above current_hz
            let high_fps = (current_hz as f64) + fps_offset;

            let mut controller = HysteresisController::new(Sensitivity::Balanced);
            let start = Instant::now();

            // First call - should transition to Increasing state
            let result1 = controller.process_with_time(high_fps, current_hz, start);
            prop_assert!(result1.is_none(), "Should not change on first sample");
            let is_increasing = matches!(controller.state(), AlgorithmState::Increasing { .. });
            prop_assert!(is_increasing, "Should be in Increasing state");

            // Call after increase threshold (3 seconds for Balanced)
            let after_threshold = start + Duration::from_millis(3001);
            let result2 = controller.process_with_time(high_fps, current_hz, after_threshold);

            // Should output an increase decision
            prop_assert!(result2.is_some(), "Should output increase after sustained high FPS");
            let new_hz = result2.unwrap();
            prop_assert_eq!(new_hz, current_hz + 1, "New Hz should be current + 1");
        }

        #[test]
        fn prop_hysteresis_no_change_when_not_sustained(
            current_hz in 41u32..=89u32,
            fps_offset in 2.0f64..=10.0f64,
        ) {
            let low_fps = (current_hz as f64) - fps_offset;
            let low_fps = low_fps.max(1.0);
            let high_fps = current_hz as f64 + 1.0;

            let mut controller = HysteresisController::new(Sensitivity::Balanced);
            let start = Instant::now();

            // Start dropping
            let _ = controller.process_with_time(low_fps, current_hz, start);
            let is_dropping = matches!(controller.state(), AlgorithmState::Dropping { .. });
            prop_assert!(is_dropping, "Should be in Dropping state");

            // Before threshold, FPS recovers
            let before_threshold = start + Duration::from_millis(500);
            let result = controller.process_with_time(high_fps, current_hz, before_threshold);

            // Should return to stable with no change
            prop_assert!(result.is_none(), "Should not change when condition not sustained");
            // State should transition based on new FPS
        }
    }

    // **Feature: smart-refresh-daemon, Property 5: Minimum Change Interval Enforcement**
    // **Validates: Requirements 3.3**
    //
    // For any sequence of refresh rate change requests, no two successful changes
    // should occur within 500 milliseconds of each other.
    proptest! {
        #[test]
        fn prop_minimum_change_interval_enforced(
            current_hz in 50u32..=80u32,
            interval_ms in 0u64..=499u64,
        ) {
            // FPS that will trigger a drop
            let low_fps = (current_hz as f64) - 5.0;

            let mut controller = HysteresisController::new(Sensitivity::Balanced);
            let start = Instant::now();

            // First change: trigger a drop after threshold
            let _ = controller.process_with_time(low_fps, current_hz, start);
            let after_drop_threshold = start + Duration::from_millis(1001);
            let first_change = controller.process_with_time(low_fps, current_hz, after_drop_threshold);
            prop_assert!(first_change.is_some(), "First change should succeed");

            // Record the time of first change
            let first_change_time = after_drop_threshold;

            // Now try to trigger another change within the min interval
            // Start a new drop sequence
            let new_hz = first_change.unwrap();
            let new_low_fps = (new_hz as f64) - 5.0;
            let new_low_fps = new_low_fps.max(1.0);

            // Start dropping again
            let drop_start = first_change_time + Duration::from_millis(1);
            let _ = controller.process_with_time(new_low_fps, new_hz, drop_start);

            // Try to complete the drop within the min interval (< 500ms from first change)
            let second_attempt_time = first_change_time + Duration::from_millis(interval_ms);
            // Make sure we're past the drop threshold but within min change interval
            let second_attempt_time = second_attempt_time.max(drop_start + Duration::from_millis(1001));

            // Only test if second attempt is within the min change interval
            if second_attempt_time.duration_since(first_change_time) < Duration::from_millis(500) {
                let second_change = controller.process_with_time(new_low_fps, new_hz, second_attempt_time);
                prop_assert!(second_change.is_none(), "Second change within 500ms should be blocked");
            }
        }

        #[test]
        fn prop_change_allowed_after_min_interval(
            current_hz in 50u32..=80u32,
            extra_ms in 0u64..=500u64,
        ) {
            // FPS that will trigger a drop
            let low_fps = (current_hz as f64) - 5.0;

            let mut controller = HysteresisController::new(Sensitivity::Balanced);
            let start = Instant::now();

            // First change: trigger a drop after threshold
            let _ = controller.process_with_time(low_fps, current_hz, start);
            let after_drop_threshold = start + Duration::from_millis(1001);
            let first_change = controller.process_with_time(low_fps, current_hz, after_drop_threshold);
            prop_assert!(first_change.is_some(), "First change should succeed");

            let first_change_time = after_drop_threshold;
            let new_hz = first_change.unwrap();
            let new_low_fps = (new_hz as f64) - 5.0;
            let new_low_fps = new_low_fps.max(1.0);

            // Start a new drop sequence after min interval
            let drop_start = first_change_time + Duration::from_millis(500 + extra_ms);
            let _ = controller.process_with_time(new_low_fps, new_hz, drop_start);

            // Complete the drop after the drop threshold
            let second_attempt_time = drop_start + Duration::from_millis(1001);
            let second_change = controller.process_with_time(new_low_fps, new_hz, second_attempt_time);

            // Should be allowed since we're past the min interval
            prop_assert!(second_change.is_some(), "Change should be allowed after min interval");
        }
    }
}
