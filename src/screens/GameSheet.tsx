import { useEffect, useState, type CSSProperties } from "react";
import { CollectionPicker } from "../components/CollectionPicker";
import { ShotGallery } from "../components/ShotGallery";
import { GameHistory } from "../components/GameHistory";
import { Cover } from "../components/Cover";
import { HintBar } from "../components/HintBar";
import { ProgressBar } from "../components/Toasts";
import { emulatorOf, isPcSystem, screenshotUrls, systemOf } from "../lib/catalog";
import { formatLastPlayed, formatPlaytime } from "../lib/format";
import { Layer, rumble, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { api } from "../lib/api";
import { actions, useStore } from "../lib/store";
import type { AchievementProgress, Game, GameInfo, PowerMode, Session, Shot, UninstallPlan } from "../lib/types";
import { L } from "../lib/i18n";

/** A game's power choices, in the order A steps through them (null: its system's). */
const GAME_POWER: (PowerMode | null)[] = [null, "silent", "performance", "turbo"];
const GAME_POWER_LABEL: Record<PowerMode, string> = {
  silent: L("Sessiz", "Silent"),
  performance: L("Performans", "Performance"),
  turbo: "Turbo",
};
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

  // The player's own screenshots of this game.
  const [shots, setShots] = useState<Shot[]>([]);
  const [viewingShots, setViewingShots] = useState(false);
  // Opened from the history: the gallery starts on this shot.
  const [shotStart, setShotStart] = useState<Shot | undefined>();
  const loadShots = () => {
    api.listScreenshots(gameId).then(setShots).catch(() => setShots([]));
  };
  useEffect(loadShots, [gameId, game?.lastPlayed]);
  // Play sessions, for "Geçmiş".
  const [sessions, setSessions] = useState<Session[]>([]);
  const [viewingHistory, setViewingHistory] = useState(false);
  useEffect(() => {
    api.gameSessions(gameId).then(setSessions).catch(() => setSessions([]));
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

  // Kids mode hides collections, power and removal.
  const kidsMode = !!overview?.settings.kids.active;
  // Per-game power mode: only where Lumen sets ASUS power modes at all.
  const powerChoice = !kidsMode && !!overview?.powerAvailable && overview.settings.power !== "off" && !pc;
  type Button = "play" | "fresh" | "fav" | "lists" | "history" | "shots" | "power" | "remove";
  const buttons: Button[] = game?.notInstalled
    ? ["play", "fav", "lists"]
    : [
        "play",
        ...(resumable ? (["fresh"] as const) : []),
        "fav",
        ...(kidsMode ? [] : (["lists"] as const)),
        ...(sessions.length ? (["history"] as const) : []),
        ...(shots.length ? (["shots"] as const) : []),
        ...(powerChoice ? (["power"] as const) : []),
        ...(kidsMode ? [] : (["remove"] as const)),
      ];
  const [picking, setPicking] = useState(false);

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

  const cyclePower = () => {
    if (!game) return;
    sfx.confirm();
    const next = GAME_POWER[(GAME_POWER.indexOf(game.power ?? null) + 1) % GAME_POWER.length];
    actions.setGamePower(game.id, next);
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
        else if (buttons[focus] === "power") cyclePower();
        else if (buttons[focus] === "shots") {
          sfx.confirm();
          setShotStart(undefined);
          setViewingShots(true);
        } else if (buttons[focus] === "history") {
          sfx.confirm();
          setViewingHistory(true);
        }
        else if (buttons[focus] === "lists") {
          sfx.confirm();
          setPicking(true);
        } else {
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
  const playLabel = game.notInstalled
    ? L("Steam'de yükle", "Install on Steam")
    : busy
    ? install
      ? L("Hazırlanıyor…", "Preparing…")
      : L("Başlatılıyor…", "Starting…")
    : missing.length
      ? L("Dosya gerekli", "File needed")
      : resumable
        ? L("Devam et", "Continue")
        : L("Oyna", "Play");
  const focusOf = (id: Button) => buttons.indexOf(id) === focus;
  const inLists = (overview?.collections ?? []).filter((c) => c.games.includes(game.id)).length;

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
              {[info.year, info.developer, info.genre, info.players && L(`${info.players} oyuncu`, `${info.players} ${info.players === "1" ? "player" : "players"}`)]
                .filter(Boolean)
                .map((fact) => (
                  <span key={fact}>{fact}</span>
                ))}
            </div>
          )}
          {info?.description && <p className="description">{info.description}</p>}

          <div className="stats">
            <div>
              <div className="stat-label">{L("Son oynanma", "Last played")}</div>
              <div className="stat-value">{formatLastPlayed(game.lastPlayed)}</div>
            </div>
            <div>
              <div className="stat-label">{L("Toplam süre", "Total time")}</div>
              <div className="stat-value">{pc ? L("Mağazada", "In store") : formatPlaytime(game.playSeconds)}</div>
            </div>
            <div>
              <div className="stat-label">{pc ? L("Mağaza", "Store") : L("Emülatör", "Emulator")}</div>
              <div className="stat-value">{pc ? system.name : emulator?.name}</div>
            </div>
          </div>

          {missing.length > 0 && (
            <div className="callout">
              <span>⚠</span>
              <div>
                {missing.some((m) => m.verdict === "wrongSize") ? (
                  L(
                    <>
                      BIOS dosyası bulundu ama bozuk ya da yanlış görünüyor (boyutu tutmuyor):{" "}
                      <b>{missing.filter((m) => m.verdict === "wrongSize").map((m) => m.file).join(", ")}</b>
                      <br />
                      Konsolundan yeniden döküm alıp <code>{bios}</code> klasörüne koy.
                    </>,
                    <>
                      A BIOS file was found but looks damaged or wrong (its size doesn't match):{" "}
                      <b>{missing.filter((m) => m.verdict === "wrongSize").map((m) => m.file).join(", ")}</b>
                      <br />
                      Dump it again from your console and put it in the <code>{bios}</code> folder.
                    </>,
                  )
                ) : (
                  L(
                    <>
                      Bu sistem için bir dosya eksik: <b>{missing.map((m) => m.label).join(", ")}</b>
                      <br />
                      Dosyayı <code>{bios}</code> klasörüne koyduktan sonra menüden kütüphaneyi yenile.
                    </>,
                    <>
                      This system needs a file that's missing: <b>{missing.map((m) => m.label).join(", ")}</b>
                      <br />
                      Put it in the <code>{bios}</code> folder, then refresh the library from the menu.
                    </>,
                  )
                )}
              </div>
            </div>
          )}
          {fallback && (
            <div className="callout info">
              <span>✓</span>
              <div>
                {L(
                  <>
                    Ek dosya gerekmez: oyun Lumen'in yerleşik BIOS'uyla çalışır. İstersen kendi konsolundan aldığın{" "}
                    <b>{missingAll.map((m) => m.label).join(", ")}</b> dosyasını <code>{bios}</code> klasörüne koyabilirsin;
                    bazı oyunlarda uyumluluk artar.
                  </>,
                  <>
                    No extra file needed: the game runs on Lumen's built-in BIOS. If you like, you can put{" "}
                    <b>{missingAll.map((m) => m.label).join(", ")}</b> dumped from your own console in the{" "}
                    <code>{bios}</code> folder; it improves compatibility in some games.
                  </>,
                )}
              </div>
            </div>
          )}
          {missing.length === 0 && needsInstall && !install && (
            <div className="callout info">
              <span>↓</span>
              <div>
                {L(
                  `İlk açılışta ${emulator?.name} otomatik olarak indirilip ayarlanacak. Bu işlem bir kez yapılır.`,
                  `${emulator?.name} will be downloaded and set up automatically on first launch. This only happens once.`,
                )}
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
                {L("Baştan başla", "Start over")}
              </button>
            )}
            <button
              className={`btn${focusOf("fav") ? " focused" : ""}`}
              onClick={() => actions.toggleFavorite(game.id)}
            >
              {game.favorite ? L("★ Favorilerde", "★ In favorites") : L("☆ Favorilere ekle", "☆ Add to favorites")}
            </button>
            {!kidsMode && (
            <button className={`btn${focusOf("lists") ? " focused" : ""}`} onClick={() => setPicking(true)}>
              {inLists
                ? L(`☰ ${inLists} listede`, `☰ In ${inLists} ${inLists === 1 ? "list" : "lists"}`)
                : L("☰ Koleksiyona ekle", "☰ Add to collection")}
            </button>
            )}
            {sessions.length > 0 && !game.notInstalled && (
              <button className={`btn${focusOf("history") ? " focused" : ""}`} onClick={() => setViewingHistory(true)}>
                {L("🕘 Geçmiş", "🕘 History")}
              </button>
            )}
            {shots.length > 0 && !game.notInstalled && (
              <button className={`btn${focusOf("shots") ? " focused" : ""}`} onClick={() => setViewingShots(true)}>
                {L(`▣ ${shots.length} ekran görüntüsü`, `▣ ${shots.length} ${shots.length === 1 ? "screenshot" : "screenshots"}`)}
              </button>
            )}
            {powerChoice && !game.notInstalled && (
              <button
                className={`btn${focusOf("power") ? " focused" : ""}`}
                onClick={cyclePower}
                title={L("Bu oyun açılınca kullanılacak ASUS güç modu", "ASUS power mode used when this game starts")}
              >
                ⚡ {game.power ? GAME_POWER_LABEL[game.power] : L("Güç: sisteme göre", "Power: system default")}
              </button>
            )}
            {!game.notInstalled && !kidsMode && (
            <button
              className={`btn danger${focusOf("remove") ? " focused" : ""}${confirmRemove ? " armed" : ""}`}
              onClick={remove}
              disabled={removing || !plan}
            >
              {removing
                ? L("Kaldırılıyor…", "Removing…")
                : confirmRemove
                  ? L("Emin misin? Tekrar bas", "Are you sure? Press again")
                  : L("Kaldır", "Remove")}
            </button>
            )}
          </div>
          {confirmRemove && plan && <p className="remove-note">{removeNote(plan)}</p>}
          {game.notInstalled && (
            <div className="callout info">
              <span>↓</span>
              <div>
                {L(
                  "Bu oyun Steam kütüphanende ama yüklü değil. Steam kurulum penceresi açılır; indirme bitince oyun burada oynanabilir olur.",
                  "This game is in your Steam library but isn't installed. Steam's install window opens; once the download finishes, you can play it here.",
                )}
              </div>
            </div>
          )}

          <Screenshots game={game} />
          <Achievements gameId={game.id} />
        </div>
      </section>

      {viewingShots ? (
        <ShotGallery shots={shots} start={shotStart} onChange={loadShots} onClose={() => setViewingShots(false)} />
      ) : viewingHistory ? (
        <GameHistory
          sessions={sessions}
          shots={shots}
          onOpenShot={(shot) => {
            setShotStart(shot);
            setViewingShots(true);
          }}
          onClose={() => setViewingHistory(false)}
        />
      ) : picking ? (
        <CollectionPicker gameId={game.id} title={game.title} onClose={() => setPicking(false)} />
      ) : (
        <HintBar
          hints={[
            { action: "a", label: L("Seç", "Select") },
            { action: "y", label: L("Favori", "Favorite") },
            { action: "b", label: L("Geri", "Back") },
          ]}
        />
      )}
    </>
  );
}

function sep(path: string) {
  return path.includes("\\") ? "\\" : "/";
}

/** Title screen + gameplay shot; each hides itself if the archive lacks it. */
/** The game's RetroAchievements: progress and badges, unlocked first. */
function Achievements({ gameId }: { gameId: string }) {
  const [progress, setProgress] = useState<AchievementProgress | null>(null);
  useEffect(() => {
    let live = true;
    setProgress(null);
    api.gameAchievements(gameId).then((p) => live && setProgress(p)).catch(() => {});
    return () => {
      live = false;
    };
  }, [gameId]);
  if (!progress || progress.achievements.length === 0) return null;

  const all = progress.achievements;
  const won = all.filter((a) => a.unlocked);
  const points = (list: typeof all) => list.reduce((sum, a) => sum + a.points, 0);
  const shown = [...won, ...all.filter((a) => !a.unlocked)];

  return (
    <section className="achievements">
      <div className="achievements-head">
        <span className="stat-label">{L("Başarımlar", "Achievements")}</span>
        <span>
          {won.length} / {all.length} · {points(won)} / {points(all)} {L("puan", "points")}
        </span>
      </div>
      <ProgressBar value={won.length / all.length} />
      <div className="badges">
        {shown.map((a) => (
          <span key={a.id} className={`badge${a.unlocked ? "" : " locked"}`} title={`${a.title} (${a.points}) · ${a.description}`}>
            {a.badge ? <img src={a.badge} alt={a.title} loading="lazy" draggable={false} /> : <span>{a.points}</span>}
          </span>
        ))}
      </div>
    </section>
  );
}

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
      return L(
        `${plan.files} dosya${plan.bytes ? ` (${formatBytes(plan.bytes)})` : ""} Geri Dönüşüm Kutusu'na taşınacak. Kayıtların silinmez.`,
        `${plan.files} ${plan.files === 1 ? "file" : "files"}${plan.bytes ? ` (${formatBytes(plan.bytes)})` : ""} will be moved to the Recycle Bin. Your saves are kept.`,
      );
    case "storeDialog":
      return L("Steam'in kaldırma penceresi açılacak; onay Steam'den istenir.", "Steam's uninstall window will open; Steam asks you to confirm.");
    case "windowsPackage":
      return L(
        "Oyun Windows'tan kaldırılacak. Bulut kayıtların Xbox hesabında kalır.",
        "The game will be uninstalled from Windows. Your cloud saves stay on your Xbox account.",
      );
    case "openStore":
      return L("Epic Games açılacak; kaldırmayı oradan yapabilirsin.", "Epic Games will open; you can uninstall it from there.");
  }
}
