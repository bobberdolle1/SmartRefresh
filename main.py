"""
SmartRefresh Decky Loader Plugin - Python Wrapper

This module manages the Rust daemon lifecycle and provides IPC proxy methods
for the React frontend to communicate with the daemon.
"""

import asyncio
import os
import signal
import socket
import subprocess
import json
import stat
from pathlib import Path
from typing import Optional, Dict, Any

import decky  # type: ignore

# Constants
DAEMON_BINARY_NAME = "smart-refresh-daemon"
SOCKET_PATH = "/tmp/smart-refresh.sock"
SOCKET_TIMEOUT = 2.0
SHUTDOWN_TIMEOUT = 2.0

defaultDir = os.environ.get("DECKY_PLUGIN_DIR")


class Plugin:
    """SmartRefresh Decky Loader Plugin."""
    
    def __init__(self):
        self._daemon_process: Optional[subprocess.Popen] = None
        self._daemon_pid: Optional[int] = None
    
    def _get_binary_path(self) -> Path:
        """Get the path to the daemon binary."""
        plugin_dir = Path(defaultDir) if defaultDir else Path(".")
        return plugin_dir / "bin" / DAEMON_BINARY_NAME
    
    def _ensure_executable(self, binary_path: Path) -> bool:
        """Ensure the daemon binary has execute permissions."""
        try:
            if not binary_path.exists():
                decky.logger.error(f"Daemon binary not found at {binary_path}")
                return False
            
            current_mode = binary_path.stat().st_mode
            if not (current_mode & stat.S_IXUSR):
                new_mode = current_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH
                binary_path.chmod(new_mode)
                decky.logger.info(f"Set execute permissions on {binary_path}")
            
            return True
        except Exception as e:
            decky.logger.error(f"Failed to set execute permissions: {e}")
            return False

    def _spawn_daemon(self) -> bool:
        """Spawn the daemon as a subprocess."""
        try:
            binary_path = self._get_binary_path()
            
            if not self._ensure_executable(binary_path):
                return False
            
            self._daemon_process = subprocess.Popen(
                [str(binary_path)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
                cwd=defaultDir
            )
            self._daemon_pid = self._daemon_process.pid
            
            decky.logger.info(f"Spawned daemon with PID {self._daemon_pid}")
            return True
        except Exception as e:
            decky.logger.error(f"Failed to spawn daemon: {e}")
            self._daemon_process = None
            self._daemon_pid = None
            return False
    
    def _stop_daemon(self) -> bool:
        """Stop the daemon process gracefully."""
        if self._daemon_process is None and self._daemon_pid is None:
            decky.logger.info("No daemon process to stop")
            return True
        
        try:
            pid = self._daemon_pid or (self._daemon_process.pid if self._daemon_process else None)
            
            if pid is None:
                return True
            
            try:
                os.kill(pid, signal.SIGTERM)
                decky.logger.info(f"Sent SIGTERM to daemon PID {pid}")
            except ProcessLookupError:
                decky.logger.info(f"Daemon PID {pid} already terminated")
                self._daemon_process = None
                self._daemon_pid = None
                return True
            
            if self._daemon_process is not None:
                try:
                    self._daemon_process.wait(timeout=SHUTDOWN_TIMEOUT)
                    decky.logger.info("Daemon shut down gracefully")
                except subprocess.TimeoutExpired:
                    decky.logger.warning("Daemon did not shut down gracefully, sending SIGKILL")
                    try:
                        os.kill(pid, signal.SIGKILL)
                        self._daemon_process.wait(timeout=1.0)
                    except (ProcessLookupError, subprocess.TimeoutExpired):
                        pass
            
            self._daemon_process = None
            self._daemon_pid = None
            return True
        except Exception as e:
            decky.logger.error(f"Error stopping daemon: {e}")
            return False

    def _send_ipc_command(self, command: Dict[str, Any]) -> Dict[str, Any]:
        """Send a command to the daemon via Unix socket."""
        try:
            sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sock.settimeout(SOCKET_TIMEOUT)
            
            try:
                sock.connect(SOCKET_PATH)
                command_json = json.dumps(command) + "\n"
                sock.sendall(command_json.encode('utf-8'))
                
                response_data = b""
                while True:
                    chunk = sock.recv(4096)
                    if not chunk:
                        break
                    response_data += chunk
                    if b"\n" in response_data:
                        break
                
                response_str = response_data.decode('utf-8').strip()
                if response_str:
                    return json.loads(response_str)
                else:
                    return {"error": "Empty response from daemon"}
                    
            finally:
                sock.close()
                
        except socket.timeout:
            decky.logger.error("IPC command timed out")
            return {"error": "Connection to daemon timed out"}
        except ConnectionRefusedError:
            decky.logger.error("Daemon not running or socket not available")
            return {"error": "Daemon not running"}
        except FileNotFoundError:
            decky.logger.error(f"Socket not found at {SOCKET_PATH}")
            return {"error": "Daemon socket not found"}
        except json.JSONDecodeError as e:
            decky.logger.error(f"Invalid JSON response: {e}")
            return {"error": f"Invalid response from daemon: {e}"}
        except Exception as e:
            decky.logger.error(f"IPC error: {e}")
            return {"error": str(e)}

    # ==================== Decky Plugin Lifecycle ====================
    
    async def init(self):
        """Plugin initialization - called when plugin loads."""
        decky.logger.info("SmartRefresh plugin loading...")
        
        if self._spawn_daemon():
            decky.logger.info("SmartRefresh plugin loaded successfully")
        else:
            decky.logger.error("SmartRefresh plugin failed to start daemon")
    
    async def _unload(self):
        """Plugin unload - called when plugin unloads."""
        decky.logger.info("SmartRefresh plugin unloading...")
        
        if self._stop_daemon():
            decky.logger.info("SmartRefresh plugin unloaded successfully")
        else:
            decky.logger.warning("SmartRefresh plugin unloaded with warnings")

    # ==================== IPC Proxy Methods ====================
    
    async def get_status(self) -> Dict[str, Any]:
        """Get the current daemon status."""
        return self._send_ipc_command({"command": "GetStatus"})
    
    async def set_settings(self, min_hz: int, max_hz: int, sensitivity: str) -> Dict[str, Any]:
        """Update daemon configuration."""
        return self._send_ipc_command({
            "command": "SetConfig",
            "min_hz": min_hz,
            "max_hz": max_hz,
            "sensitivity": sensitivity
        })
    
    async def start(self) -> Dict[str, Any]:
        """Start the refresh rate control loop."""
        return self._send_ipc_command({"command": "Start"})
    
    async def stop(self) -> Dict[str, Any]:
        """Stop the refresh rate control loop."""
        return self._send_ipc_command({"command": "Stop"})
    
    # Aliases for frontend compatibility
    async def start_daemon(self) -> Dict[str, Any]:
        """Alias for start() - used by frontend."""
        return await self.start()
    
    async def stop_daemon(self) -> Dict[str, Any]:
        """Alias for stop() - used by frontend."""
        return await self.stop()
    
    async def set_enabled(self, enabled: bool) -> Dict[str, Any]:
        """Enable or disable the refresh rate control."""
        if enabled:
            return await self.start()
        else:
            return await self.stop()
    
    async def set_range(self, min_hz: int, max_hz: int) -> Dict[str, Any]:
        """Set the refresh rate range."""
        # Get current settings first to preserve sensitivity
        status = self._send_ipc_command({"command": "GetStatus"})
        sensitivity = status.get("config", {}).get("sensitivity", "balanced")
        return self._send_ipc_command({
            "command": "SetConfig",
            "min_hz": min_hz,
            "max_hz": max_hz,
            "sensitivity": sensitivity
        })

    async def set_device_mode(self, mode: str) -> Dict[str, Any]:
        """Set the device mode (oled/lcd/custom) for hardware-specific throttling."""
        return self._send_ipc_command({
            "command": "SetDeviceMode",
            "mode": mode
        })
