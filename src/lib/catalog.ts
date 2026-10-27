import manifestJson from "../../src-tauri/emulators.json";
import type { EmulatorDef, Game, SystemDef } from "./types";

export const systems = manifestJson.systems as unknown as SystemDef[];
export const emulators = manifestJson.emulators as unknown as EmulatorDef[];

const systemsById = new Map(systems.map((s) => [s.id, s]));
const emulatorsById = new Map(emulators.map((e) => [e.id, e]));

export function systemOf(id: string): SystemDef | undefined {
  return systemsById.get(id);
}

export function emulatorOf(id: string): EmulatorDef | undefined {
  return emulatorsById.get(id);
}

/** Human readable name for an install job id (`dolphin`, `core:snes9x`). */
export function installLabel(id: string): string {
  if (id === "firmware:ps3") return "PS3 firmware (Sony)";
  if (id === "hdd:xbox") return "Xbox HDD imajı";
  if (id.startsWith("core:")) {
    const core = id.slice(5);
    const users = systems.filter((s) => s.core === core || s.fallback?.core === core).map((s) => s.short);
    return `${users.join(" / ") || core} çekirdeği`;
  }
  return emulatorOf(id)?.name ?? id;
}

export function boxArtUrl(game: Pick<Game, "systemId" | "thumbName">): string | null {
  const system = systemOf(game.systemId);
  if (!system) return null;
  return `https://thumbnails.libretro.com/${encodeURIComponent(system.thumbnails)}/Named_Boxarts/${encodeURIComponent(game.thumbName)}.png`;
}
