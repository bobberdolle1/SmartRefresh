//! Metrics collection module for SmartRefresh daemon.
//!
//! Tracks switch counts, timing, and other operational metrics.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Metrics data exposed via IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    /// Total number of refresh rate switches since daemon start
    pub total_switches: u64,
    /// Switches in the last hour
    pub switches_per_hour: u64,
    /// Average time spent in Stable state (seconds)
    pub avg_time_in_stable_sec: f64,
    /// Uptime in seconds
    pub uptime_sec: u64,
    /// Number of drops (Hz decreased)
    pub drop_count: u64,
    /// Number of increases (Hz increased)
    pub increase_count: u64,
}

/// Metrics collector for the daemon
pub struct MetricsCollector {
    /// Daemon start time
    start_time: Instant,
    /// Total switch count
    total_switches: AtomicU64,
    /// Drop count (Hz decreased)
    drop_count: AtomicU64,
    /// Increase count (Hz increased)
    increase_count: AtomicU64,
    /// Recent switches with timestamps for per-hour calculation
    recent_switches: RwLock<Vec<Instant>>,
    /// Time spent in stable state
    stable_durations: RwLock<Vec<Duration>>,
    /// Last state change time
    last_state_change: RwLock<Option<Instant>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            total_switches: AtomicU64::new(0),
            drop_count: AtomicU64::new(0),
            increase_count: AtomicU64::new(0),
            recent_switches: RwLock::new(Vec::new()),
            stable_durations: RwLock::new(Vec::new()),
            last_state_change: RwLock::new(Some(Instant::now())),
        }
    }

    /// Record a refresh rate switch
    pub fn record_switch(&self, old_hz: u32, new_hz: u32) {
        let now = Instant::now();
        
        self.total_switches.fetch_add(1, Ordering::SeqCst);
        
        if new_hz < old_hz {
            self.drop_count.fetch_add(1, Ordering::SeqCst);
        } else if new_hz > old_hz {
            self.increase_count.fetch_add(1, Ordering::SeqCst);
        }

        // Record timestamp for per-hour calculation
        if let Ok(mut switches) = self.recent_switches.write() {
            switches.push(now);
            // Keep only last hour of switches
            let hour_ago = now - Duration::from_secs(3600);
            switches.retain(|t| *t > hour_ago);
        }

        // Record stable duration
        if let Ok(mut last_change) = self.last_state_change.write() {
            if let Some(last) = *last_change {
                let duration = now.duration_since(last);
                if let Ok(mut durations) = self.stable_durations.write() {
                    durations.push(duration);
                    // Keep only last 100 durations
                    if durations.len() > 100 {
                        durations.remove(0);
                    }
                }
            }
            *last_change = Some(now);
        }
    }

    /// Get current metrics
    pub fn get_metrics(&self) -> MetricsResponse {
        let now = Instant::now();
        let uptime = now.duration_since(self.start_time);

        let switches_per_hour = self.recent_switches
            .read()
            .map(|switches| {
                let hour_ago = now - Duration::from_secs(3600);
                switches.iter().filter(|t| **t > hour_ago).count() as u64
            })
            .unwrap_or(0);

        let avg_time_in_stable = self.stable_durations
            .read()
            .map(|durations| {
                if durations.is_empty() {
                    0.0
                } else {
                    let total: Duration = durations.iter().sum();
                    total.as_secs_f64() / durations.len() as f64
                }
            })
            .unwrap_or(0.0);

        MetricsResponse {
            total_switches: self.total_switches.load(Ordering::SeqCst),
            switches_per_hour,
            avg_time_in_stable_sec: avg_time_in_stable,
            uptime_sec: uptime.as_secs(),
            drop_count: self.drop_count.load(Ordering::SeqCst),
            increase_count: self.increase_count.load(Ordering::SeqCst),
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
