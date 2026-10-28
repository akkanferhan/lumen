import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { mockApi } from "./mock";
import type {
  AppUpdate,
  BackupReport,
  EmulatorUpdate,
  Game,
  GameExited,
  GameInfo,
  GraphicsSetting,
  InstallProgress,
  Overview,
  PowerSetting,
} from "./types";

export interface Api {
  getOverview(): Promise<Overview>;
  getLibrary(): Promise<Game[]>;
  pickFolder(): Promise<string | null>;
  setRomRoot(path: string): Promise<Game[]>;
  scanLibrary(): Promise<Game[]>;
  installSystems(systemIds: string[]): Promise<void>;
  /** `fresh` starts over instead of continuing from the last session. */
  launchGame(gameId: string, fresh?: boolean): Promise<void>;
  isResumable(gameId: string): Promise<boolean>;
  gameInfo(gameId: string): Promise<GameInfo | null>;
  homebrewInstalled(): Promise<string[]>;
  installHomebrew(slug: string): Promise<Game[]>;
  resumeGame(): Promise<void>;
  stopGame(): Promise<void>;
  toggleFavorite(gameId: string): Promise<boolean>;
  setSounds(enabled: boolean): Promise<void>;
  setGraphics(graphics: GraphicsSetting): Promise<void>;
  checkUpdates(): Promise<EmulatorUpdate[]>;
  checkAppUpdate(): Promise<AppUpdate | null>;
  installAppUpdate(): Promise<void>;
  onAppUpdateProgress(cb: (progress: number) => void): () => void;
  setPower(power: PowerSetting): Promise<void>;
  setAutostart(enabled: boolean): Promise<void>;
  backupNow(): Promise<BackupReport>;
  restoreBackup(): Promise<BackupReport>;
  setBackup(enabled: boolean, dir?: string | null): Promise<void>;
  updateEmulator(id: string): Promise<void>;
  onInstallProgress(cb: (p: InstallProgress) => void): () => void;
  onGameExited(cb: (e: GameExited) => void): () => void;
  onQuickMenu(cb: () => void): () => void;
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
  launchGame: (gameId, fresh) => invoke("launch_game", { gameId, fresh: !!fresh }),
  isResumable: (gameId) => invoke("is_resumable", { gameId }),
  gameInfo: (gameId) => invoke("game_info", { gameId }),
  homebrewInstalled: () => invoke("homebrew_installed"),
  installHomebrew: (slug) => invoke("install_homebrew", { slug }),
  resumeGame: () => invoke("resume_game"),
  stopGame: () => invoke("stop_game"),
  toggleFavorite: (gameId) => invoke("toggle_favorite", { gameId }),
  setSounds: (enabled) => invoke("set_sounds", { enabled }),
  setGraphics: (graphics) => invoke("set_graphics", { graphics }),
  checkUpdates: () => invoke("check_updates"),
  checkAppUpdate: () => invoke("check_app_update"),
  installAppUpdate: () => invoke("install_app_update"),
  onAppUpdateProgress: (cb) => subscribe<number>("app-update-progress", cb),
  setPower: (power) => invoke("set_power", { power }),
  setAutostart: (enabled) => invoke("set_autostart", { enabled }),
  backupNow: () => invoke("backup_now"),
  restoreBackup: () => invoke("restore_backup"),
  setBackup: (enabled, dir) => invoke("set_backup", { enabled, dir: dir ?? null }),
  updateEmulator: (id) => invoke("update_emulator", { id }),
  onInstallProgress: (cb) => subscribe("install-progress", cb),
  onGameExited: (cb) => subscribe("game-exited", cb),
  onQuickMenu: (cb) => subscribe<null>("quick-menu", () => cb()),
};

/** In a plain browser (UI development on macOS) everything is simulated. */
export const api: Api = isTauri ? tauriApi : mockApi;
