import { invoke } from "@tauri-apps/api/core";

/**
 * The touch button Lumen floats over a running game (its own transparent
 * window). A tap opens the quick menu; it never reaches the game the way a
 * controller combo does.
 */
export function OverlayButton() {
  return (
    <button
      className="overlay-button"
      aria-label="Lumen menüsü"
      onPointerDown={(e) => {
        e.preventDefault();
        invoke("overlay_pressed").catch(() => {});
      }}
    >
      <svg viewBox="0 0 24 24" aria-hidden width="22" height="22">
        <path d="M4 7h16M4 12h16M4 17h16" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" />
      </svg>
    </button>
  );
}
