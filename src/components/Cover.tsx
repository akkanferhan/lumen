import { memo, useState } from "react";
import { boxArtUrl, systemOf } from "../lib/catalog";
import type { Game } from "../lib/types";

/** Box art if libretro has it, otherwise a typographic card in the system color. */
export const Cover = memo(function Cover({ game }: { game: Game }) {
  const url = boxArtUrl(game);
  const [state, setState] = useState<"loading" | "loaded" | "failed">(url ? "loading" : "failed");
  const system = systemOf(game.systemId);

  if (state === "failed" || !url) {
    return (
      <div className="cover-fallback">
        <span className="cover-fallback-system">{system?.short}</span>
        <span className="cover-fallback-title">{game.title}</span>
      </div>
    );
  }

  return (
    <div className={`cover ${state}`}>
      {state === "loaded" && <img className="cover-blur" src={url} alt="" aria-hidden />}
      <img
        className="cover-img"
        src={url}
        alt={game.title}
        loading="lazy"
        decoding="async"
        draggable={false}
        onLoad={() => setState("loaded")}
        onError={() => setState("failed")}
      />
    </div>
  );
});
