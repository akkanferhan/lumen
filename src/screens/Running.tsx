import { useState, type CSSProperties } from "react";
import { Cover } from "../components/Cover";
import { Glyph } from "../components/Glyph";
import { HintBar } from "../components/HintBar";
import { systemOf } from "../lib/catalog";
import { Layer, useInput } from "../lib/input";
import { isTauri } from "../lib/api";
import { sfx } from "../lib/sfx";
import { actions, useStore } from "../lib/store";

/** Shown behind the emulator; visible if the user alt-tabs back or the emulator is windowed. */
export function Running({ gameId }: { gameId: string }) {
  const game = useStore((s) => s.games.find((g) => g.id === gameId));
  const [stopping, setStopping] = useState(false);
  const system = game && systemOf(game.systemId);

  const stop = () => {
    if (stopping) return;
    setStopping(true);
    sfx.back();
    actions.stop();
  };

  useInput((action) => {
    if (action === "a" || action === "b") stop();
  }, Layer.running);

  return (
    <div className="running" style={{ "--accent": system?.accent, "--card-accent": system?.accent } as CSSProperties}>
      <div className="running-inner">
        {game && (
          <div className="running-art">
            <Cover game={game} />
          </div>
        )}
        <div className="eyebrow">
          <span lang="en">{system?.name}</span> · oynanıyor
        </div>
        <h2>{game?.title ?? "Oyun"}</h2>
        <p>
          Eve dönmek için oyun içinde{" "}
          <span className="combo">
            <Glyph action="select" /> + <Glyph action="start" />
          </span>{" "}
          tuşlarını basılı tut.
          {!isTauri && " (Tarayıcı önizlemesi: oyun simüle ediliyor.)"}
        </p>
        <button className="btn primary focused" onClick={stop} disabled={stopping}>
          {stopping ? "Kapatılıyor…" : "Oyunu kapat"}
        </button>
      </div>
      <HintBar hints={[{ action: "a", label: "Oyunu kapat" }]} />
    </div>
  );
}
