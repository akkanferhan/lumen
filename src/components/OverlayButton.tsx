import { invoke } from "@tauri-apps/api/core";
import { useBattery, useClock } from "./StatusBar";
import { L } from "../lib/i18n";

/**
 * The touch button Lumen floats over a running game (its own transparent
 * window). A tap opens the quick menu; it never reaches the game the way a
 * controller combo does. When Lumen makes the window wide enough, battery
 * and clock show beside it (see `overlay.rs`).
 */
export function OverlayButton() {
  return (
    <>
      <OverlayStatus />
      <button
        className="overlay-button"
        aria-label={L("Lumen menüsü", "Lumen menu")}
        onPointerDown={(e) => {
          e.preventDefault();
          invoke("overlay_pressed").catch(() => {});
        }}
      >
        <svg viewBox="0 0 24 24" aria-hidden width="22" height="22">
          <path d="M4 7h16M4 12h16M4 17h16" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" />
        </svg>
      </button>
    </>
  );
}

/** Battery and time; red when the battery runs low and isn't charging. */
function OverlayStatus() {
  const time = useClock();
  const battery = useBattery();
  const percent = battery ? Math.round(battery.level * 100) : null;
  const low = percent !== null && percent <= 15 && !battery?.charging;
  return (
    <div className={`overlay-status${low ? " low" : ""}`} aria-hidden>
      {percent !== null && (
        <span>
          {battery?.charging ? "⚡" : ""}
          {percent}%
        </span>
      )}
      <span>{time}</span>
    </div>
  );
}
