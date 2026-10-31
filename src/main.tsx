import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { OverlayButton } from "./components/OverlayButton";
import "./styles.css";
import "./themes.css";

// The same bundle serves the small button window Lumen floats over games.
const overlay =
  new URLSearchParams(location.search).has("overlay") ||
  ("__TAURI_INTERNALS__" in window &&
  (window as unknown as { __TAURI_INTERNALS__: { metadata?: { currentWindow?: { label?: string } } } })
    .__TAURI_INTERNALS__.metadata?.currentWindow?.label === "overlay");

// A console UI has no right-click menu (the App-menu key, which the Ally's
// back buttons are bound to, would open the webview's at the cursor).
if (import.meta.env.PROD) window.addEventListener("contextmenu", (e) => e.preventDefault());

// Transparent from the first frame (no dark square before React mounts).
if (overlay) document.documentElement.classList.add("overlay-window");

createRoot(document.getElementById("root")!).render(
  <StrictMode>{overlay ? <OverlayButton /> : <App />}</StrictMode>,
);
