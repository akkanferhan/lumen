import { useState, type ReactNode } from "react";
import { Glyph } from "./Glyph";
import { HintBar } from "./HintBar";
import type { Action } from "../lib/input";
import { useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { useStore } from "../lib/store";
import { L } from "../lib/i18n";

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
      title: L("Kütüphane", "Library"),
      lead: L(
        "Tüm oyunların tek ekranda. Üstteki sekmeler sistemlere, favorilere ve kendi koleksiyonlarına ayırır.",
        "All your games on one screen. The tabs up top sort them by system, favorites and your own collections.",
      ),
      shortcuts: [
        { keys: ["lb", "rb"], join: "/", label: L("Sekme değiştir", "Switch tab") },
        { keys: ["lt", "rt"], join: "/", label: L("Harf harf atla (A, B, C…)", "Jump by letter (A, B, C…)") },
        {
          keys: ["a"],
          label: L(
            "Oyunu aç: oyna, devam et, koleksiyona ekle, güç modu (⚡), geçmiş (🕘)",
            "Open a game: play, continue, add to collection, power mode (⚡), history (🕘)",
          ),
        },
        {
          keys: ["up"],
          label: L(
            <>En üstteki <b>Devam et</b> kartı: kaldığın oyuna tek tuşla dön</>,
            <>The <b>Continue</b> card at the top: back to your last game in one press</>,
          ),
        },
        { keys: ["y"], label: L("Favorilere ekle", "Add to favorites") },
        { keys: ["x"], label: L("Ara", "Search") },
        { keys: ["select"], label: L("Ne oynasam? Rastgele bir oyun aç", "What to play? Open a random game") },
        { keys: ["start"], label: L("Menü", "Menu") },
      ],
    },
    {
      title: L("Oyun oynarken", "While playing"),
      lead: L(
        "Oyun bıraktığın yerden devam eder; çıkarken kaydetmeyi unutmana gerek yok.",
        "Games pick up where you left off; no need to remember to save when you quit.",
      ),
      shortcuts: [
        {
          keys: [],
          label: L(
            <>Sağ üst köşedeki <b>☰</b> düğmesine dokun: hızlı menü (oyuna hiçbir tuş gitmez)</>,
            <>Tap the <b>☰</b> button in the top-right corner: quick menu (no input reaches the game)</>,
          ),
        },
        ...(backButtons
          ? ([
              {
                keys: [],
                label: L(
                  <><b>M1</b> arka tuşuna dokun: hızlı menü (oyuna hiçbir tuş gitmez)</>,
                  <>Tap the <b>M1</b> back button: quick menu (no input reaches the game)</>,
                ),
              },
              { keys: [], label: L(<><b>M1</b>'i basılı tut: <b>kaydet ve çık</b></>, <>Hold <b>M1</b>: <b>save and quit</b></>) },
              {
                keys: [],
                label: L(
                  <><b>M2</b> arka tuşu: geri sar (menüden hızlı ileri ya da ekran görüntüsü yapabilirsin)</>,
                  <><b>M2</b> back button: rewind (you can switch it to fast-forward or screenshot in the menu)</>,
                ),
              },
            ] satisfies Shortcut[])
          : ([
              { keys: ["select", "start"], join: "+", label: L("Hızlı menü: oyun duraklar", "Quick menu: pauses the game") },
              { keys: ["select", "start"], join: "+", label: L(<>Basılı tut: <b>kaydet ve çık</b></>, <>Hold: <b>save and quit</b></>) },
            ] satisfies Shortcut[])),
        { keys: ["select", "lt"], join: "+", label: L("Basılı tut: geri sar (RetroArch'ın hafif sistemleri, PS1)", "Hold: rewind (RetroArch's lighter systems, PS1)") },
        { keys: ["select", "rt"], join: "+", label: L("Basılı tut: 3× hızlı", "Hold: 3× fast-forward") },
        { keys: ["select", "y"], join: "+", label: L("Ekran görüntüsü (oyunun sayfasında galeride durur)", "Screenshot (kept in the gallery on the game's page)") },
        {
          keys: [],
          label: L(
            <>Çok diskli oyunlarda hızlı menüden <b>Sonraki diske geç</b></>,
            <>For multi-disc games, <b>Next disc</b> in the quick menu</>,
          ),
        },
        { keys: ["left"], label: L("Hızlı menüde kayıt slotu seç (RetroArch sistemleri)", "Pick a save slot in the quick menu (RetroArch systems)") },
        { keys: ["a", "x"], join: "/", label: L("Slota kaydet / slottan yükle", "Save to slot / load from slot") },
        { keys: ["b"], label: L("Oyuna dön", "Back to game") },
      ],
    },
    {
      title: L("Oyun eklemek", "Adding games"),
      lead: (
        <>
          {L(
            <>
              Oyun dosyasını oyun klasöründeki sistem klasörüne bırak, örneğin <b>ps1</b> ya da <b>gba</b>. Lumen her
              açılışta klasörü tarar.
            </>,
            <>
              Drop a game file into its system's folder inside the game folder, for example <b>ps1</b> or <b>gba</b>.
              Lumen scans the folder every time it starts.
            </>,
          )}
          {root && <span className="path-chip tour-path">{root}</span>}
        </>
      ),
      shortcuts: [
        {
          keys: ["start"],
          label: L(
            <>Menü → <b>Oyunlarını içe aktar</b>: bir oyun dosyası ya da klasör seç, Lumen doğru klasöre taşır</>,
            <>Menu → <b>Import your games</b>: pick a game file or folder and Lumen moves it to the right place</>,
          ),
        },
        {
          keys: ["start"],
          label: L(
            <>Menü → <b>Ücretsiz oyunlar</b>: lisansı izin veren homebrew'lar ve klasikler</>,
            <>Menu → <b>Free games</b>: homebrew and classics whose licenses allow it</>,
          ),
        },
        {
          keys: ["lb", "rb"], join: "/",
          label: L(
            "Boş bir sistem sekmesi o sisteme oyunun nasıl ekleneceğini anlatır",
            "An empty system tab explains how to add games for that system",
          ),
        },
      ],
    },
    {
      title: L("Menüde neler var", "What's in the menu"),
      lead: L(
        "Emülatör ayarlarıyla hiç uğraşmazsın; Lumen her oyunu cihazına göre ayarlar. Değiştirmek isteyeceğin her şey menüde.",
        "No emulator settings to fiddle with; Lumen tunes every game for your device. Anything you'd want to change is in the menu.",
      ),
      shortcuts: [
        {
          keys: ["start"],
          label: L(
            <><b>Desteklenen sistemler</b>: hangi konsollar var, hangisi hazır</>,
            <><b>Supported systems</b>: which consoles are there and which are ready</>,
          ),
        },
        {
          keys: ["start"],
          label: L(
            <><b>İstatistikler</b>: haftalık süre, en çok oynananlar</>,
            <><b>Stats</b>: weekly playtime, most played</>,
          ),
        },
        { keys: ["start"], label: L(<><b>Tema</b> ve <b>Görüntü filtresi</b> (CRT, LCD…)</>, <><b>Theme</b> and <b>Display filter</b> (CRT, LCD…)</>) },
        {
          keys: ["start"],
          label: L(
            <><b>RetroAchievements</b> ve <b>Kayıt yedekleme</b> (bulut klasörü, cihazlar arası senkron)</>,
            <><b>RetroAchievements</b> and <b>Save backup</b> (cloud folder, sync across devices)</>,
          ),
        },
        {
          keys: ["start"],
          label: L(
            <><b>Kapak görselleri</b>: kapağı olmayan oyunlara SteamGridDB'den kapak</>,
            <><b>Cover art</b>: covers from SteamGridDB for games without one</>,
          ),
        },
        {
          keys: ["start"],
          label: L(
            <><b>Çocuk modu</b>: PIN'li sade kütüphane ve günlük süre sınırı</>,
            <><b>Kids mode</b>: a simple PIN-locked library and a daily time limit</>,
          ),
        },
        {
          keys: ["start"],
          label: L(
            <><b>Sorun bildir</b>: günlükler tek dosyada masaüstüne</>,
            <><b>Report a problem</b>: logs in a single file on your desktop</>,
          ),
        },
        {
          keys: ["start"],
          label: L(
            <><b>Dil / Language</b>: Türkçe ya da İngilizce (otomatik: Windows'un dili)</>,
            <><b>Dil / Language</b>: Turkish or English (automatic: same as Windows)</>,
          ),
        },
      ],
    },
  ];

  // After an update: only what's new, for people who saw the tour before.
  const newPages: Page[] = [
    {
      title: L("Oyun oynarken", "While playing"),
      lead: L(
        "Oyna'ya basınca emülatör açılana kadar yükleme ekranı görünür. Oyunda yeni kısayollar var:",
        "After you press Play, a loading screen shows until the emulator opens. New in-game shortcuts:",
      ),
      shortcuts: [
        ...(backButtons
          ? ([
              {
                keys: [],
                label: L(
                  <><b>M1</b> arka tuşu: dokun menü, basılı tut kaydet ve çık. Oyuna hiçbir tuş gitmez</>,
                  <><b>M1</b> back button: tap for the menu, hold to save and quit. No input reaches the game</>,
                ),
              },
              {
                keys: [],
                label: L(
                  <><b>M2</b> arka tuşu: basılı tut geri sar (Menü → M2 tuşu ile değiştir)</>,
                  <><b>M2</b> back button: hold to rewind (change it in Menu → M2 button)</>,
                ),
              },
            ] satisfies Shortcut[])
          : []),
        { keys: ["select", "lt"], join: "+", label: L("Basılı tut: geri sar", "Hold: rewind") },
        { keys: ["select", "rt"], join: "+", label: L("Basılı tut: 3× hızlı", "Hold: 3× fast-forward") },
        { keys: ["select", "y"], join: "+", label: L("Ekran görüntüsü", "Screenshot") },
        {
          keys: [],
          label: L(
            <>Hızlı menüde <b>Sonraki diske geç</b> (çok diskli oyunlar) ve <b>Ekran görüntüsü al</b></>,
            <><b>Next disc</b> (multi-disc games) and <b>Take screenshot</b> in the quick menu</>,
          ),
        },
        {
          keys: [],
          label: L(
            <>Sağ üst köşede pil ve saat; pil azalınca kırmızıya döner</>,
            <>Battery and clock in the top-right corner; the battery turns red when low</>,
          ),
        },
      ],
    },
    {
      title: L("Kütüphane", "Library"),
      lead: L("Açılışta en üstte son oynadığın oyun durur.", "Your last played game now sits at the top when you start."),
      shortcuts: [
        { keys: ["a"], label: L(<><b>Devam et</b> kartında: kaldığın yerden oyna</>, <>On the <b>Continue</b> card: play from where you left off</>) },
        { keys: ["select"], label: L("Ne oynasam? Rastgele bir oyun aç", "What to play? Open a random game") },
        {
          keys: [],
          label: L(
            <>Oyun sayfasında <b>⚡</b> ile o oyunun güç modunu seç, <b>▣</b> ile ekran görüntülerine bak</>,
            <>On a game's page, <b>⚡</b> sets its power mode and <b>▣</b> shows its screenshots</>,
          ),
        },
        {
          keys: [],
          label: L(
            <>Oyun sayfasında <b>🕘 Geçmiş</b>: her oturum, süresi ve o oturumda aldığın görüntüler</>,
            <><b>🕘 History</b> on a game's page: every session, its length and the screenshots you took</>,
          ),
        },
        {
          keys: [],
          label: L(
            "Çok diskli oyunlar (Disc 1, Disc 2…) artık tek oyun olarak görünür",
            "Multi-disc games (Disc 1, Disc 2…) now show up as a single game",
          ),
        },
        {
          keys: [],
          label: L(
            "Steam, Xbox, Epic, PlayStation ve Nintendo sekmeleri en başta",
            "Steam, Xbox, Epic, PlayStation and Nintendo tabs come first",
          ),
        },
      ],
    },
    {
      title: L("Menüde yeni", "New in the menu"),
      lead: L("Hepsi menüde, istediğin zaman açıp kapatabilirsin.", "All in the menu; turn them on or off any time."),
      shortcuts: [
        {
          keys: ["start"],
          label: L(
            <><b>Oyunlarını içe aktar → Oyun dosyası seç</b>: dosyayı seç, doğru klasöre taşınsın</>,
            <><b>Import your games → Choose a game file</b>: pick the file and it moves to the right folder</>,
          ),
        },
        {
          keys: ["start"],
          label: L(
            <><b>Kayıt yedekleme</b>: bulut klasörü seçimi ve cihazlar arası senkron</>,
            <><b>Save backup</b>: choose a cloud folder and sync across devices</>,
          ),
        },
        {
          keys: ["start"],
          label: L(
            <><b>Kapak görselleri</b>: kapağı olmayan oyunlara SteamGridDB'den kapak</>,
            <><b>Cover art</b>: covers from SteamGridDB for games without one</>,
          ),
        },
        {
          keys: ["start"],
          label: L(
            <><b>Çocuk modu</b>: PIN'li sade kütüphane ve günlük süre sınırı</>,
            <><b>Kids mode</b>: a simple PIN-locked library and a daily time limit</>,
          ),
        },
        {
          keys: ["start"],
          label: L(
            <><b>Sorun bildir</b>: bir şey ters giderse günlükleri tek dosyada masaüstüne kaydeder</>,
            <><b>Report a problem</b>: if something goes wrong, saves the logs to one file on your desktop</>,
          ),
        },
        {
          keys: ["start"],
          label: L(
            <><b>Dil / Language</b>: Türkçe ya da İngilizce (otomatik: Windows'un dili)</>,
            <><b>Dil / Language</b>: Turkish or English (automatic: same as Windows)</>,
          ),
        },
        { keys: ["start"], label: L(<><b>Kısa tur</b>: tüm kısayollar bir arada</>, <><b>Quick tour</b>: all the shortcuts in one place</>) },
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
          {whatsNew ? L("Yenilikler", "What's new") : L("Kısa tur", "Quick tour")} · {page + 1} / {pages.length}
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
          ...(page > 0 ? [{ action: "left" as const, label: L("Geri", "Back") }] : []),
          { action: "a", label: last ? L("Başla", "Start") : L("İleri", "Next") },
          { action: "b", label: whatsNew ? L("Kapat", "Close") : L("Turu kapat", "Close tour") },
        ]}
      />
    </div>
  );
}
