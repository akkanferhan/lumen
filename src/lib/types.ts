/** Mirrors `src-tauri/emulators.json`; see `manifest.rs` for field docs. */
export interface SystemDef {
  id: string;
  name: string;
  short: string;
  maker: string;
  year: number;
  accent: string;
  extensions: string[];
  folders: string[];
  emulator: string;
  core?: string;
  thumbnails: string;
  bios?: { label: string; anyOf: string[]; optional?: boolean }[];
  folderMarker?: string;
  launchFile?: string;
  /** ASUS power mode for this system (default performance). */
  power?: "silent" | "performance" | "turbo";
  /** Runs on an open-source HLE BIOS when the user's BIOS is missing. */
  fallback?: { emulator: string; core?: string };
}

export interface EmulatorDef {
  id: string;
  name: string;
  exe: string[];
  args: string[];
}

export type Tier = "standard" | "high";
export type PowerSetting = "auto" | "off" | "silent" | "performance" | "turbo";
/** An ASUS power mode (a game's own choice). */
export type PowerMode = "silent" | "performance" | "turbo";
export type GraphicsSetting = "auto" | Tier;
/** Display filter for RetroArch systems. */
export type FilterSetting = "auto" | "original" | "smooth" | "crt" | "lcd";

export interface BackupSettings {
  enabled: boolean;
  dir: string | null;
  lastBackup: number | null;
  /** Newer saves are pulled from the backup folder before each game. */
  sync: boolean;
}

/** A cloud drive's synced folder on this PC (backup goes inside it). */
export interface CloudFolder {
  name: string;
  path: string;
}

export interface BackupReport {
  files: number;
  bytes: number;
}

/** Kids mode (the PIN itself never reaches the UI). */
export interface KidsSettings {
  active: boolean;
  /** Non-null once a PIN is set. */
  pinHash: string | null;
  /** The only collection shown; null shows every emulated game. */
  collection: string | null;
  /** Minutes of play per day; null for no limit. */
  dailyMinutes: number | null;
}

export interface Settings {
  romRoot: string | null;
  backup: BackupSettings;
  sounds: boolean;
  graphics: GraphicsSetting;
  power: PowerSetting;
  raHardcore: boolean;
  theme: string;
  filter: FilterSetting;
  /** Select + Start in game opens the quick menu. */
  quickCombo: boolean;
  /** Touch button over the game opens the quick menu. */
  overlayButton: boolean;
  /** Battery and clock beside the in-game menu button. */
  overlayStatus: boolean;
  kids: KidsSettings;
}

export interface RaUser {
  username: string;
  score: number | null;
}

export interface Device {
  cpu: string;
  model: string | null;
  detectedTier: Tier;
}

export interface BiosStatus {
  label: string;
  optional: boolean;
  found: boolean;
  /** "verified": a known good dump; "unknown": right size, unlisted version; "wrongSize": damaged or not a BIOS. */
  verdict: "verified" | "unknown" | "wrongSize" | null;
  /** The best matching file's name. */
  file: string | null;
}

export interface SystemStatus {
  id: string;
  emulatorInstalled: boolean;
  coreInstalled: boolean;
  /** No BIOS: running on the built-in (HLE) BIOS of the fallback emulator. */
  usingFallback: boolean;
  bios: BiosStatus[];
  gameCount: number;
}

export interface PlayStats {
  /** Last 7 days, oldest first; `start` is local midnight (Unix seconds). */
  days: { start: number; seconds: number }[];
  weekSeconds: number;
  previousWeekSeconds: number;
  year: number;
  yearSeconds: number;
  yearSessions: number;
  yearGames: number;
  topGames: { id: string; seconds: number }[];
  topSystems: { id: string; seconds: number }[];
}

/** The user's own list of games, shown as a library tab. */
export interface Collection {
  id: string;
  name: string;
  games: string[];
}

export interface Overview {
  settings: Settings;
  systems: SystemStatus[];
  runningGameId: string | null;
  device: Device;
  /** Preset level in effect. */
  tier: Tier;
  /** ASUS power-mode driver present (ROG Ally). */
  powerAvailable: boolean;
  /** Lumen starts with Windows. */
  autostart: boolean;
  /** Where backups go (chosen folder or the OneDrive/Documents default). */
  backupDir: string;
  raUser: RaUser | null;
  platformSupported: boolean;
  collections: Collection[];
  /** A Steam account is signed in to the Steam client. */
  steamAccount: boolean;
  /** A Steam Web API key is saved; owned games are listed. */
  steamLinked: boolean;
  /** A SteamGridDB key is saved: covers for games libretro has none for. */
  sgdbLinked: boolean;
}

export interface Game {
  id: string;
  systemId: string;
  title: string;
  thumbName: string;
  path: string;
  favorite: boolean;
  /** Unix seconds. */
  lastPlayed: number | null;
  playSeconds: number;
  /** Power mode chosen for this game; null follows its system. */
  power?: PowerMode | null;
  /** Downloaded cover (SteamGridDB) on disk, for games libretro has no art for. */
  cover?: string | null;
  /** Owned in a store (or in its catalog) but not installed; `path` installs it. */
  notInstalled?: boolean;
}

export type InstallStage = "downloading" | "extracting" | "configuring" | "copying" | "done" | "error";

/** A game found in a folder the user wants to import. */
export interface ImportCandidate {
  path: string;
  name: string;
  /** Detected system; null when the user has to pick one of `choices`. */
  systemId: string | null;
  choices: string[];
  size: number;
}

export interface ImportReport {
  imported: number;
  skipped: number;
  failed: string[];
}

export type ImportMode = "copy" | "move";

export interface InstallProgress {
  /** Emulator id, or `core:<name>` for libretro cores. */
  id: string;
  stage: InstallStage;
  progress: number;
  message: string | null;
}

export interface GameInfo {
  year: string | null;
  developer: string | null;
  publisher: string | null;
  genre: string | null;
  players: string | null;
  description: string | null;
}

export interface HomebrewGame {
  slug: string;
  title: string;
  system: string;
  developer: string;
  year: string | null;
  license: string;
  description: string;
  url: string;
  fileName: string;
  cover: string | null;
  source: string | null;
  /** "classic": a commercial-era game its rights holder made free. */
  collection?: "homebrew" | "classic";
  extract?: boolean;
  archivePath?: string;
}

export interface UninstallPlan {
  method: "recycleBin" | "storeDialog" | "windowsPackage" | "openStore";
  bytes: number | null;
  files: number;
}

export interface AppUpdate {
  version: string;
  notes: string | null;
}

export interface EmulatorUpdate {
  id: string;
  updateAvailable: boolean;
}

export interface Achievement {
  id: number;
  title: string;
  description: string;
  points: number;
  /** Badge image (the locked version while not unlocked). */
  badge: string;
  unlocked: boolean;
}

export interface AchievementProgress {
  gameId: number;
  achievements: Achievement[];
}

/** A save-state slot of the running game (RetroArch systems). */
/** Lumen's extra shortcuts for the running game. */
export interface GameControls {
  rewind: boolean;
  fastForward: boolean;
  /** Multi-disc playlist: the quick menu can switch discs. */
  discs: boolean;
}

/** A screenshot of a game, on disk. */
export interface Shot {
  path: string;
  /** Unix seconds. */
  takenAt: number;
}

export interface StateSlot {
  slot: number;
  /** Unix seconds; null when the slot is empty. */
  savedAt: number | null;
  /** Screenshot taken with the state, as a data URL. */
  thumbnail: string | null;
}

/** What Lumen did about a failed launch. */
export interface FixResult {
  fix: { kind: "reinstallEmulator" | "reinstallCore" | "vcRedist" | "smartAppControl" | "defender"; id?: string };
  /** "fixed": launching again should work; "openedSettings": the user decides in Windows. */
  outcome: "fixed" | "openedSettings";
}

export interface GameExited {
  gameId: string;
  sessionSeconds: number;
  /** Why the emulator closed right after starting, if it did. */
  error: string | null;
}
