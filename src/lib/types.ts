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
}

export interface EmulatorDef {
  id: string;
  name: string;
  exe: string[];
  args: string[];
}

export interface Settings {
  romRoot: string | null;
  sounds: boolean;
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
  bios: BiosStatus[];
  gameCount: number;
}

export interface Overview {
  settings: Settings;
  systems: SystemStatus[];
  runningGameId: string | null;
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

export interface GameExited {
  gameId: string;
  sessionSeconds: number;
}
