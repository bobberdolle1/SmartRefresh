//! Battery monitoring and power savings estimation for SmartRefresh daemon.
//!
//! Reads power consumption from sysfs and estimates savings from dynamic refresh rate.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::RwLock;
use std::time::Instant;
use tracing::{debug, warn};

/// Path to battery power consumption (in microwatts)
const POWER_NOW_PATH: &str = "/sys/class/power_supply/BAT1/power_now";

/// Alternative path for some systems
const POWER_NOW_PATH_ALT: &str = "/sys/class/power_supply/BAT0/power_now";

/// Number of samples for moving average
const POWER_SAMPLE_COUNT: usize = 12;

/// Power sample with Hz context
#[derive(Debug, Clone)]
struct PowerSample {
    power_uw: u64,
    hz: u32,
    timestamp: Instant,
}

/// Battery monitoring response for IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryResponse {
    /// Current power consumption in watts
    pub power_watts: f64,
    /// Average power consumption in watts
    pub avg_power_watts: f64,
    /// Estimated battery savings in minutes (compared to max Hz)
    pub estimated_savings_minutes: f64,
    /// Whether battery monitoring is available
    pub available: bool,
}

/// Battery monitor for power tracking
pub struct BatteryMonitor {
    /// Recent power samples
    samples: RwLock<VecDeque<PowerSample>>,
    /// Max Hz for savings calculation
    max_hz: RwLock<u32>,
    /// Whether battery sysfs is available
    available: RwLock<bool>,
}

impl BatteryMonitor {
    pub fn new() -> Self {
        let available = std::path::Path::new(POWER_NOW_PATH).exists()
            || std::path::Path::new(POWER_NOW_PATH_ALT).exists();
        
        Self {
            samples: RwLock::new(VecDeque::with_capacity(POWER_SAMPLE_COUNT)),
            max_hz: RwLock::new(90),
            available: RwLock::new(available),
        }
    }

    /// Set max Hz for savings calculation
    pub fn set_max_hz(&self, hz: u32) {
        if let Ok(mut max) = self.max_hz.write() {
            *max = hz;
        }
    }

    /// Read current power consumption in microwatts
    pub fn read_power_now(&self) -> Option<u64> {
        // Try primary path first
        let path = if std::path::Path::new(POWER_NOW_PATH).exists() {
            POWER_NOW_PATH
        } else if std::path::Path::new(POWER_NOW_PATH_ALT).exists() {
            POWER_NOW_PATH_ALT
        } else {
            if let Ok(mut available) = self.available.write() {
                *available = false;
            }
            return None;
        };

        match std::fs::read_to_string(path) {
            Ok(contents) => {
                contents.trim().parse::<u64>().ok()
            }
            Err(e) => {
                debug!("Failed to read power_now: {}", e);
                None
            }
        }
    }

    /// Record a power sample
    pub fn record_sample(&self, power_uw: u64, hz: u32) {
        if let Ok(mut samples) = self.samples.write() {
            if samples.len() >= POWER_SAMPLE_COUNT {
                samples.pop_front();
            }
            samples.push_back(PowerSample {
                power_uw,
                hz,
                timestamp: Instant::now(),
            });
        }
    }

    /// Get battery status response
    pub fn get_status(&self) -> BatteryResponse {
        let available = self.available.read().map(|a| *a).unwrap_or(false);
        
        if !available {
            return BatteryResponse {
                power_watts: 0.0,
                avg_power_watts: 0.0,
                estimated_savings_minutes: 0.0,
                available: false,
            };
        }

        let current_power = self.read_power_now().unwrap_or(0);
        let current_watts = current_power as f64 / 1_000_000.0;

        let (avg_watts, savings) = self.samples.read()
            .map(|samples| {
                if samples.is_empty() {
                    return (current_watts, 0.0);
                }

                // Calculate average power
                let avg_power: f64 = samples.iter()
                    .map(|s| s.power_uw as f64)
                    .sum::<f64>() / samples.len() as f64;
                let avg_watts = avg_power / 1_000_000.0;

                // Calculate weighted average Hz
                let avg_hz: f64 = samples.iter()
                    .map(|s| s.hz as f64)
                    .sum::<f64>() / samples.len() as f64;

                // Estimate savings using linear approximation
                // Power ~ Frequency (simplified model)
                let max_hz = self.max_hz.read().map(|h| *h).unwrap_or(90) as f64;
                
                if avg_hz >= max_hz || avg_hz <= 0.0 {
                    return (avg_watts, 0.0);
                }

                // Theoretical power at max Hz
                let theoretical_max_power = avg_watts * (max_hz / avg_hz);
                let power_saved_watts = theoretical_max_power - avg_watts;
                
                // Assume 40Wh battery, calculate minutes saved per hour
                // This is a rough estimate
                let battery_wh = 40.0;
                let hours_saved = if power_saved_watts > 0.0 {
                    (power_saved_watts / battery_wh) * 60.0 // minutes per hour of use
                } else {
                    0.0
                };

                (avg_watts, hours_saved)
            })
            .unwrap_or((current_watts, 0.0));

        BatteryResponse {
            power_watts: current_watts,
            avg_power_watts: avg_watts,
            estimated_savings_minutes: savings,
            available: true,
        }
    }
}

impl Default for BatteryMonitor {
    fn default() -> Self {
        Self::new()
    }
}
