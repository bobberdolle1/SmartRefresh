import { Field } from "@decky/ui";
import { useState, useEffect, useRef } from "react";
import { getStatus, DaemonStatus } from "../api";

export function DebugView() {
  const [status, setStatus] = useState<DaemonStatus | null>(null);
  const [error, setError] = useState(false);
  const intervalRef = useRef<number | null>(null);

  useEffect(() => {
    const fetchStatus = async () => {
      const result = await getStatus();
      if (result) {
        setStatus(result);
        setError(false);
      } else {
        setError(true);
      }
    };

    // Initial fetch
    fetchStatus();

    // Poll every 500ms while panel is open
    intervalRef.current = window.setInterval(fetchStatus, 500);

    return () => {
      if (intervalRef.current) {
        window.clearInterval(intervalRef.current);
      }
    };
  }, []);

  if (error) {
    return (
      <Field label="Status">
        <div style={{ color: "#ff6b6b" }}>
          ⚠️ Daemon unreachable
        </div>
      </Field>
    );
  }

  if (!status) {
    return (
      <Field label="Status">
        <div>Loading...</div>
      </Field>
    );
  }

  return (
    <div>
      <Field label="Current FPS">
        <div>{status.current_fps.toFixed(1)}</div>
      </Field>
      <Field label="Current Hz">
        <div>{status.current_hz} Hz</div>
      </Field>
      <Field label="State">
        <div>{status.state}</div>
      </Field>
    </div>
  );
}
