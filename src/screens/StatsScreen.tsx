import { useEffect, useState } from "react";
import { HintBar } from "../components/HintBar";
import { api } from "../lib/api";
import { systemOf } from "../lib/catalog";
import { formatPlaytime } from "../lib/format";
import { L } from "../lib/i18n";
import { Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { useStore } from "../lib/store";
import type { PlayStats } from "../lib/types";

const WEEKDAY = L(
  ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"],
  ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
);

/** Local midnights the backend can't compute without a time-zone database. */
function localDays() {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime() / 1000;
  const yearStart = new Date(now.getFullYear(), 0, 1).getTime() / 1000;
  return { today, year: now.getFullYear(), yearStart };
}

/** Hours or minutes, short: "2 sa 5 dk", "40 dk". */
function short(seconds: number): string {
  if (seconds < 60) return seconds > 0 ? L("<1 dk", "<1 min") : "0";
  const h = Math.floor(seconds / 3600);
  const m = Math.round((seconds % 3600) / 60);
  return L(h ? `${h} sa${m ? ` ${m} dk` : ""}` : `${m} dk`, h ? `${h} h${m ? ` ${m} min` : ""}` : `${m} min`);
}

/** Play time this week, by day, and what gets played most. */
export function StatsScreen({ onClose }: { onClose: () => void }) {
  const games = useStore((s) => s.games);
  const [stats, setStats] = useState<PlayStats | null>(null);

  useEffect(() => {
    const { today, year, yearStart } = localDays();
    api.playStats(today, year, yearStart).then(setStats).catch(() => {});
  }, []);

  useInput((action) => {
    if (action === "b") {
      sfx.back();
      onClose();
    }
  }, Layer.sheet);

  const max = Math.max(1, ...(stats?.days.map((d) => d.seconds) ?? [0]));
  const peak = stats?.days.reduce((best, d) => (d.seconds > best.seconds ? d : best), stats.days[0]);
  const change =
    stats && stats.previousWeekSeconds > 0
      ? Math.round(((stats.weekSeconds - stats.previousWeekSeconds) / stats.previousWeekSeconds) * 100)
      : null;
  const titleOf = (id: string) => games.find((g) => g.id === id)?.title ?? L("Silinmiş oyun", "Deleted game");
  const systemOfGame = (id: string) => systemOf(games.find((g) => g.id === id)?.systemId ?? "")?.short;

  return (
    <div className="store stats">
      <header className="store-head">
        <div>
          <h2>{L("İstatistikler", "Stats")}</h2>
          <p>
            {L(
              "Lumen'den açtığın oyunların süreleri. Mağaza oyunları (Steam, Epic) kendi uygulamalarında sayılır.",
              "Playtime for games you launch from Lumen. Store games (Steam, Epic) are tracked in their own apps.",
            )}
          </p>
        </div>
      </header>

      {stats && (
        <div className="store-body">
          <div className="stats-main">
            <div className="stat-hero">
              <span className="stat-label">{L("Bu hafta", "This week")}</span>
              <strong>{short(stats.weekSeconds)}</strong>
              <span className="stat-delta">
                {change === null
                  ? L("Geçen hafta oynanmadı", "No play last week")
                  : change === 0
                    ? L("Geçen haftayla aynı", "Same as last week")
                    : L(
                        `Geçen haftadan %${Math.abs(change)} ${change > 0 ? "fazla" : "az"}`,
                        `${Math.abs(change)}% ${change > 0 ? "more" : "less"} than last week`,
                      )}
              </span>
            </div>

            <figure className="day-chart" aria-label={L("Son 7 günün oynama süresi", "Playtime over the last 7 days")}>
              <figcaption className="stat-label">{L("Son 7 gün", "Last 7 days")}</figcaption>
              <div className="day-bars">
                {stats.days.map((d, i) => {
                  const label = WEEKDAY[new Date(d.start * 1000).getDay()];
                  const today = i === stats.days.length - 1;
                  // Selective labels: today's value and the busiest day's.
                  const labelled = d.seconds > 0 && (today || d === peak);
                  return (
                    <div key={d.start} className="day" title={`${label}: ${formatPlaytime(d.seconds)}`}>
                      <span className="day-value">{labelled ? short(d.seconds) : ""}</span>
                      <span className="day-track">
                        <span className="day-bar" style={{ height: `${(d.seconds / max) * 100}%` }} />
                      </span>
                      <span className={`day-label${today ? " today" : ""}`}>{today ? L("Bugün", "Today") : label}</span>
                    </div>
                  );
                })}
              </div>
            </figure>

            <div className="year-tiles">
              <div>
                <span className="stat-label">{L(`${stats.year} toplamı`, `${stats.year} total`)}</span>
                <strong>{short(stats.yearSeconds)}</strong>
              </div>
              <div>
                <span className="stat-label">{L("Oturum", "Sessions")}</span>
                <strong>{stats.yearSessions}</strong>
              </div>
              <div>
                <span className="stat-label">{L("Farklı oyun", "Games played")}</span>
                <strong>{stats.yearGames}</strong>
              </div>
            </div>
          </div>

          <aside className="store-detail stats-side">
            <h3>{L("En çok oynananlar", "Most played")}</h3>
            {stats.topGames.length === 0 ? (
              <p className="description">{L("Henüz oynanmış oyun yok.", "No games played yet.")}</p>
            ) : (
              <ol className="rank">
                {stats.topGames.map((t) => (
                  <li key={t.id}>
                    <span className="rank-name">
                      {titleOf(t.id)} <small>{systemOfGame(t.id)}</small>
                    </span>
                    <span className="rank-time">{short(t.seconds)}</span>
                  </li>
                ))}
              </ol>
            )}
            {stats.topSystems.length > 0 && (
              <>
                <h3>{L("Sistemler", "Systems")}</h3>
                <ol className="rank">
                  {stats.topSystems.map((t) => (
                    <li key={t.id}>
                      <span className="rank-name">{systemOf(t.id)?.name ?? t.id}</span>
                      <span className="rank-time">{short(t.seconds)}</span>
                    </li>
                  ))}
                </ol>
              </>
            )}
          </aside>
        </div>
      )}

      <HintBar hints={[{ action: "b", label: L("Kapat", "Close") }]} />
    </div>
  );
}
