import { call } from "@decky/api";

export interface DaemonConfig {
  min_hz: number;
  max_hz: number;
  sensitivity: "conservative" | "balanced" | "aggressive";
  enabled: boolean;
}

export interface DaemonStatus {
  running: boolean;
  current_fps: number;
  current_hz: number;
  state: string;
  device_mode: string;
  config: DaemonConfig;
}

export async function getStatus(): Promise<DaemonStatus | null> {
  try {
    const result = await call<[], DaemonStatus>("get_status");
    return result;
  } catch (error) {
    console.error("SmartRefresh: Failed to get status", error);
    return null;
  }
}

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

export async function setSettings(
  minHz: number,
  maxHz: number,
  sensitivity: string
): Promise<boolean> {
  try {
    await call<[number, number, string], void>(
      "set_settings",
      minHz,
      maxHz,
      sensitivity
    );
    return true;
  } catch (error) {
    console.error("SmartRefresh: Failed to set settings", error);
    return false;
  }
}

export type DeviceMode = "oled" | "lcd" | "custom";

export async function setDeviceMode(mode: DeviceMode): Promise<boolean> {
  try {
    await call<[string], void>("set_device_mode", mode);
    return true;
  } catch (error) {
    console.error("SmartRefresh: Failed to set device mode", error);
    return false;
  }
}
