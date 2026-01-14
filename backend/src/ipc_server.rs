//! IPC Server module for frontend communication.
//!
//! This module provides a Unix Domain Socket server for receiving
//! commands from the Decky frontend and sending status responses.

use crate::config::{Config, ConfigManager};
use crate::core_logic::{AlgorithmState, DeviceMode, HysteresisController, Sensitivity};
use crate::error::IpcError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(unix)]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

/// Default socket path for IPC communication.
pub const DEFAULT_SOCKET_PATH: &str = "/tmp/smart-refresh.sock";

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
    },
    SetDeviceMode {
        mode: String,
    },
    GetStatus,
}

/// Configuration portion of status response.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ConfigResponse {
    pub min_hz: u32,
    pub max_hz: u32,
    pub sensitivity: String,
    pub enabled: bool,
}

impl From<&Config> for ConfigResponse {
    fn from(config: &Config) -> Self {
        Self {
            min_hz: config.min_hz,
            max_hz: config.max_hz,
            sensitivity: sensitivity_to_string(config.sensitivity),
            enabled: config.enabled,
        }
    }
}


/// Status response sent to clients.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct StatusResponse {
    pub running: bool,
    pub current_fps: f64,
    pub current_hz: u32,
    pub state: String,
    pub device_mode: String,
    pub config: ConfigResponse,
}

impl StatusResponse {
    /// Check if the response contains all required fields.
    /// Returns true if all fields are present and valid.
    pub fn is_complete(&self) -> bool {
        // Check that state is a valid algorithm state string
        let valid_states = ["Stable", "Dropping", "Increasing"];
        let state_valid = valid_states.contains(&self.state.as_str());
        
        // Check that sensitivity is valid
        let valid_sensitivities = ["conservative", "balanced", "aggressive"];
        let sensitivity_valid = valid_sensitivities.contains(&self.config.sensitivity.as_str());
        
        // Check that device_mode is valid
        let valid_modes = ["oled", "lcd", "custom"];
        let mode_valid = valid_modes.contains(&self.device_mode.as_str());
        
        // current_fps and current_hz are always present as they're required fields
        // running and enabled are booleans, always valid
        state_valid && sensitivity_valid && mode_valid
    }
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
}

impl DaemonState {
    /// Create a new daemon state with the given config manager.
    pub fn new(config_manager: Arc<ConfigManager>) -> Self {
        let config = config_manager.get();
        Self {
            running: AtomicBool::new(config.enabled),
            current_fps: RwLock::new(0.0),
            current_hz: AtomicU32::new(config.max_hz),
            controller: RwLock::new(HysteresisController::new(config.sensitivity)),
            config_manager,
        }
    }

    /// Get the current status as a StatusResponse.
    pub async fn get_status(&self) -> StatusResponse {
        let config = self.config_manager.get();
        let controller = self.controller.read().await;
        let current_fps = *self.current_fps.read().await;

        StatusResponse {
            running: self.running.load(Ordering::SeqCst),
            current_fps,
            current_hz: self.current_hz.load(Ordering::SeqCst),
            state: algorithm_state_to_string(controller.state()),
            device_mode: device_mode_to_string(controller.device_mode()),
            config: ConfigResponse::from(&config),
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


/// Unix Domain Socket server for IPC.
#[cfg(unix)]
pub struct IpcServer {
    /// Path to the Unix socket
    socket_path: PathBuf,
    /// Unix listener for incoming connections
    listener: UnixListener,
}

#[cfg(unix)]
impl IpcServer {
    /// Create a new IPC server at the specified path.
    ///
    /// This will:
    /// 1. Remove any existing socket file at the path
    /// 2. Bind a new Unix socket at the path
    pub async fn new(path: &str) -> Result<Self, IpcError> {
        let socket_path = PathBuf::from(path);

        // Clean up existing socket file if it exists
        Self::cleanup_socket(&socket_path)?;

        // Bind the Unix socket
        let listener = UnixListener::bind(&socket_path).map_err(|e| IpcError::SocketBindFailed {
            path: path.to_string(),
            source: e,
        })?;

        Ok(Self {
            socket_path,
            listener,
        })
    }

    /// Create a new IPC server at the default path.
    pub async fn new_default() -> Result<Self, IpcError> {
        Self::new(DEFAULT_SOCKET_PATH).await
    }

    /// Clean up an existing socket file.
    fn cleanup_socket(path: &Path) -> Result<(), IpcError> {
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| IpcError::SocketBindFailed {
                path: path.display().to_string(),
                source: e,
            })?;
        }
        Ok(())
    }

    /// Get the socket path.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Accept and handle incoming connections.
    ///
    /// This runs in a loop, accepting connections and spawning tasks to handle them.
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
                    // Continue accepting connections even after errors
                }
            }
        }
    }

    /// Handle a single client connection.
    async fn handle_connection(
        stream: UnixStream,
        state: Arc<DaemonState>,
    ) -> Result<(), IpcError> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        // Read commands line by line (newline-delimited JSON)
        while reader.read_line(&mut line).await? > 0 {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                line.clear();
                continue;
            }

            // Parse and handle the command
            let response = match serde_json::from_str::<IpcCommand>(trimmed) {
                Ok(command) => Self::handle_command(command, &state).await,
                Err(e) => serde_json::json!({
                    "error": format!("Invalid command: {}", e)
                }),
            };

            // Send response
            let response_str = serde_json::to_string(&response)?;
            writer.write_all(response_str.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;

            line.clear();
        }

        Ok(())
    }

    /// Handle a single IPC command and return the response.
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
            } => {
                // Parse sensitivity
                let sensitivity_enum = match parse_sensitivity(&sensitivity) {
                    Ok(s) => s,
                    Err(e) => {
                        return serde_json::json!({
                            "success": false,
                            "error": e.to_string()
                        });
                    }
                };

                // Create new config
                let mut config = state.config_manager.get();
                config.min_hz = min_hz;
                config.max_hz = max_hz;
                config.sensitivity = sensitivity_enum;

                // Update and persist
                match state.config_manager.update(config) {
                    Ok(()) => {
                        // Update controller sensitivity
                        let mut controller = state.controller.write().await;
                        controller.set_sensitivity(sensitivity_enum);
                        tracing::info!(
                            "Config updated via IPC: min_hz={}, max_hz={}, sensitivity={}",
                            min_hz,
                            max_hz,
                            sensitivity
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

            IpcCommand::SetDeviceMode { mode } => {
                // Parse device mode
                let mode_enum = match parse_device_mode(&mode) {
                    Ok(m) => m,
                    Err(e) => {
                        return serde_json::json!({
                            "success": false,
                            "error": e.to_string()
                        });
                    }
                };

                // Apply mode constraints to controller
                let mut controller = state.controller.write().await;
                controller.apply_mode_constraints(mode_enum);
                
                tracing::info!(
                    "Device mode set via IPC: mode={}, min_change_interval={}ms",
                    mode,
                    if mode_enum == DeviceMode::Lcd { 2000 } else { 500 }
                );
                
                serde_json::json!({
                    "success": true,
                    "message": format!("Device mode set to {}", mode),
                    "mode": mode
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
        }
    }
}

#[cfg(unix)]
impl Drop for IpcServer {
    fn drop(&mut self) {
        // Clean up socket file on drop
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::tempdir;

    // Unit tests for IpcCommand serialization/deserialization
    #[test]
    fn test_ipc_command_start_serialization() {
        let cmd = IpcCommand::Start;
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"Start\""));

        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, IpcCommand::Start);
    }

    #[test]
    fn test_ipc_command_stop_serialization() {
        let cmd = IpcCommand::Stop;
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"Stop\""));

        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, IpcCommand::Stop);
    }

    #[test]
    fn test_ipc_command_set_config_serialization() {
        let cmd = IpcCommand::SetConfig {
            min_hz: 45,
            max_hz: 85,
            sensitivity: "aggressive".to_string(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"SetConfig\""));
        assert!(json.contains("\"min_hz\":45"));
        assert!(json.contains("\"max_hz\":85"));
        assert!(json.contains("\"sensitivity\":\"aggressive\""));

        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, cmd);
    }

    #[test]
    fn test_ipc_command_get_status_serialization() {
        let cmd = IpcCommand::GetStatus;
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"command\":\"GetStatus\""));

        let parsed: IpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, IpcCommand::GetStatus);
    }

    #[test]
    fn test_status_response_serialization() {
        let response = StatusResponse {
            running: true,
            current_fps: 58.5,
            current_hz: 60,
            state: "Stable".to_string(),
            device_mode: "oled".to_string(),
            config: ConfigResponse {
                min_hz: 40,
                max_hz: 90,
                sensitivity: "balanced".to_string(),
                enabled: true,
            },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"running\":true"));
        assert!(json.contains("\"current_fps\":58.5"));
        assert!(json.contains("\"current_hz\":60"));
        assert!(json.contains("\"state\":\"Stable\""));
        assert!(json.contains("\"device_mode\":\"oled\""));

        let parsed: StatusResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, response);
    }

    #[test]
    fn test_status_response_is_complete() {
        let valid_response = StatusResponse {
            running: true,
            current_fps: 60.0,
            current_hz: 60,
            state: "Stable".to_string(),
            device_mode: "oled".to_string(),
            config: ConfigResponse {
                min_hz: 40,
                max_hz: 90,
                sensitivity: "balanced".to_string(),
                enabled: true,
            },
        };
        assert!(valid_response.is_complete());

        // Invalid state
        let invalid_state = StatusResponse {
            state: "InvalidState".to_string(),
            ..valid_response.clone()
        };
        assert!(!invalid_state.is_complete());

        // Invalid sensitivity
        let invalid_sensitivity = StatusResponse {
            config: ConfigResponse {
                sensitivity: "invalid".to_string(),
                ..valid_response.config.clone()
            },
            ..valid_response.clone()
        };
        assert!(!invalid_sensitivity.is_complete());

        // Invalid device mode
        let invalid_mode = StatusResponse {
            device_mode: "invalid".to_string(),
            ..valid_response.clone()
        };
        assert!(!invalid_mode.is_complete());
    }

    #[test]
    fn test_parse_sensitivity() {
        assert_eq!(
            parse_sensitivity("conservative").unwrap(),
            Sensitivity::Conservative
        );
        assert_eq!(
            parse_sensitivity("balanced").unwrap(),
            Sensitivity::Balanced
        );
        assert_eq!(
            parse_sensitivity("aggressive").unwrap(),
            Sensitivity::Aggressive
        );
        assert_eq!(
            parse_sensitivity("BALANCED").unwrap(),
            Sensitivity::Balanced
        );
        assert!(parse_sensitivity("invalid").is_err());
    }

    #[test]
    fn test_sensitivity_to_string() {
        assert_eq!(
            sensitivity_to_string(Sensitivity::Conservative),
            "conservative"
        );
        assert_eq!(sensitivity_to_string(Sensitivity::Balanced), "balanced");
        assert_eq!(
            sensitivity_to_string(Sensitivity::Aggressive),
            "aggressive"
        );
    }

    #[test]
    fn test_algorithm_state_to_string() {
        assert_eq!(
            algorithm_state_to_string(AlgorithmState::Stable),
            "Stable"
        );
        assert_eq!(
            algorithm_state_to_string(AlgorithmState::Dropping {
                since: std::time::Instant::now()
            }),
            "Dropping"
        );
        assert_eq!(
            algorithm_state_to_string(AlgorithmState::Increasing {
                since: std::time::Instant::now()
            }),
            "Increasing"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_ipc_server_creation_and_cleanup() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");
        let path_str = socket_path.to_str().unwrap();

        // Create server
        let server = IpcServer::new(path_str).await.unwrap();
        assert!(socket_path.exists());

        // Drop server - should clean up socket
        drop(server);
        assert!(!socket_path.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_ipc_server_replaces_existing_socket() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");
        let path_str = socket_path.to_str().unwrap();

        // Create a file at the socket path
        std::fs::write(&socket_path, "dummy").unwrap();
        assert!(socket_path.exists());

        // Create server - should replace the file
        let server = IpcServer::new(path_str).await.unwrap();
        assert!(socket_path.exists());

        drop(server);
    }

    #[tokio::test]
    async fn test_daemon_state_start_stop() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_manager = Arc::new(ConfigManager::load_or_default(&config_path).unwrap());

        let state = DaemonState::new(config_manager);

        // Default should be running (enabled=true in default config)
        assert!(state.is_running());

        state.stop();
        assert!(!state.is_running());

        state.start();
        assert!(state.is_running());
    }

    #[tokio::test]
    async fn test_daemon_state_get_status() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_manager = Arc::new(ConfigManager::load_or_default(&config_path).unwrap());

        let state = DaemonState::new(config_manager);
        let status = state.get_status().await;

        assert!(status.running);
        assert_eq!(status.current_fps, 0.0);
        assert_eq!(status.current_hz, 90); // max_hz from default config
        assert_eq!(status.state, "Stable");
        assert_eq!(status.device_mode, "oled"); // default device mode
        assert_eq!(status.config.min_hz, 40);
        assert_eq!(status.config.max_hz, 90);
        assert_eq!(status.config.sensitivity, "balanced");
        assert!(status.config.enabled);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_handle_command_start() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_manager = Arc::new(ConfigManager::load_or_default(&config_path).unwrap());
        let state = Arc::new(DaemonState::new(config_manager));

        // Stop first
        state.stop();
        assert!(!state.is_running());

        // Handle Start command
        let response = IpcServer::handle_command(IpcCommand::Start, &state).await;
        assert!(response["success"].as_bool().unwrap());
        assert!(state.is_running());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_handle_command_stop() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_manager = Arc::new(ConfigManager::load_or_default(&config_path).unwrap());
        let state = Arc::new(DaemonState::new(config_manager));

        // Should be running by default
        assert!(state.is_running());

        // Handle Stop command
        let response = IpcServer::handle_command(IpcCommand::Stop, &state).await;
        assert!(response["success"].as_bool().unwrap());
        assert!(!state.is_running());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_handle_command_set_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_manager = Arc::new(ConfigManager::load_or_default(&config_path).unwrap());
        let state = Arc::new(DaemonState::new(config_manager));

        // Handle SetConfig command
        let response = IpcServer::handle_command(
            IpcCommand::SetConfig {
                min_hz: 50,
                max_hz: 80,
                sensitivity: "aggressive".to_string(),
            },
            &state,
        )
        .await;
        assert!(response["success"].as_bool().unwrap());

        // Verify config was updated
        let status = state.get_status().await;
        assert_eq!(status.config.min_hz, 50);
        assert_eq!(status.config.max_hz, 80);
        assert_eq!(status.config.sensitivity, "aggressive");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_handle_command_set_config_invalid_sensitivity() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_manager = Arc::new(ConfigManager::load_or_default(&config_path).unwrap());
        let state = Arc::new(DaemonState::new(config_manager));

        // Handle SetConfig with invalid sensitivity
        let response = IpcServer::handle_command(
            IpcCommand::SetConfig {
                min_hz: 50,
                max_hz: 80,
                sensitivity: "invalid".to_string(),
            },
            &state,
        )
        .await;
        assert!(!response["success"].as_bool().unwrap());
        assert!(response["error"].as_str().unwrap().contains("Invalid sensitivity"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_handle_command_get_status() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_manager = Arc::new(ConfigManager::load_or_default(&config_path).unwrap());
        let state = Arc::new(DaemonState::new(config_manager));

        // Handle GetStatus command
        let response = IpcServer::handle_command(IpcCommand::GetStatus, &state).await;
        
        // Verify response contains all required fields
        assert!(response["running"].is_boolean());
        assert!(response["current_fps"].is_number());
        assert!(response["current_hz"].is_number());
        assert!(response["state"].is_string());
        assert!(response["device_mode"].is_string());
        assert!(response["config"].is_object());
        assert!(response["config"]["min_hz"].is_number());
        assert!(response["config"]["max_hz"].is_number());
        assert!(response["config"]["sensitivity"].is_string());
        assert!(response["config"]["enabled"].is_boolean());
    }

    // Strategy to generate valid sensitivity strings
    fn sensitivity_string_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("conservative".to_string()),
            Just("balanced".to_string()),
            Just("aggressive".to_string()),
        ]
    }

    // Strategy to generate valid algorithm state strings
    fn state_string_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("Stable".to_string()),
            Just("Dropping".to_string()),
            Just("Increasing".to_string()),
        ]
    }

    // Strategy to generate valid device mode strings
    fn device_mode_string_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("oled".to_string()),
            Just("lcd".to_string()),
            Just("custom".to_string()),
        ]
    }

    // **Feature: smart-refresh-daemon, Property 8: Status Response Completeness**
    // **Validates: Requirements 5.5**
    //
    // For any daemon state, the status response JSON should contain all required fields:
    // running (boolean), current_fps (number), current_hz (number), state (string),
    // device_mode (string), and config (object with min_hz, max_hz, sensitivity, enabled).
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_status_response_contains_all_required_fields(
            running in any::<bool>(),
            current_fps in 0.0f64..=200.0f64,
            current_hz in 40u32..=90u32,
            state in state_string_strategy(),
            device_mode in device_mode_string_strategy(),
            min_hz in 40u32..=90u32,
            max_hz in 40u32..=90u32,
            sensitivity in sensitivity_string_strategy(),
            enabled in any::<bool>(),
        ) {
            // Ensure min <= max for valid config
            let (min_hz, max_hz) = if min_hz <= max_hz {
                (min_hz, max_hz)
            } else {
                (max_hz, min_hz)
            };

            let response = StatusResponse {
                running,
                current_fps,
                current_hz,
                state: state.clone(),
                device_mode: device_mode.clone(),
                config: ConfigResponse {
                    min_hz,
                    max_hz,
                    sensitivity: sensitivity.clone(),
                    enabled,
                },
            };

            // Serialize to JSON
            let json = serde_json::to_string(&response).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

            // Verify all required fields are present
            prop_assert!(parsed.get("running").is_some(), "Missing 'running' field");
            prop_assert!(parsed.get("current_fps").is_some(), "Missing 'current_fps' field");
            prop_assert!(parsed.get("current_hz").is_some(), "Missing 'current_hz' field");
            prop_assert!(parsed.get("state").is_some(), "Missing 'state' field");
            prop_assert!(parsed.get("device_mode").is_some(), "Missing 'device_mode' field");
            prop_assert!(parsed.get("config").is_some(), "Missing 'config' field");

            // Verify config sub-fields
            let config = parsed.get("config").unwrap();
            prop_assert!(config.get("min_hz").is_some(), "Missing 'config.min_hz' field");
            prop_assert!(config.get("max_hz").is_some(), "Missing 'config.max_hz' field");
            prop_assert!(config.get("sensitivity").is_some(), "Missing 'config.sensitivity' field");
            prop_assert!(config.get("enabled").is_some(), "Missing 'config.enabled' field");

            // Verify field types
            prop_assert!(parsed["running"].is_boolean(), "'running' should be boolean");
            prop_assert!(parsed["current_fps"].is_number(), "'current_fps' should be number");
            prop_assert!(parsed["current_hz"].is_number(), "'current_hz' should be number");
            prop_assert!(parsed["state"].is_string(), "'state' should be string");
            prop_assert!(parsed["device_mode"].is_string(), "'device_mode' should be string");
            prop_assert!(config["min_hz"].is_number(), "'config.min_hz' should be number");
            prop_assert!(config["max_hz"].is_number(), "'config.max_hz' should be number");
            prop_assert!(config["sensitivity"].is_string(), "'config.sensitivity' should be string");
            prop_assert!(config["enabled"].is_boolean(), "'config.enabled' should be boolean");

            // Verify values match
            prop_assert_eq!(parsed["running"].as_bool().unwrap(), running);
            prop_assert!((parsed["current_fps"].as_f64().unwrap() - current_fps).abs() < 0.001);
            prop_assert_eq!(parsed["current_hz"].as_u64().unwrap() as u32, current_hz);
            prop_assert_eq!(parsed["state"].as_str().unwrap(), state);
            prop_assert_eq!(parsed["device_mode"].as_str().unwrap(), device_mode);
            prop_assert_eq!(config["min_hz"].as_u64().unwrap() as u32, min_hz);
            prop_assert_eq!(config["max_hz"].as_u64().unwrap() as u32, max_hz);
            prop_assert_eq!(config["sensitivity"].as_str().unwrap(), sensitivity);
            prop_assert_eq!(config["enabled"].as_bool().unwrap(), enabled);

            // Verify is_complete returns true for valid responses
            prop_assert!(response.is_complete(), "Valid response should be complete");
        }

        #[test]
        fn prop_status_response_round_trip(
            running in any::<bool>(),
            current_fps in 0.0f64..=200.0f64,
            current_hz in 40u32..=90u32,
            state in state_string_strategy(),
            device_mode in device_mode_string_strategy(),
            min_hz in 40u32..=90u32,
            max_hz in 40u32..=90u32,
            sensitivity in sensitivity_string_strategy(),
            enabled in any::<bool>(),
        ) {
            let (min_hz, max_hz) = if min_hz <= max_hz {
                (min_hz, max_hz)
            } else {
                (max_hz, min_hz)
            };

            let original = StatusResponse {
                running,
                current_fps,
                current_hz,
                state,
                device_mode,
                config: ConfigResponse {
                    min_hz,
                    max_hz,
                    sensitivity,
                    enabled,
                },
            };

            // Serialize and deserialize
            let json = serde_json::to_string(&original).unwrap();
            let parsed: StatusResponse = serde_json::from_str(&json).unwrap();

            // Compare fields individually, using approximate comparison for f64
            prop_assert_eq!(original.running, parsed.running);
            prop_assert!((original.current_fps - parsed.current_fps).abs() < 1e-10,
                "current_fps mismatch: {} vs {}", original.current_fps, parsed.current_fps);
            prop_assert_eq!(original.current_hz, parsed.current_hz);
            prop_assert_eq!(original.state, parsed.state);
            prop_assert_eq!(original.device_mode, parsed.device_mode);
            prop_assert_eq!(original.config, parsed.config);
        }
    }
}
