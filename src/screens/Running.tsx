import { useEffect, useRef, useState, type CSSProperties } from "react";
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
import { L, locale } from "../lib/i18n";

/** Today's saves by time (slots are often minutes apart), older ones by day. */
function savedWhen(unixSeconds: number): string {
  const day = formatLastPlayed(unixSeconds);
  return day === L("Bugün", "Today")
    ? new Date(unixSeconds * 1000).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" })
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
  const m2Action = useStore((s) => s.overview?.settings.m2Action);
  const [stopping, setStopping] = useState(false);
  const [focus, setFocus] = useState(0);
  // Save-state slots (RetroArch systems); null hides the row.
  const [slots, setSlots] = useState<StateSlot[] | null>(null);
  const [slot, setSlot] = useState(0);
  const [slotBusy, setSlotBusy] = useState(false);
  const system = game && systemOf(game.systemId);
  // Small screens can't fit every quick-menu row: keep the focused one in view.
  const scrollRef = useRef<HTMLDivElement>(null);
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
      notify(L(`Slot ${slots[slot].slot}'e kaydedildi`, `Saved to slot ${slots[slot].slot}`), "success");
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
      notify(L(`Slot ${slots[slot].slot} yüklendi`, `Slot ${slots[slot].slot} loaded`), "success");
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
    { id: "resume", label: L("Oyuna dön", "Back to game"), run: () => actions.resume() },
    ...(slots ? [{ id: "slots", label: L("Kayıt slotları", "Save slots"), run: saveSlot }] : []),
    ...(controls?.discs ? [{ id: "disc", label: L("Sonraki diske geç", "Next disc"), run: () => actions.switchDisc(true) }] : []),
    { id: "shot", label: L("Ekran görüntüsü al", "Take screenshot"), run: () => actions.takeScreenshot() },
    { id: "quit", label: stopping ? L("Kaydediliyor…", "Saving…") : L("Kaydet ve çık", "Save and quit"), run: stop },
  ];
  const onSlots = items[focus]?.id === "slots";
  const onDisc = items[focus]?.id === "disc";

  useEffect(() => {
    scrollRef.current
      ?.querySelector(".quick-list .focused, .slot-row.focused")
      ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [focus, quickMenu]);

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
      <div className="running-scroll" ref={scrollRef}>
      <div className="running-inner">
        {game && (
          <div className="running-art">
            <Cover game={game} />
          </div>
        )}
        <div className="eyebrow">
          <span lang="en">{system?.name}</span> · {quickMenu ? L("duraklatıldı", "paused") : L("oynanıyor", "playing")}
        </div>
        <h2>{game?.title ?? L("Oyun", "Game")}</h2>

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
                        {s.thumbnail ? <img src={s.thumbnail} alt="" draggable={false} /> : <span>{s.savedAt ? "✓" : L("Boş", "Empty")}</span>}
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
                L(
                  <>
                    Oyun içinde <b>M1</b> arka tuşuna dokunmak menüyü açar, basılı tutmak kaydedip çıkar; oyuna hiçbir
                    tuş gitmez.
                  </>,
                  <>
                    In game, tap the <b>M1</b> back button to open the menu, or hold it to save and quit; the game
                    never sees the press.
                  </>,
                )
              ) : (
                L(
                  <>
                    Oyun içinde <span className="combo">
                      <Glyph action="select" /> + <Glyph action="start" />
                    </span>{" "}
                    menüyü açar, basılı tutmak kaydedip çıkar.
                  </>,
                  <>
                    In game, <span className="combo">
                      <Glyph action="select" /> + <Glyph action="start" />
                    </span>{" "}
                    opens the menu; hold to save and quit.
                  </>,
                )
              )}{" "}
              {L(
                "Ekranın sağ üst köşesindeki ☰ düğmesine dokunmak da menüyü açar.",
                "Tapping the ☰ button in the top-right corner also opens the menu.",
              )}
              {!isTauri && L(" (Tarayıcı önizlemesi: menü için Q + E'ye birlikte bas.)", " (Browser preview: press Q + E together for the menu.)")}
            </p>
            <div className="quick-list">
              <button className="btn primary focused" onClick={() => actions.resume()} disabled={stopping}>
                {L("Oyuna dön", "Back to game")}
              </button>
              <button className="btn" onClick={stop} disabled={stopping}>
                {stopping ? L("Kaydediliyor…", "Saving…") : L("Kaydet ve çık", "Save and quit")}
              </button>
            </div>
          </>
        )}
        {controls && (
          <div className="running-shortcuts">
            {backButtons && m2Action && (m2Action === "screenshot" || (m2Action === "rewind" ? controls.rewind : controls.fastForward)) && (
              <span>
                <b>M2</b>{" "}
                {m2Action === "rewind"
                  ? L("basılı tut: geri sar", "hold: rewind")
                  : m2Action === "fastForward"
                    ? L("basılı tut: 3× hızlı", "hold: 3× speed")
                    : L("ekran görüntüsü", "screenshot")}
              </span>
            )}
            {controls.rewind && (
              <span>
                <span className="combo">
                  <Glyph action="select" /> + <Glyph action="lt" />
                </span>{" "}
                {L("basılı tut: geri sar", "hold: rewind")}
              </span>
            )}
            {controls.fastForward && (
              <span>
                <span className="combo">
                  <Glyph action="select" /> + <Glyph action="rt" />
                </span>{" "}
                {L("basılı tut: 3× hızlı", "hold: 3× speed")}
              </span>
            )}
            <span>
              <span className="combo">
                <Glyph action="select" /> + <Glyph action="y" />
              </span>{" "}
              {L("ekran görüntüsü", "screenshot")}
            </span>
          </div>
        )}
      </div>
      </div>
      <HintBar
        hints={
          quickMenu
            ? onDisc
              ? [
                  { action: "a", label: L("Sonraki disk", "Next disc") },
                  { action: "left", label: L("Önceki disk", "Previous disc") },
                  { action: "b", label: L("Oyuna dön", "Back to game") },
                ]
              : onSlots
              ? [
                  { action: "left", label: "Slot" },
                  { action: "a", label: slotBusy ? L("Kaydediliyor…", "Saving…") : L("Kaydet", "Save") },
                  ...(slots?.[slot]?.savedAt ? [{ action: "x" as const, label: L("Yükle", "Load") }] : []),
                  { action: "b", label: L("Oyuna dön", "Back to game") },
                ]
              : [
                  { action: "a", label: L("Seç", "Select") },
                  { action: "b", label: L("Oyuna dön", "Back to game") },
                ]
            : [
                { action: "a", label: L("Oyuna dön", "Back to game") },
                { action: "x", label: L("Kaydet ve çık", "Save and quit") },
              ]
        }
      />
    </div>
  );
}
