import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { HintBar } from "../components/HintBar";
import { ProgressBar } from "../components/Toasts";
import { api } from "../lib/api";
import { systemOf } from "../lib/catalog";
import { formatBytes } from "../lib/format";
import { Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, notify, useStore } from "../lib/store";
import type { ImportCandidate, ImportMode, ImportReport } from "../lib/types";
import { L } from "../lib/i18n";

/** How to dump games you own, per kind of media. */
const GUIDE: { systems: string; how: string; result: string }[] = [
  {
    systems: "NES · SNES · N64 · Game Boy / Color / Advance · Master System · Mega Drive",
    how: L(
      "USB kartuş okuyucu: Sanni Cart Reader (açık kaynak), INLretro, Retrode 2 veya GB Operator.",
      "USB cartridge reader: Sanni Cart Reader (open source), INLretro, Retrode 2 or GB Operator.",
    ),
    result: ".nes .sfc .z64 .gb .gbc .gba .sms .md",
  },
  {
    systems: "PlayStation · Saturn",
    how: L(
      "Bilgisayarın CD/DVD sürücüsü ve redumper ya da ImgBurn. İstersen chdman ile .chd'ye sıkıştır.",
      "Your PC's CD/DVD drive and redumper or ImgBurn. Optionally compress to .chd with chdman.",
    ),
    result: L(".cue + .bin veya .chd", ".cue + .bin or .chd"),
  },
  {
    systems: "PlayStation 2",
    how: L(
      "DVD sürücüsü ve ImgBurn (CD'de gelen PS2 oyunları .cue/.bin olarak çıkar).",
      "A DVD drive and ImgBurn (PS2 games on CD come out as .cue/.bin).",
    ),
    result: ".iso",
  },
  {
    systems: "Dreamcast",
    how: L(
      "GD-ROM bilgisayarda okunmaz: SD kart adaptörlü Dreamcast'te DreamShell'in GD Ripper modülü.",
      "PCs can't read GD-ROMs: use DreamShell's GD Ripper module on a Dreamcast with an SD card adapter.",
    ),
    result: L(".gdi + track dosyaları", ".gdi + track files"),
  },
  {
    systems: "GameCube · Wii",
    how: L(
      "Homebrew Channel kurulu Wii'de CleanRip; Dolphin ile .rvz'ye sıkıştırabilirsin.",
      "CleanRip on a Wii with the Homebrew Channel; you can compress to .rvz with Dolphin.",
    ),
    result: L(".iso veya .rvz", ".iso or .rvz"),
  },
  {
    systems: "Wii U",
    how: L(
      "Aroma kurulu Wii U'da dumpling; Cemu'nun başlık yöneticisiyle tek dosya .wua'ya dönüştür.",
      "dumpling on a Wii U running Aroma; convert to a single .wua file with Cemu's Title Manager.",
    ),
    result: ".wua",
  },
  {
    systems: "Nintendo DS · 3DS",
    how: L("Luma3DS kurulu 3DS'te GodMode9 ile kartuşu kopyala.", "Copy the cartridge with GodMode9 on a 3DS running Luma3DS."),
    result: L(".nds veya .3ds", ".nds or .3ds"),
  },
  {
    systems: "PSP · PS Vita",
    how: L(
      "Özel yazılımlı konsolu USB ile bağla; PSP'de UMD'yi .iso olarak, Vita'da VitaShell ile oyun klasörünü kopyala.",
      "Connect the custom-firmware console over USB; on PSP copy the UMD as an .iso, on Vita copy the game folder with VitaShell.",
    ),
    result: L(".iso / .cso · Vita: klasör veya .vpk", ".iso / .cso · Vita: folder or .vpk"),
  },
  {
    systems: "PlayStation 3",
    how: L("Uyumlu bir Blu-ray sürücüsü ve PS3 Disc Dumper.", "A compatible Blu-ray drive and PS3 Disc Dumper."),
    result: L("PS3_GAME içeren klasör", "Folder containing PS3_GAME"),
  },
  {
    systems: "PlayStation 4 · PlayStation 5",
    how: L("Homebrew çalıştırabilen kendi konsolundan oyunun klasör dökümü.", "A folder dump of the game from your own homebrew-capable console."),
    result: L("sce_sys ve eboot.bin içeren klasör", "Folder containing sce_sys and eboot.bin"),
  },
  {
    systems: "Xbox · Xbox 360",
    how: L(
      "Uyumlu (Kreon yazılımlı) DVD sürücüsü; Xbox imajları extract-xiso ile küçültülebilir.",
      "A compatible DVD drive (Kreon firmware); Xbox images can be shrunk with extract-xiso.",
    ),
    result: ".iso / .xiso",
  },
  {
    systems: "Arcade",
    how: L(
      "Kendi oyun kartındaki ROM çiplerini EPROM okuyucuyla oku, FinalBurn Neo set adıyla zip'le.",
      "Read the ROM chips on your own game board with an EPROM reader and zip them under the FinalBurn Neo set name.",
    ),
    result: L("<set adı>.zip", "<set name>.zip"),
  },
];

const INTRO = [
  {
    title: L("Oyun dosyası seç", "Choose game files"),
    sub: L(
      "Seçtiğin dosyalar oyun klasöründe doğru sistemin klasörüne taşınır",
      "The files you pick are moved into the right system's folder in your games folder",
    ),
  },
  { title: L("Klasör seç", "Choose a folder"), sub: L("Seçtiğin klasör ve alt klasörleri taranır", "The folder you pick and its subfolders are scanned") },
  { title: L("Nasıl döküm alınır?", "How to dump games"), sub: L("Kartuş ve disklerini bilgisayara aktarma rehberi", "A guide to copying your cartridges and discs to your PC") },
];

type Phase = "intro" | "guide" | "scanning" | "review" | "importing" | "done";

interface Row extends ImportCandidate {
  /** Chosen system; null skips the game. */
  pick: string | null;
}

/**
 * Brings the user's own dumps from any folder into the ROM folder. Systems are
 * detected by Lumen; undetected ones are picked with left/right.
 */
export function ImportGames({ onClose }: { onClose: () => void }) {
  const installs = useStore((s) => s.installs);
  const [phase, setPhase] = useState<Phase>("intro");
  const [source, setSource] = useState<string | null>(null);
  const [rows, setRows] = useState<Row[]>([]);
  const [focus, setFocus] = useState(0);
  const [mode, setMode] = useState<ImportMode>("copy");
  const [report, setReport] = useState<ImportReport | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const chosen = useMemo(() => rows.filter((r) => r.pick), [rows]);
  const chosenBytes = chosen.reduce((sum, r) => sum + r.size, 0);
  const listLength = phase === "guide" ? GUIDE.length : phase === "intro" ? INTRO.length : rows.length;
  const index = Math.min(focus, Math.max(0, listLength - 1));

  useEffect(() => {
    listRef.current?.children[index]?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [index, phase]);

  /** Scans what the user picked; picked files are moved by default. */
  const scan = async (paths: string[], label: string, defaultMode: ImportMode) => {
    setSource(label);
    setMode(defaultMode);
    setPhase("scanning");
    try {
      const found = await api.importScan(paths);
      setRows(found.map((c) => ({ ...c, pick: c.systemId })));
      setFocus(0);
      setPhase("review");
    } catch (e) {
      notify(String(e), "error", 6000);
      setPhase("intro");
    }
  };

  const pickFiles = async () => {
    const paths = await api.pickGameFiles(L("Oyun klasörüne taşınacak oyun dosyalarını seç", "Choose game files to move into the games folder"));
    if (!paths.length) return;
    const label = paths.length === 1 ? paths[0] : L(`${paths.length} dosya seçildi`, `${paths.length} files selected`);
    await scan(paths, label, "move");
  };

  const pickFolder = async () => {
    const path = await api.pickFolder(L("İçe aktarılacak oyunların klasörünü seç", "Choose the folder of games to import"));
    if (path) await scan([path], path, "copy");
  };

  const openIntro = (i: number) => {
    if (i === 0) pickFiles();
    else if (i === 1) pickFolder();
    else {
      setPhase("guide");
      setFocus(0);
    }
  };

  const start = async () => {
    if (!chosen.length) {
      notify(L("İçe aktarılacak oyun seçilmedi", "No games selected to import"), "info");
      return;
    }
    sfx.confirm();
    setPhase("importing");
    const result = await actions.importGames(
      chosen.map((r) => ({ path: r.path, systemId: r.pick! })),
      mode,
    );
    if (result) {
      setReport(result);
      setPhase("done");
    } else {
      setPhase("review");
    }
  };

  /** Steps the row's system through its choices, with "skip" at the end. */
  const cycle = (step: 1 | -1) => {
    const row = rows[index];
    if (!row) return;
    const options: (string | null)[] = [...row.choices, null];
    const next = options[(options.indexOf(row.pick) + step + options.length) % options.length];
    sfx.move();
    setRows((all) => all.map((r, i) => (i === index ? { ...r, pick: next } : r)));
  };

  const toggle = () => {
    const row = rows[index];
    if (!row) return;
    sfx.confirm();
    const pick = row.pick ? null : (row.systemId ?? row.choices[0] ?? null);
    setRows((all) => all.map((r, i) => (i === index ? { ...r, pick } : r)));
  };

  const move = (next: number) => {
    if (next === index || next < 0 || next >= listLength) return;
    sfx.move();
    setFocus(next);
  };

  useInput((action) => {
    if (phase === "scanning" || phase === "importing") return;
    switch (action) {
      case "up":
      case "down":
        move(index + (action === "up" ? -1 : 1));
        break;
      case "left":
      case "right":
        if (phase === "review") cycle(action === "right" ? 1 : -1);
        break;
      case "a":
        if (phase === "intro") {
          sfx.confirm();
          openIntro(index);
        } else if (phase === "review") toggle();
        else if (phase === "done") onClose();
        break;
      case "x":
        if (phase === "review") {
          sfx.tab();
          setMode((m) => (m === "copy" ? "move" : "copy"));
        }
        break;
      case "y":
        if (phase === "review") start();
        break;
      case "b":
        sfx.back();
        if (phase === "guide" || phase === "review") {
          setPhase("intro");
          setFocus(phase === "guide" ? INTRO.length - 1 : 0);
        } else onClose();
        break;
    }
  }, Layer.sheet);

  const job = installs["import"];
  const row = phase === "review" ? rows[index] : undefined;
  const accent = row?.pick ? systemOf(row.pick)?.accent : undefined;

  return (
    <div className="store import" style={{ "--accent": accent } as CSSProperties}>
      <header className="store-head">
        <div>
          <h2>{phase === "guide"
              ? L("Oyunlarının dökümünü alma", "Dumping your games")
              : L("Oyunlarını içe aktar", "Import your games")}</h2>
          <p>
            {phase === "guide"
              ? L(
                  "Sadece sahip olduğun oyunların kopyasını al. Lumen oyun indirmez.",
                  "Only copy games you own. Lumen doesn't download games.",
                )
              : source && phase !== "intro"
                ? source
                : L(
                    "USB bellek, harici disk veya İndirilenler: Lumen her oyunun sistemini bulur ve doğru klasöre koyar.",
                    "USB stick, external drive or Downloads: Lumen detects each game's system and puts it in the right folder.",
                  )}
          </p>
        </div>
      </header>

      <div className="store-body">
        <div className="store-grid-scroll">
          {phase === "intro" && (
            <div className="menu-list import-list" ref={listRef}>
              {INTRO.map((item, i) => (
                <button
                  key={item.title}
                  className={`menu-item${i === index ? " focused" : ""}`}
                  onClick={() => openIntro(i)}
                  tabIndex={-1}
                >
                  <div className="menu-item-main">
                    <div className="menu-item-title">{item.title}</div>
                    <div className="menu-item-sub">{item.sub}</div>
                  </div>
                  <span className="menu-item-value">›</span>
                </button>
              ))}
            </div>
          )}

          {phase === "guide" && (
            <div className="menu-list import-list" ref={listRef}>
              {GUIDE.map((g, i) => (
                <div key={g.systems} className={`menu-item guide-item${i === index ? " focused" : ""}`}>
                  <div className="menu-item-main">
                    <div className="menu-item-title">{g.systems}</div>
                    <div className="guide-how">{g.how}</div>
                  </div>
                  <span className="pill">{g.result}</span>
                </div>
              ))}
            </div>
          )}

          {phase === "scanning" && <p className="import-status">{L("Oyunlar taranıyor…", "Scanning for games…")}</p>}

          {(phase === "review" || phase === "importing" || phase === "done") && rows.length === 0 && (
            <p className="import-status">
              {L(
                "Seçtiğin yerde tanınan bir oyun dosyası bulunamadı.",
                "No recognized game files were found there.",
              )}
            </p>
          )}

          {(phase === "review" || phase === "importing") && rows.length > 0 && (
            <div className="menu-list import-list" ref={listRef}>
              {rows.map((r, i) => {
                const system = r.pick ? systemOf(r.pick) : undefined;
                return (
                  <button
                    key={r.path}
                    className={`menu-item import-row${i === index ? " focused" : ""}${r.pick ? "" : " skipped"}`}
                    style={{ "--card-accent": system?.accent } as CSSProperties}
                    onClick={() => (i === index ? toggle() : setFocus(i))}
                    tabIndex={-1}
                  >
                    <span className="import-check">{r.pick ? "✓" : ""}</span>
                    <div className="menu-item-main">
                      <div className="menu-item-title">{r.name}</div>
                      <div className="menu-item-sub">
                        {formatBytes(r.size)}
                        {!r.systemId && L(" · sistem bulunamadı, ◀▶ ile seç", " · system not detected, pick with ◀▶")}
                      </div>
                    </div>
                    <span className="pill import-system">{system ? system.short : L("Atla", "Skip")}</span>
                  </button>
                );
              })}
            </div>
          )}

          {phase === "done" && report && (
            <div className="import-done">
              <h3>
                {L(
                  `${report.imported} oyun kütüphaneye eklendi`,
                  `${report.imported} ${report.imported === 1 ? "game" : "games"} added to the library`,
                )}
              </h3>
              {report.skipped > 0 && <p>
                  {L(
                    `${report.skipped} oyun zaten oyun klasöründeydi, atlandı.`,
                    `${report.skipped} ${report.skipped === 1 ? "game was" : "games were"} already in the games folder and skipped.`,
                  )}
                </p>}
              {report.failed.length > 0 && (
                <>
                  <p>
                    {L(
                      `${report.failed.length} oyun aktarılamadı:`,
                      `${report.failed.length} ${report.failed.length === 1 ? "game" : "games"} couldn't be imported:`,
                    )}
                  </p>
                  <ul>
                    {report.failed.map((f) => (
                      <li key={f}>{f}</li>
                    ))}
                  </ul>
                </>
              )}
            </div>
          )}
        </div>

        <aside className="store-detail">
          {phase === "review" || phase === "importing" ? (
            <>
              <span className="sheet-system">
                <span className="dot" />
                {L(`${chosen.length} / ${rows.length} oyun seçili`, `${chosen.length} / ${rows.length} games selected`)}
              </span>
              <h3>{formatBytes(chosenBytes)}</h3>
              <div className="facts">
                <span>{mode === "copy"
                    ? L("Kopyala: kaynak dosyalar kalır", "Copy: source files are kept")
                    : L("Taşı: kaynak dosyalar silinir", "Move: source files are deleted")}</span>
              </div>
              {phase === "importing" ? (
                <ProgressBar value={job?.progress ?? 0} indeterminate={!job || job.progress === 0} />
              ) : (
                <button className="btn primary focused" onClick={start} disabled={!chosen.length}>
                  {L("İçe aktar", "Import")}
                </button>
              )}
            </>
          ) : (
            <>
              <h3>{phase === "guide" ? L("Neden döküm?", "Why dump?") : L("Nasıl çalışır?", "How does it work?")}</h3>
              <p className="description">
                {phase === "guide"
                  ? L(
                      "Kendi kartuşlarını ve disklerini yedeklemek en temiz yol. Rehberdeki araçlar ücretsiz ve yaygın; çıkan dosyaları buradan içe aktarabilirsin.",
                      "Backing up your own cartridges and discs is the cleanest way. The tools in this guide are free and widely used; you can import the resulting files here.",
                    )
                  : L(
                      "Sistem; uzantıdan, ortak uzantılarda (iso, cue, zip, bin) dosyanın kendi başlığından bulunur. CUE izleri ve çok diskli oyunlar birlikte taşınır. PS3, PS4, PS5 ve Vita klasörleri bütün olarak aktarılır.",
                      "The system is detected from the file extension, or for shared extensions (iso, cue, zip, bin) from the file's own header. CUE tracks and multi-disc games are moved together. PS3, PS4, PS5 and Vita folders are imported whole.",
                    )}
              </p>
            </>
          )}
        </aside>
      </div>

      <HintBar
        left={phase === "review" ? `${index + 1} / ${rows.length}` : undefined}
        hints={
          phase === "review"
            ? [
                { action: "a", label: row?.pick ? L("Çıkar", "Remove") : L("Ekle", "Add") },
                { action: "left", label: L("Sistem", "System") },
                { action: "x", label: mode === "copy" ? L("Taşı", "Move") : L("Kopyala", "Copy") },
                { action: "y", label: L("İçe aktar", "Import") },
                { action: "b", label: L("Geri", "Back") },
              ]
            : phase === "done"
              ? [{ action: "a", label: L("Kapat", "Close") }]
              : phase === "intro"
                ? [
                    { action: "a", label: L("Seç", "Select") },
                    { action: "b", label: L("Kapat", "Close") },
                  ]
                : [{ action: "b", label: L("Geri", "Back") }]
        }
      />
    </div>
  );
}
