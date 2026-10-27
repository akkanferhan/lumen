import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { mockApi } from "./mock";
import type { Game, GameExited, GraphicsSetting, InstallProgress, Overview } from "./types";

export interface Api {
  getOverview(): Promise<Overview>;
  getLibrary(): Promise<Game[]>;
  pickFolder(): Promise<string | null>;
  setRomRoot(path: string): Promise<Game[]>;
  scanLibrary(): Promise<Game[]>;
  installSystems(systemIds: string[]): Promise<void>;
  launchGame(gameId: string): Promise<void>;
  stopGame(): Promise<void>;
  toggleFavorite(gameId: string): Promise<boolean>;
  setSounds(enabled: boolean): Promise<void>;
  setGraphics(graphics: GraphicsSetting): Promise<void>;
  onInstallProgress(cb: (p: InstallProgress) => void): () => void;
  onGameExited(cb: (e: GameExited) => void): () => void;
}

export const isTauri = "__TAURI_INTERNALS__" in window;

/** Subscribes synchronously from the caller's view; the Tauri listener resolves later. */
function subscribe<T>(event: string, cb: (payload: T) => void): () => void {
  let unlisten: (() => void) | null = null;
  let cancelled = false;
  listen<T>(event, (e) => cb(e.payload)).then((fn) => {
    if (cancelled) fn();
    else unlisten = fn;
  });
  return () => {
    cancelled = true;
    unlisten?.();
  };
}

const tauriApi: Api = {
  getOverview: () => invoke("get_overview"),
  getLibrary: () => invoke("get_library"),
  pickFolder: async () => {
    const picked = await open({ directory: true, title: "Oyun klasörünü seç" });
    return typeof picked === "string" ? picked : null;
  },
  setRomRoot: (path) => invoke("set_rom_root", { path }),
  scanLibrary: () => invoke("scan_library"),
  installSystems: (systemIds) => invoke("install_systems", { systemIds }),
  launchGame: (gameId) => invoke("launch_game", { gameId }),
  stopGame: () => invoke("stop_game"),
  toggleFavorite: (gameId) => invoke("toggle_favorite", { gameId }),
  setSounds: (enabled) => invoke("set_sounds", { enabled }),
  setGraphics: (graphics) => invoke("set_graphics", { graphics }),
  onInstallProgress: (cb) => subscribe("install-progress", cb),
  onGameExited: (cb) => subscribe("game-exited", cb),
};

/** In a plain browser (UI development on macOS) everything is simulated. */
export const api: Api = isTauri ? tauriApi : mockApi;
