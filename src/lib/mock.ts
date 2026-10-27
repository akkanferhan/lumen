// Browser-only stand-in for the Rust layer so the UI can be built and tested
// on any machine with `npm run dev`. Install and play are simulated.

import type { Api } from "./api";
import { emulatorOf, systemOf, systems } from "./catalog";
import { cleanTitle } from "./format";
import type { Game, GameExited, GraphicsSetting, InstallProgress, Overview } from "./types";

const SAMPLE: [string, string][] = [
  ["snes", "Super Mario World (USA)"],
  ["snes", "Super Metroid (Japan, USA) (En,Ja)"],
  ["snes", "Legend of Zelda, The - A Link to the Past (USA)"],
  ["snes", "Chrono Trigger (USA)"],
  ["snes", "Donkey Kong Country (USA) (Rev 2)"],
  ["nes", "Super Mario Bros. 3 (USA)"],
  ["nes", "Mega Man 2 (USA)"],
  ["n64", "Super Mario 64 (USA)"],
  ["n64", "Legend of Zelda, The - Ocarina of Time (USA)"],
  ["gba", "Metroid Fusion (USA)"],
  ["gba", "Pokemon - Emerald Version (USA, Europe)"],
  ["gba", "Advance Wars (USA)"],
  ["genesis", "Sonic The Hedgehog (USA, Europe)"],
  ["genesis", "Streets of Rage 2 (USA)"],
  ["ps1", "Castlevania - Symphony of the Night (USA)"],
  ["ps1", "Crash Bandicoot (USA)"],
  ["ps1", "Spyro the Dragon (USA)"],
  ["ps2", "Shadow of the Colossus (USA)"],
  ["ps2", "Okami (USA)"],
  ["gc", "Metroid Prime (USA)"],
  ["gc", "Legend of Zelda, The - The Wind Waker (USA)"],
  ["psp", "Patapon (USA)"],
  ["dreamcast", "Jet Grind Radio (USA)"],
];

interface MockStore {
  romRoot: string | null;
  installed: string[];
  user: Record<string, { favorite: boolean; lastPlayed: number | null; playSeconds: number }>;
}

const KEY = "lumen-mock";

function load(): MockStore {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) return JSON.parse(raw);
  } catch {
    /* storage unavailable: start fresh */
  }
  return { romRoot: null, installed: [], user: {} };
}

let store = load();
// `?demo` skips onboarding with a sample library (handy for screenshots).
if (new URLSearchParams(location.search).has("demo") && !store.romRoot) {
  store = { romRoot: "C:\\Users\\Oyuncu\\Oyunlar", installed: [], user: { "mock-3": { favorite: true, lastPlayed: Math.floor(Date.now() / 1000) - 86_400, playSeconds: 7_420 } } };
}
function save() {
  try {
    localStorage.setItem(KEY, JSON.stringify(store));
  } catch {
    /* ignore */
  }
}

const progressListeners = new Set<(p: InstallProgress) => void>();
const exitListeners = new Set<(e: GameExited) => void>();
let running: { id: string; started: number } | null = null;
let graphics: GraphicsSetting = "auto";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function games(): Game[] {
  if (!store.romRoot) return [];
  return SAMPLE.map(([systemId, stem], i) => {
    const id = `mock-${i}`;
    const user = store.user[id] ?? { favorite: false, lastPlayed: null, playSeconds: 0 };
    return { id, systemId, title: cleanTitle(stem), thumbName: stem, path: `${store.romRoot}/${systemId}/${stem}`, ...user };
  }).sort((a, b) => a.title.localeCompare(b.title, "tr"));
}

function overview(): Overview {
  const list = games();
  return {
    settings: { romRoot: store.romRoot, sounds: true, graphics },
    runningGameId: running?.id ?? null,
    device: { cpu: "AMD Ryzen Z1 Extreme", model: "ROG Ally / Ally X (Z1 Extreme)", detectedTier: "high" },
    tier: graphics === "auto" ? "high" : graphics,
    platformSupported: true,
    systems: systems.map((s) => ({
      id: s.id,
      emulatorInstalled: store.installed.includes(s.emulator),
      coreInstalled: !s.core || store.installed.includes(`core:${s.core}`),
      // Demo: PS1 has no BIOS (runs on the fallback), PS2 lacks one (blocked).
      usingFallback: s.id === "ps1",
      bios: (s.bios ?? []).map((b) => ({ label: b.label, optional: !!b.optional, found: s.id !== "ps2" && s.id !== "ps1" })),
      gameCount: list.filter((g) => g.systemId === s.id).length,
    })),
  };
}

async function simulateInstall(id: string) {
  const emit = (p: Omit<InstallProgress, "id">) => progressListeners.forEach((cb) => cb({ id, ...p }));
  for (let p = 0; p <= 1; p += 0.08) {
    emit({ stage: "downloading", progress: p, message: null });
    await sleep(70);
  }
  emit({ stage: "extracting", progress: 1, message: null });
  await sleep(300);
  emit({ stage: "configuring", progress: 1, message: null });
  await sleep(200);
  emit({ stage: "done", progress: 1, message: null });
  store.installed.push(id);
  save();
}

function touchUser(id: string) {
  return (store.user[id] ??= { favorite: false, lastPlayed: null, playSeconds: 0 });
}

export const mockApi: Api = {
  getOverview: async () => overview(),
  getLibrary: async () => games(),
  pickFolder: async () => "C:\\Users\\Oyuncu\\Oyunlar",
  setRomRoot: async (path) => {
    store.romRoot = path;
    save();
    await sleep(400);
    return games();
  },
  scanLibrary: async () => {
    await sleep(500);
    return games();
  },
  installSystems: async (systemIds) => {
    const jobs = new Set<string>();
    for (const id of systemIds) {
      const system = systemOf(id);
      if (!system || !emulatorOf(system.emulator)) continue;
      if (!store.installed.includes(system.emulator)) jobs.add(system.emulator);
      if (system.core && !store.installed.includes(`core:${system.core}`)) jobs.add(`core:${system.core}`);
    }
    for (const job of jobs) await simulateInstall(job);
  },
  launchGame: async (gameId) => {
    running = { id: gameId, started: Date.now() };
  },
  stopGame: async () => {
    if (!running) return;
    const sessionSeconds = Math.round((Date.now() - running.started) / 1000);
    const user = touchUser(running.id);
    user.playSeconds += sessionSeconds;
    user.lastPlayed = Math.floor(Date.now() / 1000);
    save();
    const event = { gameId: running.id, sessionSeconds };
    running = null;
    exitListeners.forEach((cb) => cb(event));
  },
  toggleFavorite: async (gameId) => {
    const user = touchUser(gameId);
    user.favorite = !user.favorite;
    save();
    return user.favorite;
  },
  setSounds: async () => {},
  checkUpdates: async () => {
    await sleep(700);
    // Demo: whatever is installed has an update except RetroArch.
    return store.installed
      .filter((id) => !id.startsWith("core:"))
      .map((id) => ({ id, updateAvailable: id !== "retroarch" }));
  },
  updateEmulator: async (id) => {
    store.installed = store.installed.filter((x) => x !== id);
    await simulateInstall(id);
  },
  setGraphics: async (value) => {
    graphics = value;
  },
  onInstallProgress: (cb) => {
    progressListeners.add(cb);
    return () => progressListeners.delete(cb);
  },
  onGameExited: (cb) => {
    exitListeners.add(cb);
    return () => exitListeners.delete(cb);
  },
};

/** Dev helper: `window.lumenReset()` in the console restarts onboarding. */
(window as unknown as { lumenReset: () => void }).lumenReset = () => {
  store = { romRoot: null, installed: [], user: {} };
  save();
  location.reload();
};
