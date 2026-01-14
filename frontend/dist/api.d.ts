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
    config: DaemonConfig;
}
export declare function getStatus(): Promise<DaemonStatus | null>;
export declare function startDaemon(): Promise<boolean>;
export declare function stopDaemon(): Promise<boolean>;
export declare function setSettings(minHz: number, maxHz: number, sensitivity: string): Promise<boolean>;
