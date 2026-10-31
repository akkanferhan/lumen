import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { BrandMark } from "../components/BrandMark";
import { Cover } from "../components/Cover";
import { HintBar } from "../components/HintBar";
import { StatusBar } from "../components/StatusBar";
import { SystemGuide } from "../components/SystemGuide";
import { boxArtUrl, homebrew, isPcSystem, systemOf, systems } from "../lib/catalog";
import { Layer, rumble, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, useStore } from "../lib/store";
import { api } from "../lib/api";
import type { Collection, Game } from "../lib/types";

const EMPTY_COLLECTIONS: Collection[] = [];
import { useColumns } from "../lib/useColumns";
import { fold, groupLetter, todayStart } from "../lib/format";
import { SearchKeyboard } from "../components/SearchKeyboard";
import { themeOf } from "../lib/themes";
import { formatLastPlayed, formatPlaytime } from "../lib/format";
import { screenshotUrls } from "../lib/catalog";
import { L } from "../lib/i18n";

interface Tab {
  id: string;
  label: string;
  games: Game[];
  /** A supported system with no games yet: its tab shows how to add some. */
  empty?: boolean;
  /** The first empty system: a divider sets the empty ones apart. */
  firstEmpty?: boolean;
}

function useTabs(games: Game[], collections: Collection[]): Tab[] {
  return useMemo(() => {
    const recent = games
      .filter((g) => g.lastPlayed)
      .sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))
      .slice(0, 24);
    const favorites = games.filter((g) => g.favorite);
    // Every emulated system has a tab, so users see what Lumen supports;
    // store launchers (Steam, Epic) only once they have games.
    const bySystem = systems
      .map((s) => {
        // Installed first, then the ones that only install.
        const list = games
          .filter((g) => g.systemId === s.id)
          .sort((a, b) => Number(!!a.notInstalled) - Number(!!b.notInstalled));
        return { id: s.id, label: s.short, games: list, empty: list.length === 0 };
      })
      .filter((t) => !t.empty || !isPcSystem(t.id));
    // Systems with games come first; empty ones wait at the end so LB/RB
    // doesn't have to step through them to reach your games.
    const filled = bySystem.filter((t) => !t.empty);
    const empty = bySystem.filter((t) => t.empty).map((t, i) => ({ ...t, firstEmpty: i === 0 }));

    // Store games that aren't installed live only in their store's tab.
    const installed = games.filter((g) => !g.notInstalled);
    return [
      { id: "all", label: L("Tümü", "All"), games: installed },
      ...(recent.length ? [{ id: "recent", label: L("Son oynanan", "Recent"), games: recent }] : []),
      ...(favorites.length ? [{ id: "fav", label: L("Favoriler", "Favorites"), games: favorites }] : []),
      // The user's collections, each in the order its games were added.
      ...collections
        .map((c) => ({
          id: `collection:${c.id}`,
          label: c.name,
          games: c.games.map((id) => games.find((g) => g.id === id)).filter((g): g is Game => !!g),
        }))
        .filter((t) => t.games.length > 0),
      ...filled,
      ...empty,
    ];
  }, [games, collections]);
}

export function Library({
  onOpen,
  onMenu,
  onImport,
  onStore,
  covered,
}: {
  onOpen: (game: Game) => void;
  onMenu: () => void;
  onImport: () => void;
  onStore: () => void;
  /** An overlay owns the hint bar. */
  covered: boolean;
}) {
  const allGames = useStore((s) => s.games);
  const overview = useStore((s) => s.overview);
  const layout = themeOf(overview?.settings.theme).layout;
  const scanning = useStore((s) => s.scanning);
  // Kids mode: only the chosen collection (or every console game), no
  // store games, no empty system tabs that lead to importing.
  const kids = overview?.settings.kids;
  const games = useMemo(() => {
    if (!kids?.active) return allGames;
    const allowed = kids.collection
      ? new Set(overview?.collections.find((c) => c.id === kids.collection)?.games ?? [])
      : null;
    return allGames.filter((g) => !isPcSystem(g.systemId) && !g.notInstalled && (!allowed || allowed.has(g.id)));
  }, [allGames, kids?.active, kids?.collection, overview?.collections]);
  const allTabs = useTabs(games, kids?.active ? EMPTY_COLLECTIONS : (overview?.collections ?? EMPTY_COLLECTIONS));
  const tabs = useMemo(() => (kids?.active ? allTabs.filter((t) => !t.empty) : allTabs), [allTabs, kids?.active]);

  const [tabId, setTabId] = useState("all");
  const tabIndex = Math.max(0, tabs.findIndex((t) => t.id === tabId));
  const jumpTab = useStore((s) => s.jumpTab);
  useEffect(() => {
    if (!jumpTab) return;
    setTabId(jumpTab);
    setFocus(0);
    actions.systemShown();
  }, [jumpTab]);
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
  const baseList = searching ? results : (tab?.games ?? []);
  // Raf: one shelf per system; the flat list follows shelf order so the
  // focus index maps onto (shelf, position).
  const shelves = useMemo(() => (layout === "shelves" ? groupShelves(baseList) : null), [layout, baseList]);
  const list = shelves ? shelves.flatMap((s) => s.games) : baseList;
  // Big letter shown briefly when jumping with LT/RT.
  const [jumpLetter, setJumpLetter] = useState<string | null>(null);
  useEffect(() => {
    if (!jumpLetter) return;
    const id = setTimeout(() => setJumpLetter(null), 650);
    return () => clearTimeout(id);
  }, [jumpLetter]);
  const index = Math.min(focus, Math.max(0, list.length - 1));
  // "Devam et": the last game played, above "Tümü". Lumen opens on it, so
  // one press of A continues where you left off; up from the top row
  // reaches it again.
  const heroGame = useMemo(
    () =>
      tab?.id === "all" && !searching
        ? games
            .filter((g) => g.lastPlayed && !g.notInstalled)
            .sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))[0]
        : undefined,
    [games, tab?.id, searching],
  );
  const [heroFocus, setHeroFocus] = useState(true);
  const onHero = heroFocus && !!heroGame;
  const focused = onHero ? undefined : list[index];
  // An empty system tab shows that system's guide instead of a blank page.
  const guide = !searching && tab?.empty ? tab.id : null;
  const freeGames = guide ? homebrew.filter((g) => g.system === guide).length : 0;

  const gridRef = useRef<HTMLDivElement>(null);
  const tabTrackRef = useRef<HTMLDivElement>(null);
  const measured = useColumns(gridRef);
  // Navigation follows the layout: a rail is one row, a list one column.
  const columns =
    layout === "rail" || layout === "coverflow" ? Math.max(1, list.length) : layout === "list" ? 1 : measured;

  useEffect(() => {
    if (layout === "coverflow") return; // positioned, not scrolled
    const target =
      layout === "shelves"
        ? gridRef.current?.querySelector(`[data-index="${index}"]`)
        : gridRef.current?.children[index];
    target?.scrollIntoView({
      block: layout === "shelves" ? "center" : "nearest",
      inline: layout === "rail" || layout === "shelves" ? "center" : "nearest",
      behavior: "smooth",
    });
  }, [index, tabId, layout]);

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
    setHeroFocus(false);
    setTabId(next.id);
    setFocus(focusByTab.current.get(next.id) ?? 0);
  };

  /** Up from the top: onto the "Devam et" card, when there is one. */
  const toHero = () => {
    if (!heroGame) return;
    sfx.move();
    setHeroFocus(true);
  };

  const continueHero = () => {
    if (!heroGame) return;
    sfx.confirm();
    rumble(0.6, 60);
    actions.play(heroGame);
  };

  /** Select: a random playable game from this list. Focus hops across a
   *  few covers first, then lands and opens the game. */
  const shuffling = useRef(false);
  const surprise = () => {
    const playable = list.map((g, i) => [g, i] as const).filter(([g]) => !g.notInstalled);
    if (playable.length < 2 || shuffling.current) return;
    shuffling.current = true;
    const pick = () => playable[Math.floor(Math.random() * playable.length)];
    let hops = 7;
    const hop = () => {
      const [game, i] = pick();
      sfx.move();
      setFocus(i);
      if (--hops > 0) {
        setTimeout(hop, 70 + (7 - hops) * 20);
        return;
      }
      setTimeout(() => {
        shuffling.current = false;
        sfx.confirm();
        rumble();
        onOpen(game);
      }, 350);
    };
    hop();
  };
  const canSurprise = list.filter((g) => !g.notInstalled).length >= 2;

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

  /** Raf navigation: left/right inside a shelf, up/down keeps the column. */
  const moveShelf = (action: "up" | "down" | "left" | "right") => {
    if (!shelves) return;
    const row = shelves.findIndex((s) => index >= s.start && index < s.start + s.games.length);
    const shelf = shelves[row];
    if (!shelf) return;
    const col = index - shelf.start;
    if (action === "left" && col > 0) move(index - 1);
    if (action === "right" && col < shelf.games.length - 1) move(index + 1);
    if (action === "up" || action === "down") {
      const next = shelves[row + (action === "up" ? -1 : 1)];
      if (next) move(next.start + Math.min(col, next.games.length - 1));
      else if (action === "up") toHero();
    }
  };

  useInput((action) => {
    if (onHero && heroGame) {
      if (action === "a") return continueHero();
      if (action === "y") {
        sfx.confirm();
        return onOpen(heroGame);
      }
      if (action === "down") {
        sfx.move();
        return setHeroFocus(false);
      }
      if (action === "up" || action === "left" || action === "right" || action === "lt" || action === "rt") return;
    }
    if (shelves && (action === "up" || action === "down" || action === "left" || action === "right")) {
      moveShelf(action);
      return;
    }
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
        if (index - columns < 0) toHero();
        else move(index - columns);
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
        } else if (guide) {
          sfx.confirm();
          onImport();
        }
        break;
      case "y":
        if (focused) {
          sfx.confirm();
          actions.toggleFavorite(focused.id);
        } else if (guide && freeGames > 0) {
          sfx.confirm();
          onStore();
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
      case "select":
        surprise();
        break;
    }
  }, Layer.screen, !keyboard);

  // Grids drop their focus ring while the card has focus; the gallery,
  // coverflow and list layouts always preview a game, so they keep theirs.
  const shownIndex = onHero ? -1 : index;
  const lit = onHero ? heroGame : focused;
  const accent = (lit && systemOf(lit.systemId)?.accent) || "var(--brand)";
  const art = lit ? boxArtUrl(lit) : null;
  const root = overview?.settings.romRoot ?? "";

  return (
    <div className={`library${heroGame && !guide ? " has-continue" : ""}`} style={{ "--accent": accent } as CSSProperties}>
      <Ambient art={art} />

      <header className="topbar">
        <div className="brand">
          <BrandMark className="brand-mark" />
          Lumen
        </div>
        {kids?.active && <KidsTimer minutes={kids.dailyMinutes} played={allGames} />}
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
              className={`tab${i === tabIndex ? " active" : ""}${t.empty ? " tab-empty" : ""}${t.firstEmpty ? " tab-empty-first" : ""}`}
              onClick={() => switchTab(i - tabIndex)}
            >
              {t.label}
              <span className="count">{t.empty ? "+" : t.games.length}</span>
            </button>
          ))}
        </div>
        <span className="glyph pill" hidden={searching}>RB</span>
      </nav>

      {heroGame && !guide && (
        <ContinueCard
          game={heroGame}
          focused={onHero}
          onPlay={() => (onHero ? continueHero() : setHeroFocus(true))}
        />
      )}

      {guide ? (
        <div className="empty">
          <SystemGuide systemId={guide} onImport={onImport} onStore={onStore} />
        </div>
      ) : list.length === 0 ? (
        <div className="empty">
          <div className="empty-inner">
            <h2>
              {searching
                ? L("Sonuç yok", "No results")
                : scanning
                  ? L("Oyunlar aranıyor…", "Looking for games…")
                  : L("Henüz oyun yok", "No games yet")}
            </h2>
            {searching ? (
              <p>{L(<>“{query.trim()}” ile eşleşen oyun bulunamadı.</>, <>No games match “{query.trim()}”.</>)}</p>
            ) : (
              <>
            <p>
              {L(
                <>
                  Oyun dosyalarını oyun klasöründeki sistem klasörlerine bırak, örneğin <b>ps1</b>, <b>snes</b> veya{" "}
                  <b>gba</b>. Lumen her açılışta klasörü tarar; hemen görmek için menüden kütüphaneyi yenile. Menüdeki
                  "Oyunlarını içe aktar" da dosyaları senin yerine doğru klasöre koyar.
                </>,
                <>
                  Drop game files into the system folders inside your game folder, for example <b>ps1</b>,{" "}
                  <b>snes</b> or <b>gba</b>. Lumen scans the folder every time it opens; to see them right away,
                  refresh the library from the menu. "Import your games" in the menu also puts files in the right
                  folder for you.
                </>,
              )}
            </p>
            {root && <div className="path-chip">{root}</div>}
              </>
            )}
          </div>
        </div>
      ) : layout === "rail" ? (
        <GalleryView list={list} index={index} railRef={gridRef} onPick={(i) => (i === index ? onOpen(list[i]) : move(i))} />
      ) : layout === "shelves" && shelves ? (
        <ShelvesView shelves={shelves} index={shownIndex} scrollRef={gridRef} onPick={(i) => (i === index ? onOpen(list[i]) : move(i))} />
      ) : layout === "coverflow" ? (
        <CoverflowView list={list} index={index} onPick={(i) => (i === index ? onOpen(list[i]) : move(i))} />
      ) : layout === "list" ? (
        <PlainView list={list} index={index} listRef={gridRef} onPick={(i) => (i === index ? onOpen(list[i]) : move(i))} />
      ) : (
        <div className="grid-scroll">
          <div className={`grid${layout === "cartridge" ? " cartridge-grid" : ""}`} ref={gridRef}>
            {list.map((game, i) =>
              layout === "cartridge" ? (
                <CartridgeCard
                  key={game.id}
                  game={game}
                  focused={i === shownIndex}
                  onClick={() => (i === index ? onOpen(game) : move(i))}
                />
              ) : (
                <GameCard
                  key={game.id}
                  game={game}
                  focused={i === shownIndex}
                  showSystem={searching || tab.id === "all" || tab.id === "recent" || tab.id === "fav"}
                  onClick={() => (i === index ? onOpen(game) : move(i))}
                />
              ),
            )}
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
          hints={guide ? [
            { action: "a", label: L("İçe aktar", "Import") },
            ...(freeGames > 0 ? [{ action: "y" as const, label: L("Ücretsiz oyunlar", "Free games") }] : []),
            { action: "x", label: "Ara" },
            { action: "start", label: L("Menü", "Menu") },
          ] : onHero ? [
            { action: "a", label: L("Devam et", "Resume") },
            { action: "y", label: L("Ayrıntılar", "Details") },
            { action: "down", label: L("Kütüphane", "Library") },
            { action: "start", label: L("Menü", "Menu") },
          ] : [
            { action: "a", label: L("Aç", "Open") },
            { action: "y", label: focused?.favorite ? L("Favoriden çıkar", "Unfavorite") : L("Favori", "Favorite") },
            { action: "x", label: "Ara" },
            ...(canSurprise ? [{ action: "select" as const, label: L("Rastgele", "Random") }] : []),
            ...(searching ? [{ action: "b" as const, label: L("Aramayı temizle", "Clear search") }] : []),
            { action: "start", label: L("Menü", "Menu") },
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
      className={`card${focused ? " focused" : ""}${game.notInstalled ? " not-installed" : ""}`}
      style={{ "--card-accent": system?.accent } as CSSProperties}
      onClick={onClick}
      tabIndex={-1}
    >
      <div className="card-art">
        <Cover game={game} />
        {game.favorite && <span className="fav-badge">★</span>}
        {game.notInstalled && <span className="install-badge">{L("Yüklü değil", "Not installed")}</span>}
      </div>
      <span className="card-title">{game.title}</span>
      {showSystem && <span className="card-sub">{system?.short}</span>}
    </button>
  );
}

/** Blurred box art of the focused game, cross-faded. */
/** Kids mode's chip in the top bar: time left today, or just the mode. */
function KidsTimer({ minutes, played }: { minutes: number | null; played: Game[] }) {
  const [used, setUsed] = useState<number | null>(null);
  // Re-read when a session ends (last played changes).
  const latest = played.reduce((max, g) => Math.max(max, g.lastPlayed ?? 0), 0);
  useEffect(() => {
    if (!minutes) return;
    api.playedSince(todayStart()).then(setUsed).catch(() => setUsed(null));
  }, [minutes, latest]);
  const left = minutes && used !== null ? Math.max(0, minutes - Math.floor(used / 60)) : null;
  return (
    <span className={`kids-chip${left === 0 ? " over" : ""}`}>
      {L("Çocuk modu", "Kids mode")}
      {left !== null &&
        (left > 0
          ? L(` · ${left} dk kaldı`, ` · ${left} min left`)
          : L(" · bugünlük süre doldu", " · time's up for today"))}
    </span>
  );
}

/** The last game played, one press away (see `heroGame`). */
function ContinueCard({ game, focused, onPlay }: { game: Game; focused: boolean; onPlay: () => void }) {
  const system = systemOf(game.systemId);
  return (
    <button
      className={`continue-card${focused ? " focused" : ""}`}
      style={{ "--card-accent": system?.accent } as CSSProperties}
      onClick={onPlay}
      tabIndex={-1}
    >
      <span className="continue-art">
        <Cover game={game} />
      </span>
      <span className="continue-text">
        <span className="continue-eyebrow">{L("Kaldığın yerden devam et", "Pick up where you left off")}</span>
        <span className="continue-title">{game.title}</span>
        <span className="continue-meta">
          <span lang="en">{system?.short}</span> · {formatLastPlayed(game.lastPlayed)}
          {game.playSeconds > 0 && <> · {formatPlaytime(game.playSeconds)}</>}
        </span>
      </span>
      <span className="continue-play" aria-hidden>
        ▶
      </span>
    </button>
  );
}

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

/** Galeri: the focused game fills the screen; covers ride along the bottom. */
function GalleryView({
  list,
  index,
  railRef,
  onPick,
}: {
  list: Game[];
  index: number;
  railRef: React.RefObject<HTMLDivElement | null>;
  onPick: (i: number) => void;
}) {
  const game = list[index];
  const system = game && systemOf(game.systemId);
  // Gameplay shot reads best full screen; fall back to the box art.
  const hero = game ? (screenshotUrls(game)[1] ?? boxArtUrl(game)) : null;

  return (
    <div className="gallery">
      {hero && <img key={hero} className="gallery-hero" src={hero} alt="" draggable={false} />}
      {game && (
        <div className="gallery-copy" key={game.id}>
          <span className="gallery-system" lang="en">
            {system?.name}
            {system?.year ? ` · ${system.year}` : ""}
          </span>
          <h1>{game.title}</h1>
          <p>
            {game.lastPlayed
              ? L(
                  `Son oynanma ${formatLastPlayed(game.lastPlayed).toLowerCase()} · ${formatPlaytime(game.playSeconds)}`,
                  `Last played ${formatLastPlayed(game.lastPlayed).replace(/^(Today|Yesterday)$/, (d) => d.toLowerCase())} · ${formatPlaytime(game.playSeconds)}`,
                )
              : L("Henüz oynanmadı", "Not played yet")}
            {game.favorite ? L(" · ★ Favori", " · ★ Favorite") : ""}
          </p>
        </div>
      )}
      <div className="gallery-rail" ref={railRef}>
        {list.map((g, i) => (
          <button key={g.id} className={`rail-card${i === index ? " focused" : ""}`} onClick={() => onPick(i)} tabIndex={-1}>
            <Cover game={g} />
          </button>
        ))}
      </div>
    </div>
  );
}

/** Sade: a fast text list with a large preview of the focused game. */
function PlainView({
  list,
  index,
  listRef,
  onPick,
}: {
  list: Game[];
  index: number;
  listRef: React.RefObject<HTMLDivElement | null>;
  onPick: (i: number) => void;
}) {
  const game = list[index];
  const system = game && systemOf(game.systemId);
  return (
    <div className="plain">
      <div className="plain-list" ref={listRef}>
        {list.map((g, i) => (
          <button key={g.id} className={`plain-row${i === index ? " focused" : ""}`} onClick={() => onPick(i)} tabIndex={-1}>
            <span className="plain-title">{g.title}</span>
            {g.favorite && <span className="plain-fav">★</span>}
            <span className="plain-system">{systemOf(g.systemId)?.short}</span>
          </button>
        ))}
      </div>
      {game && (
        <aside className="plain-preview" key={game.id}>
          <div className="plain-art">
            <Cover game={game} />
          </div>
          <h2>{game.title}</h2>
          <dl>
            <dt>{L("Sistem", "System")}</dt>
            <dd lang="en">{system?.name}</dd>
            <dt>{L("Son oynanma", "Last played")}</dt>
            <dd>{formatLastPlayed(game.lastPlayed)}</dd>
            <dt>{L("Toplam süre", "Total time")}</dt>
            <dd>{formatPlaytime(game.playSeconds)}</dd>
          </dl>
        </aside>
      )}
    </div>
  );
}

/** Kartuş: each game is a cartridge with its art on the label. */
function CartridgeCard({ game, focused, onClick }: { game: Game; focused: boolean; onClick: () => void }) {
  const system = systemOf(game.systemId);
  return (
    <button
      className={`cartridge${focused ? " focused" : ""}`}
      style={{ "--card-accent": system?.accent } as CSSProperties}
      onClick={onClick}
      tabIndex={-1}
    >
      <div className="cartridge-shell">
        <div className="cartridge-label">
          <Cover game={game} />
        </div>
        <div className="cartridge-name">{game.title}</div>
        <div className="cartridge-system">{system?.short}</div>
      </div>
    </button>
  );
}

interface Shelf {
  id: string;
  label: string;
  games: Game[];
  /** Index of the shelf's first game in the flat list. */
  start: number;
}

function groupShelves(games: Game[]): Shelf[] {
  const shelves: Shelf[] = [];
  let start = 0;
  for (const system of systems) {
    const row = games.filter((g) => g.systemId === system.id);
    if (!row.length) continue;
    shelves.push({ id: system.id, label: system.name, games: row, start });
    start += row.length;
  }
  return shelves;
}

/** Raf: a shelf per system, scrolling sideways; shelves stack vertically. */
function ShelvesView({
  shelves,
  index,
  scrollRef,
  onPick,
}: {
  shelves: Shelf[];
  index: number;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  onPick: (i: number) => void;
}) {
  return (
    <div className="shelves" ref={scrollRef}>
      {shelves.map((shelf) => {
        const active = index >= shelf.start && index < shelf.start + shelf.games.length;
        return (
          <section
            key={shelf.id}
            className={`shelf${active ? " active" : ""}`}
            style={{ "--card-accent": systemOf(shelf.id)?.accent } as CSSProperties}
          >
            <h3 lang="en">
              {shelf.label}
              <span>{shelf.games.length}</span>
            </h3>
            <div className="shelf-row">
              {shelf.games.map((g, i) => {
                const flat = shelf.start + i;
                return (
                  <button
                    key={g.id}
                    data-index={flat}
                    className={`shelf-item${flat === index ? " focused" : ""}`}
                    onClick={() => onPick(flat)}
                    tabIndex={-1}
                  >
                    <div className="shelf-case">
                      <Cover game={g} />
                    </div>
                    <span className="shelf-title">{g.title}</span>
                  </button>
                );
              })}
            </div>
          </section>
        );
      })}
    </div>
  );
}

/** Neon: 3D cover flow; only the covers around the focus are drawn. */
function CoverflowView({ list, index, onPick }: { list: Game[]; index: number; onPick: (i: number) => void }) {
  const game = list[index];
  const system = game && systemOf(game.systemId);
  const WINDOW = 6;
  const from = Math.max(0, index - WINDOW);
  const visible = list.slice(from, Math.min(list.length, index + WINDOW + 1));

  return (
    <div className="flow" style={{ "--card-accent": system?.accent } as CSSProperties}>
      <div className="flow-floor" />
      <div className="flow-stage">
        {visible.map((g, k) => {
          const i = from + k;
          const offset = i - index;
          const side = Math.sign(offset);
          const distance = Math.abs(offset);
          const style = {
            transform:
              offset === 0
                ? "translateX(-50%) translateZ(160px)"
                : `translateX(calc(-50% + ${side * (11 + (distance - 1) * 5.2)}rem)) rotateY(${-side * 58}deg)`,
            zIndex: 100 - distance,
            opacity: distance > WINDOW - 1 ? 0 : 1,
          } as CSSProperties;
          return (
            <button key={g.id} className={`flow-card${offset === 0 ? " focused" : ""}`} style={style} onClick={() => onPick(i)} tabIndex={-1}>
              <Cover game={g} />
            </button>
          );
        })}
      </div>
      {game && (
        <div className="flow-caption" key={game.id}>
          <span lang="en">{system?.name}</span>
          <h1>{game.title}</h1>
        </div>
      )}
    </div>
  );
}
