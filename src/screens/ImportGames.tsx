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

/** How to dump games you own, per kind of media. */
const GUIDE: { systems: string; how: string; result: string }[] = [
  {
    systems: "NES · SNES · N64 · Game Boy / Color / Advance · Master System · Mega Drive",
    how: "USB kartuş okuyucu: Sanni Cart Reader (açık kaynak), INLretro, Retrode 2 veya GB Operator.",
    result: ".nes .sfc .z64 .gb .gbc .gba .sms .md",
  },
  {
    systems: "PlayStation · Saturn",
    how: "Bilgisayarın CD/DVD sürücüsü ve redumper ya da ImgBurn. İstersen chdman ile .chd'ye sıkıştır.",
    result: ".cue + .bin veya .chd",
  },
  {
    systems: "PlayStation 2",
    how: "DVD sürücüsü ve ImgBurn (CD'de gelen PS2 oyunları .cue/.bin olarak çıkar).",
    result: ".iso",
  },
  {
    systems: "Dreamcast",
    how: "GD-ROM bilgisayarda okunmaz: SD kart adaptörlü Dreamcast'te DreamShell'in GD Ripper modülü.",
    result: ".gdi + track dosyaları",
  },
  {
    systems: "GameCube · Wii",
    how: "Homebrew Channel kurulu Wii'de CleanRip; Dolphin ile .rvz'ye sıkıştırabilirsin.",
    result: ".iso veya .rvz",
  },
  {
    systems: "Wii U",
    how: "Aroma kurulu Wii U'da dumpling; Cemu'nun başlık yöneticisiyle tek dosya .wua'ya dönüştür.",
    result: ".wua",
  },
  {
    systems: "Nintendo DS · 3DS",
    how: "Luma3DS kurulu 3DS'te GodMode9 ile kartuşu kopyala.",
    result: ".nds veya .3ds",
  },
  {
    systems: "PSP · PS Vita",
    how: "Özel yazılımlı konsolu USB ile bağla; PSP'de UMD'yi .iso olarak, Vita'da VitaShell ile oyun klasörünü kopyala.",
    result: ".iso / .cso · Vita: klasör veya .vpk",
  },
  {
    systems: "PlayStation 3",
    how: "Uyumlu bir Blu-ray sürücüsü ve PS3 Disc Dumper.",
    result: "PS3_GAME içeren klasör",
  },
  {
    systems: "PlayStation 4 · PlayStation 5",
    how: "Homebrew çalıştırabilen kendi konsolundan oyunun klasör dökümü.",
    result: "sce_sys ve eboot.bin içeren klasör",
  },
  {
    systems: "Xbox · Xbox 360",
    how: "Uyumlu (Kreon yazılımlı) DVD sürücüsü; Xbox imajları extract-xiso ile küçültülebilir.",
    result: ".iso / .xiso",
  },
  {
    systems: "Arcade",
    how: "Kendi oyun kartındaki ROM çiplerini EPROM okuyucuyla oku, FinalBurn Neo set adıyla zip'le.",
    result: "<set adı>.zip",
  },
];

const INTRO = [
  { title: "Oyun dosyası seç", sub: "Seçtiğin dosyalar oyun klasöründe doğru sistemin klasörüne taşınır" },
  { title: "Klasör seç", sub: "Seçtiğin klasör ve alt klasörleri taranır" },
  { title: "Nasıl döküm alınır?", sub: "Kartuş ve disklerini bilgisayara aktarma rehberi" },
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
    const paths = await api.pickGameFiles("Oyun klasörüne taşınacak oyun dosyalarını seç");
    if (!paths.length) return;
    const label = paths.length === 1 ? paths[0] : `${paths.length} dosya seçildi`;
    await scan(paths, label, "move");
  };

  const pickFolder = async () => {
    const path = await api.pickFolder("İçe aktarılacak oyunların klasörünü seç");
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
      notify("İçe aktarılacak oyun seçilmedi", "info");
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
          <h2>{phase === "guide" ? "Oyunlarının dökümünü alma" : "Oyunlarını içe aktar"}</h2>
          <p>
            {phase === "guide"
              ? "Sadece sahip olduğun oyunların kopyasını al. Lumen oyun indirmez."
              : source && phase !== "intro"
                ? source
                : "USB bellek, harici disk veya İndirilenler: Lumen her oyunun sistemini bulur ve doğru klasöre koyar."}
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

          {phase === "scanning" && <p className="import-status">Oyunlar taranıyor…</p>}

          {(phase === "review" || phase === "importing" || phase === "done") && rows.length === 0 && (
            <p className="import-status">Seçtiğin yerde tanınan bir oyun dosyası bulunamadı.</p>
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
                        {!r.systemId && " · sistem bulunamadı, ◀▶ ile seç"}
                      </div>
                    </div>
                    <span className="pill import-system">{system ? system.short : "Atla"}</span>
                  </button>
                );
              })}
            </div>
          )}

          {phase === "done" && report && (
            <div className="import-done">
              <h3>{report.imported} oyun kütüphaneye eklendi</h3>
              {report.skipped > 0 && <p>{report.skipped} oyun zaten oyun klasöründeydi, atlandı.</p>}
              {report.failed.length > 0 && (
                <>
                  <p>{report.failed.length} oyun aktarılamadı:</p>
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
                {chosen.length} / {rows.length} oyun seçili
              </span>
              <h3>{formatBytes(chosenBytes)}</h3>
              <div className="facts">
                <span>{mode === "copy" ? "Kopyala: kaynak dosyalar kalır" : "Taşı: kaynak dosyalar silinir"}</span>
              </div>
              {phase === "importing" ? (
                <ProgressBar value={job?.progress ?? 0} indeterminate={!job || job.progress === 0} />
              ) : (
                <button className="btn primary focused" onClick={start} disabled={!chosen.length}>
                  İçe aktar
                </button>
              )}
            </>
          ) : (
            <>
              <h3>{phase === "guide" ? "Neden döküm?" : "Nasıl çalışır?"}</h3>
              <p className="description">
                {phase === "guide"
                  ? "Kendi kartuşlarını ve disklerini yedeklemek en temiz yol. Rehberdeki araçlar ücretsiz ve yaygın; çıkan dosyaları buradan içe aktarabilirsin."
                  : "Sistem; uzantıdan, ortak uzantılarda (iso, cue, zip, bin) dosyanın kendi başlığından bulunur. CUE izleri ve çok diskli oyunlar birlikte taşınır. PS3, PS4, PS5 ve Vita klasörleri bütün olarak aktarılır."}
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
                { action: "a", label: row?.pick ? "Çıkar" : "Ekle" },
                { action: "left", label: "Sistem" },
                { action: "x", label: mode === "copy" ? "Taşı" : "Kopyala" },
                { action: "y", label: "İçe aktar" },
                { action: "b", label: "Geri" },
              ]
            : phase === "done"
              ? [{ action: "a", label: "Kapat" }]
              : phase === "intro"
                ? [
                    { action: "a", label: "Seç" },
                    { action: "b", label: "Kapat" },
                  ]
                : [{ action: "b", label: "Geri" }]
        }
      />
    </div>
  );
}
