// App-wide state with useSyncExternalStore; actions talk to the api and keep
// the UI optimistic so nothing waits on a round trip it doesn't need.

import { useSyncExternalStore } from "react";
import { api } from "./api";
import { installLabel, systemOf } from "./catalog";
import { setSoundsEnabled, sfx } from "./sfx";
import type { Game, InstallProgress, Overview } from "./types";

export interface Notice {
  id: number;
  tone: "info" | "error" | "success";
  text: string;
}

interface State {
  ready: boolean;
  overview: Overview | null;
  games: Game[];
  installs: Record<string, InstallProgress>;
  runningGameId: string | null;
  notices: Notice[];
  scanning: boolean;
}

let state: State = {
  ready: false,
  overview: null,
  games: [],
  installs: {},
  runningGameId: null,
  notices: [],
  scanning: false,
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
    api.onGameExited(async () => {
      set({ runningGameId: null });
      sfx.back();
      set({ games: await api.getLibrary() });
    });

    const [, games] = await Promise.all([refreshOverview(), api.getLibrary()]);
    set({ games, ready: true });
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

  /** Installs whatever is missing for the game's system, then launches it. */
  async play(game: Game): Promise<void> {
    const status = state.overview?.systems.find((s) => s.id === game.systemId);
    const missingBios = status?.bios.find((b) => !b.found && !b.optional);
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
      await api.launchGame(game.id);
      sfx.launch();
      set({ runningGameId: game.id });
    } catch (e) {
      sfx.error();
      notify(errorText(e), "error", 6000);
    }
  },

  async stop() {
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

  async setSounds(enabled: boolean) {
    setSoundsEnabled(enabled);
    set((s) => (s.overview ? { overview: { ...s.overview, settings: { ...s.overview.settings, sounds: enabled } } } : {}));
    await api.setSounds(enabled);
  },
};
