import manifestJson from "../../src-tauri/emulators.json";
import homebrewJson from "../../src-tauri/homebrew.json";
import type { EmulatorDef, Game, HomebrewGame, SystemDef } from "./types";

/** Store launchers shown next to the emulated systems. */
const PC_SYSTEMS: SystemDef[] = [
  { id: "steam", name: "Steam", short: "Steam", maker: "Valve", year: 2003, accent: "#66c0f4", extensions: [], folders: [], emulator: "steam", thumbnails: "" },
  { id: "epic", name: "Epic Games", short: "Epic", maker: "Epic Games", year: 2018, accent: "#c8c8c8", extensions: [], folders: [], emulator: "epic", thumbnails: "" },
  // Games installed by the Xbox app (Game Pass, purchases). Not the original Xbox system.
  { id: "xboxapp", name: "Xbox uygulaması", short: "Xbox PC", maker: "Microsoft", year: 2020, accent: "#3fa142", extensions: [], folders: [], emulator: "xboxapp", thumbnails: "" },
];

export const isPcSystem = (id: string) => id === "steam" || id === "epic" || id === "xboxapp";

export const systems = [...PC_SYSTEMS, ...(manifestJson.systems as unknown as SystemDef[])];
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
export const homebrew = homebrewJson as HomebrewGame[];

export function installLabel(id: string): string {
  if (id.startsWith("homebrew:")) return homebrew.find((g) => `homebrew:${g.slug}` === id)?.title ?? "Homebrew";
  if (id === "import") return "Oyunlar içe aktarılıyor";
  if (id === "fix:vcredist") return "Microsoft Visual C++ çalışma zamanı";
  if (id === "firmware:ps3") return "PS3 firmware (Sony)";
  if (id === "hdd:xbox") return "Xbox HDD imajı";
  if (id === "firmware:vita") return "PS Vita firmware (Sony)";
  if (id === "firmware:vita-fonts") return "PS Vita font paketi (Sony)";
  if (id.startsWith("core:")) {
    const core = id.slice(5);
    const users = systems.filter((s) => s.core === core || s.fallback?.core === core).map((s) => s.short);
    return `${users.join(" / ") || core} çekirdeği`;
  }
  return emulatorOf(id)?.name ?? id;
}

type ThumbKind = "Named_Boxarts" | "Named_Snaps" | "Named_Titles";

/** Same file-stem cleanup the library scan applies to `thumbName`. */
const thumbOf = (fileName: string) => fileName.replace(/\.[^.]+$/, "").replace(/[&*/:`<>?\\|"]/g, "_");

/** Free-store games by `<system>/<thumbName>`: libretro has no art for them. */
const storeCovers = new Map(
  homebrewJson
    .filter((g) => g.cover)
    .map((g) => [`${g.system}/${thumbOf(g.fileName)}`, g.cover as string]),
);

function thumbnailUrl(game: Pick<Game, "systemId" | "thumbName">, kind: ThumbKind): string | null {
  if (game.systemId === "steam") {
    // thumbName is the Steam app id; Steam's own portrait library art.
    return kind === "Named_Boxarts"
      ? `https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/${game.thumbName}/library_600x900_2x.jpg`
      : null;
  }
  if (game.systemId === "epic") return null;
  if (game.systemId === "xboxapp") {
    // thumbName is the Microsoft Store poster URL (resolved during the scan).
    return kind === "Named_Boxarts" && game.thumbName.startsWith("https://") ? game.thumbName : null;
  }
  const system = systemOf(game.systemId);
  if (!system) return null;
  // The store's art is a screenshot: it stands in for every kind.
  const storeCover = storeCovers.get(`${game.systemId}/${game.thumbName}`);
  if (storeCover) return kind === "Named_Titles" ? null : storeCover;
  return `https://thumbnails.libretro.com/${encodeURIComponent(system.thumbnails)}/${kind}/${encodeURIComponent(game.thumbName)}.png`;
}

export const boxArtUrl = (game: Pick<Game, "systemId" | "thumbName">) => thumbnailUrl(game, "Named_Boxarts");

/** Title screen and in-game screenshot. */
export const screenshotUrls = (game: Pick<Game, "systemId" | "thumbName">) =>
  (["Named_Titles", "Named_Snaps"] as const).map((k) => thumbnailUrl(game, k)).filter((u): u is string => !!u);
