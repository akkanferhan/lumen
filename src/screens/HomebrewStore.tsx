import { useEffect, useRef, useState, type CSSProperties } from "react";
import { HintBar } from "../components/HintBar";
import { ProgressBar } from "../components/Toasts";
import { api } from "../lib/api";
import { homebrew, systemOf, systems } from "../lib/catalog";
import { Layer, rumble, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, notify, useStore } from "../lib/store";
import type { HomebrewGame } from "../lib/types";
import { useColumns } from "../lib/useColumns";

interface Tab {
  id: string;
  label: string;
  games: HomebrewGame[];
}

/** All, the rights-holder-freed classics, then one tab per system in catalog order. */
const TABS: Tab[] = [
  { id: "all", label: "Tümü", games: homebrew },
  { id: "classic", label: "Klasikler", games: homebrew.filter((g) => g.collection === "classic") },
  ...systems.map((s) => ({ id: s.id, label: s.short, games: homebrew.filter((g) => g.system === s.id) })),
].filter((t) => t.games.length > 0);

/**
 * Free games: homebrew whose licenses allow sharing and classics their rights
 * holders made free. Downloads go straight into the right system folder and
 * show up in the library.
 */
export function HomebrewStore({ onClose }: { onClose: () => void }) {
  const installs = useStore((s) => s.installs);
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [tab, setTab] = useState(0);
  const [focus, setFocus] = useState(0);
  // Classics are free for non-commercial use only: the first press asks, the second downloads.
  const [acknowledged, setAcknowledged] = useState<string | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const tabTrackRef = useRef<HTMLDivElement>(null);
  const columns = useColumns(gridRef, 4);

  useEffect(() => {
    api.homebrewInstalled().then((slugs) => setInstalled(new Set(slugs))).catch(() => {});
  }, []);

  const list = TABS[tab].games;
  const index = Math.min(focus, Math.max(0, list.length - 1));
  const game = list[index];

  useEffect(() => {
    tabTrackRef.current?.children[tab]?.scrollIntoView({ inline: "center", behavior: "smooth", block: "nearest" });
  }, [tab]);

  useEffect(() => {
    gridRef.current?.children[index]?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [index, tab]);

  const download = async (g: HomebrewGame) => {
    if (installed.has(g.slug)) {
      notify(`${g.title} zaten kütüphanende`, "success");
      return;
    }
    if (installs[`homebrew:${g.slug}`]) return;
    if (g.collection === "classic" && acknowledged !== g.slug) {
      sfx.confirm();
      setAcknowledged(g.slug);
      notify(`${g.title}: yalnızca ticari olmayan kişisel kullanım için ücretsiz. Onaylıyorsan tekrar İndir'e bas.`, "info", 6000);
      return;
    }
    sfx.confirm();
    rumble();
    if (await actions.installHomebrew(g.slug)) {
      setInstalled((s) => new Set(s).add(g.slug));
      notify(`${g.title} kütüphaneye eklendi`, "success");
    }
  };

  const move = (next: number) => {
    if (next === index || next < 0 || next >= list.length) return;
    sfx.move();
    setFocus(next);
  };

  useInput((action) => {
    switch (action) {
      case "left":
        if (index % columns !== 0) move(index - 1);
        break;
      case "right":
        if (index % columns !== columns - 1) move(index + 1);
        break;
      case "up":
        move(index - columns);
        break;
      case "down":
        if (index + columns < list.length) move(index + columns);
        else if (Math.floor(index / columns) < Math.floor((list.length - 1) / columns)) move(list.length - 1);
        break;
      case "lb":
      case "rb": {
        sfx.tab();
        setTab((t) => (t + (action === "rb" ? 1 : TABS.length - 1)) % TABS.length);
        setFocus(0);
        break;
      }
      case "a":
        if (game) download(game);
        break;
      case "b":
        sfx.back();
        onClose();
        break;
    }
  }, Layer.sheet);

  const job = game && installs[`homebrew:${game.slug}`];
  const system = game && systemOf(game.system);

  return (
    <div className="store" style={{ "--accent": system?.accent } as CSSProperties}>
      <header className="store-head">
        <div>
          <h2>Ücretsiz oyunlar</h2>
          <p>
            Geliştiricilerinin paylaşıma izin verdiği homebrew'lar ve hak sahiplerinin ücretsiz yaptığı klasikler
          </p>
        </div>
        <nav className="tabs store-tabs">
          <span className="glyph pill">LB</span>
          <div className="tab-track" ref={tabTrackRef}>
            {TABS.map((t, i) => (
              <button key={t.id} className={`tab${i === tab ? " active" : ""}`} onClick={() => setTab(i)}>
                {t.label}
                <span className="count">{t.games.length}</span>
              </button>
            ))}
          </div>
          <span className="glyph pill">RB</span>
        </nav>
      </header>

      <div className="store-body">
        <div className="store-grid-scroll">
          <div className="store-grid" ref={gridRef}>
            {list.map((g, i) => {
              const progress = installs[`homebrew:${g.slug}`];
              return (
                <button
                  key={g.slug}
                  className={`store-card${i === index ? " focused" : ""}`}
                  style={{ "--card-accent": systemOf(g.system)?.accent } as CSSProperties}
                  onClick={() => (i === index ? download(g) : setFocus(i))}
                  tabIndex={-1}
                >
                  <div className="store-shot">
                    {g.cover && <img src={g.cover} alt="" loading="lazy" draggable={false} />}
                    <span className="store-system">{systemOf(g.system)?.short}</span>
                    {installed.has(g.slug) && <span className="store-owned">✓</span>}
                  </div>
                  <span className="store-title">{g.title}</span>
                  {progress && progress.stage === "downloading" && <ProgressBar value={progress.progress} />}
                </button>
              );
            })}
          </div>
        </div>

        {game && (
          <aside className="store-detail" lang="en">
            <span className="sheet-system">
              <span className="dot" />
              {system?.name}
            </span>
            <h3>{game.title}</h3>
            <div className="facts">
              {[game.developer, game.year].filter(Boolean).map((f) => (
                <span key={f}>{f}</span>
              ))}
              <span className="license">Lisans: {game.license}</span>
            </div>
            {game.description && <p className="description">{game.description}</p>}
            {game.source && <p className="store-source">{game.source.replace(/^https?:\/\//, "")}</p>}
            {game.collection === "classic" && (
              <p className="store-note">
                Hak sahibi bu oyunu mamedev.org üzerinden yalnızca ticari olmayan kullanım için ücretsiz yaptı;
                dosya doğrudan oradan indirilir.
              </p>
            )}
            <button
              className={`btn primary focused`}
              onClick={() => download(game)}
              disabled={!!job && job.stage !== "error"}
            >
              {installed.has(game.slug)
                ? "✓ Kütüphanede"
                : job
                  ? "İndiriliyor…"
                  : acknowledged === game.slug
                    ? "Onayla ve indir"
                    : "İndir"}
            </button>
          </aside>
        )}
      </div>

      <HintBar
        left={`${index + 1} / ${list.length}`}
        hints={[
          { action: "a", label: game && installed.has(game.slug) ? "Kütüphanede" : "İndir" },
          { action: "b", label: "Kapat" },
        ]}
      />
    </div>
  );
}
