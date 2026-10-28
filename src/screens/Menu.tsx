import { useEffect, useRef, useState } from "react";
import { HintBar } from "../components/HintBar";
import { ProgressBar } from "../components/Toasts";
import { isTauri } from "../lib/api";
import { emulators, installLabel, systems } from "../lib/catalog";
import { Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, notify, useStore } from "../lib/store";
import type { GraphicsSetting, PowerSetting, Tier } from "../lib/types";
import { formatLastPlayed } from "../lib/format";
import { THEMES, themeOf } from "../lib/themes";

const TIER_LABEL: Record<Tier, string> = { standard: "Dengeli", high: "Yüksek" };
const POWER_LABEL: Record<PowerSetting, string> = {
  auto: "Sisteme göre",
  silent: "Sessiz",
  performance: "Performans",
  turbo: "Turbo",
  off: "Lumen değiştirmesin",
};
const POWER_ORDER: PowerSetting[] = ["auto", "silent", "performance", "turbo", "off"];
const GRAPHICS_LABEL: Record<GraphicsSetting, string> = { auto: "Otomatik", ...TIER_LABEL };

interface Item {
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
  onOpenRaLogin,
}: {
  onClose: () => void;
  onChangeRoot: () => void;
  onOpenStore: () => void;
  onOpenRaLogin: () => void;
}) {
  const [page, setPage] = useState<"main" | "emulators" | "backup" | "ra" | "theme">("main");
  // Restore overwrites saves: it needs a second press to confirm.
  const [confirmRestore, setConfirmRestore] = useState(false);
  const overview = useStore((s) => s.overview);
  const games = useStore((s) => s.games);
  const installs = useStore((s) => s.installs);
  const updates = useStore((s) => s.updates);
  const checkingUpdates = useStore((s) => s.checkingUpdates);
  const pendingUpdates = Object.values(updates ?? {}).filter(Boolean).length;
  const sounds = overview?.settings.sounds ?? true;

  const appUpdate = useStore((s) => s.appUpdate);
  const appUpdateProgress = useStore((s) => s.appUpdateProgress);

  const main: Item[] = [
    ...(appUpdate
      ? [
          {
            id: "app-update",
            title: `Lumen'i güncelle (${appUpdate.version})`,
            sub: "İndirilir, kurulur ve Lumen yeniden başlar",
            value:
              appUpdateProgress !== null ? (
                <span style={{ width: "5rem", display: "inline-block" }}>
                  <ProgressBar value={appUpdateProgress} indeterminate={appUpdateProgress === 0} />
                </span>
              ) : (
                <span className="pill update">Yeni</span>
              ),
            run: () => actions.installAppUpdate(),
          },
        ]
      : []),
    {
      id: "rescan",
      title: "Kütüphaneyi yenile",
      sub: `${games.length} oyun`,
      run: () => {
        actions.rescan();
        onClose();
      },
    },
    {
      id: "store",
      title: "Homebrew mağazası",
      sub: "Paylaşıma açık ücretsiz GB, GBA ve NES oyunları",
      value: "›",
      run: onOpenStore,
    },
    {
      id: "ra",
      title: "RetroAchievements",
      sub: overview?.raUser
        ? `Bağlı: ${overview.raUser.username}${overview.raUser.score != null ? ` · ${overview.raUser.score} puan` : ""}`
        : "Retro oyunlarda başarımlar · giriş yapılmadı",
      value: "›",
      run: () => (overview?.raUser ? setPage("ra") : onOpenRaLogin()),
    },
    {
      id: "backup",
      title: "Kayıt yedekleme",
      sub: overview?.settings.backup.enabled ? "Her oyundan sonra otomatik" : "Kapalı",
      value: "›",
      run: () => setPage("backup"),
    },
    { id: "emulators", title: "Emülatörler", sub: "Kurulum, durum ve BIOS dosyaları", value: "›", run: () => setPage("emulators") },
    {
      id: "theme",
      title: "Tema",
      sub: themeOf(overview?.settings.theme).description,
      value: themeOf(overview?.settings.theme).name,
      run: () => setPage("theme"),
    },
    {
      id: "graphics",
      title: "Grafik profili",
      sub: overview
        ? `${overview.device.model ?? overview.device.cpu}${overview.settings.graphics === "auto" ? " · otomatik seçildi" : ""}`
        : undefined,
      value: overview ? `${GRAPHICS_LABEL[overview.settings.graphics]}${overview.settings.graphics === "auto" ? ` (${TIER_LABEL[overview.tier]})` : ""}` : "",
      run: () => {
        const order: GraphicsSetting[] = ["auto", "standard", "high"];
        const current = overview?.settings.graphics ?? "auto";
        actions.setGraphics(order[(order.indexOf(current) + 1) % order.length]);
      },
    },
    ...(overview?.powerAvailable
      ? [
          {
            id: "power",
            title: "Güç modu",
            sub: "Oyun açılınca ASUS güç modu ayarlanır; çıkınca Performans'a döner",
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
            title: "Konsol modu",
            sub: "Windows açılınca doğrudan Lumen başlasın",
            value: overview.autostart ? "Açık" : "Kapalı",
            run: () => actions.setAutostart(!overview.autostart),
          },
        ]
      : []),
    { id: "sounds", title: "Arayüz sesleri", value: sounds ? "Açık" : "Kapalı", run: () => actions.setSounds(!sounds) },
    {
      id: "root",
      title: "Oyun klasörünü değiştir",
      sub: overview?.settings.romRoot ?? undefined,
      run: onChangeRoot,
    },
    ...(isTauri
      ? [
          {
            id: "fullscreen",
            title: "Tam ekran",
            value: "F11",
            run: async () => {
              const { getCurrentWindow } = await import("@tauri-apps/api/window");
              const win = getCurrentWindow();
              await win.setFullscreen(!(await win.isFullscreen()));
            },
          },
          {
            id: "quit",
            title: "Lumen'den çık",
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
            title: `Hepsini güncelle (${pendingUpdates})`,
            sub: "Kayıtlar ve ayarlar korunur",
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
      id: emu.id,
      title: emu.name,
      sub: missingBios.length
        ? `Eksik: ${missingBios.map((b) => b.label).join(", ")}`
        : served.map((s) => s.short).join(" · "),
      value: job ? (
        <span style={{ width: "5rem", display: "inline-block" }}>
          <ProgressBar value={job.stage === "downloading" ? job.progress : 1} />
        </span>
      ) : installed && updates?.[emu.id] ? (
        <span className="pill update">Güncelleme var</span>
      ) : installed ? (
        <span className={`pill ${missingBios.length ? "warn" : "ok"}`}>
          {missingBios.length ? "BIOS eksik" : checkingUpdates ? "Kontrol ediliyor…" : "Güncel"}
        </span>
      ) : (
        <span className="pill">Kur</span>
      ),
      run: () => {
        if (job) return;
        if (!overview?.platformSupported) {
          sfx.error();
          notify("Emülatör kurulumu şimdilik sadece Windows'ta destekleniyor", "error");
          return;
        }
        if (installed) {
          if (updates?.[emu.id]) actions.updateEmulator(emu.id);
          else notify(`${installLabel(emu.id)} güncel`, "success");
          return;
        }
        // RetroArch serves many systems; only fetch cores that are actually needed.
        const targets = emu.id === "retroarch" && withGames.length ? withGames : served;
        notify(`${installLabel(emu.id)} kuruluyor…`);
        actions.install(targets.map((s) => s.id));
      },
    };
  })];

  const backup = overview?.settings.backup;
  const backupItems: Item[] = [
    {
      id: "auto",
      title: "Otomatik yedekleme",
      sub: "Her oyun bittiğinde o emülatörün kayıtları kopyalanır",
      value: backup?.enabled ? "Açık" : "Kapalı",
      run: () => actions.setBackup(!backup?.enabled),
    },
    {
      id: "folder",
      title: "Yedek klasörü",
      sub: overview?.backupDir,
      value: "Değiştir",
      run: () => actions.setBackup(!!backup?.enabled, true),
    },
    {
      id: "now",
      title: "Şimdi yedekle",
      sub: backup?.lastBackup ? `Son yedek: ${formatLastPlayed(backup.lastBackup).toLowerCase()}` : "Henüz yedek alınmadı",
      run: () => actions.backupNow(),
    },
    {
      id: "restore",
      title: confirmRestore ? "Onaylamak için tekrar bas" : "Yedekten geri yükle",
      sub: "Yedekteki daha yeni kayıtlar cihazdakilerin yerine geçer",
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
          ? `${overview.raUser.score} puan · RetroArch, DuckStation, PCSX2, PPSSPP ve Dolphin'de açık`
          : "RetroArch, DuckStation, PCSX2, PPSSPP ve Dolphin'de açık",
      value: "Yenile",
      run: () => actions.raRefresh(),
    },
    {
      id: "hardcore",
      title: "Hardcore modu",
      sub: "Save state ve hileler kapanır, başarımlar hardcore sayılır",
      value: overview?.settings.raHardcore ? "Açık" : "Kapalı",
      run: () => actions.setRaHardcore(!overview?.settings.raHardcore),
    },
    {
      id: "logout",
      title: "Çıkış yap",
      run: () => {
        actions.raLogout();
        setPage("main");
      },
    },
  ];

  const themeItems: Item[] = THEMES.map((t) => ({
    id: t.id,
    title: t.name,
    sub: t.description,
    value: overview?.settings.theme === t.id ? <span className="pill ok">Seçili</span> : undefined,
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
            : emulatorItems;
  const [focus, setFocus] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const index = Math.min(focus, items.length - 1);

  useEffect(() => setFocus(0), [page]);
  useEffect(() => setConfirmRestore(false), [page, index]);
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
      case "b":
        back();
        break;
      case "start":
        sfx.back();
        onClose();
        break;
    }
  }, Layer.menu);

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="menu">
        <h2>
          {page === "main"
            ? "Menü"
            : page === "backup"
              ? "Kayıt yedekleme"
              : page === "ra"
                ? "RetroAchievements"
                : page === "theme"
                  ? "Tema"
                  : "Emülatörler"}
        </h2>
        <p className="menu-sub">
          {page === "main"
            ? "Lumen 0.1"
            : page === "backup"
              ? "Kayıtlar ve save state'ler seçtiğin klasöre kopyalanır; OneDrive klasörü buluta da taşır."
              : page === "ra"
                ? "Tek giriş tüm emülatörlere yazılır. Şifren saklanmaz, sadece oturum anahtarı tutulur."
                : page === "theme"
                  ? "Her tema kendi düzenini, yazı tiplerini ve gezinmesini getirir."
              : "Emülatörler resmi kaynaklarından indirilir. Oyun ve BIOS dosyaları indirilmez."}
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
          { action: "a", label: "Seç" },
          { action: "b", label: page === "main" ? "Kapat" : "Geri" },
        ]}
      />
    </>
  );
}
