// App-wide state with useSyncExternalStore; actions talk to the api and keep
// the UI optimistic so nothing waits on a round trip it doesn't need.

import { useSyncExternalStore } from "react";
import { api } from "./api";
import { installLabel, isPcSystem, systemOf } from "./catalog";
import { setSoundsEnabled, sfx } from "./sfx";
import { formatBytes } from "./format";
import { applyTheme } from "./themes";
import type { AppUpdate, Game, GraphicsSetting, InstallProgress, Overview, PowerSetting } from "./types";

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
}

let state: State = {
  ready: false,
  bootError: null,
  overview: null,
  games: [],
  installs: {},
  runningGameId: null,
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
  return typeof e === "string" ? e : e instanceof Error ? e.message : "Beklenmeyen bir hata oluştu";
}

async function refreshOverview() {
  const overview = await api.getOverview();
  setSoundsEnabled(overview.settings.sounds);
  applyTheme(overview.settings.theme);
  set({ overview, runningGameId: overview.runningGameId });
}

export const actions = {
  async boot() {
    api.onInstallProgress((p) => {
      set((s) => ({ installs: { ...s.installs, [p.id]: p } }));
      if (p.stage === "done" || p.stage === "error") {
        if (p.stage === "error") notify(`${installLabel(p.id)}: ${p.message ?? "kurulamadı"}`, "error", 7000);
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
      set({ quickMenu: true });
    });
    api.onSystemResumed(() => notify("Uykudan döndün · oyun duraklatıldı, hazır olunca devam et", "info", 6000));
    api.onGameExited(async () => {
      set({ runningGameId: null, quickMenu: false });
      sfx.back();
      set({ games: await api.getLibrary() });
    });

    try {
      const [, games] = await Promise.all([refreshOverview(), api.getLibrary()]);
      set({ games, ready: true });
    } catch (e) {
      set({ bootError: errorText(e) });
      return;
    }

    // Quietly look for a new Lumen release once things have settled.
    api.onAppUpdateProgress((p) => set({ appUpdateProgress: p }));
    setTimeout(async () => {
      try {
        const update = await api.checkAppUpdate();
        if (update) {
          set({ appUpdate: update });
          notify(`Lumen ${update.version} hazır · Menü'den güncelleyebilirsin`, "info", 7000);
        }
      } catch {
        /* offline or no releases yet */
      }
    }, 5000);
  },

  async chooseRomRoot(): Promise<boolean> {
    const path = await api.pickFolder();
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

  async rescan() {
    if (state.scanning) return;
    set({ scanning: true });
    try {
      const games = await api.scanLibrary();
      const before = state.games.length;
      set({ games });
      await refreshOverview();
      const added = games.length - before;
      notify(added > 0 ? `${added} yeni oyun bulundu` : `Kütüphane güncel · ${games.length} oyun`, "success");
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

  async resume() {
    set({ quickMenu: false });
    sfx.back();
    await api.resumeGame();
  },

  /** Installs whatever is missing for the game's system, then launches it. */
  async play(game: Game, fresh = false): Promise<void> {
    if (isPcSystem(game.systemId)) {
      try {
        await api.launchGame(game.id);
        sfx.launch();
        notify(`${game.title} ${systemOf(game.systemId)?.name} üzerinden açılıyor…`, "success");
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
      notify(`Önce gerekli dosyayı ekle: ${missingBios.label}`, "error", 6000);
      return;
    }
    if (status && (!status.emulatorInstalled || !status.coreInstalled)) {
      notify(`${systemOf(game.systemId)?.short ?? ""} için emülatör hazırlanıyor…`);
      if (!(await actions.install([game.systemId]))) return;
    }
    try {
      await api.launchGame(game.id, fresh);
      sfx.launch();
      set({ runningGameId: game.id });
    } catch (e) {
      sfx.error();
      notify(errorText(e), "error", 6000);
    }
  },

  async stop() {
    set({ quickMenu: false });
    await api.stopGame();
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
    if (pending.length) notify("Emülatörler güncellendi", "success");
  },

  async raLogin(username: string, password: string): Promise<boolean> {
    try {
      const user = await api.raLogin(username, password);
      notify(`RetroAchievements: ${user.username} olarak bağlandın`, "success");
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
        report.files ? `${report.files} dosya yedeklendi (${formatBytes(report.bytes)})` : "Yedek güncel",
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
      notify(report.files ? `${report.files} kayıt geri yüklendi` : "Geri yüklenecek daha yeni kayıt yok", "success");
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

  async setPower(power: PowerSetting) {
    await api.setPower(power);
    await refreshOverview();
  },

  async setAutostart(enabled: boolean) {
    try {
      await api.setAutostart(enabled);
      if (enabled) notify("Lumen artık Windows ile birlikte açılacak", "success");
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

  async setSounds(enabled: boolean) {
    setSoundsEnabled(enabled);
    set((s) => (s.overview ? { overview: { ...s.overview, settings: { ...s.overview.settings, sounds: enabled } } } : {}));
    await api.setSounds(enabled);
  },
};
