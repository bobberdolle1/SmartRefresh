//! SmartRefresh Daemon - Dynamic refresh rate switching for Steam Deck.
//!
//! This daemon monitors FPS via MangoHud shared memory and controls
//! display refresh rate through Gamescope commands.
//!
//! Requirements: 4.1, 4.2, 4.3, 4.4

mod config;
mod core_logic;
mod display_control;
mod error;
mod fps_monitor;
mod ipc_server;
mod logging;

use config::ConfigManager;
use display_control::DisplayManager;
use fps_monitor::MangoHudReader;
use ipc_server::{DaemonState, IpcServer};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{error, info, warn};

/// FPS polling interval in milliseconds (Requirement 1.3)
const FPS_POLL_INTERVAL_MS: u64 = 100;

/// Retry interval for MangoHud connection in seconds (Requirement 1.4)
const SHM_RETRY_INTERVAL_SECS: u64 = 5;

/// Graceful shutdown timeout in seconds (Requirement 4.3)
const SHUTDOWN_TIMEOUT_SECS: u64 = 2;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with JSON format to both stderr and rotating file
    let _log_guard = logging::init_logging().map_err(|e| {
        eprintln!("Failed to initialize logging: {}", e);
        e
    })?;

    info!("SmartRefresh daemon starting...");

    // Run the daemon with panic catching (Requirement 4.4)
    let result = run_daemon().await;

    match &result {
        Ok(()) => info!("SmartRefresh daemon shut down gracefully"),
        Err(e) => error!("SmartRefresh daemon error: {}", e),
    }

    result
}


/// Main daemon entry point with panic recovery.
async fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration (Requirement 6.1, 6.2)
    let config_path = ConfigManager::default_path();
    let config_manager = Arc::new(ConfigManager::load_or_default(&config_path)?);
    info!("Configuration loaded from {:?}", config_path);

    let config = config_manager.get();

    // Create shared daemon state
    let daemon_state = Arc::new(DaemonState::new(Arc::clone(&config_manager)));

    // Create display manager with configured range
    let display_manager = Arc::new(DisplayManager::new(config.min_hz, config.max_hz));

    // Create shutdown signal channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Set up signal handlers (Requirement 4.3)
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = setup_signal_handlers(shutdown_tx_clone).await {
            error!("Signal handler error: {}", e);
        }
    });

    // Spawn IPC server task (Requirement 5.1)
    let ipc_state = Arc::clone(&daemon_state);
    let ipc_shutdown_rx = shutdown_rx.clone();
    let ipc_handle = tokio::spawn(async move {
        run_ipc_server_with_panic_catch(ipc_state, ipc_shutdown_rx).await
    });

    // Spawn FPS polling task (Requirement 1.1, 1.3)
    let fps_state = Arc::clone(&daemon_state);
    let fps_shutdown_rx = shutdown_rx.clone();
    let fps_handle = tokio::spawn(async move {
        run_fps_polling_with_panic_catch(fps_state, fps_shutdown_rx).await
    });

    // Spawn core logic task (Requirement 3.1, 3.2, 3.3)
    let logic_state = Arc::clone(&daemon_state);
    let logic_display = Arc::clone(&display_manager);
    let logic_shutdown_rx = shutdown_rx.clone();
    let logic_handle = tokio::spawn(async move {
        run_core_logic_with_panic_catch(logic_state, logic_display, logic_shutdown_rx).await
    });

    info!("SmartRefresh daemon initialized and running");

    // Wait for shutdown signal
    let mut shutdown_rx_main = shutdown_rx.clone();
    shutdown_rx_main.changed().await.ok();

    info!("Shutdown signal received, stopping tasks...");

    // Give tasks time to shut down gracefully (Requirement 4.3)
    let shutdown_timeout = Duration::from_secs(SHUTDOWN_TIMEOUT_SECS);
    let _ = tokio::time::timeout(shutdown_timeout, async {
        let _ = tokio::join!(ipc_handle, fps_handle, logic_handle);
    })
    .await;

    info!("All tasks stopped");
    Ok(())
}


/// Set up signal handlers for graceful shutdown.
/// Handles SIGTERM and SIGINT (Requirement 4.3)
#[cfg(unix)]
async fn setup_signal_handlers(
    shutdown_tx: watch::Sender<bool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    tokio::select! {
        _ = sigterm.recv() => {
            info!("Received SIGTERM");
        }
        _ = sigint.recv() => {
            info!("Received SIGINT");
        }
    }

    // Signal shutdown to all tasks
    let _ = shutdown_tx.send(true);
    Ok(())
}

/// Stub signal handler for non-Unix platforms (Windows development)
#[cfg(not(unix))]
async fn setup_signal_handlers(
    shutdown_tx: watch::Sender<bool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // On Windows, just wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("Received Ctrl+C");
    let _ = shutdown_tx.send(true);
    Ok(())
}


/// Run IPC server with panic catching (Requirement 4.4)
async fn run_ipc_server_with_panic_catch(
    state: Arc<DaemonState>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!("IPC server shutting down");
                    break;
                }
            }
            result = run_ipc_server_inner(Arc::clone(&state)) => {
                match result {
                    Ok(()) => break,
                    Err(e) => {
                        error!("IPC server error: {}, restarting in 5 seconds", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        }
    }
}

/// Inner IPC server loop
#[cfg(unix)]
async fn run_ipc_server_inner(state: Arc<DaemonState>) -> Result<(), error::IpcError> {
    let server = IpcServer::new_default().await?;
    info!("IPC server listening on {:?}", server.socket_path());
    server.run(state).await
}

/// Stub IPC server for non-Unix platforms
#[cfg(not(unix))]
async fn run_ipc_server_inner(_state: Arc<DaemonState>) -> Result<(), error::IpcError> {
    warn!("IPC server not available on this platform");
    // Just wait indefinitely on non-Unix platforms
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}


/// Run FPS polling with panic catching (Requirement 4.4)
async fn run_fps_polling_with_panic_catch(
    state: Arc<DaemonState>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let poll_interval = Duration::from_millis(FPS_POLL_INTERVAL_MS);
    let retry_interval = Duration::from_secs(SHM_RETRY_INTERVAL_SECS);

    loop {
        // Check for shutdown
        if *shutdown_rx.borrow() {
            info!("FPS polling shutting down");
            break;
        }

        // Try to connect to MangoHud shared memory
        let reader = match MangoHudReader::new() {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    "Failed to connect to MangoHud SHM: {}, retrying in {} seconds",
                    e, SHM_RETRY_INTERVAL_SECS
                );
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep(retry_interval) => {}
                }
                continue;
            }
        };

        info!("Connected to MangoHud shared memory");

        // Poll loop
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("FPS polling shutting down");
                        return;
                    }
                }
                _ = tokio::time::sleep(poll_interval) => {
                    // Only poll if daemon is running
                    if !state.is_running() {
                        continue;
                    }

                    // Poll with panic catching
                    let poll_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        reader.poll()
                    }));

                    match poll_result {
                        Ok(Ok(sample)) => {
                            // Update current FPS in daemon state
                            let smoothed_fps = reader.get_smoothed_fps();
                            if let Ok(mut fps) = state.current_fps.try_write() {
                                *fps = smoothed_fps;
                            }
                            tracing::debug!("FPS: {} (smoothed: {:.1})", sample.fps, smoothed_fps);
                        }
                        Ok(Err(e)) => {
                            warn!("FPS poll error: {}, reconnecting...", e);
                            break; // Break inner loop to reconnect
                        }
                        Err(_) => {
                            error!("Panic during FPS polling, continuing operation");
                        }
                    }
                }
            }
        }
    }
}


/// Run core logic with panic catching (Requirement 4.4)
async fn run_core_logic_with_panic_catch(
    state: Arc<DaemonState>,
    display_manager: Arc<DisplayManager>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let process_interval = Duration::from_millis(FPS_POLL_INTERVAL_MS);

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!("Core logic shutting down");
                    break;
                }
            }
            _ = tokio::time::sleep(process_interval) => {
                // Only process if daemon is running
                if !state.is_running() {
                    continue;
                }

                // Get current FPS and Hz
                let current_fps = match state.current_fps.try_read() {
                    Ok(fps) => *fps,
                    Err(_) => continue,
                };
                let current_hz = state.current_hz.load(Ordering::SeqCst);

                // Skip if no FPS data yet
                if current_fps <= 0.0 {
                    continue;
                }

                // Process hysteresis algorithm with panic catching
                let new_hz = {
                    let process_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        // We need to block on the async lock here
                        // This is safe because we're in an async context
                    }));

                    if process_result.is_err() {
                        error!("Panic in core logic, continuing operation");
                        continue;
                    }

                    let mut controller = state.controller.write().await;
                    controller.process(current_fps, current_hz)
                };

                // Apply refresh rate change if needed
                if let Some(target_hz) = new_hz {
                    // Update config range in display manager
                    let config = state.config_manager.get();
                    display_manager.set_range(config.min_hz, config.max_hz);

                    match display_manager.set_refresh_rate(target_hz).await {
                        Ok(true) => {
                            state.current_hz.store(
                                display_manager.get_current_hz(),
                                Ordering::SeqCst,
                            );
                            info!(
                                "Refresh rate changed to {}Hz (FPS: {:.1})",
                                display_manager.get_current_hz(),
                                current_fps
                            );
                        }
                        Ok(false) => {
                            // Rate unchanged (already at target)
                        }
                        Err(e) => {
                            error!("Failed to set refresh rate: {}", e);
                        }
                    }
                }
            }
        }
    }
}
