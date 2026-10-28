// Browser-only stand-in for the Rust layer so the UI can be built and tested
// on any machine with `npm run dev`. Install and play are simulated.

import type { Api } from "./api";
import { emulatorOf, systemOf, systems } from "./catalog";
import { cleanTitle } from "./format";
import type { BackupSettings, Game, GameExited, GameInfo, GraphicsSetting, InstallProgress, Overview, PowerSetting } from "./types";

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

const SAMPLE_INFO: Record<string, GameInfo> = {
  "Chrono Trigger": {
    year: "1995",
    developer: "Square",
    publisher: "Square",
    genre: "Role-Playing",
    players: "1",
    description: null,
  },
  "Shadow of the Colossus": {
    year: "2005",
    developer: "Sony Computer Entertainment",
    publisher: "Sony Computer Entertainment",
    genre: "Action",
    players: "1",
    description:
      "End-level bosses are standard fare in video games, but the bosses are the levels in this unusual adventure.",
  },
};

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
const homebrewInstalled = new Set<string>();
const removed = new Set<string>();
let power: PowerSetting = "auto";
let autostart = false;
let backup: BackupSettings = { enabled: true, dir: null, lastBackup: null };
let raUser: { username: string; score: number | null } | null = null;
let raHardcore = false;
let theme = (() => {
  try {
    return localStorage.getItem("lumen-theme") ?? new URLSearchParams(location.search).get("theme") ?? "lumen";
  } catch {
    return "lumen";
  }
})();

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const SAMPLE_PC: [string, string, string][] = [
  ["steam", "1145360", "Hades"],
  ["steam", "620", "Portal 2"],
  ["steam", "1245620", "Elden Ring"],
  ["epic", "Dill", "Alan Wake 2"],
  [
    "xboxapp",
    "https://store-images.s-microsoft.com/image/apps.35187.13567343664224659.1eb6fdf9-8a0b-4344-a135-ab17dfa3c609.c83b6d6a-56c3-4c3f-8b31-456cfb21c3b7",
    "Starfield",
  ],
  [
    "xboxapp",
    "https://store-images.s-microsoft.com/image/apps.49800.13718773309227929.bebdcc0e-1ed5-4778-8732-f4ef65a2f445.9ac09d39-064d-466c-81ca-2f1b6f0b95c5",
    "Forza Horizon 5",
  ],
];

function games(): Game[] {
  if (!store.romRoot) return [];
  const user = (id: string) => store.user[id] ?? { favorite: false, lastPlayed: null, playSeconds: 0 };
  const emulated = SAMPLE.map(([systemId, stem], i) => {
    const id = `mock-${i}`;
    return { id, systemId, title: cleanTitle(stem), thumbName: stem, path: `${store.romRoot}/${systemId}/${stem}`, ...user(id) };
  }).sort((a, b) => a.title.localeCompare(b.title, "tr"));
  const pc = SAMPLE_PC.map(([systemId, appId, title]) => {
    const id = `mock-${systemId}-${title.replace(/\W/g, "")}`;
    return { id, systemId, title, thumbName: appId, path: `${systemId}://${appId}`, ...user(id) };
  });
  return [...emulated, ...pc].filter((g) => !removed.has(g.id));
}

function overview(): Overview {
  const list = games();
  return {
    settings: { romRoot: store.romRoot, sounds: true, graphics, power, backup, raHardcore, theme },
    raUser,
    backupDir: backup.dir ?? "C:\\Users\\Oyuncu\\OneDrive\\Lumen Yedek",
    powerAvailable: true,
    autostart,
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
  uninstallPlan: async (gameId) => {
    const pc = /^mock-(steam|epic|xboxapp)-/.exec(gameId)?.[1];
    if (pc === "steam") return { method: "storeDialog", bytes: null, files: 0 };
    if (pc === "epic") return { method: "openStore", bytes: null, files: 0 };
    if (pc === "xboxapp") return { method: "windowsPackage", bytes: null, files: 0 };
    return { method: "recycleBin", bytes: 734_003_200, files: 2 };
  },
  uninstallGame: async (gameId) => {
    await sleep(500);
    if (!/^mock-(steam|epic)-/.test(gameId)) removed.add(gameId);
    return games();
  },
  uninstallEmulator: async (id) => {
    await sleep(500);
    store.installed = store.installed.filter((x) => x !== id && !(id === "retroarch" && x.startsWith("core:")));
    save();
  },
  homebrewInstalled: async () => [...homebrewInstalled],
  installHomebrew: async (slug) => {
    const id = `homebrew:${slug}`;
    for (let p = 0; p <= 1; p += 0.2) {
      progressListeners.forEach((cb) => cb({ id, stage: "downloading", progress: p, message: null }));
      await sleep(120);
    }
    progressListeners.forEach((cb) => cb({ id, stage: "done", progress: 1, message: null }));
    homebrewInstalled.add(slug);
    return games();
  },
  gameInfo: async (gameId) => {
    const game = games().find((g) => g.id === gameId);
    return game ? (SAMPLE_INFO[game.title] ?? null) : null;
  },
  isResumable: async (gameId) => (store.user[gameId]?.playSeconds ?? 0) > 0,
  resumeGame: async () => {},
  launchGame: async (gameId) => {
    if (/^mock-(steam|epic|xboxapp)-/.test(gameId)) {
      touchUser(gameId).lastPlayed = Math.floor(Date.now() / 1000);
      save();
      return;
    }
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
  checkAppUpdate: async () => null,
  installAppUpdate: async () => {},
  onAppUpdateProgress: () => () => {},
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
  raLogin: async (username, password) => {
    await sleep(900);
    if (password.length < 4) throw "Kullanıcı adı veya şifre hatalı";
    raUser = { username, score: 1250 };
    return raUser;
  },
  raLogout: async () => {
    raUser = null;
  },
  raRefresh: async () => raUser,
  setRaHardcore: async (enabled) => {
    raHardcore = enabled;
  },
  backupNow: async () => {
    await sleep(800);
    backup = { ...backup, lastBackup: Math.floor(Date.now() / 1000) };
    return { files: 12, bytes: 3_400_000 };
  },
  restoreBackup: async () => {
    await sleep(800);
    return { files: 3, bytes: 260_000 };
  },
  setBackup: async (enabled, dir) => {
    backup = { ...backup, enabled, dir: dir ?? backup.dir };
  },
  setPower: async (value) => {
    power = value;
  },
  setAutostart: async (value) => {
    autostart = value;
  },
  setTheme: async (value) => {
    theme = value;
    try {
      localStorage.setItem("lumen-theme", value);
    } catch {
      /* preview only */
    }
  },
  setGraphics: async (value) => {
    graphics = value;
  },
  onInstallProgress: (cb) => {
    progressListeners.add(cb);
    return () => progressListeners.delete(cb);
  },
  onSystemResumed: () => () => {},
  onQuickMenu: (cb) => {
    // Browser preview: Select+Start can't reach a real emulator, so `Q`+`E`
    // together stands in for the in-game combo.
    const keys = new Set<string>();
    const down = (e: KeyboardEvent) => {
      keys.add(e.key.toLowerCase());
      if (running && keys.has("q") && keys.has("e")) cb();
    };
    const up = (e: KeyboardEvent) => keys.delete(e.key.toLowerCase());
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
    };
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
