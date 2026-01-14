import { call } from "@decky/api";

export interface DaemonConfig {
  min_hz: number;
  max_hz: number;
  sensitivity: "conservative" | "balanced" | "aggressive";
  enabled: boolean;
  adaptive_sensitivity: boolean;
}

export interface TransitionRecord {
  timestamp: string;
  from_hz: number;
  to_hz: number;
  fps: number;
  direction: string;
}

export interface DaemonStatus {
  running: boolean;
  current_fps: number;
  current_hz: number;
  state: string;
  device_mode: string;
  config: DaemonConfig;
  mangohud_available: boolean;
  external_display_detected: boolean;
  fps_std_dev: number;
  current_app_id: string | null;
  transitions: TransitionRecord[];
}

export interface MetricsResponse {
  total_switches: number;
  switches_per_hour: number;
  avg_time_in_stable_sec: number;
  uptime_sec: number;
  drop_count: number;
  increase_count: number;
}

export interface BatteryResponse {
  power_watts: number;
  avg_power_watts: number;
  estimated_savings_minutes: number;
  available: boolean;
}

export interface GameProfile {
  app_id: string;
  name: string;
  min_hz: number;
  max_hz: number;
  sensitivity: string;
  adaptive_sensitivity: boolean;
}

export interface GlobalDefault {
  min_hz: number;
  max_hz: number;
  sensitivity: string;
  adaptive_sensitivity: boolean;
}

export interface ProfilesResponse {
  profiles: GameProfile[];
  current_app_id: string | null;
  global_default: GlobalDefault;
}

export type DeviceMode = "oled" | "lcd" | "custom";

// Status
export async function getStatus(): Promise<DaemonStatus | null> {
  try {
    const result = await call<[], DaemonStatus>("get_status");
    return result;
  } catch (error) {
    console.error("SmartRefresh: Failed to get status", error);
    return null;
  }
}

// Daemon control
export async function startDaemon(): Promise<boolean> {
  try {
    await call<[], void>("start_daemon");
    return true;
  } catch (error) {
    console.error("SmartRefresh: Failed to start daemon", error);
    return false;
  }
}

export async function stopDaemon(): Promise<boolean> {
  try {
    await call<[], void>("stop_daemon");
    return true;
  } catch (error) {
    console.error("SmartRefresh: Failed to stop daemon", error);
    return false;
  }
}

// Settings
export async function setSettings(
  minHz: number,
  maxHz: number,
  sensitivity: string,
  adaptiveSensitivity: boolean = false
): Promise<boolean> {
  try {
    await call<[number, number, string, boolean], void>(
      "set_settings",
      minHz,
      maxHz,
      sensitivity,
      adaptiveSensitivity
    );
    return true;
  } catch (error) {
    console.error("SmartRefresh: Failed to set settings", error);
    return false;
  }
}

export async function setDeviceMode(mode: DeviceMode): Promise<boolean> {
  try {
    await call<[string], void>("set_device_mode", mode);
    return true;
  } catch (error) {
    console.error("SmartRefresh: Failed to set device mode", error);
    return false;
  }
}

// Metrics
export async function getMetrics(): Promise<MetricsResponse | null> {
  try {
    const result = await call<[], MetricsResponse>("get_metrics");
    return result;
  } catch (error) {
    console.error("SmartRefresh: Failed to get metrics", error);
    return null;
  }
}

// Battery
export async function getBatteryStatus(): Promise<BatteryResponse | null> {
  try {
    const result = await call<[], BatteryResponse>("get_battery_status");
    return result;
  } catch (error) {
    console.error("SmartRefresh: Failed to get battery status", error);
    return null;
  }
}

// Profiles
export async function getProfiles(): Promise<ProfilesResponse | null> {
  try {
    const result = await call<[], ProfilesResponse>("get_profiles");
    return result;
  } catch (error) {
    console.error("SmartRefresh: Failed to get profiles", error);
    return null;
  }
}

export async function saveProfile(
  appId: string,
  name: string,
  minHz: number,
  maxHz: number,
  sensitivity: string,
  adaptiveSensitivity: boolean = false
): Promise<boolean> {
  try {
    await call<[string, string, number, number, string, boolean], void>(
      "save_profile",
      appId,
      name,
      minHz,
      maxHz,
      sensitivity,
      adaptiveSensitivity
    );
    return true;
  } catch (error) {
    console.error("SmartRefresh: Failed to save profile", error);
    return false;
  }
}

export async function deleteProfile(appId: string): Promise<boolean> {
  try {
    await call<[string], void>("delete_profile", appId);
    return true;
  } catch (error) {
    console.error("SmartRefresh: Failed to delete profile", error);
    return false;
  }
}

export async function setGameId(appId: string, name: string = ""): Promise<boolean> {
  try {
    await call<[string, string], void>("set_game_id", appId, name);
    return true;
  } catch (error) {
    console.error("SmartRefresh: Failed to set game ID", error);
    return false;
  }
}
