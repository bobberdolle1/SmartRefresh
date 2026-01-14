import { SliderField } from "@decky/ui";
import { useState, useEffect } from "react";
import { getStatus, setSettings, DaemonStatus } from "../api";

export function RefreshRangeSlider() {
  const [minHz, setMinHz] = useState(40);
  const [maxHz, setMaxHz] = useState(90);
  const [sensitivity, setSensitivity] = useState("balanced");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchStatus = async () => {
      const status = await getStatus();
      if (status) {
        setMinHz(status.config.min_hz);
        setMaxHz(status.config.max_hz);
        setSensitivity(status.config.sensitivity);
      }
      setLoading(false);
    };
    fetchStatus();
  }, []);

  const handleMinChange = async (value: number) => {
    const newMin = Math.min(value, maxHz);
    setMinHz(newMin);
    await setSettings(newMin, maxHz, sensitivity);
  };

  const handleMaxChange = async (value: number) => {
    const newMax = Math.max(value, minHz);
    setMaxHz(newMax);
    await setSettings(minHz, newMax, sensitivity);
  };

  return (
    <div>
      <SliderField
        label="Minimum Hz"
        description={`${minHz} Hz`}
        value={minHz}
        min={40}
        max={90}
        step={5}
        disabled={loading}
        onChange={handleMinChange}
      />
      <SliderField
        label="Maximum Hz"
        description={`${maxHz} Hz`}
        value={maxHz}
        min={40}
        max={90}
        step={5}
        disabled={loading}
        onChange={handleMaxChange}
      />
    </div>
  );
}
