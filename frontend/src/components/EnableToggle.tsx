import { ToggleField } from "@decky/ui";
import { useState, useEffect } from "react";
import { getStatus, startDaemon, stopDaemon } from "../api";

export function EnableToggle() {
  const [enabled, setEnabled] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchStatus = async () => {
      const status = await getStatus();
      if (status) {
        setEnabled(status.running);
      }
      setLoading(false);
    };
    fetchStatus();
  }, []);

  const handleToggle = async (value: boolean) => {
    setLoading(true);
    const success = value ? await startDaemon() : await stopDaemon();
    if (success) {
      setEnabled(value);
    }
    setLoading(false);
  };

  return (
    <ToggleField
      label="Enable SmartRefresh"
      description="Toggle dynamic refresh rate control"
      checked={enabled}
      disabled={loading}
      onChange={handleToggle}
    />
  );
}
