import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { BrandMark } from "../components/BrandMark";
import { Cover } from "../components/Cover";
import { HintBar } from "../components/HintBar";
import { StatusBar } from "../components/StatusBar";
import { boxArtUrl, systemOf, systems } from "../lib/catalog";
import { Layer, rumble, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, useStore } from "../lib/store";
import type { Game } from "../lib/types";
import { useColumns } from "../lib/useColumns";
import { fold, groupLetter } from "../lib/format";
import { SearchKeyboard } from "../components/SearchKeyboard";

interface Tab {
  id: string;
  label: string;
  games: Game[];
}

function useTabs(games: Game[]): Tab[] {
  return useMemo(() => {
    const recent = games
      .filter((g) => g.lastPlayed)
      .sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))
      .slice(0, 24);
    const favorites = games.filter((g) => g.favorite);
    const bySystem = systems
      .map((s) => ({ id: s.id, label: s.short, games: games.filter((g) => g.systemId === s.id) }))
      .filter((t) => t.games.length > 0);

    return [
      { id: "all", label: "Tümü", games },
      ...(recent.length ? [{ id: "recent", label: "Son oynanan", games: recent }] : []),
      ...(favorites.length ? [{ id: "fav", label: "Favoriler", games: favorites }] : []),
      ...bySystem,
    ];
  }, [games]);
}

export function Library({
  onOpen,
  onMenu,
  covered,
}: {
  onOpen: (game: Game) => void;
  onMenu: () => void;
  /** An overlay owns the hint bar. */
  covered: boolean;
}) {
  const games = useStore((s) => s.games);
  const overview = useStore((s) => s.overview);
  const scanning = useStore((s) => s.scanning);
  const tabs = useTabs(games);

  const [tabId, setTabId] = useState("all");
  const tabIndex = Math.max(0, tabs.findIndex((t) => t.id === tabId));
  const tab = tabs[tabIndex];
  // Focus is remembered per tab, so switching back lands where you left off.
  const focusByTab = useRef(new Map<string, number>());
  const [focus, setFocus] = useState(0);
  // Search spans the whole library, whatever tab is selected.
  const [query, setQuery] = useState("");
  const [keyboard, setKeyboard] = useState(false);
  const searching = query.trim().length > 0;
  const results = useMemo(() => {
    const needle = fold(query.trim());
    return needle ? games.filter((g) => fold(g.title).includes(needle)) : [];
  }, [games, query]);
  const list = searching ? results : (tab?.games ?? []);
  // Big letter shown briefly when jumping with LT/RT.
  const [jumpLetter, setJumpLetter] = useState<string | null>(null);
  useEffect(() => {
    if (!jumpLetter) return;
    const id = setTimeout(() => setJumpLetter(null), 650);
    return () => clearTimeout(id);
  }, [jumpLetter]);
  const index = Math.min(focus, Math.max(0, list.length - 1));
  const focused = list[index];

  const gridRef = useRef<HTMLDivElement>(null);
  const tabTrackRef = useRef<HTMLDivElement>(null);
  const columns = useColumns(gridRef);

  useEffect(() => {
    gridRef.current?.children[index]?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [index, tabId]);

  useEffect(() => {
    tabTrackRef.current?.children[tabIndex]?.scrollIntoView({ inline: "center", behavior: "smooth", block: "nearest" });
  }, [tabIndex]);

  const move = (next: number) => {
    if (next === index || next < 0 || next >= list.length) return;
    sfx.move();
    setFocus(next);
  };

  const switchTab = (delta: number) => {
    const next = tabs[(tabIndex + delta + tabs.length) % tabs.length];
    if (!next || next.id === tab?.id) return;
    sfx.tab();
    focusByTab.current.set(tab.id, index);
    setTabId(next.id);
    setFocus(focusByTab.current.get(next.id) ?? 0);
  };

  /** LT/RT: previous/next first letter (A, B, C…) in the current list. */
  const jumpGroup = (delta: 1 | -1) => {
    if (!list.length) return;
    const letterAt = (i: number) => groupLetter(list[i].title);
    const current = letterAt(index);
    let i = index;
    if (delta > 0) {
      while (i < list.length && letterAt(i) === current) i++;
      if (i >= list.length) return;
    } else {
      // Start of this group first; from there, start of the previous one.
      while (i > 0 && letterAt(i - 1) === current) i--;
      if (i === index) {
        if (i === 0) return;
        const previous = letterAt(i - 1);
        i--;
        while (i > 0 && letterAt(i - 1) === previous) i--;
      }
    }
    sfx.tab();
    setFocus(i);
    setJumpLetter(letterAt(i));
  };

  useInput((action) => {
    switch (action) {
      case "lt":
        jumpGroup(-1);
        break;
      case "rt":
        jumpGroup(1);
        break;
      case "b":
        if (searching) {
          sfx.back();
          setQuery("");
          setFocus(focusByTab.current.get(tab?.id ?? "") ?? 0);
        }
        break;
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
        // From a partial last row's gap, land on the last game instead of doing nothing.
        if (index + columns < list.length) move(index + columns);
        else if (Math.floor(index / columns) < Math.floor((list.length - 1) / columns)) move(list.length - 1);
        break;
      case "lb":
        switchTab(-1);
        break;
      case "rb":
        switchTab(1);
        break;
      case "a":
        if (focused) {
          sfx.confirm();
          rumble();
          onOpen(focused);
        }
        break;
      case "y":
        if (focused) {
          sfx.confirm();
          actions.toggleFavorite(focused.id);
        }
        break;
      case "x":
        sfx.confirm();
        if (!searching) focusByTab.current.set(tab?.id ?? "", index);
        setKeyboard(true);
        break;
      case "start":
        sfx.confirm();
        onMenu();
        break;
    }
  }, Layer.screen, !keyboard);

  const accent = (focused && systemOf(focused.systemId)?.accent) || "var(--brand)";
  const art = focused ? boxArtUrl(focused) : null;
  const root = overview?.settings.romRoot ?? "";

  return (
    <div className="library" style={{ "--accent": accent } as CSSProperties}>
      <Ambient art={art} />

      <header className="topbar">
        <div className="brand">
          <BrandMark className="brand-mark" />
          Lumen
        </div>
        <StatusBar />
      </header>

      <nav className="tabs">
        {searching && (
          <button className="tab active search-chip" onClick={() => setKeyboard(true)}>
            ⌕ “{query.trim()}”<span className="count">{results.length}</span>
          </button>
        )}
        <span className="glyph pill" hidden={searching}>LB</span>
        <div className="tab-track" ref={tabTrackRef} hidden={searching}>
          {tabs.map((t, i) => (
            <button
              key={t.id}
              className={`tab${i === tabIndex ? " active" : ""}`}
              onClick={() => switchTab(i - tabIndex)}
            >
              {t.label}
              <span className="count">{t.games.length}</span>
            </button>
          ))}
        </div>
        <span className="glyph pill" hidden={searching}>RB</span>
      </nav>

      {list.length === 0 ? (
        <div className="empty">
          <div className="empty-inner">
            <h2>{searching ? "Sonuç yok" : scanning ? "Oyunlar aranıyor…" : "Henüz oyun yok"}</h2>
            {searching ? (
              <p>“{query.trim()}” ile eşleşen oyun bulunamadı.</p>
            ) : (
              <>
            <p>
              Oyun dosyalarını sistem klasörlerine koy, örneğin <b>snes</b>, <b>ps2</b> veya <b>gc</b>. Ardından
              kütüphaneyi yenile.
            </p>
            {root && <div className="path-chip">{root}</div>}
              </>
            )}
          </div>
        </div>
      ) : (
        <div className="grid-scroll">
          <div className="grid" ref={gridRef}>
            {list.map((game, i) => (
              <GameCard
                key={game.id}
                game={game}
                focused={i === index}
                showSystem={searching || tab.id === "all" || tab.id === "recent" || tab.id === "fav"}
                onClick={() => (i === index ? onOpen(game) : move(i))}
              />
            ))}
          </div>
        </div>
      )}

      {jumpLetter && <div className="jump-letter">{jumpLetter}</div>}

      {keyboard && (
        <SearchKeyboard
          query={query}
          results={results.length}
          onChange={(q) => {
            setQuery(q);
            setFocus(0);
          }}
          onDone={() => setKeyboard(false)}
          onCancel={() => {
            setQuery("");
            setKeyboard(false);
          }}
        />
      )}

      {!covered && !keyboard && (
        <HintBar
          left={focused ? `${index + 1} / ${list.length}` : null}
          hints={[
            { action: "a", label: "Aç" },
            { action: "y", label: focused?.favorite ? "Favoriden çıkar" : "Favori" },
            { action: "x", label: "Ara" },
            ...(searching ? [{ action: "b" as const, label: "Aramayı temizle" }] : []),
            { action: "start", label: "Menü" },
          ]}
        />
      )}
    </div>
  );
}

function GameCard({
  game,
  focused,
  showSystem,
  onClick,
}: {
  game: Game;
  focused: boolean;
  showSystem: boolean;
  onClick: () => void;
}) {
  const system = systemOf(game.systemId);
  return (
    <button
      className={`card${focused ? " focused" : ""}`}
      style={{ "--card-accent": system?.accent } as CSSProperties}
      onClick={onClick}
      tabIndex={-1}
    >
      <div className="card-art">
        <Cover game={game} />
        {game.favorite && <span className="fav-badge">★</span>}
      </div>
      <span className="card-title">{game.title}</span>
      {showSystem && <span className="card-sub">{system?.short}</span>}
    </button>
  );
}

/** Blurred box art of the focused game, cross-faded. */
export function Ambient({ art }: { art: string | null }) {
  const [layers, setLayers] = useState<{ url: string; key: number }[]>([]);
  const key = useRef(0);

  useEffect(() => {
    if (!art) return;
    // Debounced so fast scrolling doesn't thrash image loads.
    const id = setTimeout(() => {
      const img = new Image();
      img.onload = () => setLayers((l) => [...l.slice(-1), { url: art, key: ++key.current }]);
      img.src = art;
    }, 180);
    return () => clearTimeout(id);
  }, [art]);

  return (
    <div className="ambient">
      {layers.map((l, i) => (
        <AmbientLayer key={l.key} url={l.url} visible={i === layers.length - 1} />
      ))}
    </div>
  );
}

function AmbientLayer({ url, visible }: { url: string; visible: boolean }) {
  const [shown, setShown] = useState(false);
  useEffect(() => {
    const id = requestAnimationFrame(() => setShown(true));
    return () => cancelAnimationFrame(id);
  }, []);
  return (
    <div className={`ambient-art${shown && visible ? " visible" : ""}`} style={{ backgroundImage: `url("${url}")` }} />
  );
}
