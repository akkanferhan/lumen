import { useEffect, useState } from "react";
import { locale } from "../lib/i18n";

interface BatteryManager extends EventTarget {
  level: number;
  charging: boolean;
}

export function useClock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 15_000);
    return () => clearInterval(id);
  }, []);
  return now.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
}

export function useBattery() {
  const [battery, setBattery] = useState<{ level: number; charging: boolean } | null>(null);
  useEffect(() => {
    const nav = navigator as Navigator & { getBattery?: () => Promise<BatteryManager> };
    let manager: BatteryManager | null = null;
    const update = () => manager && setBattery({ level: manager.level, charging: manager.charging });
    nav.getBattery?.().then((m) => {
      manager = m;
      update();
      m.addEventListener("levelchange", update);
      m.addEventListener("chargingchange", update);
    });
    return () => {
      manager?.removeEventListener("levelchange", update);
      manager?.removeEventListener("chargingchange", update);
    };
  }, []);
  return battery;
}

export function StatusBar() {
  const time = useClock();
  const battery = useBattery();
  const percent = battery ? Math.round(battery.level * 100) : null;

  return (
    <div className="status">
      {battery && percent !== null && (
        <span className={`battery${percent <= 15 && !battery.charging ? " low" : ""}${battery.charging ? " charging" : ""}`}>
          <span className="battery-shell">
            <span className="battery-level" style={{ width: `${percent}%` }} />
          </span>
          {percent}%
        </span>
      )}
      <span>{time}</span>
    </div>
  );
}
