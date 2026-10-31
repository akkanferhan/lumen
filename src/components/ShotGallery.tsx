import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { HintBar } from "./HintBar";
import { api } from "../lib/api";
import { formatLastPlayed } from "../lib/format";
import { Layer, rumble, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { notify } from "../lib/store";
import type { Shot } from "../lib/types";
import { L, locale } from "../lib/i18n";

/** A screenshot's address: files on disk go through Tauri's asset protocol. */
export const shotUrl = (path: string) => (/^https?:/.test(path) ? path : convertFileSrc(path));

/**
 * A game's screenshots, one at a time, full size: left/right to browse,
 * X twice to delete one.
 */
export function ShotGallery({
  shots,
  start,
  onChange,
  onClose,
}: {
  shots: Shot[];
  /** Opens on this screenshot (from the history). */
  start?: Shot;
  onChange: () => void;
  onClose: () => void;
}) {
  const [index, setIndex] = useState(() => Math.max(0, start ? shots.findIndex((s) => s.path === start.path) : 0));
  const [armed, setArmed] = useState(false);
  const shot = shots[Math.min(index, shots.length - 1)];
  useEffect(() => setArmed(false), [index]);
  useEffect(() => {
    if (!shots.length) onClose();
  }, [shots.length, onClose]);

  const remove = async () => {
    if (!shot) return;
    if (!armed) {
      sfx.error();
      rumble(0.3, 40);
      setArmed(true);
      return;
    }
    sfx.back();
    try {
      await api.deleteScreenshot(shot.path);
      setIndex((i) => Math.max(0, Math.min(i, shots.length - 2)));
      onChange();
    } catch (e) {
      notify(String(e), "error");
    }
    setArmed(false);
  };

  useInput((action) => {
    if (action === "left" || action === "right") {
      const next = Math.min(shots.length - 1, Math.max(0, index + (action === "left" ? -1 : 1)));
      if (next !== index) {
        sfx.move();
        setIndex(next);
      }
    } else if (action === "x") remove();
    else if (action === "b") {
      sfx.back();
      onClose();
    }
  }, Layer.menu);

  if (!shot) return null;
  const when = formatLastPlayed(shot.takenAt);
  const time = new Date(shot.takenAt * 1000).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });

  return (
    <div className="shot-gallery" role="dialog" aria-label={L("Ekran görüntüleri", "Screenshots")}>
      <img key={shot.path} className="shot-full" src={shotUrl(shot.path)} alt="" draggable={false} />
      <div className="shot-caption">
        {index + 1} / {shots.length} · {when} {time}
        {armed && <b>{L(" · Silmek için X'e tekrar bas", " · Press X again to delete")}</b>}
      </div>
      <HintBar
        hints={[
          ...(shots.length > 1 ? [{ action: "left" as const, label: L("Önceki / sonraki", "Previous / next") }] : []),
          { action: "x", label: armed ? L("Emin misin? Sil", "Are you sure? Delete") : L("Sil", "Delete") },
          { action: "b", label: L("Kapat", "Close") },
        ]}
      />
    </div>
  );
}
