//! IPC Server module for frontend communication.
//!
//! This module provides a Unix Domain Socket server for receiving
//! commands from the Decky frontend and sending status responses.
//!
//! v2.0 additions:
//! - GetMetrics command
//! - Profile management commands
//! - Battery status in response
//! - Transition history

use crate::battery::BatteryMonitor;
use crate::config::{Config, ConfigManager};
use crate::core_logic::{AlgorithmState, DeviceMode, HysteresisController, Sensitivity};
use crate::error::IpcError;
use crate::metrics::MetricsCollector;
use crate::profiles::{GameProfile, ProfileListResponse, ProfileManager};

use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

#[cfg(unix)]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

/// Default socket path for IPC communication.
pub const DEFAULT_SOCKET_PATH: &str = "/tmp/smart-refresh.sock";

/// Maximum transition history entries
const MAX_TRANSITION_HISTORY: usize = 20;

/// Commands that can be received via IPC.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "command")]
pub enum IpcCommand {
    Start,
    Stop,
    SetConfig {
        min_hz: u32,
        max_hz: u32,
        sensitivity: String,
        #[serde(default)]
        adaptive_sensitivity: Option<bool>,
        #[serde(default)]
        fps_tolerance: Option<f64>,
        #[serde(default)]
        sync_frame_limiter: Option<bool>,
    },
    SetDeviceMode {
        mode: String,
    },
    SetAdvancedConfig {
        fps_tolerance: Option<f64>,
        resume_cooldown_secs: Option<u64>,
        sync_frame_limiter: Option<bool>,
    },
    GetStatus,
    GetMetrics,
    // Profile commands
    SetGameId {
        app_id: String,
        #[serde(default)]
        name: Option<String>,
    },
    SaveProfile {
        app_id: String,
        name: String,
        min_hz: u32,
        max_hz: u32,
        sensitivity: String,
        #[serde(default)]
        adaptive_sensitivity: bool,
    },
    DeleteProfile {
        app_id: String,
    },
    GetProfiles,
    // Battery
    GetBatteryStatus,
}

/// Transition record for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionRecord {
    pub timestamp: String,
    pub from_hz: u32,
    pub to_hz: u32,
    pub fps: f64,
    pub direction: String, // "Dropped" or "Increased"
}

/// Configuration portion of status response.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ConfigResponse {
    pub min_hz: u32,
    pub max_hz: u32,
    pub sensitivity: String,
    pub enabled: bool,
    pub adaptive_sensitivity: bool,
}

impl ConfigResponse {
    pub fn from_config(config: &Config, adaptive: bool) -> Self {
        Self {
            min_hz: config.min_hz,
            max_hz: config.max_hz,
            sensitivity: sensitivity_to_string(config.sensitivity),
            enabled: config.enabled,
            adaptive_sensitivity: adaptive,
        }
    }
}

/// Status response sent to clients.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatusResponse {
    pub running: bool,
    pub current_fps: f64,
    pub current_hz: u32,
    pub state: String,
    pub device_mode: String,
    pub config: ConfigResponse,
    pub mangohud_available: bool,
    pub external_display_detected: bool,
    pub fps_std_dev: f64,
    pub current_app_id: Option<String>,
    pub transitions: Vec<TransitionRecord>,
    // v2.0.1 advanced fields
    pub fps_tolerance: f64,
    pub resume_cooldown_remaining: f64,
    pub sync_frame_limiter: bool,
}

/// Convert Sensitivity enum to string.
pub fn sensitivity_to_string(sensitivity: Sensitivity) -> String {
    match sensitivity {
        Sensitivity::Conservative => "conservative".to_string(),
        Sensitivity::Balanced => "balanced".to_string(),
        Sensitivity::Aggressive => "aggressive".to_string(),
    }
}

/// Parse sensitivity string to enum.
pub fn parse_sensitivity(s: &str) -> Result<Sensitivity, IpcError> {
    match s.to_lowercase().as_str() {
        "conservative" => Ok(Sensitivity::Conservative),
        "balanced" => Ok(Sensitivity::Balanced),
        "aggressive" => Ok(Sensitivity::Aggressive),
        _ => Err(IpcError::InvalidCommand(format!(
            "Invalid sensitivity '{}', expected one of: conservative, balanced, aggressive",
            s
        ))),
    }
}

/// Parse device mode string to enum.
pub fn parse_device_mode(s: &str) -> Result<DeviceMode, IpcError> {
    match s.to_lowercase().as_str() {
        "oled" => Ok(DeviceMode::Oled),
        "lcd" => Ok(DeviceMode::Lcd),
        "custom" => Ok(DeviceMode::Custom),
        _ => Err(IpcError::InvalidCommand(format!(
            "Invalid device mode '{}', expected one of: oled, lcd, custom",
            s
        ))),
    }
}

/// Convert DeviceMode enum to string.
pub fn device_mode_to_string(mode: DeviceMode) -> String {
    match mode {
        DeviceMode::Oled => "oled".to_string(),
        DeviceMode::Lcd => "lcd".to_string(),
        DeviceMode::Custom => "custom".to_string(),
    }
}

/// Convert AlgorithmState to string for status response.
pub fn algorithm_state_to_string(state: AlgorithmState) -> String {
    match state {
        AlgorithmState::Stable => "Stable".to_string(),
        AlgorithmState::Dropping { .. } => "Dropping".to_string(),
        AlgorithmState::Increasing { .. } => "Increasing".to_string(),
    }
}

/// Shared daemon state accessible by the IPC server.
pub struct DaemonState {
    /// Whether the refresh rate control loop is running
    pub running: AtomicBool,
    /// Current FPS value
    pub current_fps: RwLock<f64>,
    /// Current refresh rate in Hz
    pub current_hz: AtomicU32,
    /// Hysteresis controller for algorithm state
    pub controller: RwLock<HysteresisController>,
    /// Configuration manager
    pub config_manager: Arc<ConfigManager>,
    /// Profile manager
    pub profile_manager: Arc<RwLock<ProfileManager>>,
    /// Metrics collector
    pub metrics: Arc<MetricsCollector>,
    /// Battery monitor
    pub battery_monitor: Arc<BatteryMonitor>,
    /// MangoHud availability
    mangohud_available: AtomicBool,
    /// Transition history
    transitions: RwLock<Vec<TransitionRecord>>,
}

impl DaemonState {
    /// Create a new daemon state with the given managers.
    pub fn new(
        config_manager: Arc<ConfigManager>,
        profile_manager: Arc<RwLock<ProfileManager>>,
        metrics: Arc<MetricsCollector>,
        battery_monitor: Arc<BatteryMonitor>,
    ) -> Self {
        let config = config_manager.get();
        let mut controller = HysteresisController::new(config.sensitivity);
        controller.set_user_range(config.min_hz, config.max_hz);
        
        Self {
            running: AtomicBool::new(config.enabled),
            current_fps: RwLock::new(0.0),
            current_hz: AtomicU32::new(config.max_hz),
            controller: RwLock::new(controller),
            config_manager,
            profile_manager,
            metrics,
            battery_monitor,
            mangohud_available: AtomicBool::new(false),
            transitions: RwLock::new(Vec::new()),
        }
    }

    /// Set MangoHud availability
    pub fn set_mangohud_available(&self, available: bool) {
        self.mangohud_available.store(available, Ordering::SeqCst);
    }

    /// Record a transition for UI display
    pub async fn record_transition(&self, from_hz: u32, to_hz: u32, fps: f64) {
        let direction = if to_hz < from_hz { "Dropped" } else { "Increased" };
        let timestamp = chrono_lite_timestamp();
        
        let record = TransitionRecord {
            timestamp,
            from_hz,
            to_hz,
            fps,
            direction: direction.to_string(),
        };

        if let Ok(mut transitions) = self.transitions.write() {
            transitions.push(record);
            if transitions.len() > MAX_TRANSITION_HISTORY {
                transitions.remove(0);
            }
        }
    }

    /// Get the current status as a StatusResponse.
    pub async fn get_status(&self) -> StatusResponse {
        let config = self.config_manager.get();
        let controller = self.controller.read().await;
        let current_fps = *self.current_fps.read().await;
        let profile_manager = self.profile_manager.read().await;
        let transitions = self.transitions.read()
            .map(|t| t.clone())
            .unwrap_or_default();

        StatusResponse {
            running: self.running.load(Ordering::SeqCst),
            current_fps,
            current_hz: self.current_hz.load(Ordering::SeqCst),
            state: algorithm_state_to_string(controller.state()),
            device_mode: device_mode_to_string(controller.device_mode()),
            config: ConfigResponse::from_config(&config, controller.is_adaptive_sensitivity_enabled()),
            mangohud_available: self.mangohud_available.load(Ordering::SeqCst),
            external_display_detected: controller.is_external_display_detected(),
            fps_std_dev: controller.get_fps_std_dev(),
            current_app_id: profile_manager.get_current_game().cloned(),
            transitions,
            // v2.0.1 advanced fields
            fps_tolerance: controller.fps_tolerance(),
            resume_cooldown_remaining: controller.resume_cooldown_remaining(),
            sync_frame_limiter: controller.is_sync_frame_limiter_enabled(),
        }
    }

    /// Start the refresh rate control loop.
    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    /// Stop the refresh rate control loop.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if the daemon is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Simple timestamp without chrono dependency
fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    let secs = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

/// Unix Domain Socket server for IPC.
#[cfg(unix)]
pub struct IpcServer {
    socket_path: PathBuf,
    listener: UnixListener,
}

#[cfg(unix)]
impl IpcServer {
    pub async fn new(path: &str) -> Result<Self, IpcError> {
        let socket_path = PathBuf::from(path);
        Self::cleanup_socket(&socket_path)?;

        let listener = UnixListener::bind(&socket_path).map_err(|e| IpcError::SocketBindFailed {
            path: path.to_string(),
            source: e,
        })?;

        Ok(Self {
            socket_path,
            listener,
        })
    }

    pub async fn new_default() -> Result<Self, IpcError> {
        Self::new(DEFAULT_SOCKET_PATH).await
    }

    fn cleanup_socket(path: &Path) -> Result<(), IpcError> {
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| IpcError::SocketBindFailed {
                path: path.display().to_string(),
                source: e,
            })?;
        }
        Ok(())
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub async fn run(&self, state: Arc<DaemonState>) -> Result<(), IpcError> {
        loop {
            match self.listener.accept().await {
                Ok((stream, _addr)) => {
                    let state = Arc::clone(&state);
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, state).await {
                            tracing::warn!("Error handling IPC connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Error accepting IPC connection: {}", e);
                }
            }
        }
    }

    async fn handle_connection(
        stream: UnixStream,
        state: Arc<DaemonState>,
    ) -> Result<(), IpcError> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        while reader.read_line(&mut line).await? > 0 {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                line.clear();
                continue;
            }

            let response = match serde_json::from_str::<IpcCommand>(trimmed) {
                Ok(command) => Self::handle_command(command, &state).await,
                Err(e) => serde_json::json!({
                    "error": format!("Invalid command: {}", e)
                }),
            };

            let response_str = serde_json::to_string(&response)?;
            writer.write_all(response_str.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;

            line.clear();
        }

        Ok(())
    }

    pub async fn handle_command(
        command: IpcCommand,
        state: &Arc<DaemonState>,
    ) -> serde_json::Value {
        match command {
            IpcCommand::Start => {
                state.start();
                tracing::info!("Daemon started via IPC");
                serde_json::json!({ "success": true, "message": "Daemon started" })
            }

            IpcCommand::Stop => {
                state.stop();
                tracing::info!("Daemon stopped via IPC");
                serde_json::json!({ "success": true, "message": "Daemon stopped" })
            }

            IpcCommand::SetConfig {
                min_hz,
                max_hz,
                sensitivity,
                adaptive_sensitivity,
                fps_tolerance,
                sync_frame_limiter,
            } => {
                let sensitivity_enum = match parse_sensitivity(&sensitivity) {
                    Ok(s) => s,
                    Err(e) => {
                        return serde_json::json!({
                            "success": false,
                            "error": e.to_string()
                        });
                    }
                };

                let mut config = state.config_manager.get();
                config.min_hz = min_hz;
                config.max_hz = max_hz;
                config.sensitivity = sensitivity_enum;

                match state.config_manager.update(config) {
                    Ok(()) => {
                        let mut controller = state.controller.write().await;
                        controller.set_user_range(min_hz, max_hz);
                        controller.set_sensitivity(sensitivity_enum);
                        if let Some(adaptive) = adaptive_sensitivity {
                            controller.set_adaptive_sensitivity(adaptive);
                        }
                        if let Some(tolerance) = fps_tolerance {
                            controller.set_fps_tolerance(tolerance);
                        }
                        if let Some(sync_fl) = sync_frame_limiter {
                            controller.set_sync_frame_limiter(sync_fl);
                        }
                        tracing::info!(
                            "Config updated via IPC: min_hz={}, max_hz={}, sensitivity={}",
                            min_hz, max_hz, sensitivity
                        );
                        serde_json::json!({ "success": true, "message": "Configuration updated" })
                    }
                    Err(e) => {
                        tracing::warn!("Failed to update config via IPC: {}", e);
                        serde_json::json!({
                            "success": false,
                            "error": e.to_string()
                        })
                    }
                }
            }

            IpcCommand::SetAdvancedConfig {
                fps_tolerance,
                resume_cooldown_secs,
                sync_frame_limiter,
            } => {
                let mut controller = state.controller.write().await;
                
                if let Some(tolerance) = fps_tolerance {
                    controller.set_fps_tolerance(tolerance);
                }
                if let Some(cooldown) = resume_cooldown_secs {
                    controller.set_resume_cooldown(cooldown);
                }
                if let Some(sync_fl) = sync_frame_limiter {
                    controller.set_sync_frame_limiter(sync_fl);
                }
                
                tracing::info!(
                    "Advanced config updated: fps_tolerance={:?}, resume_cooldown={:?}, sync_frame_limiter={:?}",
                    fps_tolerance, resume_cooldown_secs, sync_frame_limiter
                );
                
                serde_json::json!({
                    "success": true,
                    "message": "Advanced configuration updated",
                    "fps_tolerance": controller.fps_tolerance(),
                    "sync_frame_limiter": controller.is_sync_frame_limiter_enabled()
                })
            }

            IpcCommand::SetDeviceMode { mode } => {
                let mode_enum = match parse_device_mode(&mode) {
                    Ok(m) => m,
                    Err(e) => {
                        return serde_json::json!({
                            "success": false,
                            "error": e.to_string()
                        });
                    }
                };

                let mut controller = state.controller.write().await;
                controller.apply_mode_constraints(mode_enum);
                
                let effective_sens = sensitivity_to_string(controller.effective_sensitivity());
                let min_interval = if mode_enum == DeviceMode::Lcd { 2000 } else { 500 };
                
                tracing::info!(
                    "Device mode set via IPC: mode={}, min_change_interval={}ms",
                    mode, min_interval
                );
                
                serde_json::json!({
                    "success": true,
                    "message": format!("Device mode set to {}", mode),
                    "mode": mode,
                    "effective_sensitivity": effective_sens,
                    "min_change_interval_ms": min_interval
                })
            }

            IpcCommand::GetStatus => {
                let status = state.get_status().await;
                serde_json::to_value(status).unwrap_or_else(|e| {
                    serde_json::json!({
                        "error": format!("Failed to serialize status: {}", e)
                    })
                })
            }

            IpcCommand::GetMetrics => {
                let metrics = state.metrics.get_metrics();
                serde_json::to_value(metrics).unwrap_or_else(|e| {
                    serde_json::json!({
                        "error": format!("Failed to serialize metrics: {}", e)
                    })
                })
            }

            IpcCommand::SetGameId { app_id, name } => {
                let mut profile_manager = state.profile_manager.write().await;
                
                let app_id_opt = if app_id.is_empty() || app_id == "0" {
                    None
                } else {
                    Some(app_id.clone())
                };
                
                profile_manager.set_current_game(app_id_opt.clone());
                
                // Apply profile settings if exists
                if let Some(ref id) = app_id_opt {
                    if let Some(profile) = profile_manager.get_profile(id) {
                        let mut controller = state.controller.write().await;
                        controller.set_user_range(profile.min_hz, profile.max_hz);
                        controller.set_sensitivity(profile.get_sensitivity());
                        controller.set_adaptive_sensitivity(profile.adaptive_sensitivity);
                        
                        tracing::info!("Applied profile for {} ({})", profile.name, id);
                        return serde_json::json!({
                            "success": true,
                            "message": format!("Loaded profile for {}", profile.name),
                            "profile_applied": true,
                            "profile_name": profile.name
                        });
                    }
                }
                
                // Revert to global defaults
                let (min_hz, max_hz, sensitivity, adaptive) = profile_manager.get_current_settings();
                let mut controller = state.controller.write().await;
                controller.set_user_range(min_hz, max_hz);
                controller.set_sensitivity(sensitivity);
                controller.set_adaptive_sensitivity(adaptive);
                
                serde_json::json!({
                    "success": true,
                    "message": "Game ID updated, using global defaults",
                    "profile_applied": false
                })
            }

            IpcCommand::SaveProfile {
                app_id,
                name,
                min_hz,
                max_hz,
                sensitivity,
                adaptive_sensitivity,
            } => {
                let profile = GameProfile {
                    app_id: app_id.clone(),
                    name: name.clone(),
                    min_hz,
                    max_hz,
                    sensitivity,
                    adaptive_sensitivity,
                };

                let mut profile_manager = state.profile_manager.write().await;
                profile_manager.set_profile(profile);
                
                if let Err(e) = profile_manager.save() {
                    tracing::warn!("Failed to save profiles: {}", e);
                    return serde_json::json!({
                        "success": false,
                        "error": format!("Failed to save profile: {}", e)
                    });
                }

                tracing::info!("Saved profile for {} ({})", name, app_id);
                serde_json::json!({
                    "success": true,
                    "message": format!("Profile saved for {}", name)
                })
            }

            IpcCommand::DeleteProfile { app_id } => {
                let mut profile_manager = state.profile_manager.write().await;
                
                if profile_manager.remove_profile(&app_id).is_some() {
                    if let Err(e) = profile_manager.save() {
                        tracing::warn!("Failed to save profiles after delete: {}", e);
                    }
                    serde_json::json!({
                        "success": true,
                        "message": "Profile deleted"
                    })
                } else {
                    serde_json::json!({
                        "success": false,
                        "error": "Profile not found"
                    })
                }
            }

            IpcCommand::GetProfiles => {
                let profile_manager = state.profile_manager.read().await;
                let response = ProfileListResponse::from(&*profile_manager);
                serde_json::to_value(response).unwrap_or_else(|e| {
                    serde_json::json!({
                        "error": format!("Failed to serialize profiles: {}", e)
                    })
                })
            }

            IpcCommand::GetBatteryStatus => {
                let status = state.battery_monitor.get_status();
                serde_json::to_value(status).unwrap_or_else(|e| {
                    serde_json::json!({
                        "error": format!("Failed to serialize battery status: {}", e)
                    })
                })
            }
        }
    }
}

#[cfg(unix)]
impl Drop for IpcServer {
    fn drop(&mut self) {
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}
