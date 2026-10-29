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
  FixResult,
  GraphicsSetting,
  ImportCandidate,
  ImportMode,
  ImportReport,
  InstallProgress,
  Overview,
  PowerSetting,
  RaUser,
  UninstallPlan,
} from "./types";

export interface Api {
  getOverview(): Promise<Overview>;
  getLibrary(): Promise<Game[]>;
  takeCrashReport(): Promise<string | null>;
  pickFolder(title?: string): Promise<string | null>;
  setRomRoot(path: string): Promise<Game[]>;
  /** "Lumen Oyunlar" on the desktop. */
  defaultRomRoot(): Promise<string>;
  scanLibrary(): Promise<Game[]>;
  installSystems(systemIds: string[]): Promise<void>;
  /** `fresh` starts over instead of continuing from the last session. */
  launchGame(gameId: string, fresh?: boolean): Promise<void>;
  /** Repairs what made this game's last launch fail; null when there's nothing to try. */
  autoFix(gameId: string): Promise<FixResult | null>;
  isResumable(gameId: string): Promise<boolean>;
  gameInfo(gameId: string): Promise<GameInfo | null>;
  uninstallPlan(gameId: string): Promise<UninstallPlan>;
  uninstallGame(gameId: string): Promise<Game[]>;
  uninstallEmulator(id: string): Promise<void>;
  homebrewInstalled(): Promise<string[]>;
  installHomebrew(slug: string): Promise<Game[]>;
  importScan(path: string): Promise<ImportCandidate[]>;
  importGames(
    items: { path: string; systemId: string }[],
    mode: ImportMode,
  ): Promise<{ report: ImportReport; games: Game[] }>;
  resumeGame(): Promise<void>;
  stopGame(): Promise<void>;
  toggleFavorite(gameId: string): Promise<boolean>;
  setSounds(enabled: boolean): Promise<void>;
  setGraphics(graphics: GraphicsSetting): Promise<void>;
  setTheme(theme: string): Promise<void>;
  checkUpdates(): Promise<EmulatorUpdate[]>;
  checkAppUpdate(): Promise<AppUpdate | null>;
  installAppUpdate(): Promise<void>;
  onAppUpdateProgress(cb: (progress: number) => void): () => void;
  setPower(power: PowerSetting): Promise<void>;
  setAutostart(enabled: boolean): Promise<void>;
  backupNow(): Promise<BackupReport>;
  raLogin(username: string, password: string): Promise<RaUser>;
  raLogout(): Promise<void>;
  raRefresh(): Promise<RaUser | null>;
  setRaHardcore(enabled: boolean): Promise<void>;
  restoreBackup(): Promise<BackupReport>;
  setBackup(enabled: boolean, dir?: string | null): Promise<void>;
  updateEmulator(id: string): Promise<void>;
  onInstallProgress(cb: (p: InstallProgress) => void): () => void;
  onGameExited(cb: (e: GameExited) => void): () => void;
  onQuickMenu(cb: () => void): () => void;
  onSystemResumed(cb: () => void): () => void;
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
  takeCrashReport: () => invoke("take_crash_report"),
  pickFolder: async (title = "Oyun klasörünü seç") => {
    const picked = await open({ directory: true, title });
    return typeof picked === "string" ? picked : null;
  },
  setRomRoot: (path) => invoke("set_rom_root", { path }),
  defaultRomRoot: () => invoke("default_rom_root"),
  scanLibrary: () => invoke("scan_library"),
  installSystems: (systemIds) => invoke("install_systems", { systemIds }),
  launchGame: (gameId, fresh) => invoke("launch_game", { gameId, fresh: !!fresh }),
  autoFix: (gameId) => invoke("auto_fix", { gameId }),
  isResumable: (gameId) => invoke("is_resumable", { gameId }),
  gameInfo: (gameId) => invoke("game_info", { gameId }),
  uninstallPlan: (gameId) => invoke("uninstall_plan", { gameId }),
  uninstallGame: (gameId) => invoke("uninstall_game", { gameId }),
  uninstallEmulator: (id) => invoke("uninstall_emulator", { id }),
  homebrewInstalled: () => invoke("homebrew_installed"),
  installHomebrew: (slug) => invoke("install_homebrew", { slug }),
  importScan: (path) => invoke("import_scan", { path }),
  importGames: (items, mode) => invoke("import_games", { items, mode }),
  resumeGame: () => invoke("resume_game"),
  stopGame: () => invoke("stop_game"),
  toggleFavorite: (gameId) => invoke("toggle_favorite", { gameId }),
  setSounds: (enabled) => invoke("set_sounds", { enabled }),
  setGraphics: (graphics) => invoke("set_graphics", { graphics }),
  setTheme: (theme) => invoke("set_theme", { theme }),
  checkUpdates: () => invoke("check_updates"),
  checkAppUpdate: () => invoke("check_app_update"),
  installAppUpdate: () => invoke("install_app_update"),
  onAppUpdateProgress: (cb) => subscribe<number>("app-update-progress", cb),
  setPower: (power) => invoke("set_power", { power }),
  setAutostart: (enabled) => invoke("set_autostart", { enabled }),
  backupNow: () => invoke("backup_now"),
  raLogin: (username, password) => invoke("ra_login", { username, password }),
  raLogout: () => invoke("ra_logout"),
  raRefresh: () => invoke("ra_refresh"),
  setRaHardcore: (enabled) => invoke("set_ra_hardcore", { enabled }),
  restoreBackup: () => invoke("restore_backup"),
  setBackup: (enabled, dir) => invoke("set_backup", { enabled, dir: dir ?? null }),
  updateEmulator: (id) => invoke("update_emulator", { id }),
  onInstallProgress: (cb) => subscribe("install-progress", cb),
  onGameExited: (cb) => subscribe("game-exited", cb),
  onQuickMenu: (cb) => subscribe<null>("quick-menu", () => cb()),
  onSystemResumed: (cb) => subscribe<null>("system-resumed", () => cb()),
};

/** In a plain browser (UI development on macOS) everything is simulated. */
export const api: Api = isTauri ? tauriApi : mockApi;
