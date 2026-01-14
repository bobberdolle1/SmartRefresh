import React, { useState, useEffect, useRef, FC, useCallback } from "react";
import {
  definePlugin,
  PanelSection,
  PanelSectionRow,
  ToggleField,
  SliderField,
  DropdownItem,
  DropdownOption,
  Field,
  ButtonItem,
  staticClasses,
} from "@decky/ui";
import { FaSync } from "react-icons/fa";
import {
  getStatus,
  startDaemon,
  stopDaemon,
  setSettings,
  setDeviceMode,
  getMetrics,
  getBatteryStatus,
  getProfiles,
  saveProfile,
  DaemonStatus,
  MetricsResponse,
  BatteryResponse,
  ProfilesResponse,
  TransitionRecord,
} from "./api";

// Types
type DeviceModelType = "oled" | "lcd";
type PresetType = "oled" | "lcd" | "custom";

// Constants
const DEVICE_MODEL_OPTIONS: DropdownOption[] = [
  { data: "oled", label: "Steam Deck OLED" },
  { data: "lcd", label: "Steam Deck LCD" },
];

const PRESET_OPTIONS: DropdownOption[] = [
  { data: "oled", label: "OLED Preset (45-90 Hz)" },
  { data: "lcd", label: "LCD Preset (40-60 Hz)" },
  { data: "custom", label: "Custom" },
];

const PRESET_VALUES: Record<string, { minHz: number; maxHz: number }> = {
  oled: { minHz: 45, maxHz: 90 },
  lcd: { minHz: 40, maxHz: 60 },
};

const SENSITIVITY_OPTIONS = ["conservative", "balanced", "aggressive"] as const;
const SENSITIVITY_LABELS: Record<string, string> = {
  conservative: "Conservative",
  balanced: "Balanced",
  aggressive: "Aggressive",
};

// FPS History for sparkline
interface FpsHistoryPoint {
  fps: number;
  hz: number;
  timestamp: number;
}

// Sparkline Component
const FpsSparkline: FC<{ history: FpsHistoryPoint[] }> = ({ history }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || history.length < 2) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    const padding = 4;

    // Clear
    ctx.clearRect(0, 0, width, height);

    // Find min/max
    const fpsValues = history.map((p) => p.fps);
    const hzValues = history.map((p) => p.hz);
    const minVal = Math.min(...fpsValues, ...hzValues) - 5;
    const maxVal = Math.max(...fpsValues, ...hzValues) + 5;
    const range = maxVal - minVal || 1;

    const scaleY = (val: number) =>
      height - padding - ((val - minVal) / range) * (height - padding * 2);
    const scaleX = (i: number) =>
      padding + (i / (history.length - 1)) * (width - padding * 2);

    // Draw Hz line (blue)
    ctx.beginPath();
    ctx.strokeStyle = "#4a9eff";
    ctx.lineWidth = 1.5;
    history.forEach((point, i) => {
      const x = scaleX(i);
      const y = scaleY(point.hz);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();

    // Draw FPS line (green)
    ctx.beginPath();
    ctx.strokeStyle = "#4ade80";
    ctx.lineWidth = 1.5;
    history.forEach((point, i) => {
      const x = scaleX(i);
      const y = scaleY(point.fps);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();
  }, [history]);

  return (
    <div style={{ padding: "8px 0" }}>
      <canvas
        ref={canvasRef}
        width={280}
        height={60}
        style={{
          width: "100%",
          height: "60px",
          backgroundColor: "rgba(0,0,0,0.2)",
          borderRadius: "4px",
        }}
      />
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          fontSize: "0.7em",
          color: "#888",
          marginTop: "4px",
        }}
      >
        <span>
          <span style={{ color: "#4ade80" }}>●</span> FPS
        </span>
        <span>
          <span style={{ color: "#4a9eff" }}>●</span> Hz
        </span>
      </div>
    </div>
  );
};

// Transition Log Component
const TransitionLog: FC<{ transitions: TransitionRecord[] }> = ({
  transitions,
}) => {
  if (transitions.length === 0) {
    return (
      <div style={{ color: "#666", fontSize: "0.85em", padding: "8px 0" }}>
        No transitions yet
      </div>
    );
  }

  return (
    <div
      style={{
        maxHeight: "120px",
        overflowY: "auto",
        fontSize: "0.8em",
        backgroundColor: "rgba(0,0,0,0.2)",
        borderRadius: "4px",
        padding: "8px",
      }}
    >
      {transitions
        .slice(-5)
        .reverse()
        .map((t, i) => (
          <div
            key={i}
            style={{
              display: "flex",
              justifyContent: "space-between",
              padding: "2px 0",
              borderBottom:
                i < 4 ? "1px solid rgba(255,255,255,0.1)" : "none",
            }}
          >
            <span style={{ color: "#888" }}>{t.timestamp}</span>
            <span>
              {t.from_hz}Hz → {t.to_hz}Hz
            </span>
            <span
              style={{
                color: t.direction === "Dropped" ? "#f87171" : "#4ade80",
              }}
            >
              {t.direction}
            </span>
          </div>
        ))}
    </div>
  );
};

// Main Content Component
const Content: FC = () => {
  // Basic state
  const [enabled, setEnabled] = useState(false);
  const [minHz, setMinHz] = useState(45);
  const [maxHz, setMaxHz] = useState(90);
  const [sensitivityIndex, setSensitivityIndex] = useState(1);
  const [deviceModel, setDeviceModel] = useState<DeviceModelType>("oled");
  const [preset, setPreset] = useState<PresetType>("oled");
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState<DaemonStatus | null>(null);

  // Advanced state
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [adaptiveSensitivity, setAdaptiveSensitivity] = useState(false);
  const [metrics, setMetrics] = useState<MetricsResponse | null>(null);
  const [battery, setBattery] = useState<BatteryResponse | null>(null);
  const [profiles, setProfiles] = useState<ProfilesResponse | null>(null);

  // FPS history for sparkline
  const [fpsHistory, setFpsHistory] = useState<FpsHistoryPoint[]>([]);
  const intervalRef = useRef<number | null>(null);

  // Detect preset from values
  const detectPreset = (min: number, max: number): PresetType => {
    if (min === 45 && max === 90) return "oled";
    if (min === 40 && max === 60) return "lcd";
    return "custom";
  };

  // Detect device model from status
  const detectDeviceModel = (mode: string | undefined): DeviceModelType => {
    if (mode === "lcd") return "lcd";
    return "oled";
  };

  // Fetch all data
  const fetchData = useCallback(async () => {
    const result = await getStatus();
    if (result) {
      setStatus(result);
      setEnabled(result.running);
      setMinHz(result.config.min_hz);
      setMaxHz(result.config.max_hz);
      setPreset(detectPreset(result.config.min_hz, result.config.max_hz));
      setDeviceModel(detectDeviceModel(result.device_mode));
      setAdaptiveSensitivity(result.config.adaptive_sensitivity);

      const sensIndex = SENSITIVITY_OPTIONS.indexOf(result.config.sensitivity);
      setSensitivityIndex(sensIndex >= 0 ? sensIndex : 1);

      // Update FPS history
      setFpsHistory((prev) => {
        const newPoint: FpsHistoryPoint = {
          fps: result.current_fps,
          hz: result.current_hz,
          timestamp: Date.now(),
        };
        const updated = [...prev, newPoint];
        // Keep last 30 seconds (at 1s intervals)
        return updated.slice(-30);
      });

      setLoading(false);
    }

    // Fetch advanced data if showing
    if (showAdvanced) {
      const [metricsResult, batteryResult, profilesResult] = await Promise.all([
        getMetrics(),
        getBatteryStatus(),
        getProfiles(),
      ]);
      setMetrics(metricsResult);
      setBattery(batteryResult);
      setProfiles(profilesResult);
    }
  }, [showAdvanced]);

  // Initial load and polling
  useEffect(() => {
    fetchData();
    intervalRef.current = window.setInterval(fetchData, 1000);

    return () => {
      if (intervalRef.current) {
        window.clearInterval(intervalRef.current);
      }
    };
  }, [fetchData]);

  // Handlers
  const handleToggle = async (value: boolean) => {
    setLoading(true);
    const success = value ? await startDaemon() : await stopDaemon();
    if (success) {
      setEnabled(value);
    }
    setLoading(false);
  };

  const handleDeviceModelChange = async (option: DropdownOption) => {
    const newModel = option.data as DeviceModelType;
    setDeviceModel(newModel);
    await setDeviceMode(newModel);

    if (newModel === "lcd") {
      const { minHz: newMin, maxHz: newMax } = PRESET_VALUES.lcd;
      setMinHz(newMin);
      setMaxHz(newMax);
      setPreset("lcd");
      setSensitivityIndex(0);
      await setSettings(newMin, newMax, SENSITIVITY_OPTIONS[0], adaptiveSensitivity);
    }
  };

  const handlePresetChange = async (option: DropdownOption) => {
    const newPreset = option.data as PresetType;
    setPreset(newPreset);

    if (newPreset !== "custom" && PRESET_VALUES[newPreset]) {
      const { minHz: newMin, maxHz: newMax } = PRESET_VALUES[newPreset];
      setMinHz(newMin);
      setMaxHz(newMax);

      const effectiveSensitivity =
        newPreset === "lcd" || deviceModel === "lcd" ? 0 : sensitivityIndex;
      if (newPreset === "lcd" || deviceModel === "lcd") {
        setSensitivityIndex(0);
      }
      await setSettings(
        newMin,
        newMax,
        SENSITIVITY_OPTIONS[effectiveSensitivity],
        adaptiveSensitivity
      );
    }
  };

  const handleMinHzChange = async (value: number) => {
    const quantized = Math.round(value / 5) * 5;
    const newMin = Math.min(quantized, maxHz);
    setMinHz(newMin);
    setPreset("custom");
    await setSettings(newMin, maxHz, SENSITIVITY_OPTIONS[sensitivityIndex], adaptiveSensitivity);
  };

  const handleMaxHzChange = async (value: number) => {
    const quantized = Math.round(value / 5) * 5;
    const newMax = Math.max(quantized, minHz);
    setMaxHz(newMax);
    setPreset("custom");
    await setSettings(minHz, newMax, SENSITIVITY_OPTIONS[sensitivityIndex], adaptiveSensitivity);
  };

  const handleSensitivityChange = async (value: number) => {
    if (deviceModel === "lcd" && value !== 0) return;
    setSensitivityIndex(value);
    await setSettings(minHz, maxHz, SENSITIVITY_OPTIONS[value], adaptiveSensitivity);
  };

  const handleAdaptiveSensitivityChange = async (value: boolean) => {
    setAdaptiveSensitivity(value);
    await setSettings(minHz, maxHz, SENSITIVITY_OPTIONS[sensitivityIndex], value);
  };

  const handleSaveProfile = async () => {
    if (!status?.current_app_id) return;
    
    const success = await saveProfile(
      status.current_app_id,
      `Game ${status.current_app_id}`,
      minHz,
      maxHz,
      SENSITIVITY_OPTIONS[sensitivityIndex],
      adaptiveSensitivity
    );
    
    if (success) {
      // Refresh profiles
      const profilesResult = await getProfiles();
      setProfiles(profilesResult);
    }
  };

  const isLCD = deviceModel === "lcd";
  const effectiveSensitivity = isLCD
    ? "conservative"
    : SENSITIVITY_OPTIONS[sensitivityIndex];
  const sliderMin = isLCD ? 40 : 40;
  const sliderMax = isLCD ? 60 : 90;

  // Warning conditions
  const showDangerWarning =
    deviceModel === "lcd" &&
    (maxHz > 60 || sensitivityIndex === 2);

  return (
    <>
      {/* Main Section */}
      <PanelSection title="SmartRefresh">
        <PanelSectionRow>
          <ToggleField
            label="Enable SmartRefresh"
            description="Dynamic refresh rate control"
            checked={enabled}
            disabled={loading}
            onChange={handleToggle}
          />
        </PanelSectionRow>

        <PanelSectionRow>
          <DropdownItem
            label="Device Model"
            description="Select your Steam Deck hardware"
            menuLabel="Device Model"
            rgOptions={DEVICE_MODEL_OPTIONS}
            selectedOption={
              DEVICE_MODEL_OPTIONS.find((o) => o.data === deviceModel)?.data
            }
            onChange={handleDeviceModelChange}
          />
        </PanelSectionRow>

        {isLCD && (
          <PanelSectionRow>
            <div
              style={{
                color: "#e6a23c",
                fontSize: "0.85em",
                padding: "8px 12px",
                lineHeight: "1.4",
                backgroundColor: "rgba(230, 162, 60, 0.1)",
                borderRadius: "4px",
                border: "1px solid rgba(230, 162, 60, 0.3)",
              }}
            >
              ⚠️ LCD VRR range is limited (40-60 Hz). Refresh rate changes are
              throttled to prevent flickering.
            </div>
          </PanelSectionRow>
        )}

        {status?.external_display_detected && (
          <PanelSectionRow>
            <div
              style={{
                color: "#60a5fa",
                fontSize: "0.85em",
                padding: "8px 12px",
                backgroundColor: "rgba(96, 165, 250, 0.1)",
                borderRadius: "4px",
                border: "1px solid rgba(96, 165, 250, 0.3)",
              }}
            >
              🖥️ External display detected - SmartRefresh paused
            </div>
          </PanelSectionRow>
        )}

        {!status?.mangohud_available && (
          <PanelSectionRow>
            <div
              style={{
                color: "#f87171",
                fontSize: "0.85em",
                padding: "8px 12px",
                backgroundColor: "rgba(248, 113, 113, 0.1)",
                borderRadius: "4px",
                border: "1px solid rgba(248, 113, 113, 0.3)",
              }}
            >
              ⚠️ MangoHud not detected - Enable Performance Overlay
            </div>
          </PanelSectionRow>
        )}
      </PanelSection>

      {/* Configuration Section */}
      <PanelSection title="Configuration">
        <PanelSectionRow>
          <DropdownItem
            label="Preset"
            description="Quick configuration preset"
            menuLabel="Preset"
            rgOptions={PRESET_OPTIONS}
            selectedOption={PRESET_OPTIONS.find((o) => o.data === preset)?.data}
            onChange={handlePresetChange}
            disabled={loading || !enabled}
          />
        </PanelSectionRow>

        <PanelSectionRow>
          <SliderField
            label="Minimum Hz"
            description={`${minHz} Hz`}
            value={minHz}
            min={sliderMin}
            max={sliderMax}
            step={5}
            disabled={loading || !enabled}
            onChange={handleMinHzChange}
          />
        </PanelSectionRow>

        <PanelSectionRow>
          <SliderField
            label="Maximum Hz"
            description={`${maxHz} Hz`}
            value={maxHz}
            min={sliderMin}
            max={sliderMax}
            step={5}
            disabled={loading || !enabled}
            onChange={handleMaxHzChange}
          />
        </PanelSectionRow>

        <PanelSectionRow>
          <SliderField
            label="Sensitivity"
            description={
              isLCD
                ? `${SENSITIVITY_LABELS[effectiveSensitivity]} (LCD forces conservative)`
                : SENSITIVITY_LABELS[SENSITIVITY_OPTIONS[sensitivityIndex]]
            }
            value={isLCD ? 0 : sensitivityIndex}
            min={0}
            max={2}
            step={1}
            notchCount={3}
            notchLabels={[
              { notchIndex: 0, label: "Low" },
              { notchIndex: 1, label: "Med" },
              { notchIndex: 2, label: "High" },
            ]}
            disabled={loading || !enabled || isLCD}
            onChange={handleSensitivityChange}
          />
        </PanelSectionRow>
      </PanelSection>

      {/* Status Section */}
      <PanelSection title="Status">
        {status ? (
          <>
            <PanelSectionRow>
              <Field label="Current FPS">
                <span>{status.current_fps.toFixed(1)}</span>
              </Field>
            </PanelSectionRow>
            <PanelSectionRow>
              <Field label="Current Hz">
                <span>{status.current_hz} Hz</span>
              </Field>
            </PanelSectionRow>
            <PanelSectionRow>
              <Field label="State">
                <span>{status.state}</span>
              </Field>
            </PanelSectionRow>

            {/* FPS Sparkline */}
            {fpsHistory.length > 2 && (
              <PanelSectionRow>
                <FpsSparkline history={fpsHistory} />
              </PanelSectionRow>
            )}
          </>
        ) : (
          <PanelSectionRow>
            <Field label="Status">
              <span style={{ color: "#ff6b6b" }}>
                {loading ? "Loading..." : "⚠️ Daemon unreachable"}
              </span>
            </Field>
          </PanelSectionRow>
        )}
      </PanelSection>

      {/* Advanced Toggle */}
      <PanelSection>
        <PanelSectionRow>
          <ToggleField
            label="Show Advanced Settings"
            checked={showAdvanced}
            onChange={setShowAdvanced}
          />
        </PanelSectionRow>
      </PanelSection>

      {/* Advanced Section */}
      {showAdvanced && (
        <>
          <PanelSection title="Advanced">
            <PanelSectionRow>
              <ToggleField
                label="Adaptive Sensitivity"
                description="Auto-adjust based on FPS stability"
                checked={adaptiveSensitivity}
                disabled={loading || !enabled || isLCD}
                onChange={handleAdaptiveSensitivityChange}
              />
            </PanelSectionRow>

            {status && (
              <PanelSectionRow>
                <Field label="FPS Std Dev">
                  <span>{status.fps_std_dev.toFixed(2)}</span>
                </Field>
              </PanelSectionRow>
            )}
          </PanelSection>

          {/* Transition Log */}
          <PanelSection title="Transition Log">
            <PanelSectionRow>
              <TransitionLog transitions={status?.transitions || []} />
            </PanelSectionRow>
          </PanelSection>

          {/* Profiles */}
          <PanelSection title="Profiles">
            {status?.current_app_id && (
              <PanelSectionRow>
                <ButtonItem
                  layout="below"
                  onClick={handleSaveProfile}
                  disabled={loading}
                >
                  Save Profile for Current Game
                </ButtonItem>
              </PanelSectionRow>
            )}

            {profiles && profiles.profiles.length > 0 && (
              <PanelSectionRow>
                <div style={{ fontSize: "0.85em", color: "#888" }}>
                  {profiles.profiles.length} saved profile(s)
                </div>
              </PanelSectionRow>
            )}
          </PanelSection>

          {/* Metrics */}
          {metrics && (
            <PanelSection title="Metrics">
              <PanelSectionRow>
                <Field label="Total Switches">
                  <span>{metrics.total_switches}</span>
                </Field>
              </PanelSectionRow>
              <PanelSectionRow>
                <Field label="Switches/Hour">
                  <span>{metrics.switches_per_hour}</span>
                </Field>
              </PanelSectionRow>
              <PanelSectionRow>
                <Field label="Avg Stable Time">
                  <span>{metrics.avg_time_in_stable_sec.toFixed(1)}s</span>
                </Field>
              </PanelSectionRow>
              <PanelSectionRow>
                <Field label="Uptime">
                  <span>{Math.floor(metrics.uptime_sec / 60)}m</span>
                </Field>
              </PanelSectionRow>
            </PanelSection>
          )}

          {/* Battery */}
          {battery?.available && (
            <PanelSection title="Battery">
              <PanelSectionRow>
                <Field label="Current Power">
                  <span>{battery.power_watts.toFixed(1)}W</span>
                </Field>
              </PanelSectionRow>
              <PanelSectionRow>
                <Field label="Avg Power">
                  <span>{battery.avg_power_watts.toFixed(1)}W</span>
                </Field>
              </PanelSectionRow>
              <PanelSectionRow>
                <Field label="Est. Savings">
                  <span style={{ color: "#4ade80" }}>
                    +{battery.estimated_savings_minutes.toFixed(1)} min/hr
                  </span>
                </Field>
              </PanelSectionRow>
            </PanelSection>
          )}
        </>
      )}

      {/* Danger Warning */}
      {showDangerWarning && (
        <PanelSection>
          <PanelSectionRow>
            <div
              style={{
                color: "#f87171",
                fontSize: "0.85em",
                padding: "8px 12px",
                backgroundColor: "rgba(248, 113, 113, 0.1)",
                borderRadius: "4px",
                border: "1px solid rgba(248, 113, 113, 0.3)",
              }}
            >
              ⚠️ Warning: Current settings may cause LCD flickering
            </div>
          </PanelSectionRow>
        </PanelSection>
      )}
    </>
  );
};

export default definePlugin(() => {
  return {
    name: "SmartRefresh",
    title: <div className={staticClasses.Title}>SmartRefresh</div>,
    content: <Content />,
    icon: <FaSync />,
  };
});
