//! Error types for the SmartRefresh daemon v2.0.
//!
//! This module defines custom error enums for each component of the daemon,
//! providing descriptive error messages with context information.

use thiserror::Error;

/// Errors related to shared memory operations for FPS monitoring.
#[derive(Error, Debug)]
pub enum ShmError {
    #[error("Failed to open shared memory segment '{name}': {source}")]
    OpenFailed {
        name: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to map shared memory: {0}")]
    MmapFailed(std::io::Error),

    #[error("Invalid data read from shared memory: {0}")]
    InvalidData(String),

    #[error("Shared memory segment not available, MangoHud may not be running")]
    NotAvailable,
}

/// Errors related to profile management.
#[derive(Error, Debug)]
pub enum ProfileError {
    #[error("Failed to load profiles: {0}")]
    LoadFailed(String),

    #[error("Failed to save profiles: {0}")]
    SaveFailed(String),

    #[error("Profile not found for app_id: {0}")]
    NotFound(String),
}

/// Errors related to display control operations.
#[derive(Error, Debug)]
pub enum DisplayError {
    #[error("gamescope-cmd not found in PATH")]
    CommandNotFound,

    #[error("gamescope-cmd failed with exit code {exit_code:?}: {stderr}")]
    CommandFailed {
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("Requested refresh rate {requested}Hz is outside valid range [{min}-{max}Hz]")]
    RateOutOfRange { requested: u32, min: u32, max: u32 },

    #[error("Failed to execute command: {0}")]
    ExecutionFailed(#[from] std::io::Error),
}

/// Errors related to IPC server operations.
#[derive(Error, Debug)]
pub enum IpcError {
    #[error("Failed to bind socket at '{path}': {source}")]
    SocketBindFailed {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Client connection dropped unexpectedly")]
    ConnectionDropped,

    #[error("Invalid command received: {0}")]
    InvalidCommand(String),

    #[error("Failed to serialize response: {0}")]
    SerializationFailed(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Errors related to configuration management.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration file not found at '{0}'")]
    FileNotFound(String),

    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    #[error("Configuration validation failed: {0}")]
    ValidationError(String),

    #[error("Failed to write configuration: {0}")]
    WriteError(#[from] std::io::Error),
}

/// Top-level daemon errors.
#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Shared memory error: {0}")]
    Shm(#[from] ShmError),

    #[error("Display control error: {0}")]
    Display(#[from] DisplayError),

    #[error("IPC error: {0}")]
    Ipc(#[from] IpcError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Runtime error: {0}")]
    Runtime(String),
}
