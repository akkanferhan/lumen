import { useEffect, useRef, useState } from "react";
import { HintBar } from "../components/HintBar";
import { L, languageSetting, setLanguageSetting, type Language } from "../lib/i18n";
import { TextKeyboard } from "../components/TextKeyboard";
import { ProgressBar } from "../components/Toasts";
import { api, isTauri } from "../lib/api";
import { emulatorOf, emulators, installLabel, isPcSystem, systems } from "../lib/catalog";
import { EXPERIMENTAL } from "../components/SystemGuide";
import { Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, notify, useStore } from "../lib/store";
import type { CloudFolder, FilterSetting, M2Action, GraphicsSetting, PowerSetting, Tier } from "../lib/types";
import { formatLastPlayed } from "../lib/format";
import { THEMES, themeOf } from "../lib/themes";

const TIER_LABEL: Record<Tier, string> = { standard: L("Dengeli", "Balanced"), high: L("Yüksek", "High") };
const POWER_LABEL: Record<PowerSetting, string> = {
  auto: L("Sisteme göre", "System default"),
  silent: L("Sessiz", "Silent"),
  performance: L("Performans", "Performance"),
  turbo: "Turbo",
  off: L("Lumen değiştirmesin", "Don't change"),
};
const M2_LABEL: Record<M2Action, string> = {
  rewind: L("Basılı tut: geri sar", "Hold: rewind"),
  fastForward: L("Basılı tut: 3× hızlı", "Hold: 3× speed"),
  screenshot: L("Ekran görüntüsü", "Screenshot"),
};
const M2_ORDER: M2Action[] = ["rewind", "fastForward", "screenshot"];
const POWER_ORDER: PowerSetting[] = ["auto", "silent", "performance", "turbo", "off"];
const FILTER_ORDER: FilterSetting[] = ["original", "auto", "smooth", "crt", "lcd"];
const FILTER_LABEL: Record<FilterSetting, [string, string]> = {
  original: [L("Orijinal", "Original"), L("Keskin pikseller, olduğu gibi", "Sharp pixels, as they were")],
  auto: [L("Otomatik", "Auto"), L("El konsollarında LCD, 2D konsollarda CRT, 3D'de orijinal", "LCD on handhelds, CRT on 2D consoles, original on 3D")],
  smooth: [L("Pürüzsüz", "Smooth"), L("Pikseller yumuşatılır", "Pixels are smoothed")],
  crt: ["CRT", L("Tüplü televizyon tarama çizgileri", "Tube TV scanlines")],
  lcd: ["LCD", L("El konsolu ekranının ızgarası", "Handheld screen grid")],
};
const GRAPHICS_LABEL: Record<GraphicsSetting, string> = { auto: L("Otomatik", "Auto"), ...TIER_LABEL };

interface Item {
  /** X button action (e.g. uninstall), with its hint label. */
  secondary?: { label: string; run: () => void };
  id: string;
  title: string;
  sub?: string;
  value?: React.ReactNode;
  run: () => void;
}

export function Menu({
  onClose,
  onChangeRoot,
  onOpenStore,
  onOpenImport,
  onOpenStats,
  onOpenTour,
  onOpenSteam,
  onOpenArt,
  onOpenRaLogin,
}: {
  onClose: () => void;
  onChangeRoot: () => void;
  onOpenStore: () => void;
  onOpenImport: () => void;
  onOpenStats: () => void;
  onOpenTour: () => void;
  onOpenSteam: () => void;
  onOpenArt: () => void;
  onOpenRaLogin: () => void;
}) {
  const [page, setPage] = useState<"main" | "emulators" | "backup" | "ra" | "theme" | "systems" | "kids">("main");
  // Restore overwrites saves: it needs a second press to confirm.
  const [confirmRestore, setConfirmRestore] = useState(false);
  // Same for removing an emulator (holds the emulator id being confirmed).
  const [confirmRemove, setConfirmRemove] = useState<string | null>(null);
  const overview = useStore((s) => s.overview);
  const games = useStore((s) => s.games);
  const installs = useStore((s) => s.installs);
  const updates = useStore((s) => s.updates);
  const checkingUpdates = useStore((s) => s.checkingUpdates);
  const pendingUpdates = Object.values(updates ?? {}).filter(Boolean).length;
  const sounds = overview?.settings.sounds ?? true;

  const appUpdate = useStore((s) => s.appUpdate);
  const appUpdateProgress = useStore((s) => s.appUpdateProgress);

  // Kids mode: what will be allowed, and the PIN being typed (set or exit).
  const kids = overview?.settings.kids;
  const collections = overview?.collections ?? [];
  const [kidsCollection, setKidsCollection] = useState<string | null>(kids?.collection ?? null);
  const [kidsMinutes, setKidsMinutes] = useState<number | null>(kids?.dailyMinutes ?? null);
  const [newPin, setNewPin] = useState("");
  const [pinPrompt, setPinPrompt] = useState<null | "set" | "exit">(null);
  const [pin, setPin] = useState("");
  const [pinBusy, setPinBusy] = useState(false);
  const hasPin = kids?.pinHash != null || newPin.length > 0;
  const KIDS_MINUTES = [null, 30, 60, 90, 120];

  const consoleSystems = systems.filter((s) => !isPcSystem(s.id));
  const gameCount = (id: string) => overview?.systems.find((s) => s.id === id)?.gameCount ?? 0;

  const kidsItems: Item[] = [
    {
      id: "kids-pin",
      title: "PIN",
      sub: L("Çocuk modundan çıkmak için gerekir; en az 4 rakam", "Needed to leave kids mode; at least 4 digits"),
      value: hasPin ? "••••" : L("Belirle", "Set"),
      run: () => {
        setPin("");
        setPinPrompt("set");
      },
    },
    {
      id: "kids-games",
      title: L("Görünen oyunlar", "Visible games"),
      sub: collections.length ? L("Bir koleksiyon seç ya da tüm konsol oyunları", "Pick a collection, or all console games") : L("Koleksiyon oluşturursan yalnızca onu gösterebilirsin", "Create a collection to show only its games"),
      value: collections.find((c) => c.id === kidsCollection)?.name ?? L("Tüm oyunlar", "All games"),
      run: () => {
        const ids = [null, ...collections.map((c) => c.id)];
        setKidsCollection(ids[(ids.indexOf(kidsCollection) + 1) % ids.length]);
      },
    },
    {
      id: "kids-limit",
      title: L("Günlük oyun süresi", "Daily play time"),
      sub: L("Süre dolunca oyun kaydedilip kapanır; ertesi gün yeniden açılır", "When time's up the game saves and closes; it unlocks again the next day"),
      value: kidsMinutes ? L(`${kidsMinutes} dk`, `${kidsMinutes} min`) : L("Sınırsız", "Unlimited"),
      run: () => setKidsMinutes(KIDS_MINUTES[(KIDS_MINUTES.indexOf(kidsMinutes) + 1) % KIDS_MINUTES.length]),
    },
    {
      id: "kids-start",
      title: L("Çocuk modunu başlat", "Start kids mode"),
      sub: hasPin ? L("Ayarlar, mağazalar ve oyun silme gizlenir", "Settings, stores and game deletion are hidden") : L("Önce bir PIN belirle", "Set a PIN first"),
      run: async () => {
        if (!hasPin) {
          sfx.error();
          setPin("");
          setPinPrompt("set");
          return;
        }
        if (await actions.kidsStart(newPin, kidsCollection, kidsMinutes)) onClose();
      },
    },
  ];

  const kidsActiveItems: Item[] = [
    {
      id: "kids-exit",
      title: L("Çocuk modundan çık", "Exit kids mode"),
      sub: L("PIN gerekir", "PIN required"),
      run: () => {
        setPin("");
        setPinPrompt("exit");
      },
    },
  ];

  const main: Item[] = kids?.active ? kidsActiveItems : [
    ...(appUpdate
      ? [
          {
            id: "app-update",
            title: L(`Lumen'i güncelle (${appUpdate.version})`, `Update Lumen (${appUpdate.version})`),
            sub: L("İndirilir, kurulur ve Lumen yeniden başlar", "Downloads, installs and restarts Lumen"),
            value:
              appUpdateProgress !== null ? (
                <span style={{ width: "5rem", display: "inline-block" }}>
                  <ProgressBar value={appUpdateProgress} indeterminate={appUpdateProgress === 0} />
                </span>
              ) : (
                <span className="pill update">{L("Yeni", "New")}</span>
              ),
            run: () => actions.installAppUpdate(),
          },
        ]
      : []),
    {
      id: "rescan",
      title: L("Kütüphaneyi yenile", "Refresh library"),
      sub: L(`${games.length} oyun`, `${games.length} ${games.length === 1 ? "game" : "games"}`),
      run: () => {
        actions.rescan();
        onClose();
      },
    },
    {
      id: "store",
      title: L("Ücretsiz oyunlar", "Free games"),
      sub: L("Homebrew'lar ve hak sahiplerinin ücretsiz yaptığı klasikler", "Homebrew and classics made free by their owners"),
      value: "›",
      run: onOpenStore,
    },
    {
      id: "import",
      title: L("Oyunlarını içe aktar", "Import your games"),
      sub: L("Bir oyun dosyası veya klasör seç, Lumen doğru sistemin klasörüne koyar", "Pick a game file or folder; Lumen puts it in the right system's folder"),
      value: "›",
      run: onOpenImport,
    },
    {
      id: "steam",
      title: L("Steam kütüphanesi", "Steam library"),
      sub: overview?.steamLinked
        ? L("Bağlı · yüklü olmayan oyunların Steam sekmesinde", "Linked · uninstalled games are in the Steam tab")
        : L("Yüklü olmayan Steam oyunlarını göster ve buradan kur", "Show uninstalled Steam games and install them from here"),
      value: overview?.steamLinked ? <span className="pill ok">{L("Bağlı", "Linked")}</span> : "›",
      run: onOpenSteam,
    },
    {
      id: "art",
      title: L("Kapak görselleri", "Cover art"),
      sub: overview?.sgdbLinked
        ? L("SteamGridDB bağlı · eksik kapaklar indirilir", "SteamGridDB linked · missing covers are downloaded")
        : L("Kapağı olmayan oyunlar için SteamGridDB'den kapak indir", "Download covers from SteamGridDB for games without one"),
      value: overview?.sgdbLinked ? <span className="pill ok">{L("Bağlı", "Linked")}</span> : "›",
      run: onOpenArt,
    },
    {
      id: "stats",
      title: L("İstatistikler", "Stats"),
      sub: L("Haftalık oynama süresi, en çok oynanan oyunlar ve sistemler", "Weekly play time, most played games and systems"),
      value: "›",
      run: onOpenStats,
    },
    {
      id: "ra",
      title: "RetroAchievements",
      sub: overview?.raUser
        ? L(`Bağlı: ${overview.raUser.username}${overview.raUser.score != null ? ` · ${overview.raUser.score} puan` : ""}`, `Signed in: ${overview.raUser.username}${overview.raUser.score != null ? ` · ${overview.raUser.score} ${overview.raUser.score === 1 ? "point" : "points"}` : ""}`)
        : L("Retro oyunlarda başarımlar · giriş yapılmadı", "Achievements for retro games · not signed in"),
      value: "›",
      run: () => (overview?.raUser ? setPage("ra") : onOpenRaLogin()),
    },
    {
      id: "backup",
      title: L("Kayıt yedekleme", "Save backup"),
      sub: overview?.settings.backup.enabled ? L("Her oyundan sonra otomatik", "Automatic after every game") : L("Kapalı", "Off"),
      value: "›",
      run: () => setPage("backup"),
    },
    {
      id: "systems",
      title: L("Desteklenen sistemler", "Supported systems"),
      sub: L(`${consoleSystems.length} sistem · ${consoleSystems.filter((s) => gameCount(s.id) > 0).length} sistemde oyun var`, `${consoleSystems.length} systems · ${consoleSystems.filter((s) => gameCount(s.id) > 0).length} with games`),
      value: "›",
      run: () => setPage("systems"),
    },
    { id: "emulators", title: L("Emülatörler", "Emulators"), sub: L("Kurulum, durum ve BIOS dosyaları", "Setup, status and BIOS files"), value: "›", run: () => setPage("emulators") },
    {
      id: "theme",
      title: L("Tema", "Theme"),
      sub: themeOf(overview?.settings.theme).description,
      value: themeOf(overview?.settings.theme).name,
      run: () => setPage("theme"),
    },
    {
      id: "graphics",
      title: L("Grafik profili", "Graphics profile"),
      sub: overview
        ? `${overview.device.model ?? overview.device.cpu}${overview.settings.graphics === "auto" ? L(" · otomatik seçildi", " · picked automatically") : ""}`
        : undefined,
      value: overview ? `${GRAPHICS_LABEL[overview.settings.graphics]}${overview.settings.graphics === "auto" ? ` (${TIER_LABEL[overview.tier]})` : ""}` : "",
      run: () => {
        const order: GraphicsSetting[] = ["auto", "standard", "high"];
        const current = overview?.settings.graphics ?? "auto";
        actions.setGraphics(order[(order.indexOf(current) + 1) % order.length]);
      },
    },
    {
      id: "overlay",
      title: L("Oyun içi menü düğmesi", "In-game menu button"),
      sub: L("Oyunun sağ üst köşesinde küçük bir düğme; dokununca menü açılır, oyuna hiçbir tuş gitmez", "A small button in the game's top-right corner; tap it to open the menu, no input reaches the game"),
      value: overview?.settings.overlayButton ? L("Açık", "On") : L("Kapalı", "Off"),
      run: () => actions.setOverlayButton(!overview?.settings.overlayButton),
    },
    ...(overview?.settings.overlayButton
      ? [
          {
            id: "overlay-status",
            title: L("Oyun içi pil ve saat", "In-game battery and clock"),
            sub: L("Menü düğmesinin yanında pil yüzdesi ve saat; pil azalınca kırmızıya döner", "Battery percentage and time next to the menu button; turns red when battery is low"),
            value: overview.settings.overlayStatus ? L("Açık", "On") : L("Kapalı", "Off"),
            run: () => actions.setOverlayStatus(!overview.settings.overlayStatus),
          },
        ]
      : []),
    ...(overview?.device.model
      ? [
          {
            id: "back-buttons",
            title: L("Arka tuşlar (M1/M2)", "Back buttons (M1/M2)"),
            sub: L("M1: dokun menü, basılı tut kaydet ve çık. M2: aşağıda seçtiğin kısayol. Oyuna hiçbir tuş gitmez", "M1: tap for menu, hold to save and quit. M2: the shortcut you pick below. No input reaches the game"),
            value: overview.settings.backButtons ? L("Açık", "On") : L("Kapalı", "Off"),
            run: () => actions.setBackButtons(!overview.settings.backButtons),
          },
          ...(overview.settings.backButtons
            ? [
                {
                  id: "m2",
                  title: L("M2 tuşu", "M2 button"),
                  sub: L("Oyunda M2'nin işi; oyuna hiçbir tuş gitmez", "What M2 does in game; no input reaches the game"),
                  value: M2_LABEL[overview.settings.m2Action],
                  run: () =>
                    actions.setM2Action(
                      M2_ORDER[(M2_ORDER.indexOf(overview.settings.m2Action) + 1) % M2_ORDER.length],
                    ),
                },
              ]
            : []),
        ]
      : []),
    {
      id: "combo",
      title: L("Select + Start kısayolu", "Select + Start shortcut"),
      sub: L("Oyunda menüyü açar, basılı tutmak kaydedip çıkar. Bu tuşlar oyuna da gider (Start oyunu duraklatabilir)", "Opens the menu in game; hold to save and quit. These buttons also reach the game (Start may pause it)"),
      value: overview?.settings.quickCombo ? L("Açık", "On") : L("Kapalı", "Off"),
      run: () => actions.setQuickCombo(!overview?.settings.quickCombo),
    },
    {
      id: "filter",
      title: L("Görüntü filtresi", "Display filter"),
      sub: FILTER_LABEL[overview?.settings.filter ?? "original"][1],
      value: FILTER_LABEL[overview?.settings.filter ?? "original"][0],
      run: () => {
        const current = overview?.settings.filter ?? "original";
        actions.setFilter(FILTER_ORDER[(FILTER_ORDER.indexOf(current) + 1) % FILTER_ORDER.length]);
      },
    },
    ...(overview?.powerAvailable
      ? [
          {
            id: "power",
            title: L("Güç modu", "Power mode"),
            sub: L("Oyun açılınca ASUS güç modu ayarlanır; çıkınca Performans'a döner", "Sets the ASUS power mode when a game starts; back to Performance when you quit"),
            value: POWER_LABEL[overview.settings.power],
            run: () => {
              const current = overview.settings.power;
              actions.setPower(POWER_ORDER[(POWER_ORDER.indexOf(current) + 1) % POWER_ORDER.length]);
            },
          },
        ]
      : []),
    ...(overview?.platformSupported
      ? [
          {
            id: "autostart",
            title: L("Konsol modu", "Console mode"),
            sub: L("Windows açılınca doğrudan Lumen başlasın", "Start Lumen right when Windows starts"),
            value: overview.autostart ? L("Açık", "On") : L("Kapalı", "Off"),
            run: () => actions.setAutostart(!overview.autostart),
          },
        ]
      : []),
    {
      id: "kids",
      title: L("Çocuk modu", "Kids mode"),
      sub: L("PIN korumalı sade kütüphane ve günlük oyun süresi", "PIN-protected simple library and daily play time"),
      value: "›",
      run: () => setPage("kids"),
    },
    {
      id: "language",
      title: "Dil / Language",
      sub: L("Otomatik: Windows'un diliyle aynı", "Automatic: same as Windows"),
      value: { auto: L("Otomatik", "Automatic"), tr: "Türkçe", en: "English" }[languageSetting()],
      run: () => {
        const order: Language[] = ["auto", "tr", "en"];
        setLanguageSetting(order[(order.indexOf(languageSetting()) + 1) % order.length]);
      },
    },
    { id: "tour", title: L("Kısa tur", "Quick tour"), sub: L("Nerede ne var, önemli kısayollar", "Where things are, key shortcuts"), value: "›", run: onOpenTour },
    {
      id: "report",
      title: L("Sorun bildir", "Report a problem"),
      sub: L("Günlükler ve cihaz bilgisi masaüstüne zip olarak kaydedilir; hesap anahtarların eklenmez", "Saves logs and device info as a zip on the desktop; your account keys aren't included"),
      run: () => actions.reportProblem(),
    },
    { id: "sounds", title: L("Arayüz sesleri", "UI sounds"), value: sounds ? L("Açık", "On") : L("Kapalı", "Off"), run: () => actions.setSounds(!sounds) },
    {
      id: "root",
      title: L("Oyun klasörünü değiştir", "Change game folder"),
      sub: overview?.settings.romRoot ?? undefined,
      run: onChangeRoot,
    },
    ...(isTauri
      ? [
          {
            id: "fullscreen",
            title: L("Tam ekran", "Fullscreen"),
            value: "F11",
            run: async () => {
              const { getCurrentWindow } = await import("@tauri-apps/api/window");
              const win = getCurrentWindow();
              await win.setFullscreen(!(await win.isFullscreen()));
            },
          },
          {
            id: "quit",
            title: L("Lumen'den çık", "Quit Lumen"),
            run: async () => {
              const { getCurrentWindow } = await import("@tauri-apps/api/window");
              await getCurrentWindow().close();
            },
          },
        ]
      : []),
  ];

  const updateAllItem: Item[] =
    pendingUpdates > 0
      ? [
          {
            id: "update-all",
            title: L(`Hepsini güncelle (${pendingUpdates})`, `Update all (${pendingUpdates})`),
            sub: L("Kayıtlar ve ayarlar korunur", "Saves and settings are kept"),
            run: () => actions.updateAll(),
          },
        ]
      : [];

  const emulatorItems: Item[] = [...updateAllItem, ...emulators.map((emu) => {
    const served = systems.filter((s) => s.emulator === emu.id);
    const statuses = overview?.systems.filter((s) => served.some((x) => x.id === s.id)) ?? [];
    const installed = statuses.some((s) => s.emulatorInstalled);
    const missingBios = statuses
      .filter((s) => !s.usingFallback)
      .flatMap((s) => s.bios.filter((b) => !b.found && !b.optional));
    const job = installs[emu.id] ?? Object.values(installs).find((p) => p.id.startsWith("core:") && emu.id === "retroarch");
    const withGames = served.filter((s) => statuses.find((x) => x.id === s.id)?.gameCount);

    return {
      secondary: installed
        ? {
            label: confirmRemove === emu.id ? L("Onayla", "Confirm") : L("Kaldır", "Remove"),
            run: () => {
              if (confirmRemove !== emu.id) {
                setConfirmRemove(emu.id);
                return;
              }
              setConfirmRemove(null);
              actions.uninstallEmulator(emu.id, emu.name);
            },
          }
        : undefined,
      id: emu.id,
      title: confirmRemove === emu.id ? L(`${emu.name} kaldırılsın mı? X ile onayla`, `Remove ${emu.name}? Press X to confirm`) : emu.name,
      sub: missingBios.length
        ? L(`Eksik: ${missingBios.map((b) => b.label).join(", ")}`, `Missing: ${missingBios.map((b) => b.label).join(", ")}`)
        : served.map((s) => s.short).join(" · "),
      value: job ? (
        <span style={{ width: "5rem", display: "inline-block" }}>
          <ProgressBar value={job.stage === "downloading" ? job.progress : 1} />
        </span>
      ) : installed && updates?.[emu.id] ? (
        <span className="pill update">{L("Güncelleme var", "Update available")}</span>
      ) : installed ? (
        <span className={`pill ${missingBios.length ? "warn" : "ok"}`}>
          {missingBios.length ? L("BIOS eksik", "BIOS missing") : checkingUpdates ? L("Kontrol ediliyor…", "Checking…") : L("Güncel", "Up to date")}
        </span>
      ) : (
        <span className="pill">{L("Kur", "Install")}</span>
      ),
      run: () => {
        if (job) return;
        if (!overview?.platformSupported) {
          sfx.error();
          notify(L("Emülatör kurulumu şimdilik sadece Windows'ta destekleniyor", "Installing emulators is only supported on Windows for now"), "error");
          return;
        }
        if (installed) {
          if (updates?.[emu.id]) actions.updateEmulator(emu.id);
          else notify(L(`${installLabel(emu.id)} güncel`, `${installLabel(emu.id)} is up to date`), "success");
          return;
        }
        // RetroArch serves many systems; only fetch cores that are actually needed.
        const targets = emu.id === "retroarch" && withGames.length ? withGames : served;
        notify(L(`${installLabel(emu.id)} kuruluyor…`, `Installing ${installLabel(emu.id)}…`));
        actions.install(targets.map((s) => s.id));
      },
    };
  })];

  const backup = overview?.settings.backup;
  // Cloud drives found on this PC, offered as backup folders.
  const [clouds, setClouds] = useState<CloudFolder[]>([]);
  useEffect(() => {
    if (page === "backup") api.cloudFolders().then(setClouds).catch(() => setClouds([]));
  }, [page]);
  const cloudNow = clouds.find((c) => c.path === overview?.backupDir);
  const backupItems: Item[] = [
    {
      id: "auto",
      title: L("Otomatik yedekleme", "Automatic backup"),
      sub: L("Her oyun bittiğinde o emülatörün kayıtları kopyalanır", "When a game ends, that emulator's saves are copied"),
      value: backup?.enabled ? L("Açık", "On") : L("Kapalı", "Off"),
      run: () => actions.setBackup(!backup?.enabled),
    },
    ...(clouds.length
      ? [
          {
            id: "cloud",
            title: L("Bulut klasörü", "Cloud folder"),
            sub: cloudNow
              ? L("Kayıtlar bu bulut hesabına da gider", "Saves also go to this cloud account")
              : L(`Bu bilgisayarda bulunan: ${clouds.map((c) => c.name).join(", ")}`, `Found on this PC: ${clouds.map((c) => c.name).join(", ")}`),
            value: cloudNow?.name ?? L("Seç", "Choose"),
            run: () => {
              const next = clouds[(clouds.findIndex((c) => c.path === overview?.backupDir) + 1) % clouds.length];
              actions.setBackupDir(next.path);
            },
          },
        ]
      : []),
    {
      id: "folder",
      title: L("Yedek klasörü", "Backup folder"),
      sub: overview?.backupDir,
      value: L("Değiştir", "Change"),
      run: () => actions.setBackup(!!backup?.enabled, true),
    },
    {
      id: "sync",
      title: L("Cihazlar arası senkron", "Sync across devices"),
      sub: L("Oyun açılmadan önce yedek klasöründeki daha yeni kayıtlar alınır; başka cihazda kaldığın yerden devam edersin", "Newer saves in the backup folder are pulled before a game starts, so you pick up where you left off on another device"),
      value: backup?.sync ? L("Açık", "On") : L("Kapalı", "Off"),
      run: () => actions.setBackupSync(!backup?.sync),
    },
    {
      id: "now",
      title: L("Şimdi yedekle", "Back up now"),
      sub: backup?.lastBackup ? L(`Son yedek: ${formatLastPlayed(backup.lastBackup).toLowerCase()}`, `Last backup: ${formatLastPlayed(backup.lastBackup).toLowerCase()}`) : L("Henüz yedek alınmadı", "No backup yet"),
      run: () => actions.backupNow(),
    },
    {
      id: "restore",
      title: confirmRestore ? L("Onaylamak için tekrar bas", "Press again to confirm") : L("Yedekten geri yükle", "Restore from backup"),
      sub: L("Yedekteki daha yeni kayıtlar cihazdakilerin yerine geçer", "Newer saves in the backup replace the ones on this device"),
      run: () => {
        if (!confirmRestore) {
          setConfirmRestore(true);
          return;
        }
        setConfirmRestore(false);
        actions.restoreBackup();
      },
    },
  ];

  const raItems: Item[] = [
    {
      id: "account",
      title: overview?.raUser?.username ?? "",
      sub:
        overview?.raUser?.score != null
          ? L(`${overview.raUser.score} puan · RetroArch, DuckStation, PCSX2, PPSSPP ve Dolphin'de açık`, `${overview.raUser.score} ${overview.raUser.score === 1 ? "point" : "points"} · On in RetroArch, DuckStation, PCSX2, PPSSPP and Dolphin`)
          : L("RetroArch, DuckStation, PCSX2, PPSSPP ve Dolphin'de açık", "On in RetroArch, DuckStation, PCSX2, PPSSPP and Dolphin"),
      value: L("Yenile", "Refresh"),
      run: () => actions.raRefresh(),
    },
    {
      id: "hardcore",
      title: L("Hardcore modu", "Hardcore mode"),
      sub: L("Save state ve hileler kapanır, başarımlar hardcore sayılır", "Save states and cheats are off; achievements count as hardcore"),
      value: overview?.settings.raHardcore ? L("Açık", "On") : L("Kapalı", "Off"),
      run: () => actions.setRaHardcore(!overview?.settings.raHardcore),
    },
    {
      id: "logout",
      title: L("Çıkış yap", "Sign out"),
      run: () => {
        actions.raLogout();
        setPage("main");
      },
    },
  ];

  const systemItems: Item[] = consoleSystems.map((s) => {
    const status = overview?.systems.find((x) => x.id === s.id);
    const count = gameCount(s.id);
    const missing = status && !status.usingFallback ? status.bios.filter((b) => !b.found && !b.optional) : [];
    const runner = status?.usingFallback && s.fallback ? s.fallback.emulator : s.emulator;
    return {
      id: s.id,
      title: s.name,
      sub: [s.maker, emulatorOf(runner)?.name, count ? L(`${count} oyun`, `${count} ${count === 1 ? "game" : "games"}`) : L("oyun yok", "no games")].filter(Boolean).join(" · "),
      value: EXPERIMENTAL.has(s.id) ? (
        <span className="pill warn">{L("Deneysel", "Experimental")}</span>
      ) : missing.length ? (
        <span className="pill warn">{L("BIOS gerekli", "BIOS needed")}</span>
      ) : status?.emulatorInstalled && status.coreInstalled ? (
        <span className="pill ok">{L("Hazır", "Ready")}</span>
      ) : (
        <span className="pill">{L("İlk oyunda kurulur", "Installs on first game")}</span>
      ),
      run: () => {
        actions.showSystem(s.id);
        onClose();
      },
    };
  });

  const themeItems: Item[] = THEMES.map((t) => ({
    id: t.id,
    title: t.name,
    sub: t.description,
    value: overview?.settings.theme === t.id ? <span className="pill ok">{L("Seçili", "Selected")}</span> : undefined,
    run: () => actions.setTheme(t.id),
  }));

  const items =
    page === "main"
      ? main
      : page === "backup"
        ? backupItems
        : page === "ra"
          ? raItems
          : page === "theme"
            ? themeItems
            : page === "systems"
              ? systemItems
              : page === "kids"
                ? kidsItems
                : emulatorItems;
  const [focus, setFocus] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const index = Math.min(focus, items.length - 1);

  useEffect(() => setFocus(0), [page]);
  useEffect(() => {
    setConfirmRestore(false);
    setConfirmRemove(null);
  }, [page, index]);
  useEffect(() => {
    if (page === "ra") actions.raRefresh();
  }, [page]);
  useEffect(() => {
    if (page === "emulators" && overview?.platformSupported) actions.checkUpdates();
  }, [page, overview?.platformSupported]);
  useEffect(() => {
    listRef.current?.children[index]?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [index]);

  const back = () => {
    sfx.back();
    if (page === "main") onClose();
    else setPage("main");
  };

  useInput((action) => {
    switch (action) {
      case "up":
      case "down": {
        const next = Math.min(items.length - 1, Math.max(0, index + (action === "up" ? -1 : 1)));
        if (next !== index) {
          sfx.move();
          setFocus(next);
        }
        break;
      }
      case "a":
        sfx.confirm();
        items[index]?.run();
        break;
      case "x": {
        const secondary = items[index]?.secondary;
        if (secondary) {
          sfx.confirm();
          secondary.run();
        }
        break;
      }
      case "b":
        back();
        break;
      case "start":
        sfx.back();
        onClose();
        break;
    }
  }, Layer.menu, !pinPrompt);

  if (pinPrompt) {
    return (
      <TextKeyboard
        title={pinPrompt === "set" ? L("Yeni PIN (en az 4 rakam)", "New PIN (at least 4 digits)") : L("Çocuk modundan çıkmak için PIN", "PIN to exit kids mode")}
        placeholder="PIN"
        value={pin}
        secret
        busy={pinBusy}
        onChange={(v) => setPin(v.replace(/\D/g, ""))}
        onDone={async () => {
          if (pinPrompt === "set") {
            if (pin.length < 4) {
              sfx.error();
              notify(L("PIN en az 4 rakam olmalı", "PIN must be at least 4 digits"), "error");
              return;
            }
            setNewPin(pin);
            setPinPrompt(null);
            return;
          }
          setPinBusy(true);
          const ok = await actions.kidsStop(pin);
          setPinBusy(false);
          setPin("");
          if (ok) {
            setPinPrompt(null);
            onClose();
          }
        }}
        onCancel={() => setPinPrompt(null)}
      />
    );
  }

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="menu">
        <h2>
          {page === "main"
            ? kids?.active
              ? L("Çocuk modu", "Kids mode")
              : L("Menü", "Menu")
            : page === "kids"
              ? L("Çocuk modu", "Kids mode")
            : page === "backup"
              ? L("Kayıt yedekleme", "Save backup")
              : page === "ra"
                ? "RetroAchievements"
                : page === "theme"
                  ? L("Tema", "Theme")
                  : page === "systems"
                    ? L("Desteklenen sistemler", "Supported systems")
                    : L("Emülatörler", "Emulators")}
        </h2>
        <p className="menu-sub">
          {page === "main"
            ? kids?.active
              ? L("Ayarlar kapalı. Çıkmak için PIN gerekir.", "Settings are off. A PIN is needed to exit.")
              : "Lumen 0.1"
            : page === "kids"
              ? L("Kütüphanede sadece seçtiğin oyunlar görünür; ayarlar, mağazalar ve oyun silme gizlenir.", "Only the games you pick show in the library; settings, stores and game deletion are hidden.")
            : page === "backup"
              ? L("Kayıtlar ve save state'ler seçtiğin klasöre kopyalanır; OneDrive, Google Drive veya Dropbox klasörü buluta da taşır.", "Saves and save states are copied to the folder you pick; a OneDrive, Google Drive or Dropbox folder also takes them to the cloud.")
              : page === "ra"
                ? L("Tek giriş tüm emülatörlere yazılır. Şifren saklanmaz, sadece oturum anahtarı tutulur.", "One sign-in works for every emulator. Your password isn't stored, only a session token.")
                : page === "theme"
                  ? L("Her tema kendi düzenini, yazı tiplerini ve gezinmesini getirir.", "Each theme brings its own layout, fonts and navigation.")
                : page === "systems"
                  ? L("Lumen'in çalıştırdığı tüm sistemler. Birini seçince kütüphanede o sistemin sekmesi açılır; orada oyunların nereye konacağı yazar.", "Every system Lumen runs. Pick one to open its tab in the library; it shows where to put the games.")
              : L("Emülatörler resmi kaynaklarından indirilir. Oyun ve BIOS dosyaları indirilmez.", "Emulators are downloaded from their official sources. Games and BIOS files are not.")}
        </p>
        <div className="menu-list" ref={listRef}>
          {items.map((item, i) => (
            <button
              key={item.id}
              className={`menu-item${i === index ? " focused" : ""}`}
              onClick={() => {
                setFocus(i);
                item.run();
              }}
              tabIndex={-1}
            >
              <div className="menu-item-main">
                <div className="menu-item-title">{item.title}</div>
                {item.sub && <div className="menu-item-sub">{item.sub}</div>}
              </div>
              {item.value !== undefined && <span className="menu-item-value">{item.value}</span>}
            </button>
          ))}
        </div>
      </aside>
      <HintBar
        hints={[
          { action: "a", label: L("Seç", "Select") },
          ...(items[index]?.secondary ? [{ action: "x" as const, label: items[index].secondary!.label }] : []),
          { action: "b", label: page === "main" ? L("Kapat", "Close") : L("Geri", "Back") },
        ]}
      />
    </>
  );
}
