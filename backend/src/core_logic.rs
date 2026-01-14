//! Core Logic module implementing the hysteresis algorithm.
//!
//! This module contains the state machine for refresh rate decisions
//! based on sustained FPS patterns.
//!
//! v2.0 additions:
//! - FPS Jitter Tolerance ("Sticky Target")
//! - Adaptive Sensitivity based on FPS variance
//! - Sliding window for FPS sample analysis
//!
//! v2.0.1 additions:
//! - Configurable FPS tolerance (2.0-5.0)
//! - Resume cooldown (silence period after wake)
//! - Gamescope frame limiter integration

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Default FPS tolerance for "sticky target" - prevents switching when FPS is close to current Hz
pub const DEFAULT_FPS_TOLERANCE: f64 = 3.0;

/// Minimum FPS tolerance (for aggressive users)
pub const MIN_FPS_TOLERANCE: f64 = 2.0;

/// Maximum FPS tolerance (for stability-focused users)
pub const MAX_FPS_TOLERANCE: f64 = 5.0;

/// Number of samples for adaptive sensitivity sliding window
pub const ADAPTIVE_WINDOW_SIZE: usize = 10;

/// Standard deviation threshold for stable FPS (allow user sensitivity)
pub const STD_DEV_STABLE: f64 = 2.0;

/// Standard deviation threshold for unstable FPS (force conservative)
pub const STD_DEV_UNSTABLE: f64 = 5.0;

/// Default resume cooldown duration (seconds of silence after wake)
pub const DEFAULT_RESUME_COOLDOWN_SECS: u64 = 5;

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

/// Sliding window for FPS samples used in adaptive sensitivity
#[derive(Debug, Clone)]
pub struct FpsSlidingWindow {
    samples: VecDeque<f64>,
    capacity: usize,
}

impl FpsSlidingWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, fps: f64) {
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(fps);
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_full(&self) -> bool {
        self.samples.len() >= self.capacity
    }

    /// Calculate mean of samples
    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    /// Calculate standard deviation of samples
    pub fn std_dev(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        let mean = self.mean();
        let variance = self.samples.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / (self.samples.len() - 1) as f64;
        variance.sqrt()
    }
}

impl Default for FpsSlidingWindow {
    fn default() -> Self {
        Self::new(ADAPTIVE_WINDOW_SIZE)
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
    /// User-selected sensitivity setting (stored)
    user_sensitivity: Sensitivity,
    /// Effective sensitivity (used - may differ for LCD or adaptive)
    effective_sensitivity: Sensitivity,
    /// Current device mode
    device_mode: DeviceMode,
    /// User-configured min Hz
    user_min_hz: u32,
    /// User-configured max Hz
    user_max_hz: u32,
    /// Sliding window for adaptive sensitivity
    fps_window: FpsSlidingWindow,
    /// Whether adaptive sensitivity is enabled
    adaptive_sensitivity_enabled: bool,
    /// External display detected - pause processing
    external_display_detected: bool,
    /// Configurable FPS tolerance (2.0-5.0)
    fps_tolerance: f64,
    /// Resume cooldown - timestamp when resume occurred
    resume_cooldown_until: Option<Instant>,
    /// Resume cooldown duration
    resume_cooldown_duration: Duration,
    /// Whether to sync Gamescope frame limiter with Hz
    sync_frame_limiter: bool,
    /// Last Hz that was set (for frame limiter sync)
    last_set_hz: Option<u32>,
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
            fps_window: FpsSlidingWindow::default(),
            adaptive_sensitivity_enabled: false,
            external_display_detected: false,
            fps_tolerance: DEFAULT_FPS_TOLERANCE,
            resume_cooldown_until: None,
            resume_cooldown_duration: Duration::from_secs(DEFAULT_RESUME_COOLDOWN_SECS),
            sync_frame_limiter: false,
            last_set_hz: None,
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

    /// Get the effective sensitivity (may differ for LCD mode or adaptive).
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

    /// Enable or disable adaptive sensitivity
    pub fn set_adaptive_sensitivity(&mut self, enabled: bool) {
        self.adaptive_sensitivity_enabled = enabled;
        if !enabled {
            // Restore user sensitivity when disabled
            self.update_effective_sensitivity();
        }
    }

    /// Check if adaptive sensitivity is enabled
    pub fn is_adaptive_sensitivity_enabled(&self) -> bool {
        self.adaptive_sensitivity_enabled
    }

    /// Set external display detection state
    pub fn set_external_display_detected(&mut self, detected: bool) {
        self.external_display_detected = detected;
        if detected {
            self.state = AlgorithmState::Stable;
        }
    }

    /// Check if external display is detected
    pub fn is_external_display_detected(&self) -> bool {
        self.external_display_detected
    }

    /// Get the current FPS standard deviation from sliding window
    pub fn get_fps_std_dev(&self) -> f64 {
        self.fps_window.std_dev()
    }

    /// Reset state to Stable and clear last_change timestamp
    /// Used after suspend/resume to prevent stale timestamp issues
    /// Activates resume cooldown period
    pub fn reset_state(&mut self) {
        self.state = AlgorithmState::Stable;
        self.last_change = None;
        self.fps_window.clear();
        // Activate resume cooldown - no changes for N seconds after wake
        self.resume_cooldown_until = Some(Instant::now() + self.resume_cooldown_duration);
        tracing::info!("State reset with {}s resume cooldown", self.resume_cooldown_duration.as_secs());
    }

    /// Check if currently in resume cooldown period
    pub fn is_in_resume_cooldown(&self) -> bool {
        match self.resume_cooldown_until {
            Some(until) => Instant::now() < until,
            None => false,
        }
    }

    /// Get remaining resume cooldown time in seconds
    pub fn resume_cooldown_remaining(&self) -> f64 {
        match self.resume_cooldown_until {
            Some(until) => {
                let now = Instant::now();
                if now < until {
                    (until - now).as_secs_f64()
                } else {
                    0.0
                }
            }
            None => 0.0,
        }
    }

    /// Set resume cooldown duration
    pub fn set_resume_cooldown(&mut self, secs: u64) {
        self.resume_cooldown_duration = Duration::from_secs(secs);
    }

    /// Get FPS tolerance value
    pub fn fps_tolerance(&self) -> f64 {
        self.fps_tolerance
    }

    /// Set FPS tolerance (clamped to 2.0-5.0 range)
    pub fn set_fps_tolerance(&mut self, tolerance: f64) {
        self.fps_tolerance = tolerance.clamp(MIN_FPS_TOLERANCE, MAX_FPS_TOLERANCE);
        tracing::debug!("FPS tolerance set to {:.1}", self.fps_tolerance);
    }

    /// Enable/disable Gamescope frame limiter sync
    pub fn set_sync_frame_limiter(&mut self, enabled: bool) {
        self.sync_frame_limiter = enabled;
    }

    /// Check if frame limiter sync is enabled
    pub fn is_sync_frame_limiter_enabled(&self) -> bool {
        self.sync_frame_limiter
    }

    /// Get the last Hz that was set (for frame limiter)
    pub fn last_set_hz(&self) -> Option<u32> {
        self.last_set_hz
    }

    /// Record the Hz that was set
    pub fn set_last_hz(&mut self, hz: u32) {
        self.last_set_hz = Some(hz);
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

    /// Update effective sensitivity based on device mode
    fn update_effective_sensitivity(&mut self) {
        if self.device_mode == DeviceMode::Lcd {
            self.effective_sensitivity = Sensitivity::Conservative;
        } else {
            self.effective_sensitivity = self.user_sensitivity;
        }
        self.drop_threshold = self.effective_sensitivity.drop_threshold();
        self.increase_threshold = self.effective_sensitivity.increase_threshold();
    }

    /// Update sensitivity (adjusts thresholds).
    /// Stores user preference but may apply different effective sensitivity for LCD.
    pub fn set_sensitivity(&mut self, sensitivity: Sensitivity) {
        self.user_sensitivity = sensitivity;
        self.update_effective_sensitivity();
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
                self.min_change_interval = Duration::from_millis(Self::MIN_CHANGE_INTERVAL_LCD_MS);
            }
            DeviceMode::Oled | DeviceMode::Custom => {
                self.min_change_interval = Duration::from_millis(Self::MIN_CHANGE_INTERVAL_OLED_MS);
            }
        }
        
        self.update_effective_sensitivity();
        // Reset state when mode changes
        self.state = AlgorithmState::Stable;
    }

    /// Get the effective Hz range based on device mode and user settings.
    fn get_effective_range(&self) -> (u32, u32) {
        match self.device_mode {
            DeviceMode::Lcd => {
                let effective_min = self.user_min_hz.max(Self::LCD_MIN_HZ);
                let effective_max = self.user_max_hz.min(Self::LCD_MAX_HZ);
                (effective_min, effective_max)
            }
            _ => (self.user_min_hz, self.user_max_hz),
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

    /// Apply adaptive sensitivity based on FPS variance
    fn apply_adaptive_sensitivity(&mut self) {
        if !self.adaptive_sensitivity_enabled || self.device_mode == DeviceMode::Lcd {
            return;
        }

        if !self.fps_window.is_full() {
            return;
        }

        let std_dev = self.fps_window.std_dev();
        
        if std_dev > STD_DEV_UNSTABLE {
            // Unstable FPS - force conservative
            if self.effective_sensitivity != Sensitivity::Conservative {
                self.effective_sensitivity = Sensitivity::Conservative;
                self.drop_threshold = Sensitivity::Conservative.drop_threshold();
                self.increase_threshold = Sensitivity::Conservative.increase_threshold();
                tracing::debug!("Adaptive: FPS unstable (std_dev={:.2}), forcing Conservative", std_dev);
            }
        } else if std_dev < STD_DEV_STABLE {
            // Stable FPS - allow user preference
            if self.effective_sensitivity != self.user_sensitivity {
                self.effective_sensitivity = self.user_sensitivity;
                self.drop_threshold = self.user_sensitivity.drop_threshold();
                self.increase_threshold = self.user_sensitivity.increase_threshold();
                tracing::debug!("Adaptive: FPS stable (std_dev={:.2}), restoring user sensitivity", std_dev);
            }
        }
        // Between thresholds - keep current effective sensitivity
    }

    /// Process FPS sample and determine if refresh rate should change.
    ///
    /// Returns `Some(new_hz)` if a change is needed, `None` otherwise.
    ///
    /// # Algorithm
    /// - If external display detected → return None (paused)
    /// - If FPS within tolerance of current Hz → force Stable, return None (sticky target)
    /// - If FPS < (CurrentHz - 1) for `drop_threshold` duration → decrease Hz
    /// - If FPS >= CurrentHz for `increase_threshold` duration → increase Hz by 5Hz step
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
        // Add FPS to sliding window for adaptive sensitivity
        self.fps_window.push(current_fps);
        
        // Apply adaptive sensitivity if enabled
        self.apply_adaptive_sensitivity();

        // If external display detected, pause processing
        if self.external_display_detected {
            self.state = AlgorithmState::Stable;
            return None;
        }

        // Check resume cooldown - no changes during cooldown period
        if self.is_in_resume_cooldown() {
            self.state = AlgorithmState::Stable;
            tracing::trace!("Resume cooldown active, {:.1}s remaining", self.resume_cooldown_remaining());
            return None;
        }

        let (effective_min, effective_max) = self.get_effective_range();
        
        // FPS Jitter Tolerance ("Sticky Target")
        // If FPS is within tolerance of current Hz, force stable state
        // Uses configurable fps_tolerance instead of constant
        let fps_diff = (current_fps - current_hz as f64).abs();
        if fps_diff < self.fps_tolerance {
            self.state = AlgorithmState::Stable;
            return None;
        }

        // Check if FPS is below the drop threshold (CurrentHz - 1)
        let fps_below_threshold = current_fps < (current_hz as f64 - 1.0);
        // Check if FPS is at or above current Hz (can potentially increase)
        let fps_at_or_above = current_fps >= current_hz as f64;

        match self.state {
            AlgorithmState::Stable => {
                if fps_below_threshold && current_hz > effective_min {
                    self.state = AlgorithmState::Dropping { since: now };
                } else if fps_at_or_above && current_hz < effective_max {
                    self.state = AlgorithmState::Increasing { since: now };
                }
                None
            }

            AlgorithmState::Dropping { since } => {
                if !fps_below_threshold {
                    self.state = AlgorithmState::Stable;
                    None
                } else if now.duration_since(since) >= self.drop_threshold {
                    if self.can_change(now) {
                        let target_hz = self.target_hz_for_drop(current_fps);
                        
                        if current_hz.abs_diff(target_hz) < HZ_STEP_SIZE {
                            self.state = AlgorithmState::Stable;
                            return None;
                        }
                        
                        self.state = AlgorithmState::Stable;
                        self.record_change(now);
                        self.last_set_hz = Some(target_hz);
                        Some(target_hz)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }

            AlgorithmState::Increasing { since } => {
                if fps_below_threshold {
                    self.state = AlgorithmState::Dropping { since: now };
                    None
                } else if !fps_at_or_above {
                    self.state = AlgorithmState::Stable;
                    None
                } else if now.duration_since(since) >= self.increase_threshold {
                    if self.can_change(now) {
                        let new_hz = self.next_step_up(current_hz);
                        
                        if new_hz <= current_hz {
                            self.state = AlgorithmState::Stable;
                            return None;
                        }
                        
                        self.state = AlgorithmState::Stable;
                        self.record_change(now);
                        self.last_set_hz = Some(new_hz);
                        Some(new_hz)
                    } else {
                        None
                    }
                } else {
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

    #[test]
    fn test_sensitivity_thresholds() {
        assert_eq!(Sensitivity::Conservative.drop_threshold(), Duration::from_secs(2));
        assert_eq!(Sensitivity::Conservative.increase_threshold(), Duration::from_secs(5));
        assert_eq!(Sensitivity::Balanced.drop_threshold(), Duration::from_secs(1));
        assert_eq!(Sensitivity::Balanced.increase_threshold(), Duration::from_secs(3));
        assert_eq!(Sensitivity::Aggressive.drop_threshold(), Duration::from_millis(500));
        assert_eq!(Sensitivity::Aggressive.increase_threshold(), Duration::from_millis(1500));
    }

    #[test]
    fn test_new_controller_starts_stable() {
        let controller = HysteresisController::new(Sensitivity::Balanced);
        assert_eq!(controller.state(), AlgorithmState::Stable);
        assert!(controller.last_change().is_none());
    }

    #[test]
    fn test_fps_jitter_tolerance_sticky_target() {
        let mut controller = HysteresisController::new(Sensitivity::Balanced);
        controller.set_user_range(40, 90);
        let start = Instant::now();

        // FPS at 58, Hz at 60 - within tolerance (diff = 2 < 3)
        let result = controller.process_with_time(58.0, 60, start);
        assert!(result.is_none());
        assert_eq!(controller.state(), AlgorithmState::Stable);

        // FPS at 62, Hz at 60 - within tolerance (diff = 2 < 3)
        let result = controller.process_with_time(62.0, 60, start);
        assert!(result.is_none());
        assert_eq!(controller.state(), AlgorithmState::Stable);

        // FPS at 55, Hz at 60 - outside tolerance (diff = 5 > 3)
        let result = controller.process_with_time(55.0, 60, start);
        assert!(result.is_none()); // First sample, enters Dropping state
        assert!(matches!(controller.state(), AlgorithmState::Dropping { .. }));
    }

    #[test]
    fn test_reset_state_clears_timestamps() {
        let mut controller = HysteresisController::new(Sensitivity::Balanced);
        controller.set_user_range(40, 90);
        let start = Instant::now();

        // Make a change to set last_change
        controller.state = AlgorithmState::Dropping { since: start };
        controller.last_change = Some(start);

        // Reset state
        controller.reset_state();

        assert_eq!(controller.state(), AlgorithmState::Stable);
        assert!(controller.last_change().is_none());
        // Should have resume cooldown active
        assert!(controller.is_in_resume_cooldown());
    }

    #[test]
    fn test_resume_cooldown_blocks_changes() {
        let mut controller = HysteresisController::new(Sensitivity::Balanced);
        controller.set_user_range(40, 90);
        controller.set_resume_cooldown(2); // 2 seconds
        
        // Trigger resume cooldown
        controller.reset_state();
        
        let start = Instant::now();
        
        // Even with low FPS, should return None during cooldown
        let result = controller.process_with_time(30.0, 60, start);
        assert!(result.is_none());
        assert_eq!(controller.state(), AlgorithmState::Stable);
        assert!(controller.is_in_resume_cooldown());
    }

    #[test]
    fn test_configurable_fps_tolerance() {
        let mut controller = HysteresisController::new(Sensitivity::Balanced);
        controller.set_user_range(40, 90);
        
        // Default tolerance is 3.0
        assert_eq!(controller.fps_tolerance(), DEFAULT_FPS_TOLERANCE);
        
        // Set to 5.0 (max stability)
        controller.set_fps_tolerance(5.0);
        assert_eq!(controller.fps_tolerance(), 5.0);
        
        let start = Instant::now();
        
        // FPS at 56, Hz at 60 - within tolerance of 5.0 (diff = 4)
        let result = controller.process_with_time(56.0, 60, start);
        assert!(result.is_none());
        assert_eq!(controller.state(), AlgorithmState::Stable);
        
        // Set to 2.0 (aggressive)
        controller.set_fps_tolerance(2.0);
        
        // FPS at 56, Hz at 60 - outside tolerance of 2.0 (diff = 4)
        let result = controller.process_with_time(56.0, 60, start);
        assert!(result.is_none()); // First sample enters Dropping
        assert!(matches!(controller.state(), AlgorithmState::Dropping { .. }));
    }

    #[test]
    fn test_fps_tolerance_clamping() {
        let mut controller = HysteresisController::new(Sensitivity::Balanced);
        
        // Try to set below minimum
        controller.set_fps_tolerance(1.0);
        assert_eq!(controller.fps_tolerance(), MIN_FPS_TOLERANCE);
        
        // Try to set above maximum
        controller.set_fps_tolerance(10.0);
        assert_eq!(controller.fps_tolerance(), MAX_FPS_TOLERANCE);
    }

    #[test]
    fn test_external_display_pauses_processing() {
        let mut controller = HysteresisController::new(Sensitivity::Balanced);
        controller.set_user_range(40, 90);
        controller.set_external_display_detected(true);
        let start = Instant::now();

        // Even with low FPS, should return None when external display detected
        let result = controller.process_with_time(30.0, 60, start);
        assert!(result.is_none());
        assert_eq!(controller.state(), AlgorithmState::Stable);
    }

    #[test]
    fn test_fps_sliding_window() {
        let mut window = FpsSlidingWindow::new(5);
        
        // Push samples
        for i in 1..=5 {
            window.push(i as f64 * 10.0);
        }
        
        assert!(window.is_full());
        assert_eq!(window.len(), 5);
        assert_eq!(window.mean(), 30.0); // (10+20+30+40+50)/5
        
        // Std dev of [10,20,30,40,50] = sqrt(250) ≈ 15.81
        let std_dev = window.std_dev();
        assert!((std_dev - 15.81).abs() < 0.1);
    }

    #[test]
    fn test_adaptive_sensitivity_unstable_fps() {
        let mut controller = HysteresisController::new(Sensitivity::Aggressive);
        controller.set_user_range(40, 90);
        controller.set_adaptive_sensitivity(true);

        // Push highly variable FPS samples (std_dev > 5)
        let fps_samples = [30.0, 60.0, 35.0, 55.0, 40.0, 65.0, 32.0, 58.0, 38.0, 62.0];
        let start = Instant::now();
        
        for fps in fps_samples {
            controller.process_with_time(fps, 60, start);
        }

        // Should have switched to Conservative due to high variance
        assert_eq!(controller.effective_sensitivity(), Sensitivity::Conservative);
    }

    #[test]
    fn test_adaptive_sensitivity_stable_fps() {
        let mut controller = HysteresisController::new(Sensitivity::Aggressive);
        controller.set_user_range(40, 90);
        controller.set_adaptive_sensitivity(true);

        // Push stable FPS samples (std_dev < 2)
        let fps_samples = [60.0, 60.5, 59.5, 60.2, 59.8, 60.1, 59.9, 60.3, 59.7, 60.0];
        let start = Instant::now();
        
        for fps in fps_samples {
            controller.process_with_time(fps, 60, start);
        }

        // Should keep user preference (Aggressive) due to low variance
        assert_eq!(controller.effective_sensitivity(), Sensitivity::Aggressive);
    }

    #[test]
    fn test_lcd_mode_forces_conservative() {
        let mut controller = HysteresisController::new(Sensitivity::Aggressive);
        assert_eq!(controller.effective_sensitivity(), Sensitivity::Aggressive);
        
        controller.apply_mode_constraints(DeviceMode::Lcd);
        
        assert_eq!(controller.sensitivity(), Sensitivity::Aggressive);
        assert_eq!(controller.effective_sensitivity(), Sensitivity::Conservative);
        assert_eq!(controller.min_change_interval, Duration::from_millis(2000));
    }

    #[test]
    fn test_quantize_hz() {
        assert_eq!(HysteresisController::quantize_hz(42), 40);
        assert_eq!(HysteresisController::quantize_hz(43), 45);
        assert_eq!(HysteresisController::quantize_hz(47), 45);
        assert_eq!(HysteresisController::quantize_hz(48), 50);
        assert_eq!(HysteresisController::quantize_hz(50), 50);
    }

    proptest! {
        #[test]
        fn prop_sticky_target_prevents_oscillation(
            current_hz in 45u32..=85u32,
            fps_offset in 0.0f64..2.9f64,
        ) {
            let current_hz = (current_hz / 5) * 5;
            let mut controller = HysteresisController::new(Sensitivity::Balanced);
            controller.set_user_range(40, 90);
            let start = Instant::now();

            // FPS within tolerance should not trigger state change
            let fps_above = current_hz as f64 + fps_offset;
            let fps_below = current_hz as f64 - fps_offset;

            let result1 = controller.process_with_time(fps_above, current_hz, start);
            prop_assert!(result1.is_none());
            prop_assert_eq!(controller.state(), AlgorithmState::Stable);

            let result2 = controller.process_with_time(fps_below, current_hz, start);
            prop_assert!(result2.is_none());
            prop_assert_eq!(controller.state(), AlgorithmState::Stable);
        }

        #[test]
        fn prop_hz_always_quantized_to_5hz(
            fps in 35.0f64..=95.0f64,
            current_hz in 40u32..=90u32,
        ) {
            let current_hz = (current_hz / 5) * 5;
            let mut controller = HysteresisController::new(Sensitivity::Balanced);
            controller.set_user_range(40, 90);
            let start = Instant::now();

            let _ = controller.process_with_time(fps, current_hz, start);
            let after_threshold = start + Duration::from_millis(5001);
            let result = controller.process_with_time(fps, current_hz, after_threshold);

            if let Some(new_hz) = result {
                prop_assert_eq!(new_hz % 5, 0, "Output Hz must be on 5Hz step boundary");
                prop_assert!(new_hz >= 40 && new_hz <= 90, "Output Hz must be in valid range");
            }
        }
    }
}
