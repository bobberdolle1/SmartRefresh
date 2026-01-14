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
  DaemonStatus,
} from "./api";

type DevicePreset = "oled" | "lcd" | "custom";

const DEVICE_PRESETS: DropdownOption[] = [
  { data: "oled", label: "Steam Deck OLED" },
  { data: "lcd", label: "Steam Deck LCD" },
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

const Content: VFC = () => {
  const [enabled, setEnabled] = useState(false);
  const [minHz, setMinHz] = useState(45);
  const [maxHz, setMaxHz] = useState(90);
  const [sensitivityIndex, setSensitivityIndex] = useState(1);
  const [preset, setPreset] = useState<DevicePreset>("oled");
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState<DaemonStatus | null>(null);
  const intervalRef = useRef<number | null>(null);

  // Determine preset from current values
  const detectPreset = (min: number, max: number): DevicePreset => {
    if (min === 45 && max === 90) return "oled";
    if (min === 40 && max === 60) return "lcd";
    return "custom";
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

  const handlePresetChange = async (option: DropdownOption) => {
    const newPreset = option.data as DevicePreset;
    setPreset(newPreset);

    if (newPreset !== "custom" && PRESET_VALUES[newPreset]) {
      const { minHz: newMin, maxHz: newMax } = PRESET_VALUES[newPreset];
      setMinHz(newMin);
      setMaxHz(newMax);
      await setSettings(newMin, newMax, SENSITIVITY_OPTIONS[sensitivityIndex]);
    }
  };

  const handleMinHzChange = async (value: number) => {
    const newMin = Math.min(value, maxHz);
    setMinHz(newMin);
    setPreset("custom");
    await setSettings(newMin, maxHz, SENSITIVITY_OPTIONS[sensitivityIndex]);
  };

  const handleMaxHzChange = async (value: number) => {
    const newMax = Math.max(value, minHz);
    setMaxHz(newMax);
    setPreset("custom");
    await setSettings(minHz, newMax, SENSITIVITY_OPTIONS[sensitivityIndex]);
  };

  const handleSensitivityChange = async (value: number) => {
    setSensitivityIndex(value);
    await setSettings(minHz, maxHz, SENSITIVITY_OPTIONS[value]);
  };

  const isLCD = preset === "lcd";

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
            label="Device Preset"
            description="Select your Steam Deck model"
            menuLabel="Device Preset"
            rgOptions={DEVICE_PRESETS}
            selectedOption={DEVICE_PRESETS.find((o) => o.data === preset)?.data}
            onChange={handlePresetChange}
          />
        </PanelSectionRow>

        {isLCD && (
          <PanelSectionRow>
            <div
              style={{
                color: "#f0a030",
                fontSize: "0.85em",
                padding: "4px 0",
                lineHeight: "1.4",
              }}
            >
              ⚠️ LCD screens have limited VRR range. Flickering may occur at
              certain refresh rates.
            </div>
          </PanelSectionRow>
        )}
      </PanelSection>

      <PanelSection title="Configuration">
        <PanelSectionRow>
          <SliderField
            label="Minimum Hz"
            description={`${minHz} Hz`}
            value={minHz}
            min={40}
            max={90}
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
            min={40}
            max={90}
            step={5}
            disabled={loading || !enabled}
            onChange={handleMaxHzChange}
          />
        </PanelSectionRow>

        <PanelSectionRow>
          <SliderField
            label="Sensitivity"
            description={SENSITIVITY_LABELS[SENSITIVITY_OPTIONS[sensitivityIndex]]}
            value={sensitivityIndex}
            min={0}
            max={2}
            step={1}
            notchCount={3}
            notchLabels={[
              { notchIndex: 0, label: "Low" },
              { notchIndex: 1, label: "Med" },
              { notchIndex: 2, label: "High" },
            ]}
            disabled={loading || !enabled}
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
