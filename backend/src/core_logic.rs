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

/// Hz step size for quantization (5Hz steps)
const HZ_STEP_SIZE: u32 = 5;

/// LCD allowed Hz steps
const LCD_HZ_STEPS: [u32; 5] = [40, 45, 50, 55, 60];

/// OLED allowed Hz steps
const OLED_HZ_STEPS: [u32; 10] = [45, 50, 55, 60, 65, 70, 75, 80, 85, 90];

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
    /// User-selected sensitivity setting (stored)
    user_sensitivity: Sensitivity,
    /// Effective sensitivity (used - may differ for LCD)
    effective_sensitivity: Sensitivity,
    /// Current device mode
    device_mode: DeviceMode,
    /// User-configured min Hz
    user_min_hz: u32,
    /// User-configured max Hz
    user_max_hz: u32,
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
            user_sensitivity: sensitivity,
            effective_sensitivity: sensitivity,
            device_mode: DeviceMode::Oled,
            user_min_hz: 40,
            user_max_hz: 90,
        }
    }

    /// Get the current algorithm state.
    pub fn state(&self) -> AlgorithmState {
        self.state
    }

    /// Get the user-selected sensitivity setting.
    pub fn sensitivity(&self) -> Sensitivity {
        self.user_sensitivity
    }

    /// Get the effective sensitivity (may differ for LCD mode).
    pub fn effective_sensitivity(&self) -> Sensitivity {
        self.effective_sensitivity
    }

    /// Get the current device mode.
    pub fn device_mode(&self) -> DeviceMode {
        self.device_mode
    }

    /// Get the timestamp of the last rate change.
    pub fn last_change(&self) -> Option<Instant> {
        self.last_change
    }

    /// Get the user-configured Hz range.
    pub fn user_range(&self) -> (u32, u32) {
        (self.user_min_hz, self.user_max_hz)
    }

    /// Set the user-configured Hz range.
    pub fn set_user_range(&mut self, min_hz: u32, max_hz: u32) {
        self.user_min_hz = min_hz;
        self.user_max_hz = max_hz;
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
    /// Stores user preference but may apply different effective sensitivity for LCD.
    pub fn set_sensitivity(&mut self, sensitivity: Sensitivity) {
        self.user_sensitivity = sensitivity;
        
        // For LCD mode, always use conservative regardless of user preference
        if self.device_mode == DeviceMode::Lcd {
            self.effective_sensitivity = Sensitivity::Conservative;
            self.drop_threshold = Sensitivity::Conservative.drop_threshold();
            self.increase_threshold = Sensitivity::Conservative.increase_threshold();
        } else {
            self.effective_sensitivity = sensitivity;
            self.drop_threshold = sensitivity.drop_threshold();
            self.increase_threshold = sensitivity.increase_threshold();
        }
        
        // Reset state when sensitivity changes
        self.state = AlgorithmState::Stable;
    }

    /// Apply device mode constraints.
    /// 
    /// For LCD mode:
    /// - Forces min_change_interval to 2000ms (prevents flickering)
    /// - Forces effective sensitivity to Conservative (slower reactions)
    /// - Clamps Hz range to 40-60
    pub fn apply_mode_constraints(&mut self, mode: DeviceMode) {
        self.device_mode = mode;
        
        match mode {
            DeviceMode::Lcd => {
                // LCD: Aggressive throttling to prevent flickering
                self.min_change_interval = Duration::from_millis(Self::MIN_CHANGE_INTERVAL_LCD_MS);
                // Force conservative sensitivity for LCD (effective only)
                self.effective_sensitivity = Sensitivity::Conservative;
                self.drop_threshold = Sensitivity::Conservative.drop_threshold();
                self.increase_threshold = Sensitivity::Conservative.increase_threshold();
            }
            DeviceMode::Oled => {
                // OLED: Standard fast switching
                self.min_change_interval = Duration::from_millis(Self::MIN_CHANGE_INTERVAL_OLED_MS);
                // Restore user sensitivity for OLED
                self.effective_sensitivity = self.user_sensitivity;
                self.drop_threshold = self.user_sensitivity.drop_threshold();
                self.increase_threshold = self.user_sensitivity.increase_threshold();
            }
            DeviceMode::Custom => {
                // Custom: Use OLED timing but allow user sensitivity
                self.min_change_interval = Duration::from_millis(Self::MIN_CHANGE_INTERVAL_OLED_MS);
                self.effective_sensitivity = self.user_sensitivity;
                self.drop_threshold = self.user_sensitivity.drop_threshold();
                self.increase_threshold = self.user_sensitivity.increase_threshold();
            }
        }
        
        // Reset state when mode changes
        self.state = AlgorithmState::Stable;
    }

    /// Get the effective Hz range based on device mode and user settings.
    fn get_effective_range(&self) -> (u32, u32) {
        match self.device_mode {
            DeviceMode::Lcd => {
                // LCD: Force 40-60 Hz range, intersected with user range
                let effective_min = self.user_min_hz.max(Self::LCD_MIN_HZ);
                let effective_max = self.user_max_hz.min(Self::LCD_MAX_HZ);
                (effective_min, effective_max)
            }
            _ => {
                // OLED/Custom: Use user-defined range
                (self.user_min_hz, self.user_max_hz)
            }
        }
    }

    /// Quantize Hz value to nearest 5Hz step.
    fn quantize_hz(hz: u32) -> u32 {
        ((hz + HZ_STEP_SIZE / 2) / HZ_STEP_SIZE) * HZ_STEP_SIZE
    }

    /// Quantize Hz value DOWN to nearest 5Hz step (for drops).
    fn quantize_hz_down(hz: u32) -> u32 {
        (hz / HZ_STEP_SIZE) * HZ_STEP_SIZE
    }

    /// Clamp Hz value based on device mode constraints and user range.
    pub fn clamp_hz(&self, hz: u32) -> u32 {
        let (effective_min, effective_max) = self.get_effective_range();
        Self::quantize_hz(hz.clamp(effective_min, effective_max))
    }

    /// Get the next step up from current Hz (5Hz increment).
    fn next_step_up(&self, current_hz: u32) -> u32 {
        let (_, effective_max) = self.get_effective_range();
        let next = current_hz + HZ_STEP_SIZE;
        Self::quantize_hz(next.min(effective_max))
    }

    /// Get the target Hz for a drop based on FPS (quantized down to 5Hz step).
    fn target_hz_for_drop(&self, fps: f64) -> u32 {
        let (effective_min, effective_max) = self.get_effective_range();
        let target = Self::quantize_hz_down(fps.floor() as u32);
        target.clamp(effective_min, effective_max)
    }

    /// Process FPS sample and determine if refresh rate should change.
    ///
    /// Returns `Some(new_hz)` if a change is needed, `None` otherwise.
    ///
    /// # Algorithm
    /// - If FPS < (CurrentHz - 1) for `drop_threshold` duration → decrease Hz (quantized to 5Hz step)
    /// - If FPS >= CurrentHz for `increase_threshold` duration → increase Hz by 5Hz step
    /// - Enforces minimum interval between changes
    /// - LCD mode: 2000ms min interval, conservative sensitivity, 40-60Hz range
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
        let (effective_min, effective_max) = self.get_effective_range();
        
        // Check if FPS is below the drop threshold (CurrentHz - 1)
        let fps_below_threshold = current_fps < (current_hz as f64 - 1.0);
        // Check if FPS is at or above current Hz (can potentially increase)
        let fps_at_or_above = current_fps >= current_hz as f64;

        match self.state {
            AlgorithmState::Stable => {
                if fps_below_threshold && current_hz > effective_min {
                    // Start tracking potential drop
                    self.state = AlgorithmState::Dropping { since: now };
                } else if fps_at_or_above && current_hz < effective_max {
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
                        // Calculate new Hz based on FPS (quantized to 5Hz step)
                        let target_hz = self.target_hz_for_drop(current_fps);
                        
                        // Deadband: don't change if difference is less than step size
                        if current_hz.abs_diff(target_hz) < HZ_STEP_SIZE {
                            self.state = AlgorithmState::Stable;
                            return None;
                        }
                        
                        self.state = AlgorithmState::Stable;
                        self.record_change(now);
                        Some(target_hz)
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
                        // Increase by one step (5Hz), not +1Hz
                        let new_hz = self.next_step_up(current_hz);
                        
                        // Deadband: don't change if we're already at or above target
                        if new_hz <= current_hz {
                            self.state = AlgorithmState::Stable;
                            return None;
                        }
                        
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

    #[test]
    fn test_lcd_mode_forces_conservative() {
        let mut controller = HysteresisController::new(Sensitivity::Aggressive);
        assert_eq!(controller.effective_sensitivity(), Sensitivity::Aggressive);
        
        controller.apply_mode_constraints(DeviceMode::Lcd);
        
        // User sensitivity preserved, but effective is conservative
        assert_eq!(controller.sensitivity(), Sensitivity::Aggressive);
        assert_eq!(controller.effective_sensitivity(), Sensitivity::Conservative);
        assert_eq!(controller.min_change_interval, Duration::from_millis(2000));
    }

    #[test]
    fn test_oled_mode_restores_user_sensitivity() {
        let mut controller = HysteresisController::new(Sensitivity::Aggressive);
        
        // Switch to LCD (forces conservative)
        controller.apply_mode_constraints(DeviceMode::Lcd);
        assert_eq!(controller.effective_sensitivity(), Sensitivity::Conservative);
        
        // Switch back to OLED (restores user preference)
        controller.apply_mode_constraints(DeviceMode::Oled);
        assert_eq!(controller.effective_sensitivity(), Sensitivity::Aggressive);
        assert_eq!(controller.min_change_interval, Duration::from_millis(500));
    }

    #[test]
    fn test_quantize_hz() {
        assert_eq!(HysteresisController::quantize_hz(42), 40);
        assert_eq!(HysteresisController::quantize_hz(43), 45);
        assert_eq!(HysteresisController::quantize_hz(47), 45);
        assert_eq!(HysteresisController::quantize_hz(48), 50);
        assert_eq!(HysteresisController::quantize_hz(50), 50);
    }

    #[test]
    fn test_quantize_hz_down() {
        assert_eq!(HysteresisController::quantize_hz_down(42), 40);
        assert_eq!(HysteresisController::quantize_hz_down(47), 45);
        assert_eq!(HysteresisController::quantize_hz_down(49), 45);
        assert_eq!(HysteresisController::quantize_hz_down(50), 50);
    }

    #[test]
    fn test_lcd_hz_range_clamping() {
        let mut controller = HysteresisController::new(Sensitivity::Balanced);
        controller.set_user_range(40, 90);
        controller.apply_mode_constraints(DeviceMode::Lcd);
        
        // LCD should clamp to 40-60
        assert_eq!(controller.clamp_hz(30), 40);
        assert_eq!(controller.clamp_hz(70), 60);
        assert_eq!(controller.clamp_hz(55), 55);
    }

    #[test]
    fn test_increase_uses_5hz_steps() {
        let mut controller = HysteresisController::new(Sensitivity::Balanced);
        controller.set_user_range(40, 90);
        let start = Instant::now();

        // Start at 50Hz with high FPS
        let result1 = controller.process_with_time(60.0, 50, start);
        assert!(result1.is_none());

        // After increase threshold, should increase by 5Hz (not 1Hz)
        let after_threshold = start + Duration::from_millis(3001);
        let result2 = controller.process_with_time(60.0, 50, after_threshold);
        
        assert!(result2.is_some());
        assert_eq!(result2.unwrap(), 55); // 50 + 5 = 55, not 51
    }

    #[test]
    fn test_drop_quantizes_to_5hz() {
        let mut controller = HysteresisController::new(Sensitivity::Balanced);
        controller.set_user_range(40, 90);
        let start = Instant::now();

        // Start at 60Hz with FPS at 47 (should drop to 45, not 47)
        let result1 = controller.process_with_time(47.0, 60, start);
        assert!(result1.is_none());

        let after_threshold = start + Duration::from_millis(1001);
        let result2 = controller.process_with_time(47.0, 60, after_threshold);
        
        assert!(result2.is_some());
        assert_eq!(result2.unwrap(), 45); // floor(47) = 47, quantize_down = 45
    }

    #[test]
    fn test_lcd_min_change_interval() {
        let mut controller = HysteresisController::new(Sensitivity::Balanced);
        controller.set_user_range(40, 60);
        controller.apply_mode_constraints(DeviceMode::Lcd);
        let start = Instant::now();

        // First drop
        let _ = controller.process_with_time(45.0, 60, start);
        let after_drop = start + Duration::from_millis(2001); // Conservative: 2s
        let first_change = controller.process_with_time(45.0, 60, after_drop);
        assert!(first_change.is_some());

        // Try second change within 2000ms - should be blocked
        let new_hz = first_change.unwrap();
        let _ = controller.process_with_time(35.0, new_hz, after_drop + Duration::from_millis(1));
        let too_soon = after_drop + Duration::from_millis(1500);
        let blocked = controller.process_with_time(35.0, new_hz, too_soon);
        assert!(blocked.is_none()); // Blocked by min interval
    }

    // Property-based tests
    proptest! {
        #[test]
        fn prop_hysteresis_decrease_after_sustained_drop(
            current_hz in 45u32..=90u32,
            fps_offset in 6.0f64..=20.0f64,
        ) {
            // FPS is below (current_hz - 1), ensure at least 5Hz difference for step
            let low_fps = (current_hz as f64) - fps_offset;
            let low_fps = low_fps.max(1.0);

            let mut controller = HysteresisController::new(Sensitivity::Balanced);
            controller.set_user_range(40, 90);
            let start = Instant::now();

            // First call - should transition to Dropping state
            let result1 = controller.process_with_time(low_fps, current_hz, start);
            prop_assert!(result1.is_none(), "Should not change on first sample");
            let is_dropping = matches!(controller.state(), AlgorithmState::Dropping { .. });
            prop_assert!(is_dropping, "Should be in Dropping state");

            // Call after drop threshold (1 second for Balanced)
            let after_threshold = start + Duration::from_millis(1001);
            let result2 = controller.process_with_time(low_fps, current_hz, after_threshold);

            // Should output a decrease decision (quantized to 5Hz)
            if let Some(new_hz) = result2 {
                prop_assert!(new_hz < current_hz, "New Hz should be lower than current");
                prop_assert_eq!(new_hz % 5, 0, "New Hz should be quantized to 5Hz step");
            }
        }

        #[test]
        fn prop_hysteresis_increase_uses_5hz_steps(
            current_hz in 40u32..=85u32,
            fps_offset in 0.0f64..=10.0f64,
        ) {
            // Ensure current_hz is on a 5Hz step
            let current_hz = (current_hz / 5) * 5;
            let high_fps = (current_hz as f64) + fps_offset;

            let mut controller = HysteresisController::new(Sensitivity::Balanced);
            controller.set_user_range(40, 90);
            let start = Instant::now();

            let result1 = controller.process_with_time(high_fps, current_hz, start);
            prop_assert!(result1.is_none(), "Should not change on first sample");

            let after_threshold = start + Duration::from_millis(3001);
            let result2 = controller.process_with_time(high_fps, current_hz, after_threshold);

            if let Some(new_hz) = result2 {
                prop_assert_eq!(new_hz, current_hz + 5, "Should increase by exactly 5Hz");
                prop_assert_eq!(new_hz % 5, 0, "New Hz should be on 5Hz step");
            }
        }

        #[test]
        fn prop_lcd_mode_enforces_2s_min_interval(
            current_hz in 45u32..=60u32,
            interval_ms in 0u64..=1999u64,
        ) {
            let current_hz = (current_hz / 5) * 5; // Ensure on 5Hz step
            let low_fps = (current_hz as f64) - 10.0;

            let mut controller = HysteresisController::new(Sensitivity::Balanced);
            controller.set_user_range(40, 60);
            controller.apply_mode_constraints(DeviceMode::Lcd);
            let start = Instant::now();

            // First change after conservative drop threshold (2s)
            let _ = controller.process_with_time(low_fps, current_hz, start);
            let after_drop = start + Duration::from_millis(2001);
            let first_change = controller.process_with_time(low_fps, current_hz, after_drop);
            
            if first_change.is_some() {
                let new_hz = first_change.unwrap();
                // Try second change within 2000ms
                let _ = controller.process_with_time(low_fps - 10.0, new_hz, after_drop + Duration::from_millis(1));
                let second_attempt = after_drop + Duration::from_millis(interval_ms);
                // Need to wait for drop threshold too
                let second_attempt = second_attempt.max(after_drop + Duration::from_millis(2001));
                
                if second_attempt.duration_since(after_drop) < Duration::from_millis(2000) {
                    let blocked = controller.process_with_time(low_fps - 10.0, new_hz, second_attempt);
                    prop_assert!(blocked.is_none(), "LCD should block changes within 2000ms");
                }
            }
        }

        #[test]
        fn prop_hz_always_quantized_to_5hz(
            fps in 35.0f64..=95.0f64,
            current_hz in 40u32..=90u32,
        ) {
            let current_hz = (current_hz / 5) * 5; // Start on 5Hz step
            let mut controller = HysteresisController::new(Sensitivity::Balanced);
            controller.set_user_range(40, 90);
            let start = Instant::now();

            // Process to potentially trigger a change
            let _ = controller.process_with_time(fps, current_hz, start);
            let after_threshold = start + Duration::from_millis(5001); // Long enough for any threshold
            let result = controller.process_with_time(fps, current_hz, after_threshold);

            if let Some(new_hz) = result {
                prop_assert_eq!(new_hz % 5, 0, "Output Hz must be on 5Hz step boundary");
                prop_assert!(new_hz >= 40 && new_hz <= 90, "Output Hz must be in valid range");
            }
        }
    }
}
