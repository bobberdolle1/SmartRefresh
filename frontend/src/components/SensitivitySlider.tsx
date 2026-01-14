import { SliderField } from "@decky/ui";
import { useState, useEffect } from "react";
import { getStatus, setSettings } from "../api";

const SENSITIVITY_OPTIONS = ["conservative", "balanced", "aggressive"] as const;
const SENSITIVITY_LABELS: Record<string, string> = {
  conservative: "Conservative (slower transitions)",
  balanced: "Balanced (default)",
  aggressive: "Aggressive (faster transitions)",
};

export function SensitivitySlider() {
  const [sensitivityIndex, setSensitivityIndex] = useState(1);
  const [minHz, setMinHz] = useState(40);
  const [maxHz, setMaxHz] = useState(90);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchStatus = async () => {
      const status = await getStatus();
      if (status) {
        const index = SENSITIVITY_OPTIONS.indexOf(status.config.sensitivity);
        setSensitivityIndex(index >= 0 ? index : 1);
        setMinHz(status.config.min_hz);
        setMaxHz(status.config.max_hz);
      }
      setLoading(false);
    };
    fetchStatus();
  }, []);

  const handleChange = async (value: number) => {
    setSensitivityIndex(value);
    const sensitivity = SENSITIVITY_OPTIONS[value];
    await setSettings(minHz, maxHz, sensitivity);
  };

  const currentSensitivity = SENSITIVITY_OPTIONS[sensitivityIndex];

  return (
    <SliderField
      label="Sensitivity"
      description={SENSITIVITY_LABELS[currentSensitivity]}
      value={sensitivityIndex}
      min={0}
      max={2}
      step={1}
      notchCount={3}
      notchLabels={[
        { notchIndex: 0, label: "Conservative" },
        { notchIndex: 1, label: "Balanced" },
        { notchIndex: 2, label: "Aggressive" },
      ]}
      disabled={loading}
      onChange={handleChange}
    />
  );
}
