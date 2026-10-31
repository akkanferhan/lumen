import { convertFileSrc } from "@tauri-apps/api/core";
import manifestJson from "../../src-tauri/emulators.json";
import homebrewJson from "../../src-tauri/homebrew.json";
import { L } from "./i18n";
import type { EmulatorDef, Game, HomebrewGame, SystemDef } from "./types";

/** Store launchers shown next to the emulated systems. */
const PC_SYSTEMS: SystemDef[] = [
  { id: "steam", name: "Steam", short: "Steam", maker: "Valve", year: 2003, accent: "#66c0f4", extensions: [], folders: [], emulator: "steam", thumbnails: "" },
  { id: "epic", name: "Epic Games", short: "Epic", maker: "Epic Games", year: 2018, accent: "#c8c8c8", extensions: [], folders: [], emulator: "epic", thumbnails: "" },
  // Games installed by the Xbox app (Game Pass, purchases). Not the original Xbox system.
  { id: "xboxapp", name: L("Xbox uygulaması", "Xbox app"), short: "Xbox PC", maker: "Microsoft", year: 2020, accent: "#3fa142", extensions: [], folders: [], emulator: "xboxapp", thumbnails: "" },
  { id: "gog", name: "GOG", short: "GOG", maker: "GOG.com", year: 2008, accent: "#a855f7", extensions: [], folders: [], emulator: "gog", thumbnails: "" },
  { id: "itch", name: "itch.io", short: "itch", maker: "itch.io", year: 2013, accent: "#fa5c5c", extensions: [], folders: [], emulator: "itch", thumbnails: "" },
];

export const isPcSystem = (id: string) => PC_SYSTEMS.some((s) => s.id === id);

/** Every file extension an emulator opens, for the file picker's filter. */
export const gameExtensions = () => [
  ...new Set((manifestJson.systems as unknown as SystemDef[]).flatMap((s) => s.extensions)),
];

/** Platforms listed first, in this order; the rest keep their own order. */
const LEADING: ((s: SystemDef) => boolean)[] = [
  (s) => s.id === "steam",
  // The Xbox app's PC games, then the Xbox consoles.
  (s) => s.id === "xboxapp" || s.maker === "Microsoft",
  (s) => s.id === "epic",
  (s) => s.maker === "Sony",
  (s) => s.maker === "Nintendo",
];
const rank = (s: SystemDef) => {
  const i = LEADING.findIndex((matches) => matches(s));
  return i === -1 ? LEADING.length : i;
};

/** Every platform, Steam, Xbox, Epic, PlayStation and Nintendo first (a stable sort). */
export const systems = [...PC_SYSTEMS, ...(manifestJson.systems as unknown as SystemDef[])].sort(
  (a, b) => rank(a) - rank(b),
);
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
  if (id === "import") return L("Oyunlar içe aktarılıyor", "Importing games");
  if (id === "shaders") return L("Görüntü filtreleri", "Display filters");
  if (id === "fix:vcredist") return L("Microsoft Visual C++ çalışma zamanı", "Microsoft Visual C++ runtime");
  if (id === "firmware:ps3") return "PS3 firmware (Sony)";
  if (id === "hdd:xbox") return L("Xbox HDD imajı", "Xbox HDD image");
  if (id === "firmware:vita") return "PS Vita firmware (Sony)";
  if (id === "firmware:vita-fonts") return L("PS Vita font paketi (Sony)", "PS Vita font pack (Sony)");
  if (id.startsWith("core:")) {
    const core = id.slice(5);
    const users = systems.filter((s) => s.core === core || s.fallback?.core === core).map((s) => s.short);
    return L(`${users.join(" / ") || core} çekirdeği`, `${users.join(" / ") || core} core`);
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
  // Resolved during the scan: the Store poster, GOG product art, itch cover.
  if (["xboxapp", "gog", "itch"].includes(game.systemId)) {
    return kind === "Named_Boxarts" && game.thumbName.startsWith("https://") ? game.thumbName : null;
  }
  const system = systemOf(game.systemId);
  if (!system) return null;
  // The store's art is a screenshot: it stands in for every kind.
  const storeCover = storeCovers.get(`${game.systemId}/${game.thumbName}`);
  if (storeCover) return kind === "Named_Titles" ? null : storeCover;
  return `https://thumbnails.libretro.com/${encodeURIComponent(system.thumbnails)}/${kind}/${encodeURIComponent(game.thumbName)}.png`;
}

/** Box art: a downloaded cover (SteamGridDB) wins, else libretro / the store. */
export const boxArtUrl = (game: Pick<Game, "systemId" | "thumbName"> & { cover?: string | null }) =>
  game.cover ? convertFileSrc(game.cover) : thumbnailUrl(game, "Named_Boxarts");

/** Title screen and in-game screenshot. */
export const screenshotUrls = (game: Pick<Game, "systemId" | "thumbName">) =>
  (["Named_Titles", "Named_Snaps"] as const).map((k) => thumbnailUrl(game, k)).filter((u): u is string => !!u);
