import { useEffect, useState, type CSSProperties } from "react";
import { Cover } from "../components/Cover";
import { HintBar } from "../components/HintBar";
import { ProgressBar } from "../components/Toasts";
import { emulatorOf, isPcSystem, screenshotUrls, systemOf } from "../lib/catalog";
import { formatLastPlayed, formatPlaytime } from "../lib/format";
import { Layer, rumble, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { api } from "../lib/api";
import { actions, useStore } from "../lib/store";
import type { Game, GameInfo, UninstallPlan } from "../lib/types";
import { formatBytes } from "../lib/format";

export function GameSheet({ gameId, onClose }: { gameId: string; onClose: () => void }) {
  // Read live so favorite / playtime updates show immediately.
  const game = useStore((s) => s.games.find((g) => g.id === gameId));
  const overview = useStore((s) => s.overview);
  const installs = useStore((s) => s.installs);
  const [focus, setFocus] = useState(0);
  const [busy, setBusy] = useState(false);
  // A save from the last session exists: offer "continue" and "start over".
  const [resumable, setResumable] = useState(false);
  const [info, setInfo] = useState<GameInfo | null>(null);
  useEffect(() => {
    let live = true;
    setInfo(null);
    api.gameInfo(gameId).then((i) => live && setInfo(i)).catch(() => {});
    return () => {
      live = false;
    };
  }, [gameId]);
  useEffect(() => {
    let live = true;
    api.isResumable(gameId).then((r) => live && setResumable(r)).catch(() => {});
    return () => {
      live = false;
    };
  }, [gameId, game?.lastPlayed]);

  const system = game && systemOf(game.systemId);
  const status = overview?.systems.find((s) => s.id === game?.systemId);
  const fallback = !!status?.usingFallback;
  const missingAll = status?.bios.filter((b) => !b.found && !b.optional) ?? [];
  // With a fallback the game still runs (HLE BIOS), so nothing blocks it.
  const missing = fallback ? [] : missingAll;
  const emulator = system && emulatorOf(fallback && system.fallback ? system.fallback.emulator : system.emulator);
  const pc = !!system && isPcSystem(system.id);
  const needsInstall = !!status && (!status.emulatorInstalled || !status.coreInstalled);
  const install = system
    ? installs[system.emulator] ?? (system.core ? installs[`core:${system.core}`] : undefined)
    : undefined;
  const canPlay = missing.length === 0 && !busy;

  const buttons = resumable
    ? (["play", "fresh", "fav", "remove"] as const)
    : (["play", "fav", "remove"] as const);

  // Removing needs a second press; the first shows what will happen.
  const [plan, setPlan] = useState<UninstallPlan | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [removing, setRemoving] = useState(false);
  useEffect(() => {
    api.uninstallPlan(gameId).then(setPlan).catch(() => {});
  }, [gameId]);
  useEffect(() => setConfirmRemove(false), [focus]);

  const remove = async () => {
    if (!game || !plan || removing) return;
    if (!confirmRemove) {
      sfx.error();
      rumble(0.3, 40);
      setConfirmRemove(true);
      return;
    }
    setRemoving(true);
    sfx.back();
    const gone = await actions.uninstallGame(game, plan.method);
    setRemoving(false);
    setConfirmRemove(false);
    if (gone) onClose();
  };

  const play = async (g: Game, fresh = false) => {
    if (!canPlay) {
      sfx.error();
      return;
    }
    setBusy(true);
    rumble(0.6, 60);
    await actions.play(g, fresh);
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
        else if (buttons[focus] === "fresh") play(game, true);
        else if (buttons[focus] === "remove") remove();
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
  const playLabel = busy
    ? install
      ? "Hazırlanıyor…"
      : "Başlatılıyor…"
    : missing.length
      ? "Dosya gerekli"
      : resumable
        ? "Devam et"
        : "Oyna";
  const focusOf = (id: (typeof buttons)[number]) => (buttons as readonly string[]).indexOf(id) === focus;

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
          {info && (
            <div className="facts" lang="en">
              {[info.year, info.developer, info.genre, info.players && `${info.players} oyuncu`]
                .filter(Boolean)
                .map((fact) => (
                  <span key={fact}>{fact}</span>
                ))}
            </div>
          )}
          {info?.description && <p className="description">{info.description}</p>}

          <div className="stats">
            <div>
              <div className="stat-label">Son oynanma</div>
              <div className="stat-value">{formatLastPlayed(game.lastPlayed)}</div>
            </div>
            <div>
              <div className="stat-label">Toplam süre</div>
              <div className="stat-value">{pc ? "Mağazada" : formatPlaytime(game.playSeconds)}</div>
            </div>
            <div>
              <div className="stat-label">{pc ? "Mağaza" : "Emülatör"}</div>
              <div className="stat-value">{pc ? system.name : emulator?.name}</div>
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
          {fallback && (
            <div className="callout info">
              <span>i</span>
              <div>
                BIOS bulunamadı; oyun açık kaynak yerleşik BIOS ile çalışacak. Daha iyi uyumluluk için{" "}
                <b>{missingAll.map((m) => m.label).join(", ")}</b> dosyasını <code>{bios}</code> klasörüne koyabilirsin.
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
              className={`btn primary${focusOf("play") ? " focused" : ""}`}
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
            {resumable && (
              <button
                className={`btn${focusOf("fresh") ? " focused" : ""}`}
                disabled={!canPlay}
                onClick={() => play(game, true)}
              >
                Baştan başla
              </button>
            )}
            <button
              className={`btn${focusOf("fav") ? " focused" : ""}`}
              onClick={() => actions.toggleFavorite(game.id)}
            >
              {game.favorite ? "★ Favorilerde" : "☆ Favorilere ekle"}
            </button>
            <button
              className={`btn danger${focusOf("remove") ? " focused" : ""}${confirmRemove ? " armed" : ""}`}
              onClick={remove}
              disabled={removing || !plan}
            >
              {removing ? "Kaldırılıyor…" : confirmRemove ? "Emin misin? Tekrar bas" : "Kaldır"}
            </button>
          </div>
          {confirmRemove && plan && <p className="remove-note">{removeNote(plan)}</p>}

          <Screenshots game={game} />
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

/** Title screen + gameplay shot; each hides itself if the archive lacks it. */
function Screenshots({ game }: { game: Game }) {
  const [failed, setFailed] = useState<Set<string>>(new Set());
  const urls = screenshotUrls(game).filter((u) => !failed.has(u));
  if (!urls.length) return null;
  return (
    <div className="shots">
      {urls.map((url) => (
        <img
          key={url}
          src={url}
          alt=""
          loading="lazy"
          draggable={false}
          onError={() => setFailed((f) => new Set(f).add(url))}
        />
      ))}
    </div>
  );
}

function removeNote(plan: UninstallPlan): string {
  switch (plan.method) {
    case "recycleBin":
      return `${plan.files} dosya${plan.bytes ? ` (${formatBytes(plan.bytes)})` : ""} Geri Dönüşüm Kutusu'na taşınacak. Kayıtların silinmez.`;
    case "storeDialog":
      return "Steam'in kaldırma penceresi açılacak; onay Steam'den istenir.";
    case "windowsPackage":
      return "Oyun Windows'tan kaldırılacak. Bulut kayıtların Xbox hesabında kalır.";
    case "openStore":
      return "Epic Games açılacak; kaldırmayı oradan yapabilirsin.";
  }
}
