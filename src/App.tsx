import { useEffect, useState } from "react";
import { BrandMark } from "./components/BrandMark";
import { Toasts } from "./components/Toasts";
import { isTauri } from "./lib/api";
import { actions, useStore } from "./lib/store";
import { GameSheet } from "./screens/GameSheet";
import { Library } from "./screens/Library";
import { Menu } from "./screens/Menu";
import { Onboarding } from "./screens/Onboarding";
import { Running } from "./screens/Running";

export function App() {
  const ready = useStore((s) => s.ready);
  const romRoot = useStore((s) => s.overview?.settings.romRoot);
  const runningGameId = useStore((s) => s.runningGameId);

  const [openGameId, setOpenGameId] = useState<string | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  // Setup stays on screen until the user leaves it, even after a root is chosen.
  const [onboarding, setOnboarding] = useState<null | "welcome" | "folder">(null);

  useEffect(() => {
    actions.boot();
    if (isTauri && import.meta.env.PROD) {
      import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().setFullscreen(true));
    }
  }, []);

  useEffect(() => {
    if (ready && !romRoot) setOnboarding((o) => o ?? "welcome");
  }, [ready, romRoot]);

  if (!ready) return <Splash />;

  return (
    <div className="app">
      {onboarding ? (
        <>
          <div className="ambient" />
          <Onboarding startAt={onboarding} onDone={() => setOnboarding(null)} />
        </>
      ) : (
        <>
          <Library
            onOpen={(g) => setOpenGameId(g.id)}
            onMenu={() => setMenuOpen(true)}
            covered={!!openGameId || menuOpen}
          />
          {openGameId && <GameSheet gameId={openGameId} onClose={() => setOpenGameId(null)} />}
          {menuOpen && (
            <Menu
              onClose={() => setMenuOpen(false)}
              onChangeRoot={() => {
                setMenuOpen(false);
                setOnboarding("folder");
              }}
            />
          )}
        </>
      )}
      {runningGameId && <Running gameId={runningGameId} />}
      <Toasts hideInstalls={!!onboarding} />
    </div>
  );
}

/** Startup screen: the mark while loading, and an explanation instead of a
 *  blank screen if the backend errors or doesn't answer. */
function Splash() {
  const bootError = useStore((s) => s.bootError);
  const [slow, setSlow] = useState(false);
  useEffect(() => {
    const id = setTimeout(() => setSlow(true), 10_000);
    return () => clearTimeout(id);
  }, []);
  const problem = bootError ?? (slow ? "Lumen 10 saniyedir yanıt vermiyor." : null);

  return (
    <div className="app splash">
      <div className="ambient" />
      <div className="splash-inner">
        <BrandMark className="splash-mark" />
        {problem && (
          <>
            <h2>Lumen başlatılamadı</h2>
            <p>{problem}</p>
            <button className="btn primary focused" onClick={() => location.reload()}>
              Tekrar dene
            </button>
          </>
        )}
      </div>
    </div>
  );
}
