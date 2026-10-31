import { useEffect, useState, type CSSProperties } from "react";
import { Cover } from "../components/Cover";
import { Glyph } from "../components/Glyph";
import { HintBar } from "../components/HintBar";
import { api, isTauri } from "../lib/api";
import { systemOf } from "../lib/catalog";
import { Layer, rumble, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, notify, useStore } from "../lib/store";
import { formatLastPlayed } from "../lib/format";
import type { GameControls, StateSlot } from "../lib/types";

/** Today's saves by time (slots are often minutes apart), older ones by day. */
function savedWhen(unixSeconds: number): string {
  const day = formatLastPlayed(unixSeconds);
  return day === "Bugün"
    ? new Date(unixSeconds * 1000).toLocaleTimeString("tr-TR", { hour: "2-digit", minute: "2-digit" })
    : day;
}

/**
 * While a game runs Lumen sits behind the emulator showing this screen.
 * Tapping a back button (Ally) or Select + Start in game brings it to the front as the quick menu
 * (the emulator pauses on focus loss).
 */
export function Running({ gameId }: { gameId: string }) {
  const game = useStore((s) => s.games.find((g) => g.id === gameId));
  const quickMenu = useStore((s) => s.quickMenu);
  const backButtons = useStore((s) => !!s.overview?.device.model && !!s.overview?.settings.backButtons);
  const [stopping, setStopping] = useState(false);
  const [focus, setFocus] = useState(0);
  // Save-state slots (RetroArch systems); null hides the row.
  const [slots, setSlots] = useState<StateSlot[] | null>(null);
  const [slot, setSlot] = useState(0);
  const [slotBusy, setSlotBusy] = useState(false);
  const system = game && systemOf(game.systemId);
  // Rewind / fast-forward hints, for emulators Lumen can drive.
  const [controls, setControls] = useState<GameControls | null>(null);

  useEffect(() => {
    api.gameControls().then(setControls).catch(() => setControls(null));
  }, [gameId]);

  useEffect(() => {
    if (!quickMenu) return;
    setFocus(0);
    api.stateSlots().then(setSlots).catch(() => setSlots(null));
    api.gameControls().then(setControls).catch(() => setControls(null));
  }, [quickMenu]);

  const saveSlot = async () => {
    if (slotBusy || !slots) return;
    setSlotBusy(true);
    try {
      setSlots(await api.saveState(slots[slot].slot));
      sfx.confirm();
      notify(`Slot ${slots[slot].slot}'e kaydedildi`, "success");
    } catch (e) {
      sfx.error();
      notify(String(e), "error");
    } finally {
      setSlotBusy(false);
    }
  };

  const loadSlot = async () => {
    if (slotBusy || !slots?.[slot]?.savedAt) return;
    setSlotBusy(true);
    try {
      await api.loadState(slots[slot].slot);
      notify(`Slot ${slots[slot].slot} yüklendi`, "success");
      actions.resume();
    } catch (e) {
      sfx.error();
      notify(String(e), "error");
    } finally {
      setSlotBusy(false);
    }
  };

  const stop = () => {
    if (stopping) return;
    setStopping(true);
    sfx.back();
    rumble(0.5, 60);
    actions.stop();
  };

  const items = [
    { id: "resume", label: "Oyuna dön", run: () => actions.resume() },
    ...(slots ? [{ id: "slots", label: "Kayıt slotları", run: saveSlot }] : []),
    ...(controls?.discs ? [{ id: "disc", label: "Sonraki diske geç", run: () => actions.switchDisc(true) }] : []),
    { id: "shot", label: "Ekran görüntüsü al", run: () => actions.takeScreenshot() },
    { id: "quit", label: stopping ? "Kaydediliyor…" : "Kaydet ve çık", run: stop },
  ];
  const onSlots = items[focus]?.id === "slots";
  const onDisc = items[focus]?.id === "disc";

  useInput((action) => {
    if (!quickMenu) {
      // Lumen only shows this while the emulator isn't in front: a press
      // here meant "play", so it returns to the game. Quitting takes X.
      if (action === "a") actions.resume();
      else if (action === "x") stop();
      return;
    }
    switch (action) {
      case "up":
      case "down": {
        const next = Math.min(items.length - 1, Math.max(0, focus + (action === "up" ? -1 : 1)));
        if (next !== focus) {
          sfx.move();
          setFocus(next);
        }
        break;
      }
      case "left":
      case "right":
        if (onDisc && action === "left") actions.switchDisc(false);
        else if (onSlots && slots) {
          const next = Math.min(slots.length - 1, Math.max(0, slot + (action === "left" ? -1 : 1)));
          if (next !== slot) {
            sfx.move();
            setSlot(next);
          }
        }
        break;
      case "a":
        if (!onSlots) sfx.confirm();
        items[focus].run();
        break;
      case "x":
        if (onSlots) loadSlot();
        break;
      case "b":
        actions.resume();
        break;
    }
  }, Layer.running);

  return (
    <div
      className={`running${quickMenu ? " quick" : ""}`}
      style={{ "--accent": system?.accent, "--card-accent": system?.accent } as CSSProperties}
    >
      <div className="running-inner">
        {game && (
          <div className="running-art">
            <Cover game={game} />
          </div>
        )}
        <div className="eyebrow">
          <span lang="en">{system?.name}</span> · {quickMenu ? "duraklatıldı" : "oynanıyor"}
        </div>
        <h2>{game?.title ?? "Oyun"}</h2>

        {quickMenu ? (
          <div className="quick-list">
            {items.map((item, i) =>
              item.id === "slots" && slots ? (
                <div key="slots" className={`slot-row${i === focus ? " focused" : ""}`}>
                  {slots.map((s, j) => (
                    <button
                      key={s.slot}
                      className={`slot${j === slot ? " selected" : ""}`}
                      onClick={() => {
                        setFocus(i);
                        setSlot(j);
                      }}
                      tabIndex={-1}
                    >
                      <span className="slot-shot">
                        {s.thumbnail ? <img src={s.thumbnail} alt="" draggable={false} /> : <span>{s.savedAt ? "✓" : "Boş"}</span>}
                      </span>
                      <span className="slot-label">
                        Slot {s.slot}
                        {s.savedAt && <small>{savedWhen(s.savedAt)}</small>}
                      </span>
                    </button>
                  ))}
                </div>
              ) : (
                <button
                  key={item.id}
                  className={`btn${i === 0 ? " primary" : ""}${i === focus ? " focused" : ""}`}
                  onClick={item.run}
                  disabled={stopping}
                  tabIndex={-1}
                >
                  {item.label}
                </button>
              ),
            )}
          </div>
        ) : (
          <>
            <p>
              {backButtons ? (
                <>
                  Oyun içinde <b>arka tuşa (M1/M2)</b> dokunmak menüyü açar, basılı tutmak kaydedip çıkar; oyuna hiçbir
                  tuş gitmez.
                </>
              ) : (
                <>
                  Oyun içinde <span className="combo">
                    <Glyph action="select" /> + <Glyph action="start" />
                  </span>{" "}
                  menüyü açar, basılı tutmak kaydedip çıkar.
                </>
              )}{" "}
              Ekranın sağ üst köşesindeki ☰ düğmesine dokunmak da menüyü açar.
              {!isTauri && " (Tarayıcı önizlemesi: menü için Q + E'ye birlikte bas.)"}
            </p>
            <div className="quick-list">
              <button className="btn primary focused" onClick={() => actions.resume()} disabled={stopping}>
                Oyuna dön
              </button>
              <button className="btn" onClick={stop} disabled={stopping}>
                {stopping ? "Kaydediliyor…" : "Kaydet ve çık"}
              </button>
            </div>
          </>
        )}
        {controls && (
          <div className="running-shortcuts">
            {controls.rewind && (
              <span>
                <span className="combo">
                  <Glyph action="select" /> + <Glyph action="lt" />
                </span>{" "}
                basılı tut: geri sar
              </span>
            )}
            {controls.fastForward && (
              <span>
                <span className="combo">
                  <Glyph action="select" /> + <Glyph action="rt" />
                </span>{" "}
                basılı tut: 3× hızlı
              </span>
            )}
            <span>
              <span className="combo">
                <Glyph action="select" /> + <Glyph action="y" />
              </span>{" "}
              ekran görüntüsü
            </span>
          </div>
        )}
      </div>
      <HintBar
        hints={
          quickMenu
            ? onDisc
              ? [
                  { action: "a", label: "Sonraki disk" },
                  { action: "left", label: "Önceki disk" },
                  { action: "b", label: "Oyuna dön" },
                ]
              : onSlots
              ? [
                  { action: "left", label: "Slot" },
                  { action: "a", label: slotBusy ? "Kaydediliyor…" : "Kaydet" },
                  ...(slots?.[slot]?.savedAt ? [{ action: "x" as const, label: "Yükle" }] : []),
                  { action: "b", label: "Oyuna dön" },
                ]
              : [
                  { action: "a", label: "Seç" },
                  { action: "b", label: "Oyuna dön" },
                ]
            : [
                { action: "a", label: "Oyuna dön" },
                { action: "x", label: "Kaydet ve çık" },
              ]
        }
      />
    </div>
  );
}
