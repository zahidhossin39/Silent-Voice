import { useEffect, useState } from "react";
import type { HardwareInfo } from "../types";
import { getHardwareInfo } from "../services/tauriBridge";

export function useHardwareInfo() {
  const [hardware, setHardware] = useState<HardwareInfo | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let alive = true;
    getHardwareInfo()
      .then((hw) => alive && setHardware(hw))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, []);

  return { hardware, loading };
}
