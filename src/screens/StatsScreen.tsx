import { useEffect, useState } from "react";
import { HintBar } from "../components/HintBar";
import { api } from "../lib/api";
import { systemOf } from "../lib/catalog";
import { formatPlaytime } from "../lib/format";
import { Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { useStore } from "../lib/store";
import type { PlayStats } from "../lib/types";

const WEEKDAY = ["Paz", "Pzt", "Sal", "Çar", "Per", "Cum", "Cmt"];

/** Local midnights the backend can't compute without a time-zone database. */
function localDays() {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime() / 1000;
  const yearStart = new Date(now.getFullYear(), 0, 1).getTime() / 1000;
  return { today, year: now.getFullYear(), yearStart };
}

/** Hours or minutes, short: "2 sa 5 dk", "40 dk". */
function short(seconds: number): string {
  if (seconds < 60) return seconds > 0 ? "<1 dk" : "0";
  const h = Math.floor(seconds / 3600);
  const m = Math.round((seconds % 3600) / 60);
  return h ? `${h} sa${m ? ` ${m} dk` : ""}` : `${m} dk`;
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
  const titleOf = (id: string) => games.find((g) => g.id === id)?.title ?? "Silinmiş oyun";
  const systemOfGame = (id: string) => systemOf(games.find((g) => g.id === id)?.systemId ?? "")?.short;

  return (
    <div className="store stats">
      <header className="store-head">
        <div>
          <h2>İstatistikler</h2>
          <p>Lumen'den açtığın oyunların süreleri. Mağaza oyunları (Steam, Epic) kendi uygulamalarında sayılır.</p>
        </div>
      </header>

      {stats && (
        <div className="store-body">
          <div className="stats-main">
            <div className="stat-hero">
              <span className="stat-label">Bu hafta</span>
              <strong>{short(stats.weekSeconds)}</strong>
              <span className="stat-delta">
                {change === null
                  ? "Geçen hafta oynanmadı"
                  : change === 0
                    ? "Geçen haftayla aynı"
                    : `Geçen haftadan %${Math.abs(change)} ${change > 0 ? "fazla" : "az"}`}
              </span>
            </div>

            <figure className="day-chart" aria-label="Son 7 günün oynama süresi">
              <figcaption className="stat-label">Son 7 gün</figcaption>
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
                      <span className={`day-label${today ? " today" : ""}`}>{today ? "Bugün" : label}</span>
                    </div>
                  );
                })}
              </div>
            </figure>

            <div className="year-tiles">
              <div>
                <span className="stat-label">{stats.year} toplamı</span>
                <strong>{short(stats.yearSeconds)}</strong>
              </div>
              <div>
                <span className="stat-label">Oturum</span>
                <strong>{stats.yearSessions}</strong>
              </div>
              <div>
                <span className="stat-label">Farklı oyun</span>
                <strong>{stats.yearGames}</strong>
              </div>
            </div>
          </div>

          <aside className="store-detail stats-side">
            <h3>En çok oynananlar</h3>
            {stats.topGames.length === 0 ? (
              <p className="description">Henüz oynanmış oyun yok.</p>
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
                <h3>Sistemler</h3>
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

      <HintBar hints={[{ action: "b", label: "Kapat" }]} />
    </div>
  );
}
