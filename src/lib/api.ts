import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { gameExtensions } from "./catalog";
import { todayStart } from "./format";
import { L } from "./i18n";
import { mockApi } from "./mock";
import type {
  AppUpdate,
  BackupReport,
  EmulatorUpdate,
  Game,
  GameExited,
  GameInfo,
  Collection,
  PlayStats,
  FilterSetting,
  FixResult,
  GraphicsSetting,
  ImportCandidate,
  ImportMode,
  ImportReport,
  StateSlot,
  Session,
  M2Action,
  CloudFolder,
  Shot,
  PowerMode,
  GameControls,
  AchievementProgress,
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
  /** Game files picked by the user; empty when cancelled. */
  pickGameFiles(title: string): Promise<string[]>;
  setRomRoot(path: string): Promise<Game[]>;
  /** "Lumen Oyunlar" on the desktop. */
  defaultRomRoot(): Promise<string>;
  scanLibrary(): Promise<Game[]>;
  installSystems(systemIds: string[]): Promise<void>;
  /** `fresh` starts over instead of continuing from the last session. */
  launchGame(gameId: string, fresh?: boolean): Promise<void>;
  /** Kids mode on; an empty PIN keeps the one set before. */
  kidsStart(pin: string, collection: string | null, dailyMinutes: number | null): Promise<void>;
  kidsStop(pin: string): Promise<void>;
  /** Seconds played since `dayStart` (local midnight). */
  playedSince(dayStart: number): Promise<number>;
  /** Kids mode: today's time ran out, the game is being saved and closed. */
  onKidsTimeUp(cb: () => void): () => void;
  /** Repairs what made this game's last launch fail; null when there's nothing to try. */
  autoFix(gameId: string): Promise<FixResult | null>;
  isResumable(gameId: string): Promise<boolean>;
  gameInfo(gameId: string): Promise<GameInfo | null>;
  /** RetroAchievements of the game; null when signed out or not covered. */
  gameAchievements(gameId: string): Promise<AchievementProgress | null>;
  uninstallPlan(gameId: string): Promise<UninstallPlan>;
  uninstallGame(gameId: string): Promise<Game[]>;
  uninstallEmulator(id: string): Promise<void>;
  homebrewInstalled(): Promise<string[]>;
  installHomebrew(slug: string): Promise<Game[]>;
  importScan(paths: string[]): Promise<ImportCandidate[]>;
  importGames(
    items: { path: string; systemId: string }[],
    mode: ImportMode,
  ): Promise<{ report: ImportReport; games: Game[] }>;
  resumeGame(): Promise<void>;
  /** Slots of the running game; null when its emulator has none. */
  stateSlots(): Promise<StateSlot[] | null>;
  gameControls(): Promise<GameControls | null>;
  /** Back to the game, then its next (or previous) disc. */
  switchDisc(next: boolean): Promise<void>;
  /** Back to the game, then a screenshot of it. */
  takeScreenshot(): Promise<void>;
  listScreenshots(gameId: string): Promise<Shot[]>;
  gameSessions(gameId: string): Promise<Session[]>;
  deleteScreenshot(path: string): Promise<void>;
  saveState(slot: number): Promise<StateSlot[]>;
  loadState(slot: number): Promise<void>;
  stopGame(): Promise<void>;
  toggleFavorite(gameId: string): Promise<boolean>;
  setGamePower(gameId: string, power: PowerMode | null): Promise<void>;
  setSounds(enabled: boolean): Promise<void>;
  setGraphics(graphics: GraphicsSetting): Promise<void>;
  setFilter(filter: FilterSetting): Promise<void>;
  /** Checks and saves the key; resolves to the number of owned games. "" signs out. */
  setSteamKey(key: string): Promise<number>;
  /** SteamGridDB key for covers libretro lacks; empty removes it. */
  setSgdbKey(key: string): Promise<void>;
  setQuickCombo(enabled: boolean): Promise<void>;
  setOverlayButton(enabled: boolean): Promise<void>;
  setOverlayStatus(enabled: boolean): Promise<void>;
  setBackButtons(enabled: boolean): Promise<void>;
  setM2Action(action: M2Action): Promise<void>;
  /** "Sorun bildir": writes the report zip to the desktop; returns its path. */
  exportDiagnostics(name: string): Promise<string>;
  /** Which language backend messages are written in. */
  setLanguage(english: boolean): Promise<void>;
  createCollection(name: string): Promise<Collection[]>;
  /** Local midnights come from the UI, which knows the time zone. */
  playStats(today: number, year: number, yearStart: number): Promise<PlayStats>;
  deleteCollection(id: string): Promise<Collection[]>;
  /** Adds the game, or removes it if it's already in the collection. */
  toggleCollection(id: string, gameId: string): Promise<Collection[]>;
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
  setBackupSync(enabled: boolean): Promise<void>;
  cloudFolders(): Promise<CloudFolder[]>;
  updateEmulator(id: string): Promise<void>;
  onInstallProgress(cb: (p: InstallProgress) => void): () => void;
  onGameExited(cb: (e: GameExited) => void): () => void;
  /** The game's window is up (or Lumen stopped waiting): loading is over. */
  onGameShown(cb: () => void): () => void;
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
  pickFolder: async (title = L("Oyun klasörünü seç", "Choose your game folder")) => {
    const picked = await open({ directory: true, title });
    return typeof picked === "string" ? picked : null;
  },
  pickGameFiles: async (title) => {
    const picked = await open({
      multiple: true,
      title,
      filters: [
        { name: L("Oyun dosyaları", "Game files"), extensions: gameExtensions() },
        { name: L("Tüm dosyalar", "All files"), extensions: ["*"] },
      ],
    });
    return Array.isArray(picked) ? picked : typeof picked === "string" ? [picked] : [];
  },
  setRomRoot: (path) => invoke("set_rom_root", { path }),
  defaultRomRoot: () => invoke("default_rom_root"),
  scanLibrary: () => invoke("scan_library"),
  installSystems: (systemIds) => invoke("install_systems", { systemIds }),
  launchGame: (gameId, fresh) => invoke("launch_game", { gameId, fresh: !!fresh, dayStart: todayStart() }),
  kidsStart: (pin, collection, dailyMinutes) => invoke("kids_start", { pin, collection, dailyMinutes }),
  kidsStop: (pin) => invoke("kids_stop", { pin }),
  playedSince: (dayStart) => invoke("played_since", { dayStart }),
  onKidsTimeUp: (cb) => subscribe<null>("kids-time-up", () => cb()),
  autoFix: (gameId) => invoke("auto_fix", { gameId }),
  isResumable: (gameId) => invoke("is_resumable", { gameId }),
  gameInfo: (gameId) => invoke("game_info", { gameId }),
  gameAchievements: (gameId) => invoke("game_achievements", { gameId }),
  uninstallPlan: (gameId) => invoke("uninstall_plan", { gameId }),
  uninstallGame: (gameId) => invoke("uninstall_game", { gameId }),
  uninstallEmulator: (id) => invoke("uninstall_emulator", { id }),
  homebrewInstalled: () => invoke("homebrew_installed"),
  installHomebrew: (slug) => invoke("install_homebrew", { slug }),
  importScan: (paths) => invoke("import_scan", { paths }),
  importGames: (items, mode) => invoke("import_games", { items, mode }),
  resumeGame: () => invoke("resume_game"),
  stateSlots: () => invoke("state_slots"),
  gameControls: () => invoke("game_controls"),
  switchDisc: (next) => invoke("switch_disc", { next }),
  takeScreenshot: () => invoke("take_screenshot"),
  listScreenshots: (gameId) => invoke("list_screenshots", { gameId }),
  gameSessions: (gameId) => invoke("game_sessions", { gameId }),
  deleteScreenshot: (path) => invoke("delete_screenshot", { path }),
  saveState: (slot) => invoke("save_state", { slot }),
  loadState: (slot) => invoke("load_state", { slot }),
  stopGame: () => invoke("stop_game"),
  toggleFavorite: (gameId) => invoke("toggle_favorite", { gameId }),
  setGamePower: (gameId, power) => invoke("set_game_power", { gameId, power }),
  setSounds: (enabled) => invoke("set_sounds", { enabled }),
  setGraphics: (graphics) => invoke("set_graphics", { graphics }),
  setFilter: (filter) => invoke("set_filter", { filter }),
  setSteamKey: (key) => invoke("set_steam_key", { key }),
  setSgdbKey: (key) => invoke("set_sgdb_key", { key }),
  setQuickCombo: (enabled) => invoke("set_quick_combo", { enabled }),
  setOverlayButton: (enabled) => invoke("set_overlay_button", { enabled }),
  setOverlayStatus: (enabled) => invoke("set_overlay_status", { enabled }),
  setBackButtons: (enabled) => invoke("set_back_buttons", { enabled }),
  setM2Action: (action) => invoke("set_m2_action", { action }),
  exportDiagnostics: (name) => invoke("export_diagnostics", { name }),
  setLanguage: (english) => invoke("set_language", { english }),
  createCollection: (name) => invoke("create_collection", { name }),
  playStats: (today, year, yearStart) => invoke("play_stats", { today, year, yearStart }),
  deleteCollection: (id) => invoke("delete_collection", { id }),
  toggleCollection: (id, gameId) => invoke("toggle_collection", { id, gameId }),
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
  setBackupSync: (enabled) => invoke("set_backup_sync", { enabled }),
  cloudFolders: () => invoke("cloud_folders"),
  updateEmulator: (id) => invoke("update_emulator", { id }),
  onInstallProgress: (cb) => subscribe("install-progress", cb),
  onGameExited: (cb) => subscribe("game-exited", cb),
  onGameShown: (cb) => subscribe<null>("game-shown", () => cb()),
  onQuickMenu: (cb) => subscribe<null>("quick-menu", () => cb()),
  onSystemResumed: (cb) => subscribe<null>("system-resumed", () => cb()),
};

/** In a plain browser (UI development on macOS) everything is simulated. */
export const api: Api = isTauri ? tauriApi : mockApi;
