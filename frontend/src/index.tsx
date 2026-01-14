import React, { useState, useEffect, useRef, VFC } from "react";
import {
  definePlugin,
  PanelSection,
  PanelSectionRow,
  ToggleField,
  SliderField,
  DropdownItem,
  DropdownOption,
  Field,
  staticClasses,
} from "@decky/ui";
import { FaSync } from "react-icons/fa";
import {
  getStatus,
  startDaemon,
  stopDaemon,
  setSettings,
  setDeviceMode,
  DaemonStatus,
  DeviceMode,
} from "./api";

// Device model (hardware) - separate from preset
type DeviceModelType = "oled" | "lcd";

// Preset (configuration) - can be oled, lcd, or custom
type PresetType = "oled" | "lcd" | "custom";

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

// LCD allowed Hz steps (quantized to 5Hz)
const LCD_HZ_STEPS = [40, 45, 50, 55, 60];
// OLED allowed Hz steps
const OLED_HZ_STEPS = [45, 50, 55, 60, 65, 70, 75, 80, 85, 90];

const SENSITIVITY_OPTIONS = ["conservative", "balanced", "aggressive"] as const;
const SENSITIVITY_LABELS: Record<string, string> = {
  conservative: "Conservative",
  balanced: "Balanced",
  aggressive: "Aggressive",
};

const Content: VFC = () => {
  const [enabled, setEnabled] = useState(false);
  const [minHz, setMinHz] = useState(45);
  const [maxHz, setMaxHz] = useState(90);
  const [sensitivityIndex, setSensitivityIndex] = useState(1);
  const [deviceModel, setDeviceModel] = useState<DeviceModelType>("oled");
  const [preset, setPreset] = useState<PresetType>("oled");
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState<DaemonStatus | null>(null);
  const intervalRef = useRef<number | null>(null);

  // Determine preset from current values
  const detectPreset = (min: number, max: number): PresetType => {
    if (min === 45 && max === 90) return "oled";
    if (min === 40 && max === 60) return "lcd";
    return "custom";
  };

  // Detect device model from daemon status
  const detectDeviceModel = (mode: string | undefined): DeviceModelType => {
    if (mode === "lcd") return "lcd";
    return "oled";
  };

  // Initial load and polling
  useEffect(() => {
    const fetchStatus = async () => {
      const result = await getStatus();
      if (result) {
        setStatus(result);
        setEnabled(result.running);
        setMinHz(result.config.min_hz);
        setMaxHz(result.config.max_hz);
        setPreset(detectPreset(result.config.min_hz, result.config.max_hz));
        setDeviceModel(detectDeviceModel(result.device_mode));
        const sensIndex = SENSITIVITY_OPTIONS.indexOf(result.config.sensitivity);
        setSensitivityIndex(sensIndex >= 0 ? sensIndex : 1);
        setLoading(false);
      }
    };

    fetchStatus();
    intervalRef.current = window.setInterval(fetchStatus, 1000);

    return () => {
      if (intervalRef.current) {
        window.clearInterval(intervalRef.current);
      }
    };
  }, []);

  const handleToggle = async (value: boolean) => {
    setLoading(true);
    const success = value ? await startDaemon() : await stopDaemon();
    if (success) {
      setEnabled(value);
    }
    setLoading(false);
  };

  // Handle device model change (hardware selection)
  const handleDeviceModelChange = async (option: DropdownOption) => {
    const newModel = option.data as DeviceModelType;
    setDeviceModel(newModel);

    // Notify backend of device mode change for LCD throttling
    await setDeviceMode(newModel);

    // If switching to LCD, apply LCD preset automatically
    if (newModel === "lcd") {
      const { minHz: newMin, maxHz: newMax } = PRESET_VALUES.lcd;
      setMinHz(newMin);
      setMaxHz(newMax);
      setPreset("lcd");
      // LCD forces conservative sensitivity
      setSensitivityIndex(0);
      await setSettings(newMin, newMax, SENSITIVITY_OPTIONS[0]);
    }
  };

  // Handle preset change (configuration preset)
  const handlePresetChange = async (option: DropdownOption) => {
    const newPreset = option.data as PresetType;
    setPreset(newPreset);

    if (newPreset !== "custom" && PRESET_VALUES[newPreset]) {
      const { minHz: newMin, maxHz: newMax } = PRESET_VALUES[newPreset];
      setMinHz(newMin);
      setMaxHz(newMax);
      
      // For LCD preset or LCD device model, force conservative sensitivity
      const effectiveSensitivity = (newPreset === "lcd" || deviceModel === "lcd") ? 0 : sensitivityIndex;
      if (newPreset === "lcd" || deviceModel === "lcd") {
        setSensitivityIndex(0);
      }
      await setSettings(newMin, newMax, SENSITIVITY_OPTIONS[effectiveSensitivity]);
    }
  };

  const handleMinHzChange = async (value: number) => {
    // Quantize to 5Hz steps
    const quantized = Math.round(value / 5) * 5;
    const newMin = Math.min(quantized, maxHz);
    setMinHz(newMin);
    // Only change preset to custom, not device model
    setPreset("custom");
    await setSettings(newMin, maxHz, SENSITIVITY_OPTIONS[sensitivityIndex]);
  };

  const handleMaxHzChange = async (value: number) => {
    // Quantize to 5Hz steps
    const quantized = Math.round(value / 5) * 5;
    const newMax = Math.max(quantized, minHz);
    setMaxHz(newMax);
    // Only change preset to custom, not device model
    setPreset("custom");
    await setSettings(minHz, newMax, SENSITIVITY_OPTIONS[sensitivityIndex]);
  };

  const handleSensitivityChange = async (value: number) => {
    // LCD mode forces conservative - don't allow change
    if (deviceModel === "lcd" && value !== 0) {
      return;
    }
    setSensitivityIndex(value);
    await setSettings(minHz, maxHz, SENSITIVITY_OPTIONS[value]);
  };

  const isLCD = deviceModel === "lcd";
  
  // Get effective sensitivity (LCD forces conservative)
  const effectiveSensitivity = isLCD ? "conservative" : SENSITIVITY_OPTIONS[sensitivityIndex];
  
  // Determine slider range based on device model
  const sliderMin = isLCD ? 40 : 40;
  const sliderMax = isLCD ? 60 : 90;

  return (
    <>
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
            selectedOption={DEVICE_MODEL_OPTIONS.find((o) => o.data === deviceModel)?.data}
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
      </PanelSection>

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
            <PanelSectionRow>
              <Field label="Device Mode">
                <span>
                  {status.device_mode === "lcd" ? (
                    <span style={{ color: "#e6a23c" }}>LCD (throttled)</span>
                  ) : (
                    status.device_mode?.toUpperCase() || "OLED"
                  )}
                </span>
              </Field>
            </PanelSectionRow>
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
