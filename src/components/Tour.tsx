import { useState, type ReactNode } from "react";
import { Glyph } from "./Glyph";
import { HintBar } from "./HintBar";
import type { Action } from "../lib/input";
import { useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { useStore } from "../lib/store";

interface Shortcut {
  keys: Action[];
  /** "+" for a combo pressed together, "/" for alternatives. */
  join?: "+" | "/";
  label: ReactNode;
}

interface Page {
  title: string;
  lead: ReactNode;
  shortcuts: Shortcut[];
}

/**
 * A short tour of where things are and the shortcuts that matter. Shown once
 * at the end of setup and from the menu any time.
 */
export function Tour({ layer, onDone, whatsNew = false }: { layer: number; onDone: () => void; whatsNew?: boolean }) {
  const root = useStore((s) => s.overview?.settings.romRoot);
  const backButtons = useStore((s) => !!s.overview?.device.model && !!s.overview?.settings.backButtons);
  const [page, setPage] = useState(0);

  const pages: Page[] = [
    {
      title: "Kütüphane",
      lead: "Tüm oyunların tek ekranda. Üstteki sekmeler sistemlere, favorilere ve kendi koleksiyonlarına ayırır.",
      shortcuts: [
        { keys: ["lb", "rb"], join: "/", label: "Sekme değiştir" },
        { keys: ["lt", "rt"], join: "/", label: "Harf harf atla (A, B, C…)" },
        { keys: ["a"], label: "Oyunu aç: oyna, devam et, koleksiyona ekle, güç modu (⚡), geçmiş (🕘)" },
        { keys: ["up"], label: <>En üstteki <b>Devam et</b> kartı: kaldığın oyuna tek tuşla dön</> },
        { keys: ["y"], label: "Favorilere ekle" },
        { keys: ["x"], label: "Ara" },
        { keys: ["select"], label: "Ne oynasam? Rastgele bir oyun aç" },
        { keys: ["start"], label: "Menü" },
      ],
    },
    {
      title: "Oyun oynarken",
      lead: "Oyun bıraktığın yerden devam eder; çıkarken kaydetmeyi unutmana gerek yok.",
      shortcuts: [
        { keys: [], label: <>Sağ üst köşedeki <b>☰</b> düğmesine dokun: hızlı menü (oyuna hiçbir tuş gitmez)</> },
        ...(backButtons
          ? ([
              { keys: [], label: <><b>M1</b> arka tuşuna dokun: hızlı menü (oyuna hiçbir tuş gitmez)</> },
              { keys: [], label: <><b>M1</b>'i basılı tut: <b>kaydet ve çık</b></> },
              { keys: [], label: <><b>M2</b> arka tuşu: geri sar (menüden hızlı ileri ya da ekran görüntüsü yapabilirsin)</> },
            ] satisfies Shortcut[])
          : ([
              { keys: ["select", "start"], join: "+", label: "Hızlı menü: oyun duraklar" },
              { keys: ["select", "start"], join: "+", label: <>Basılı tut: <b>kaydet ve çık</b></> },
            ] satisfies Shortcut[])),
        { keys: ["select", "lt"], join: "+", label: "Basılı tut: geri sar (RetroArch'ın hafif sistemleri, PS1)" },
        { keys: ["select", "rt"], join: "+", label: "Basılı tut: 3× hızlı" },
        { keys: ["select", "y"], join: "+", label: "Ekran görüntüsü (oyunun sayfasında galeride durur)" },
        { keys: [], label: <>Çok diskli oyunlarda hızlı menüden <b>Sonraki diske geç</b></> },
        { keys: ["left"], label: "Hızlı menüde kayıt slotu seç (RetroArch sistemleri)" },
        { keys: ["a", "x"], join: "/", label: "Slota kaydet / slottan yükle" },
        { keys: ["b"], label: "Oyuna dön" },
      ],
    },
    {
      title: "Oyun eklemek",
      lead: (
        <>
          Oyun dosyasını oyun klasöründeki sistem klasörüne bırak, örneğin <b>ps1</b> ya da <b>gba</b>. Lumen her
          açılışta klasörü tarar.
          {root && <span className="path-chip tour-path">{root}</span>}
        </>
      ),
      shortcuts: [
        { keys: ["start"], label: <>Menü → <b>Oyunlarını içe aktar</b>: bir oyun dosyası ya da klasör seç, Lumen doğru klasöre taşır</> },
        { keys: ["start"], label: <>Menü → <b>Ücretsiz oyunlar</b>: lisansı izin veren homebrew'lar ve klasikler</> },
        { keys: ["lb", "rb"], join: "/", label: "Boş bir sistem sekmesi o sisteme oyunun nasıl ekleneceğini anlatır" },
      ],
    },
    {
      title: "Menüde neler var",
      lead: "Emülatör ayarlarıyla hiç uğraşmazsın; Lumen her oyunu cihazına göre ayarlar. Değiştirmek isteyeceğin her şey menüde.",
      shortcuts: [
        { keys: ["start"], label: <><b>Desteklenen sistemler</b>: hangi konsollar var, hangisi hazır</> },
        { keys: ["start"], label: <><b>İstatistikler</b>: haftalık süre, en çok oynananlar</> },
        { keys: ["start"], label: <><b>Tema</b> ve <b>Görüntü filtresi</b> (CRT, LCD…)</> },
        { keys: ["start"], label: <><b>RetroAchievements</b> ve <b>Kayıt yedekleme</b> (bulut klasörü, cihazlar arası senkron)</> },
        { keys: ["start"], label: <><b>Kapak görselleri</b>: kapağı olmayan oyunlara SteamGridDB'den kapak</> },
        { keys: ["start"], label: <><b>Çocuk modu</b>: PIN'li sade kütüphane ve günlük süre sınırı</> },
        { keys: ["start"], label: <><b>Sorun bildir</b>: günlükler tek dosyada masaüstüne</> },
      ],
    },
  ];

  // After an update: only what's new, for people who saw the tour before.
  const newPages: Page[] = [
    {
      title: "Oyun oynarken",
      lead: "Oyna'ya basınca emülatör açılana kadar yükleme ekranı görünür. Oyunda yeni kısayollar var:",
      shortcuts: [
        ...(backButtons
          ? ([
              { keys: [], label: <><b>M1</b> arka tuşu: dokun menü, basılı tut kaydet ve çık. Oyuna hiçbir tuş gitmez</> },
              { keys: [], label: <><b>M2</b> arka tuşu: basılı tut geri sar (Menü → M2 tuşu ile değiştir)</> },
            ] satisfies Shortcut[])
          : []),
        { keys: ["select", "lt"], join: "+", label: "Basılı tut: geri sar" },
        { keys: ["select", "rt"], join: "+", label: "Basılı tut: 3× hızlı" },
        { keys: ["select", "y"], join: "+", label: "Ekran görüntüsü" },
        { keys: [], label: <>Hızlı menüde <b>Sonraki diske geç</b> (çok diskli oyunlar) ve <b>Ekran görüntüsü al</b></> },
        { keys: [], label: <>Sağ üst köşede pil ve saat; pil azalınca kırmızıya döner</> },
      ],
    },
    {
      title: "Kütüphane",
      lead: "Açılışta en üstte son oynadığın oyun durur.",
      shortcuts: [
        { keys: ["a"], label: <><b>Devam et</b> kartında: kaldığın yerden oyna</> },
        { keys: ["select"], label: "Ne oynasam? Rastgele bir oyun aç" },
        { keys: [], label: <>Oyun sayfasında <b>⚡</b> ile o oyunun güç modunu seç, <b>▣</b> ile ekran görüntülerine bak</> },
        { keys: [], label: <>Oyun sayfasında <b>🕘 Geçmiş</b>: her oturum, süresi ve o oturumda aldığın görüntüler</> },
        { keys: [], label: "Çok diskli oyunlar (Disc 1, Disc 2…) artık tek oyun olarak görünür" },
        { keys: [], label: "Steam, Xbox, Epic, PlayStation ve Nintendo sekmeleri en başta" },
      ],
    },
    {
      title: "Menüde yeni",
      lead: "Hepsi menüde, istediğin zaman açıp kapatabilirsin.",
      shortcuts: [
        { keys: ["start"], label: <><b>Oyunlarını içe aktar → Oyun dosyası seç</b>: dosyayı seç, doğru klasöre taşınsın</> },
        { keys: ["start"], label: <><b>Kayıt yedekleme</b>: bulut klasörü seçimi ve cihazlar arası senkron</> },
        { keys: ["start"], label: <><b>Kapak görselleri</b>: kapağı olmayan oyunlara SteamGridDB'den kapak</> },
        { keys: ["start"], label: <><b>Çocuk modu</b>: PIN'li sade kütüphane ve günlük süre sınırı</> },
        { keys: ["start"], label: <><b>Sorun bildir</b>: bir şey ters giderse günlükleri tek dosyada masaüstüne kaydeder</> },
        { keys: ["start"], label: <><b>Kısa tur</b>: tüm kısayollar bir arada</> },
      ],
    },
  ];
  if (whatsNew) pages.splice(0, pages.length, ...newPages);

  const last = page === pages.length - 1;
  const go = (delta: number) => {
    const next = page + delta;
    if (next < 0) return;
    if (next >= pages.length) {
      sfx.confirm();
      onDone();
      return;
    }
    sfx.move();
    setPage(next);
  };

  useInput((action) => {
    if (action === "a" || action === "right") go(1);
    else if (action === "left") go(-1);
    else if (action === "b") {
      sfx.back();
      onDone();
    }
  }, layer);

  const current = pages[page];
  return (
    <div className="tour">
      <div className="tour-card">
        <div className="step-count">
          {whatsNew ? "Yenilikler" : "Kısa tur"} · {page + 1} / {pages.length}
        </div>
        <h1>{current.title}</h1>
        <p className="lead">{current.lead}</p>
        <ul className="tour-keys">
          {current.shortcuts.map((s, i) => (
            <li key={i}>
              <span className="tour-combo">
                {s.keys.map((k, j) => (
                  <span key={j} className="tour-key">
                    {j > 0 && <span className="tour-join">{s.join ?? "+"}</span>}
                    <Glyph action={k} />
                  </span>
                ))}
              </span>
              <span>{s.label}</span>
            </li>
          ))}
        </ul>
        <div className="tour-dots">
          {pages.map((p, i) => (
            <span key={p.title} className={i === page ? "active" : ""} />
          ))}
        </div>
      </div>
      <HintBar
        hints={[
          ...(page > 0 ? [{ action: "left" as const, label: "Geri" }] : []),
          { action: "a", label: last ? "Başla" : "İleri" },
          { action: "b", label: whatsNew ? "Kapat" : "Turu kapat" },
        ]}
      />
    </div>
  );
}
