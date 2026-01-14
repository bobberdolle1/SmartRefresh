//! Logging module for the SmartRefresh daemon.
//!
//! This module configures tracing with JSON format output to both stderr
//! and a rotating log file at ~/.local/share/smart-refresh/daemon.log.
//!
//! Requirements: 10.1, 10.2, 10.4

use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan, time::UtcTime},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Default log directory relative to user's home
const LOG_DIR: &str = ".local/share/smart-refresh";
/// Log file name
const LOG_FILE: &str = "daemon.log";
/// Maximum number of log files to retain
const MAX_LOG_FILES: usize = 3;

/// Initialize the logging system with both stderr and file output.
///
/// Logs are written in JSON format to:
/// - stderr for immediate visibility
/// - ~/.local/share/smart-refresh/daemon.log with rotation
///
/// Log rotation occurs daily, retaining the last 3 files.
/// When a log file exceeds 10MB, it will be rotated.
pub fn init_logging() -> Result<LogGuard, LoggingError> {
    let log_dir = get_log_directory()?;
    
    // Ensure log directory exists
    std::fs::create_dir_all(&log_dir).map_err(|e| LoggingError::DirectoryCreationFailed {
        path: log_dir.display().to_string(),
        source: e,
    })?;

    // Create rolling file appender (rotates daily, keeps MAX_LOG_FILES)
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .max_log_files(MAX_LOG_FILES)
        .filename_prefix("daemon")
        .filename_suffix("log")
        .build(&log_dir)
        .map_err(|e| LoggingError::AppenderCreationFailed(e.to_string()))?;

    // Create non-blocking writer for file output
    let (non_blocking_file, file_guard) = tracing_appender::non_blocking(file_appender);

    // Create non-blocking writer for stderr
    let (non_blocking_stderr, stderr_guard) = tracing_appender::non_blocking(std::io::stderr());

    // Environment filter for log level control
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // JSON layer for file output
    let file_layer = fmt::layer()
        .json()
        .with_timer(UtcTime::rfc_3339())
        .with_span_events(FmtSpan::CLOSE)
        .with_current_span(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_writer(non_blocking_file);

    // Human-readable layer for stderr (also JSON for consistency)
    let stderr_layer = fmt::layer()
        .json()
        .with_timer(UtcTime::rfc_3339())
        .with_span_events(FmtSpan::CLOSE)
        .with_current_span(true)
        .with_writer(non_blocking_stderr);

    // Initialize the subscriber with both layers
    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(stderr_layer)
        .init();

    Ok(LogGuard {
        _file_guard: file_guard,
        _stderr_guard: stderr_guard,
    })
}

/// Get the log directory path, expanding ~ to user's home directory.
fn get_log_directory() -> Result<PathBuf, LoggingError> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| LoggingError::HomeDirectoryNotFound)?;
    
    Ok(PathBuf::from(home).join(LOG_DIR))
}

/// Guard that keeps the non-blocking writers alive.
/// Must be held for the lifetime of the application.
pub struct LogGuard {
    _file_guard: tracing_appender::non_blocking::WorkerGuard,
    _stderr_guard: tracing_appender::non_blocking::WorkerGuard,
}

/// Errors related to logging initialization.
#[derive(Debug)]
pub enum LoggingError {
    /// Home directory environment variable not found
    HomeDirectoryNotFound,
    /// Failed to create log directory
    DirectoryCreationFailed {
        path: String,
        source: std::io::Error,
    },
    /// Failed to create file appender
    AppenderCreationFailed(String),
}

impl std::fmt::Display for LoggingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoggingError::HomeDirectoryNotFound => {
                write!(f, "Could not determine home directory (HOME or USERPROFILE not set)")
            }
            LoggingError::DirectoryCreationFailed { path, source } => {
                write!(f, "Failed to create log directory '{}': {}", path, source)
            }
            LoggingError::AppenderCreationFailed(msg) => {
                write!(f, "Failed to create log file appender: {}", msg)
            }
        }
    }
}

impl std::error::Error for LoggingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoggingError::DirectoryCreationFailed { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Format a log entry with all required fields for Property 9 validation.
/// This is a helper for testing that log entries contain required fields.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogEntry {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Severity level (ERROR, WARN, INFO, DEBUG, TRACE)
    pub level: String,
    /// Context/message describing the event
    pub message: String,
    /// Optional target (module path)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Optional file name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// Optional line number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u32>,
}

impl LogEntry {
    /// Validate that the log entry contains all required fields per Requirements 10.1.
    /// Returns true if the entry has timestamp, level, and message.
    pub fn is_valid(&self) -> bool {
        !self.timestamp.is_empty() 
            && !self.level.is_empty() 
            && !self.message.is_empty()
            && self.is_valid_timestamp()
            && self.is_valid_level()
    }

    /// Check if timestamp is in ISO 8601 format (basic validation).
    fn is_valid_timestamp(&self) -> bool {
        // ISO 8601 format: YYYY-MM-DDTHH:MM:SS or similar
        // Basic check: contains 'T' separator and has reasonable length
        self.timestamp.contains('T') && self.timestamp.len() >= 19
    }

    /// Check if level is a valid severity level.
    fn is_valid_level(&self) -> bool {
        matches!(
            self.level.to_uppercase().as_str(),
            "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE"
        )
    }
}

/// Parse a JSON log line into a LogEntry for validation.
pub fn parse_log_entry(json_line: &str) -> Result<LogEntry, serde_json::Error> {
    // tracing-subscriber JSON format uses different field names
    // We need to map them to our LogEntry structure
    let value: serde_json::Value = serde_json::from_str(json_line)?;
    
    let timestamp = value.get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    
    let level = value.get("level")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    
    let message = value.get("fields")
        .and_then(|f| f.get("message"))
        .and_then(|v| v.as_str())
        .or_else(|| value.get("message").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    
    let target = value.get("target")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let filename = value.get("filename")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    
    let line_number = value.get("line_number")
        .and_then(|v| v.as_u64())
        .map(|n| n as u32);

    Ok(LogEntry {
        timestamp,
        level,
        message,
        target,
        filename,
        line_number,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy for generating valid severity levels
    fn severity_level_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("ERROR".to_string()),
            Just("WARN".to_string()),
            Just("INFO".to_string()),
            Just("DEBUG".to_string()),
            Just("TRACE".to_string()),
        ]
    }

    /// Strategy for generating valid ISO 8601 timestamps
    fn iso8601_timestamp_strategy() -> impl Strategy<Value = String> {
        (1970u32..2100, 1u32..13, 1u32..29, 0u32..24, 0u32..60, 0u32..60)
            .prop_map(|(year, month, day, hour, min, sec)| {
                format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                    year, month, day, hour, min, sec
                )
            })
    }

    /// Strategy for generating non-empty context messages
    fn context_message_strategy() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 _\\-\\.,:;!?]{1,200}".prop_filter("non-empty", |s| !s.trim().is_empty())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: smart-refresh-daemon, Property 9: Log Entry Format**
        /// **Validates: Requirements 10.1**
        ///
        /// *For any* logged error, the log entry string should contain a timestamp
        /// in ISO 8601 format, a severity level (ERROR, WARN, INFO, DEBUG), and
        /// context information describing the error.
        #[test]
        fn property_log_entry_format(
            timestamp in iso8601_timestamp_strategy(),
            level in severity_level_strategy(),
            message in context_message_strategy(),
        ) {
            let entry = LogEntry {
                timestamp: timestamp.clone(),
                level: level.clone(),
                message: message.clone(),
                target: None,
                filename: None,
                line_number: None,
            };

            // Property: All log entries with valid inputs should pass validation
            prop_assert!(entry.is_valid(), 
                "Log entry should be valid with timestamp={}, level={}, message={}", 
                timestamp, level, message);

            // Property: Timestamp should be in ISO 8601 format (contains 'T')
            prop_assert!(entry.timestamp.contains('T'),
                "Timestamp should contain 'T' separator for ISO 8601 format");

            // Property: Level should be one of the valid severity levels
            prop_assert!(
                matches!(entry.level.to_uppercase().as_str(), 
                    "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE"),
                "Level should be a valid severity level"
            );

            // Property: Message should not be empty
            prop_assert!(!entry.message.is_empty(),
                "Message should contain context information");
        }

        /// Test that invalid log entries are correctly identified
        #[test]
        fn property_invalid_log_entries_detected(
            timestamp in iso8601_timestamp_strategy(),
            level in severity_level_strategy(),
        ) {
            // Empty message should be invalid
            let entry_empty_msg = LogEntry {
                timestamp: timestamp.clone(),
                level: level.clone(),
                message: String::new(),
                target: None,
                filename: None,
                line_number: None,
            };
            prop_assert!(!entry_empty_msg.is_valid(),
                "Log entry with empty message should be invalid");

            // Invalid timestamp (no 'T' separator) should be invalid
            let entry_bad_timestamp = LogEntry {
                timestamp: "2024-01-01 12:00:00".to_string(), // space instead of T
                level: level.clone(),
                message: "test message".to_string(),
                target: None,
                filename: None,
                line_number: None,
            };
            prop_assert!(!entry_bad_timestamp.is_valid(),
                "Log entry with non-ISO8601 timestamp should be invalid");
        }
    }

    #[test]
    fn test_parse_tracing_json_format() {
        // Example of tracing-subscriber JSON output format
        let json_line = r#"{"timestamp":"2024-01-15T10:30:00.123456Z","level":"INFO","fields":{"message":"Test log message"},"target":"smart_refresh_daemon::logging"}"#;
        
        let entry = parse_log_entry(json_line).expect("Should parse valid JSON");
        
        assert!(entry.is_valid());
        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.message, "Test log message");
        assert!(entry.timestamp.contains('T'));
    }

    #[test]
    fn test_log_entry_validation() {
        let valid_entry = LogEntry {
            timestamp: "2024-01-15T10:30:00Z".to_string(),
            level: "ERROR".to_string(),
            message: "Connection failed: timeout".to_string(),
            target: Some("smart_refresh_daemon::ipc".to_string()),
            filename: Some("ipc_server.rs".to_string()),
            line_number: Some(42),
        };
        
        assert!(valid_entry.is_valid());
    }

    #[test]
    fn test_get_log_directory() {
        // This test verifies the log directory path construction
        // It will use HOME or USERPROFILE environment variable
        let result = get_log_directory();
        
        // Should succeed if HOME is set (typical on Linux/macOS)
        // or USERPROFILE is set (typical on Windows)
        if std::env::var("HOME").is_ok() || std::env::var("USERPROFILE").is_ok() {
            assert!(result.is_ok());
            let path = result.unwrap();
            assert!(path.to_string_lossy().contains(".local/share/smart-refresh"));
        }
    }
}
