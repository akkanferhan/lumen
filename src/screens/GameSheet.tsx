import { useState, type CSSProperties } from "react";
import { Cover } from "../components/Cover";
import { HintBar } from "../components/HintBar";
import { ProgressBar } from "../components/Toasts";
import { emulatorOf, systemOf } from "../lib/catalog";
import { formatLastPlayed, formatPlaytime } from "../lib/format";
import { Layer, rumble, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, useStore } from "../lib/store";
import type { Game } from "../lib/types";

export function GameSheet({ gameId, onClose }: { gameId: string; onClose: () => void }) {
  // Read live so favorite / playtime updates show immediately.
  const game = useStore((s) => s.games.find((g) => g.id === gameId));
  const overview = useStore((s) => s.overview);
  const installs = useStore((s) => s.installs);
  const [focus, setFocus] = useState(0);
  const [busy, setBusy] = useState(false);

  const system = game && systemOf(game.systemId);
  const status = overview?.systems.find((s) => s.id === game?.systemId);
  const missing = status?.bios.filter((b) => !b.found && !b.optional) ?? [];
  const emulator = system && emulatorOf(system.emulator);
  const needsInstall = !!status && (!status.emulatorInstalled || !status.coreInstalled);
  const install = system
    ? installs[system.emulator] ?? (system.core ? installs[`core:${system.core}`] : undefined)
    : undefined;
  const canPlay = missing.length === 0 && !busy;

  const buttons = ["play", "fav"] as const;

  const play = async (g: Game) => {
    if (!canPlay) {
      sfx.error();
      return;
    }
    setBusy(true);
    rumble(0.6, 60);
    await actions.play(g);
    setBusy(false);
  };

  useInput((action) => {
    if (!game) return;
    switch (action) {
      case "left":
      case "right": {
        const next = action === "left" ? Math.max(0, focus - 1) : Math.min(buttons.length - 1, focus + 1);
        if (next !== focus) {
          sfx.move();
          setFocus(next);
        }
        break;
      }
      case "a":
        if (buttons[focus] === "play") play(game);
        else {
          sfx.confirm();
          actions.toggleFavorite(game.id);
        }
        break;
      case "y":
        sfx.confirm();
        actions.toggleFavorite(game.id);
        break;
      case "b":
        sfx.back();
        onClose();
        break;
    }
  }, Layer.sheet);

  if (!game || !system) return null;

  const bios = overview?.settings.romRoot ? `${overview.settings.romRoot}${sep(overview.settings.romRoot)}bios` : "bios";
  const playLabel = busy ? (install ? "Hazırlanıyor…" : "Başlatılıyor…") : missing.length ? "Dosya gerekli" : "Oyna";

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <section className="sheet" style={{ "--accent": system.accent, "--card-accent": system.accent } as CSSProperties}>
        <div className="sheet-art">
          <Cover game={game} />
        </div>

        <div>
          <span className="sheet-system" lang="en">
            <span className="dot" />
            {system.name}
          </span>
          <h1>{game.title}</h1>

          <div className="stats">
            <div>
              <div className="stat-label">Son oynanma</div>
              <div className="stat-value">{formatLastPlayed(game.lastPlayed)}</div>
            </div>
            <div>
              <div className="stat-label">Toplam süre</div>
              <div className="stat-value">{formatPlaytime(game.playSeconds)}</div>
            </div>
            <div>
              <div className="stat-label">Emülatör</div>
              <div className="stat-value">{emulator?.name}</div>
            </div>
          </div>

          {missing.length > 0 && (
            <div className="callout">
              <span>⚠</span>
              <div>
                Bu sistem için bir dosya eksik: <b>{missing.map((m) => m.label).join(", ")}</b>
                <br />
                Dosyayı <code>{bios}</code> klasörüne koyduktan sonra menüden kütüphaneyi yenile.
              </div>
            </div>
          )}
          {missing.length === 0 && needsInstall && !install && (
            <div className="callout info">
              <span>↓</span>
              <div>
                İlk açılışta {emulator?.name} otomatik olarak indirilip ayarlanacak. Bu işlem bir kez yapılır.
              </div>
            </div>
          )}

          <div className="actions">
            <button
              className={`btn primary${focus === 0 ? " focused" : ""}`}
              disabled={!canPlay}
              onClick={() => play(game)}
            >
              {playLabel}
              {install && install.stage === "downloading" && (
                <span className="btn-progress">
                  <ProgressBar value={install.progress} />
                </span>
              )}
            </button>
            <button
              className={`btn${focus === 1 ? " focused" : ""}`}
              onClick={() => actions.toggleFavorite(game.id)}
            >
              {game.favorite ? "★ Favorilerde" : "☆ Favorilere ekle"}
            </button>
          </div>
        </div>
      </section>

      <HintBar
        hints={[
          { action: "a", label: "Seç" },
          { action: "y", label: "Favori" },
          { action: "b", label: "Geri" },
        ]}
      />
    </>
  );
}

function sep(path: string) {
  return path.includes("\\") ? "\\" : "/";
}
