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
export type GraphicsSetting = "auto" | Tier;

export interface Settings {
  romRoot: string | null;
  sounds: boolean;
  graphics: GraphicsSetting;
  power: PowerSetting;
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
  platformSupported: boolean;
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
}

export type InstallStage = "downloading" | "extracting" | "configuring" | "done" | "error";

export interface InstallProgress {
  /** Emulator id, or `core:<name>` for libretro cores. */
  id: string;
  stage: InstallStage;
  progress: number;
  message: string | null;
}

export interface EmulatorUpdate {
  id: string;
  updateAvailable: boolean;
}

export interface GameExited {
  gameId: string;
  sessionSeconds: number;
}
