import { useEffect, useRef, useState } from "react";
import { HintBar } from "../components/HintBar";
import { ProgressBar } from "../components/Toasts";
import { isTauri } from "../lib/api";
import { emulators, installLabel, systems } from "../lib/catalog";
import { Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, notify, useStore } from "../lib/store";
import type { GraphicsSetting, Tier } from "../lib/types";

const TIER_LABEL: Record<Tier, string> = { standard: "Dengeli", high: "Yüksek" };
const GRAPHICS_LABEL: Record<GraphicsSetting, string> = { auto: "Otomatik", ...TIER_LABEL };

interface Item {
  id: string;
  title: string;
  sub?: string;
  value?: React.ReactNode;
  run: () => void;
}

export function Menu({ onClose, onChangeRoot }: { onClose: () => void; onChangeRoot: () => void }) {
  const [page, setPage] = useState<"main" | "emulators">("main");
  const overview = useStore((s) => s.overview);
  const games = useStore((s) => s.games);
  const installs = useStore((s) => s.installs);
  const sounds = overview?.settings.sounds ?? true;

  const main: Item[] = [
    {
      id: "rescan",
      title: "Kütüphaneyi yenile",
      sub: `${games.length} oyun`,
      run: () => {
        actions.rescan();
        onClose();
      },
    },
    { id: "emulators", title: "Emülatörler", sub: "Kurulum, durum ve BIOS dosyaları", value: "›", run: () => setPage("emulators") },
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

  const emulatorItems: Item[] = emulators.map((emu) => {
    const served = systems.filter((s) => s.emulator === emu.id);
    const statuses = overview?.systems.filter((s) => served.some((x) => x.id === s.id)) ?? [];
    const installed = statuses.some((s) => s.emulatorInstalled);
    const missingBios = statuses.flatMap((s) => s.bios.filter((b) => !b.found && !b.optional));
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
      ) : installed ? (
        <span className={`pill ${missingBios.length ? "warn" : "ok"}`}>{missingBios.length ? "BIOS eksik" : "Kurulu"}</span>
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
        // RetroArch serves many systems; only fetch cores that are actually needed.
        const targets = emu.id === "retroarch" && withGames.length ? withGames : served;
        notify(`${installLabel(emu.id)} ${installed ? "güncelleniyor" : "kuruluyor"}…`);
        actions.install(targets.map((s) => s.id));
      },
    };
  });

  const items = page === "main" ? main : emulatorItems;
  const [focus, setFocus] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);
  const index = Math.min(focus, items.length - 1);

  useEffect(() => setFocus(0), [page]);
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
        <h2>{page === "main" ? "Menü" : "Emülatörler"}</h2>
        <p className="menu-sub">
          {page === "main"
            ? "Lumen 0.1"
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
