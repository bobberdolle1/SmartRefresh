//! SmartRefresh Daemon v2.0 - Dynamic refresh rate switching for Steam Deck.
//!
//! This daemon monitors FPS via MangoHud shared memory and controls
//! display refresh rate through Gamescope commands.
//!
//! v2.0 Features:
//! - D-Bus suspend/resume handling
//! - Per-game profiles
//! - Battery tracking and savings estimation
//! - Metrics collection
//! - Multi-monitor detection
//! - Adaptive sensitivity

mod config;
mod core_logic;
mod display_control;
mod error;
mod fps_monitor;
mod ipc_server;
mod logging;
mod metrics;
mod profiles;
mod battery;
mod monitor_detect;

use config::ConfigManager;
use display_control::DisplayManager;
use fps_monitor::MangoHudReader;
use ipc_server::DaemonState;
use metrics::MetricsCollector;
use profiles::ProfileManager;
use battery::BatteryMonitor;
use monitor_detect::MonitorDetector;

#[cfg(unix)]
use ipc_server::IpcServer;

use std::panic::AssertUnwindSafe;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{error, info, warn, debug};

/// FPS polling interval in milliseconds
const FPS_POLL_INTERVAL_MS: u64 = 100;

/// Retry interval for MangoHud connection in seconds
const SHM_RETRY_INTERVAL_SECS: u64 = 5;

/// Graceful shutdown timeout in seconds
const SHUTDOWN_TIMEOUT_SECS: u64 = 2;

/// Monitor detection interval in seconds
const MONITOR_CHECK_INTERVAL_SECS: u64 = 10;

/// Battery polling interval in seconds
const BATTERY_POLL_INTERVAL_SECS: u64 = 5;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let _log_guard = logging::init_logging().map_err(|e| {
        eprintln!("Failed to initialize logging: {}", e);
        e
    })?;

    info!("SmartRefresh daemon v2.0 starting...");

    let result = run_daemon().await;

    match &result {
        Ok(()) => info!("SmartRefresh daemon shut down gracefully"),
        Err(e) => error!("SmartRefresh daemon error: {}", e),
    }

    result
}

/// Main daemon entry point with panic recovery.
async fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config_path = ConfigManager::default_path();
    let config_manager = Arc::new(ConfigManager::load_or_default(&config_path)?);
    info!("Configuration loaded from {:?}", config_path);

    let config = config_manager.get();

    // Load profile manager
    let profile_manager = Arc::new(tokio::sync::RwLock::new(
        ProfileManager::load_or_default().unwrap_or_default()
    ));
    info!("Profile manager initialized");

    // Create metrics collector
    let metrics = Arc::new(MetricsCollector::new());

    // Create battery monitor
    let battery_monitor = Arc::new(BatteryMonitor::new());

    // Create monitor detector
    let monitor_detector = Arc::new(MonitorDetector::new());

    // Create shared daemon state
    let daemon_state = Arc::new(DaemonState::new(
        Arc::clone(&config_manager),
        Arc::clone(&profile_manager),
        Arc::clone(&metrics),
        Arc::clone(&battery_monitor),
    ));

    // Create display manager with configured range
    let display_manager = Arc::new(DisplayManager::new(config.min_hz, config.max_hz));

    // Create shutdown signal channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Set up signal handlers
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = setup_signal_handlers(shutdown_tx_clone).await {
            error!("Signal handler error: {}", e);
        }
    });

    // Spawn D-Bus suspend/resume monitor
    #[cfg(unix)]
    {
        let dbus_state = Arc::clone(&daemon_state);
        let dbus_shutdown_rx = shutdown_rx.clone();
        tokio::spawn(async move {
            run_dbus_monitor(dbus_state, dbus_shutdown_rx).await;
        });
    }

    // Spawn IPC server task
    let ipc_state = Arc::clone(&daemon_state);
    let ipc_shutdown_rx = shutdown_rx.clone();
    let ipc_handle = tokio::spawn(async move {
        run_ipc_server_with_panic_catch(ipc_state, ipc_shutdown_rx).await
    });

    // Spawn FPS polling task
    let fps_state = Arc::clone(&daemon_state);
    let fps_shutdown_rx = shutdown_rx.clone();
    let fps_handle = tokio::spawn(async move {
        run_fps_polling_with_panic_catch(fps_state, fps_shutdown_rx).await
    });

    // Spawn core logic task
    let logic_state = Arc::clone(&daemon_state);
    let logic_display = Arc::clone(&display_manager);
    let logic_metrics = Arc::clone(&metrics);
    let logic_shutdown_rx = shutdown_rx.clone();
    let logic_handle = tokio::spawn(async move {
        run_core_logic_with_panic_catch(logic_state, logic_display, logic_metrics, logic_shutdown_rx).await
    });

    // Spawn monitor detection task
    let monitor_state = Arc::clone(&daemon_state);
    let monitor_detector_clone = Arc::clone(&monitor_detector);
    let monitor_shutdown_rx = shutdown_rx.clone();
    tokio::spawn(async move {
        run_monitor_detection(monitor_state, monitor_detector_clone, monitor_shutdown_rx).await
    });

    // Spawn battery monitoring task
    let battery_state = Arc::clone(&daemon_state);
    let battery_monitor_clone = Arc::clone(&battery_monitor);
    let battery_shutdown_rx = shutdown_rx.clone();
    tokio::spawn(async move {
        run_battery_monitoring(battery_state, battery_monitor_clone, battery_shutdown_rx).await
    });

    info!("SmartRefresh daemon v2.0 initialized and running");

    // Wait for shutdown signal
    let mut shutdown_rx_main = shutdown_rx.clone();
    shutdown_rx_main.changed().await.ok();

    info!("Shutdown signal received, stopping tasks...");

    // Give tasks time to shut down gracefully
    let shutdown_timeout = Duration::from_secs(SHUTDOWN_TIMEOUT_SECS);
    let _ = tokio::time::timeout(shutdown_timeout, async {
        let _ = tokio::join!(ipc_handle, fps_handle, logic_handle);
    })
    .await;

    info!("All tasks stopped");
    Ok(())
}

/// Set up signal handlers for graceful shutdown.
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

    let _ = shutdown_tx.send(true);
    Ok(())
}

#[cfg(not(unix))]
async fn setup_signal_handlers(
    shutdown_tx: watch::Sender<bool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tokio::signal::ctrl_c().await?;
    info!("Received Ctrl+C");
    let _ = shutdown_tx.send(true);
    Ok(())
}

/// Monitor D-Bus for suspend/resume events
#[cfg(unix)]
async fn run_dbus_monitor(
    state: Arc<DaemonState>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    use zbus::Connection;

    info!("Starting D-Bus suspend/resume monitor");

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!("D-Bus monitor shutting down");
                    break;
                }
            }
            result = monitor_sleep_signals(&state) => {
                if let Err(e) = result {
                    warn!("D-Bus monitor error: {}, retrying in 5s", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }
}

#[cfg(unix)]
async fn monitor_sleep_signals(state: &Arc<DaemonState>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use zbus::Connection;
    use futures_util::StreamExt;

    let connection = Connection::system().await?;
    
    // Subscribe to PrepareForSleep signal
    let proxy = zbus::fdo::DBusProxy::new(&connection).await?;
    
    // Use match rule for login1 PrepareForSleep
    connection.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "AddMatch",
        &("type='signal',interface='org.freedesktop.login1.Manager',member='PrepareForSleep'",),
    ).await?;

    info!("Subscribed to PrepareForSleep D-Bus signal");

    // Listen for signals
    let mut stream = zbus::MessageStream::from(&connection);
    
    while let Some(msg) = stream.next().await {
        if let Ok(msg) = msg {
            if msg.member().map(|m| m.as_str()) == Some("PrepareForSleep") {
                // Parse the boolean argument (true = going to sleep, false = waking up)
                if let Ok(body) = msg.body() {
                    let going_to_sleep: bool = body.deserialize()?;
                    
                    if going_to_sleep {
                        info!("System going to sleep");
                    } else {
                        info!("System waking up - resetting hysteresis state");
                        // Reset controller state on resume
                        let mut controller = state.controller.write().await;
                        controller.reset_state();
                        drop(controller);
                        info!("Hysteresis controller reset after resume");
                    }
                }
            }
        }
    }

    Ok(())
}

/// Run IPC server with panic catching
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

#[cfg(unix)]
async fn run_ipc_server_inner(state: Arc<DaemonState>) -> Result<(), error::IpcError> {
    let server = IpcServer::new_default().await?;
    info!("IPC server listening on {:?}", server.socket_path());
    server.run(state).await
}

#[cfg(not(unix))]
async fn run_ipc_server_inner(_state: Arc<DaemonState>) -> Result<(), error::IpcError> {
    warn!("IPC server not available on this platform");
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}

/// Run FPS polling with panic catching and MangoHud fallback
async fn run_fps_polling_with_panic_catch(
    state: Arc<DaemonState>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let poll_interval = Duration::from_millis(FPS_POLL_INTERVAL_MS);
    let retry_interval = Duration::from_secs(SHM_RETRY_INTERVAL_SECS);

    loop {
        if *shutdown_rx.borrow() {
            info!("FPS polling shutting down");
            break;
        }

        // Try to connect to MangoHud shared memory with fallback
        let reader = match MangoHudReader::new() {
            Ok(r) => Some(r),
            Err(e) => {
                // MangoHud fallback: log warning but keep daemon alive
                warn!("MangoHud not active: {}. Running in fallback mode.", e);
                state.set_mangohud_available(false);
                
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

        if let Some(reader) = reader {
            info!("Connected to MangoHud shared memory");
            state.set_mangohud_available(true);

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
                        if !state.is_running() {
                            continue;
                        }

                        let poll_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                            reader.poll()
                        }));

                        match poll_result {
                            Ok(Ok(sample)) => {
                                let smoothed_fps = reader.get_smoothed_fps();
                                if let Ok(mut fps) = state.current_fps.try_write() {
                                    *fps = smoothed_fps;
                                }
                                debug!("FPS: {} (smoothed: {:.1})", sample.fps, smoothed_fps);
                            }
                            Ok(Err(e)) => {
                                warn!("FPS poll error: {}, reconnecting...", e);
                                state.set_mangohud_available(false);
                                break;
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
}

/// Run core logic with panic catching
async fn run_core_logic_with_panic_catch(
    state: Arc<DaemonState>,
    display_manager: Arc<DisplayManager>,
    metrics: Arc<MetricsCollector>,
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
                if !state.is_running() {
                    continue;
                }

                let current_fps = match state.current_fps.try_read() {
                    Ok(fps) => *fps,
                    Err(_) => continue,
                };
                let current_hz = state.current_hz.load(Ordering::SeqCst);

                if current_fps <= 0.0 {
                    continue;
                }

                // Process hysteresis algorithm
                let new_hz = {
                    let process_result = std::panic::catch_unwind(AssertUnwindSafe(|| {}));

                    if process_result.is_err() {
                        error!("Panic in core logic, continuing operation");
                        continue;
                    }

                    let mut controller = state.controller.write().await;
                    controller.process(current_fps, current_hz)
                };

                // Apply refresh rate change if needed
                if let Some(target_hz) = new_hz {
                    let config = state.config_manager.get();
                    display_manager.set_range(config.min_hz, config.max_hz);

                    let old_hz = display_manager.get_current_hz();
                    
                    match display_manager.set_refresh_rate(target_hz).await {
                        Ok(true) => {
                            let new_hz_actual = display_manager.get_current_hz();
                            state.current_hz.store(new_hz_actual, Ordering::SeqCst);
                            
                            // Record metrics
                            metrics.record_switch(old_hz, new_hz_actual);
                            
                            // Record transition for UI
                            state.record_transition(old_hz, new_hz_actual, current_fps).await;
                            
                            info!(
                                "Refresh rate changed: {}Hz → {}Hz (FPS: {:.1})",
                                old_hz, new_hz_actual, current_fps
                            );
                        }
                        Ok(false) => {}
                        Err(e) => {
                            error!("Failed to set refresh rate: {}", e);
                        }
                    }
                }
            }
        }
    }
}

/// Run monitor detection task
async fn run_monitor_detection(
    state: Arc<DaemonState>,
    detector: Arc<MonitorDetector>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let check_interval = Duration::from_secs(MONITOR_CHECK_INTERVAL_SECS);

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!("Monitor detection shutting down");
                    break;
                }
            }
            _ = tokio::time::sleep(check_interval) => {
                let external_detected = detector.has_external_display().await;
                
                let mut controller = state.controller.write().await;
                let was_detected = controller.is_external_display_detected();
                
                if external_detected != was_detected {
                    controller.set_external_display_detected(external_detected);
                    if external_detected {
                        info!("External display detected - Pausing SmartRefresh");
                    } else {
                        info!("External display disconnected - Resuming SmartRefresh");
                    }
                }
            }
        }
    }
}

/// Run battery monitoring task
async fn run_battery_monitoring(
    state: Arc<DaemonState>,
    monitor: Arc<BatteryMonitor>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let poll_interval = Duration::from_secs(BATTERY_POLL_INTERVAL_SECS);

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!("Battery monitoring shutting down");
                    break;
                }
            }
            _ = tokio::time::sleep(poll_interval) => {
                if let Some(power_uw) = monitor.read_power_now() {
                    monitor.record_sample(power_uw, state.current_hz.load(Ordering::SeqCst));
                }
            }
        }
    }
}
