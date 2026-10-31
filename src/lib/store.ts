// App-wide state with useSyncExternalStore; actions talk to the api and keep
// the UI optimistic so nothing waits on a round trip it doesn't need.

import { useSyncExternalStore } from "react";
import { english, L } from "./i18n";
import { api } from "./api";
import { installLabel, isPcSystem, systemOf } from "./catalog";
import { setSoundsEnabled, sfx } from "./sfx";
import { formatBytes } from "./format";
import { applyTheme } from "./themes";
import type { M2Action,
  AppUpdate,
  Game,
  Collection,
  FilterSetting,
  FixResult,
  GraphicsSetting,
  ImportMode,
  ImportReport,
  InstallProgress,
  Overview,
  PowerSetting,
  UninstallPlan,
} from "./types";

export interface Notice {
  id: number;
  tone: "info" | "error" | "success";
  text: string;
}

interface State {
  ready: boolean;
  /** Startup failed; shown instead of an empty screen. */
  bootError: string | null;
  overview: Overview | null;
  games: Game[];
  installs: Record<string, InstallProgress>;
  runningGameId: string | null;
  /** Game starting: from "play" until its window is up (loading screen). */
  launchingGameId: string | null;
  notices: Notice[];
  scanning: boolean;
  /** Emulator id → update available; null until checked this session. */
  updates: Record<string, boolean> | null;
  checkingUpdates: boolean;
  /** In-game quick menu is open (Lumen is in front of the paused game). */
  quickMenu: boolean;
  /** Newer Lumen release, and download progress once installing (0–1). */
  appUpdate: AppUpdate | null;
  appUpdateProgress: number | null;
  /** A system tab the library should switch to (from the systems page). */
  jumpTab: string | null;
}

let state: State = {
  jumpTab: null,
  ready: false,
  bootError: null,
  overview: null,
  games: [],
  installs: {},
  runningGameId: null,
  launchingGameId: null,
  notices: [],
  scanning: false,
  updates: null,
  checkingUpdates: false,
  quickMenu: false,
  appUpdate: null,
  appUpdateProgress: null,
};

const subscribers = new Set<() => void>();

function set(patch: Partial<State> | ((s: State) => Partial<State>)) {
  state = { ...state, ...(typeof patch === "function" ? patch(state) : patch) };
  subscribers.forEach((fn) => fn());
}

export function useStore<T>(select: (s: State) => T): T {
  return useSyncExternalStore(
    (fn) => {
      subscribers.add(fn);
      return () => subscribers.delete(fn);
    },
    () => select(state),
  );
}

export const getState = () => state;

let noticeId = 0;
export function notify(text: string, tone: Notice["tone"] = "info", ms = 4200) {
  const notice = { id: ++noticeId, tone, text };
  set((s) => ({ notices: [...s.notices, notice] }));
  setTimeout(() => set((s) => ({ notices: s.notices.filter((n) => n.id !== notice.id) })), ms);
}

function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : L("Beklenmeyen bir hata oluştu", "Something went wrong");
}

const FIXED: Record<string, string> = {
  reinstallEmulator: L("Emülatör yeniden kuruldu", "Emulator reinstalled"),
  reinstallCore: L("Çekirdek yeniden indirildi", "Core downloaded again"),
  vcRedist: L("Visual C++ çalışma zamanı kuruldu", "Visual C++ runtime installed"),
};

/** After a failed launch: let Lumen repair what it can, then try once more. */
async function tryAutoFix(game: Game, fresh: boolean) {
  let result: FixResult | null;
  try {
    result = await api.autoFix(game.id);
  } catch (e) {
    notify(L(`Otomatik onarım başarısız: ${errorText(e)}`, `Auto-repair failed: ${errorText(e)}`), "error", 8000);
    return;
  }
  if (!result) return;
  if (result.outcome === "fixed") {
    notify(
      L(
        `${FIXED[result.fix.kind] ?? "Sorun giderildi"} · oyun yeniden açılıyor`,
        `${FIXED[result.fix.kind] ?? "Problem fixed"} · relaunching the game`,
      ),
      "success",
      5000,
    );
    await actions.play(game, fresh);
  } else {
    notify(
      L(
        "Windows Güvenliği açıldı: ayarı değiştirirsen oyunu yeniden başlat.",
        "Windows Security is open: if you change the setting, restart the game.",
      ),
      "info",
      10000,
    );
  }
}

function setCollections(collections: Collection[]) {
  set((s) => (s.overview ? { overview: { ...s.overview, collections } } : {}));
}

async function refreshOverview() {
  const overview = await api.getOverview();
  setSoundsEnabled(overview.settings.sounds);
  applyTheme(overview.settings.theme);
  set({ overview, runningGameId: overview.runningGameId });
}

export const actions = {
  async boot() {
    // Backend messages in the UI's language, before anything can fail.
    api.setLanguage(english).catch(() => {});
    api.onInstallProgress((p) => {
      set((s) => ({ installs: { ...s.installs, [p.id]: p } }));
      if (p.stage === "done" || p.stage === "error") {
        if (p.stage === "error") notify(`${installLabel(p.id)}: ${p.message ?? L("kurulamadı", "couldn't be installed")}`, "error", 7000);
        setTimeout(() => {
          set((s) => {
            const { [p.id]: _, ...rest } = s.installs;
            return { installs: rest };
          });
        }, 1600);
      }
    });
    api.onQuickMenu(() => {
      if (!state.runningGameId || state.quickMenu) return;
      sfx.confirm();
      set({ quickMenu: true, launchingGameId: null });
    });
    api.onGameShown(() => set({ launchingGameId: null }));
    api.onKidsTimeUp(() =>
      notify(
        L(
          "Bugünkü oyun süresi doldu · oyun kaydedilip kapatılıyor",
          "Today's playtime is up · saving and closing the game",
        ),
        "info",
        12000,
      ),
    );
    api.onSystemResumed(() =>
      notify(
        L(
          "Uykudan döndün · oyun duraklatıldı, hazır olunca devam et",
          "Welcome back · the game is paused, resume when you're ready",
        ),
        "info",
        6000,
      ),
    );
    api.onGameExited(async (exited) => {
      set({ runningGameId: null, launchingGameId: null, quickMenu: false });
      if (exited.error) {
        sfx.error();
        notify(exited.error, "error", 15000);
        const game = state.games.find((g) => g.id === exited.gameId);
        if (game) await tryAutoFix(game, false);
      } else sfx.back();
      set({ games: await api.getLibrary() });
    });

    try {
      const [, games] = await Promise.all([refreshOverview(), api.getLibrary()]);
      set({ games, ready: true });
    } catch (e) {
      set({ bootError: errorText(e) });
      return;
    }

    // Games added or removed while Lumen was closed show up on their own.
    if (state.overview?.settings.romRoot) actions.rescan(true);

    // The previous run crashed: say so instead of failing silently again.
    api
      .takeCrashReport()
      .then((report) => {
        if (!report) return;
        const reason = report.split("\n")[0].replace(/^\[\d+\]\s*/, "");
        notify(
          L(
            `Lumen geçen sefer beklenmedik şekilde kapandı: ${reason}`,
            `Lumen closed unexpectedly last time: ${reason}`,
          ),
          "error",
          12000,
        );
      })
      .catch(() => {});

    // Quietly look for a new Lumen release once things have settled.
    api.onAppUpdateProgress((p) => set({ appUpdateProgress: p }));
    setTimeout(async () => {
      try {
        const update = await api.checkAppUpdate();
        if (update) {
          set({ appUpdate: update });
          notify(
            L(
              `Lumen ${update.version} hazır · Menü'den güncelleyebilirsin`,
              `Lumen ${update.version} is ready · update it from the Menu`,
            ),
            "info",
            7000,
          );
        }
      } catch {
        /* offline or no releases yet */
      }
    }, 5000);
  },

  /** `path` defaults to asking the user; `useDefaultRomRoot` passes the desktop folder. */
  async chooseRomRoot(path?: string | null): Promise<boolean> {
    path ??= await api.pickFolder();
    if (!path) return false;
    set({ scanning: true });
    try {
      const games = await api.setRomRoot(path);
      set({ games });
      await refreshOverview();
      return true;
    } catch (e) {
      notify(errorText(e), "error");
      return false;
    } finally {
      set({ scanning: false });
    }
  },

  /** Creates "Lumen Oyunlar" on the desktop and uses it. */
  async useDefaultRomRoot(): Promise<boolean> {
    try {
      return await actions.chooseRomRoot(await api.defaultRomRoot());
    } catch (e) {
      notify(errorText(e), "error");
      return false;
    }
  },

  /** `quiet`: the automatic scan at start only speaks up when it finds games. */
  async rescan(quiet = false) {
    if (state.scanning) return;
    set({ scanning: true });
    try {
      const games = await api.scanLibrary();
      const before = state.games.length;
      set({ games });
      await refreshOverview();
      const added = games.length - before;
      if (added > 0) notify(L(`${added} yeni oyun bulundu`, `Found ${added} new ${added === 1 ? "game" : "games"}`), "success");
      else if (!quiet)
        notify(
          L(
            `Kütüphane güncel · ${games.length} oyun`,
            `Library up to date · ${games.length} ${games.length === 1 ? "game" : "games"}`,
          ),
          "success",
        );
    } catch (e) {
      notify(errorText(e), "error");
    } finally {
      set({ scanning: false });
    }
  },

  async install(systemIds: string[]): Promise<boolean> {
    try {
      await api.installSystems(systemIds);
      return true;
    } catch (e) {
      notify(errorText(e), "error", 7000);
      return false;
    } finally {
      await refreshOverview();
    }
  },

  /** Opens a system's tab in the library. */
  showSystem(id: string) {
    set({ jumpTab: id });
  },

  /** The library switched to `jumpTab`. */
  systemShown() {
    set({ jumpTab: null });
  },

  async resume() {
    set({ quickMenu: false });
    sfx.back();
    await api.resumeGame();
  },

  /** Leaves the quick menu and takes a screenshot of the game. */
  async takeScreenshot() {
    set({ quickMenu: false });
    sfx.confirm();
    try {
      await api.takeScreenshot();
    } catch (e) {
      notify(errorText(e), "error");
    }
  },

  /** Leaves the quick menu and swaps to the next (or previous) disc. */
  async switchDisc(next: boolean) {
    set({ quickMenu: false });
    sfx.confirm();
    try {
      await api.switchDisc(next);
    } catch (e) {
      notify(errorText(e), "error");
    }
  },

  /** Installs whatever is missing for the game's system, then launches it. */
  async play(game: Game, fresh = false): Promise<void> {
    if (isPcSystem(game.systemId)) {
      try {
        await api.launchGame(game.id);
        sfx.launch();
        notify(
          L(
            `${game.title} ${systemOf(game.systemId)?.name} üzerinden açılıyor…`,
            `Opening ${game.title} in ${systemOf(game.systemId)?.name}…`,
          ),
          "success",
        );
        set({ games: await api.getLibrary() });
      } catch (e) {
        sfx.error();
        notify(errorText(e), "error", 6000);
      }
      return;
    }
    const status = state.overview?.systems.find((s) => s.id === game.systemId);
    const missingBios = status?.usingFallback ? undefined : status?.bios.find((b) => !b.found && !b.optional);
    if (missingBios) {
      sfx.error();
      notify(L(`Önce gerekli dosyayı ekle: ${missingBios.label}`, `Add the required file first: ${missingBios.label}`), "error", 6000);
      return;
    }
    if (status && (!status.emulatorInstalled || !status.coreInstalled)) {
      notify(
        L(
          `${systemOf(game.systemId)?.short ?? ""} için emülatör hazırlanıyor…`,
          `Setting up the ${systemOf(game.systemId)?.short ?? ""} emulator…`,
        ),
      );
      if (!(await actions.install([game.systemId]))) return;
    }
    set({ launchingGameId: game.id });
    try {
      await api.launchGame(game.id, fresh);
      sfx.launch();
      set({ runningGameId: game.id });
    } catch (e) {
      set({ launchingGameId: null });
      sfx.error();
      notify(errorText(e), "error", 12000);
      await tryAutoFix(game, fresh);
    }
  },

  async stop() {
    set({ quickMenu: false });
    await api.stopGame();
  },

  /** A power mode for one game (null: follow its system). */
  async setGamePower(gameId: string, power: Game["power"]) {
    const apply = (value: Game["power"]) =>
      set((s) => ({ games: s.games.map((g) => (g.id === gameId ? { ...g, power: value } : g)) }));
    const before = state.games.find((g) => g.id === gameId)?.power ?? null;
    apply(power);
    try {
      await api.setGamePower(gameId, power ?? null);
    } catch (e) {
      apply(before);
      notify(errorText(e), "error");
    }
  },

  async toggleFavorite(gameId: string) {
    // Optimistic: flip locally, reconcile with the answer.
    const flip = (value?: boolean) =>
      set((s) => ({ games: s.games.map((g) => (g.id === gameId ? { ...g, favorite: value ?? !g.favorite } : g)) }));
    flip();
    try {
      flip(await api.toggleFavorite(gameId));
    } catch (e) {
      flip();
      notify(errorText(e), "error");
    }
  },

  async installAppUpdate() {
    if (state.appUpdateProgress !== null) return;
    set({ appUpdateProgress: 0 });
    try {
      await api.installAppUpdate(); // restarts Lumen on success
    } catch (e) {
      set({ appUpdateProgress: null });
      notify(errorText(e), "error", 7000);
    }
  },

  /** Downloads a homebrew game into its system folder; the library rescans. */
  async installHomebrew(slug: string): Promise<boolean> {
    try {
      const games = await api.installHomebrew(slug);
      set({ games });
      await refreshOverview();
      return true;
    } catch (e) {
      notify(errorText(e), "error", 6000);
      return false;
    }
  },

  /** Copies or moves the user's own dumps into the ROM folder. */
  async importGames(
    items: { path: string; systemId: string }[],
    mode: ImportMode,
  ): Promise<ImportReport | null> {
    try {
      const { report, games } = await api.importGames(items, mode);
      set({ games });
      await refreshOverview();
      return report;
    } catch (e) {
      notify(errorText(e), "error", 6000);
      return null;
    }
  },

  /** Removes a game; true when it has left the library. */
  async uninstallGame(game: Game, method: UninstallPlan["method"]): Promise<boolean> {
    try {
      const games = await api.uninstallGame(game.id);
      set({ games });
      await refreshOverview();
      const gone = !games.some((g) => g.id === game.id);
      notify(
        method === "recycleBin"
          ? L(
              `${game.title} Geri Dönüşüm Kutusu'na taşındı · kayıtların duruyor`,
              `${game.title} moved to the Recycle Bin · your saves are kept`,
            )
          : method === "windowsPackage"
            ? L(`${game.title} kaldırıldı`, `${game.title} uninstalled`)
            : method === "storeDialog"
              ? L(
                  `${game.title} için Steam'de kaldırmayı onayla, ardından kütüphaneyi yenile`,
                  `Confirm uninstalling ${game.title} in Steam, then refresh the library`,
                )
              : L(
                  `Epic Games açıldı: ${game.title} oyununu oradan kaldır, ardından kütüphaneyi yenile`,
                  `Epic Games is open: uninstall ${game.title} there, then refresh the library`,
                ),
        gone ? "success" : "info",
        7000,
      );
      return gone;
    } catch (e) {
      sfx.error();
      notify(errorText(e), "error", 7000);
      return false;
    }
  },

  async uninstallEmulator(id: string, name: string) {
    try {
      await api.uninstallEmulator(id);
      notify(
        L(
          `${name} kaldırıldı · kayıtları yedeklendi ve Geri Dönüşüm Kutusu'nda`,
          `${name} uninstalled · its saves were backed up and it's in the Recycle Bin`,
        ),
        "success",
        7000,
      );
    } catch (e) {
      notify(errorText(e), "error", 7000);
    }
    await refreshOverview();
  },

  async checkUpdates() {
    if (state.checkingUpdates) return;
    set({ checkingUpdates: true });
    try {
      const list = await api.checkUpdates();
      set({ updates: Object.fromEntries(list.map((u) => [u.id, u.updateAvailable])) });
    } catch {
      set({ updates: {} });
    } finally {
      set({ checkingUpdates: false });
    }
  },

  async updateEmulator(id: string): Promise<boolean> {
    try {
      await api.updateEmulator(id);
      set((s) => ({ updates: { ...s.updates, [id]: false } }));
      return true;
    } catch (e) {
      notify(errorText(e), "error", 7000);
      return false;
    } finally {
      await refreshOverview();
    }
  },

  async updateAll() {
    const pending = Object.entries(state.updates ?? {})
      .filter(([, available]) => available)
      .map(([id]) => id);
    for (const id of pending) await actions.updateEmulator(id);
    if (pending.length) notify(L("Emülatörler güncellendi", "Emulators updated"), "success");
  },

  async raLogin(username: string, password: string): Promise<boolean> {
    try {
      const user = await api.raLogin(username, password);
      notify(L(`RetroAchievements: ${user.username} olarak bağlandın`, `RetroAchievements: signed in as ${user.username}`), "success");
      await refreshOverview();
      return true;
    } catch (e) {
      sfx.error();
      notify(errorText(e), "error", 6000);
      return false;
    }
  },

  async raLogout() {
    await api.raLogout();
    await refreshOverview();
  },

  async raRefresh() {
    try {
      await api.raRefresh();
    } catch (e) {
      notify(errorText(e), "error", 7000);
    }
    await refreshOverview();
  },

  async setRaHardcore(enabled: boolean) {
    await api.setRaHardcore(enabled);
    await refreshOverview();
  },

  async backupNow() {
    try {
      const report = await api.backupNow();
      notify(
        report.files
          ? L(
              `${report.files} dosya yedeklendi (${formatBytes(report.bytes)})`,
              `Backed up ${report.files} ${report.files === 1 ? "file" : "files"} (${formatBytes(report.bytes)})`,
            )
          : L("Yedek güncel", "Backup is up to date"),
        "success",
      );
    } catch (e) {
      notify(errorText(e), "error", 6000);
    }
    await refreshOverview();
  },

  async restoreBackup() {
    try {
      const report = await api.restoreBackup();
      notify(
        report.files
          ? L(`${report.files} kayıt geri yüklendi`, `Restored ${report.files} ${report.files === 1 ? "save" : "saves"}`)
          : L("Geri yüklenecek daha yeni kayıt yok", "No newer saves to restore"),
        "success",
      );
    } catch (e) {
      notify(errorText(e), "error", 6000);
    }
  },

  async setBackup(enabled: boolean, chooseFolder = false) {
    const dir = chooseFolder ? await api.pickFolder() : null;
    if (chooseFolder && !dir) return;
    await api.setBackup(enabled, dir);
    await refreshOverview();
  },

  /** Back up into a cloud drive's folder (one of `api.cloudFolders`). */
  async setBackupDir(dir: string) {
    await api.setBackup(true, dir);
    await refreshOverview();
  },

  async setBackupSync(enabled: boolean) {
    await api.setBackupSync(enabled);
    await refreshOverview();
    if (enabled) notify(L("Her oyundan önce buluttaki daha yeni kayıtlar alınacak", "Newer cloud saves will be pulled before every game"), "success");
  },

  async setPower(power: PowerSetting) {
    await api.setPower(power);
    await refreshOverview();
  },

  async setAutostart(enabled: boolean) {
    try {
      await api.setAutostart(enabled);
      if (enabled) notify(L("Lumen artık Windows ile birlikte açılacak", "Lumen will now start with Windows"), "success");
    } catch (e) {
      notify(errorText(e), "error");
    }
    await refreshOverview();
  },

  async setTheme(theme: string) {
    applyTheme(theme); // instant; the setting follows
    await api.setTheme(theme);
    await refreshOverview();
  },

  async setGraphics(graphics: GraphicsSetting) {
    await api.setGraphics(graphics);
    await refreshOverview();
  },

  /** Collection changes come back as the full list; the overview carries it. */
  async createCollection(name: string): Promise<boolean> {
    try {
      setCollections(await api.createCollection(name));
      return true;
    } catch (e) {
      notify(errorText(e), "error");
      return false;
    }
  },

  async deleteCollection(id: string) {
    try {
      setCollections(await api.deleteCollection(id));
    } catch (e) {
      notify(errorText(e), "error");
    }
  },

  async toggleCollection(id: string, gameId: string) {
    try {
      setCollections(await api.toggleCollection(id, gameId));
    } catch (e) {
      notify(errorText(e), "error");
    }
  },

  /** Saves the Steam key and rescans so owned games appear; true on success. */
  async setSteamKey(key: string): Promise<boolean> {
    try {
      const count = await api.setSteamKey(key);
      notify(
        key.trim()
          ? L(`Steam bağlandı · ${count} oyun`, `Steam connected · ${count} ${count === 1 ? "game" : "games"}`)
          : L("Steam bağlantısı kaldırıldı", "Steam disconnected"), "success");
      await actions.rescan(true);
      await refreshOverview();
      return true;
    } catch (e) {
      notify(errorText(e), "error", 9000);
      return false;
    }
  },

  async setSgdbKey(key: string): Promise<boolean> {
    try {
      await api.setSgdbKey(key);
      notify(
        key.trim()
          ? L("SteamGridDB bağlandı · eksik kapaklar indiriliyor", "SteamGridDB connected · downloading missing covers")
          : L("SteamGridDB bağlantısı kaldırıldı", "SteamGridDB disconnected"), "success");
      await refreshOverview();
      await actions.rescan(true);
      return true;
    } catch (e) {
      notify(errorText(e), "error", 9000);
      return false;
    }
  },

  async setQuickCombo(enabled: boolean) {
    await api.setQuickCombo(enabled);
    await refreshOverview();
  },

  /** Kids mode on; refreshes what the library shows. */
  async kidsStart(pin: string, collection: string | null, dailyMinutes: number | null): Promise<boolean> {
    try {
      await api.kidsStart(pin, collection, dailyMinutes);
      await refreshOverview();
      notify(L("Çocuk modu açık", "Kids mode on"), "success");
      return true;
    } catch (e) {
      notify(errorText(e), "error");
      return false;
    }
  },

  async kidsStop(pin: string): Promise<boolean> {
    try {
      await api.kidsStop(pin);
      await refreshOverview();
      notify(L("Çocuk modu kapandı", "Kids mode off"), "success");
      return true;
    } catch (e) {
      sfx.error();
      notify(errorText(e), "error");
      return false;
    }
  },

  /** Saves the problem report to the desktop and says where. */
  async reportProblem() {
    const now = new Date();
    const pad = (n: number) => String(n).padStart(2, "0");
    const name = `${L("Lumen sorun raporu", "Lumen problem report")} ${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}.${pad(now.getMinutes())}`;
    try {
      await api.exportDiagnostics(name);
      notify(
        L(
          `Masaüstüne kaydedildi: ${name}.zip · bu dosyayı geliştiriciye gönder`,
          `Saved to the desktop: ${name}.zip · send this file to the developer`,
        ),
        "success",
        10000,
      );
    } catch (e) {
      notify(errorText(e), "error");
    }
  },

  async setM2Action(action: M2Action) {
    await api.setM2Action(action);
    await refreshOverview();
  },

  async setBackButtons(enabled: boolean) {
    await api.setBackButtons(enabled);
    await refreshOverview();
  },

  async setOverlayStatus(enabled: boolean) {
    await api.setOverlayStatus(enabled);
    await refreshOverview();
  },

  async setOverlayButton(enabled: boolean) {
    await api.setOverlayButton(enabled);
    await refreshOverview();
  },

  async setFilter(filter: FilterSetting) {
    await api.setFilter(filter);
    await refreshOverview();
  },

  async setSounds(enabled: boolean) {
    setSoundsEnabled(enabled);
    set((s) => (s.overview ? { overview: { ...s.overview, settings: { ...s.overview.settings, sounds: enabled } } } : {}));
    await api.setSounds(enabled);
  },
};
