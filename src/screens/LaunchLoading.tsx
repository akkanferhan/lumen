import { useEffect, useState, type CSSProperties } from "react";
import { Cover } from "../components/Cover";
import { HintBar } from "../components/HintBar";
import { systemOf } from "../lib/catalog";
import { Layer, useInput } from "../lib/input";
import { actions, useStore } from "../lib/store";

/** After this long, say that a slow start is normal. */
const SLOW_MS = 8000;

/**
 * Covers the screen from "play" until the emulator's window is up, so a
 * slow-starting emulator doesn't look like nothing happened. B gives up
 * once the emulator is running.
 */
export function LaunchLoading({ gameId }: { gameId: string }) {
  const game = useStore((s) => s.games.find((g) => g.id === gameId));
  const started = useStore((s) => s.runningGameId === gameId);
  const [slow, setSlow] = useState(false);
  const system = game && systemOf(game.systemId);

  useEffect(() => {
    const id = setTimeout(() => setSlow(true), SLOW_MS);
    return () => clearTimeout(id);
  }, []);

  // Takes every press while loading, so nothing reaches the screens below.
  useInput((action) => {
    if (action === "b" && started) actions.stop();
  }, Layer.loading);

  return (
    <div
      className="launch-loading"
      role="status"
      aria-live="polite"
      style={{ "--accent": system?.accent, "--card-accent": system?.accent } as CSSProperties}
    >
      <div className="launch-loading-inner">
        {game && (
          <div className="running-art">
            <Cover game={game} />
          </div>
        )}
        <div className="eyebrow">
          <span lang="en">{system?.name}</span>
        </div>
        <h2>{game?.title ?? "Oyun"}</h2>
        <div className="launch-spinner" aria-hidden />
        <p>
          Yükleniyor…
          {slow && (
            <>
              <br />
              <small>Bazı emülatörlerin açılması biraz sürebilir, özellikle ilk seferde.</small>
            </>
          )}
        </p>
      </div>
      <HintBar hints={started ? [{ action: "b", label: "Vazgeç" }] : []} />
    </div>
  );
}
