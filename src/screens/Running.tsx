import { useEffect, useState, type CSSProperties } from "react";
import { Cover } from "../components/Cover";
import { Glyph } from "../components/Glyph";
import { HintBar } from "../components/HintBar";
import { isTauri } from "../lib/api";
import { systemOf } from "../lib/catalog";
import { Layer, rumble, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, useStore } from "../lib/store";

/**
 * While a game runs Lumen sits behind the emulator showing this screen.
 * Tapping Select + Start in-game brings it to the front as the quick menu
 * (the emulator pauses on focus loss).
 */
export function Running({ gameId }: { gameId: string }) {
  const game = useStore((s) => s.games.find((g) => g.id === gameId));
  const quickMenu = useStore((s) => s.quickMenu);
  const [stopping, setStopping] = useState(false);
  const [focus, setFocus] = useState(0);
  const system = game && systemOf(game.systemId);

  useEffect(() => {
    if (quickMenu) setFocus(0);
  }, [quickMenu]);

  const stop = () => {
    if (stopping) return;
    setStopping(true);
    sfx.back();
    rumble(0.5, 60);
    actions.stop();
  };

  const items = [
    { id: "resume", label: "Oyuna dön", run: () => actions.resume() },
    { id: "quit", label: stopping ? "Kaydediliyor…" : "Kaydet ve çık", run: stop },
  ];

  useInput((action) => {
    if (!quickMenu) {
      if (action === "a" || action === "b") stop();
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
      case "a":
        sfx.confirm();
        items[focus].run();
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
            {items.map((item, i) => (
              <button
                key={item.id}
                className={`btn${i === 0 ? " primary" : ""}${i === focus ? " focused" : ""}`}
                onClick={item.run}
                disabled={stopping}
                tabIndex={-1}
              >
                {item.label}
              </button>
            ))}
          </div>
        ) : (
          <>
            <p>
              Oyun içinde <span className="combo">
                <Glyph action="select" /> + <Glyph action="start" />
              </span>{" "}
              menüyü açar, basılı tutmak kaydedip çıkar.
              {!isTauri && " (Tarayıcı önizlemesi: menü için Q + E'ye birlikte bas.)"}
            </p>
            <button className="btn primary focused" onClick={stop} disabled={stopping}>
              {stopping ? "Kaydediliyor…" : "Kaydet ve çık"}
            </button>
          </>
        )}
      </div>
      <HintBar
        hints={
          quickMenu
            ? [
                { action: "a", label: "Seç" },
                { action: "b", label: "Oyuna dön" },
              ]
            : [{ action: "a", label: "Kaydet ve çık" }]
        }
      />
    </div>
  );
}
