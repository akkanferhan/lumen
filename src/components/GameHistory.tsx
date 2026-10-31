import { useEffect, useRef, useState } from "react";
import { HintBar } from "./HintBar";
import { shotUrl } from "./ShotGallery";
import { formatLastPlayed, formatPlaytime } from "../lib/format";
import { Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import type { Session, Shot } from "../lib/types";

/** A minute of slack: a shot saved just after the session ended still belongs to it. */
const SLACK = 60;

/**
 * "Geçmiş": a game's sessions, newest first, each with the screenshots
 * taken during it. Up/down to move, A on a session with shots opens them.
 */
export function GameHistory({
  sessions,
  shots,
  onOpenShot,
  onClose,
}: {
  sessions: Session[];
  shots: Shot[];
  onOpenShot: (shot: Shot) => void;
  onClose: () => void;
}) {
  const [index, setIndex] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const rows = sessions.map((s) => ({
    session: s,
    shots: shots.filter((shot) => shot.takenAt >= s.start && shot.takenAt <= s.start + s.seconds + SLACK),
  }));
  const row = rows[index];

  useEffect(() => {
    listRef.current?.children[index]?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [index]);

  useInput((action) => {
    if (action === "up" || action === "down") {
      const next = Math.min(rows.length - 1, Math.max(0, index + (action === "up" ? -1 : 1)));
      if (next !== index) {
        sfx.move();
        setIndex(next);
      }
    } else if (action === "a" && row?.shots.length) {
      sfx.confirm();
      onOpenShot(row.shots[row.shots.length - 1]);
    } else if (action === "b") {
      sfx.back();
      onClose();
    }
  }, Layer.menu);

  const total = sessions.reduce((sum, s) => sum + s.seconds, 0);
  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="menu history">
        <h2>Geçmiş</h2>
        <p className="menu-sub">
          {sessions.length} oturum · toplam {formatPlaytime(total)}
        </p>
        <div className="menu-list" ref={listRef}>
          {rows.map(({ session, shots: taken }, i) => (
            <button
              key={`${session.start}-${i}`}
              className={`menu-item history-row${i === index ? " focused" : ""}`}
              onClick={() => (i === index && taken.length ? onOpenShot(taken[taken.length - 1]) : setIndex(i))}
              tabIndex={-1}
            >
              <div className="menu-item-main">
                <div className="menu-item-title">
                  {formatLastPlayed(session.start)} ·{" "}
                  {new Date(session.start * 1000).toLocaleTimeString("tr-TR", { hour: "2-digit", minute: "2-digit" })}
                </div>
                <div className="menu-item-sub">{formatPlaytime(session.seconds)}</div>
                {taken.length > 0 && (
                  <div className="history-shots">
                    {taken.slice(-4).map((shot) => (
                      <img key={shot.path} src={shotUrl(shot.path)} alt="" loading="lazy" draggable={false} />
                    ))}
                    {taken.length > 4 && <span>+{taken.length - 4}</span>}
                  </div>
                )}
              </div>
            </button>
          ))}
        </div>
      </aside>
      <HintBar
        hints={[
          ...(row?.shots.length ? [{ action: "a" as const, label: "Görüntülere bak" }] : []),
          { action: "b", label: "Kapat" },
        ]}
      />
    </>
  );
}
