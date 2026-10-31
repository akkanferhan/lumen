import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { BrandMark } from "../components/BrandMark";
import { HintBar } from "../components/HintBar";
import { Tour } from "../components/Tour";
import { ProgressBar } from "../components/Toasts";
import { emulatorOf, installLabel, isPcSystem, systemOf, systems as allSystems } from "../lib/catalog";

// Store launchers (Steam, Epic) have nothing to install.
const systems = allSystems.filter((s) => !isPcSystem(s.id));
import { api } from "../lib/api";
import { Layer, rumble, useInput } from "../lib/input";
import { L } from "../lib/i18n";
import { sfx } from "../lib/sfx";
import { actions, useStore } from "../lib/store";
import type { InstallProgress } from "../lib/types";
import { useColumns } from "../lib/useColumns";

type Step = "welcome" | "folder" | "systems" | "install" | "tour";

export function Onboarding({ onDone, startAt = "welcome" }: { onDone: () => void; startAt?: Step }) {
  const [step, setStep] = useState<Step>(startAt);
  const [finished, setFinished] = useState(false);
  const overview = useStore((s) => s.overview);

  const next = () => {
    if (step === "welcome") setStep("folder");
    else if (step === "folder") {
      setFinished(!overview?.platformSupported);
      setStep(overview?.platformSupported ? "systems" : "install");
    }
  };

  const install = (systemIds: string[]) => {
    setStep("install");
    actions.install(systemIds).finally(() => {
      setFinished(true);
      sfx.launch();
    });
  };

  return (
    <div className="onboarding">
      {step === "welcome" && <Welcome onNext={next} />}
      {step === "folder" && <Folder onNext={next} onBack={() => setStep("welcome")} />}
      {step === "systems" && <Systems onInstall={install} onSkip={() => setStep("tour")} />}
      {step === "install" && <Install finished={finished} onDone={() => setStep("tour")} />}
      {step === "tour" && <Tour layer={Layer.screen} onDone={onDone} />}
    </div>
  );
}

function Welcome({ onNext }: { onNext: () => void }) {
  useInput((a) => {
    if (a === "a") {
      sfx.confirm();
      rumble();
      onNext();
    }
  }, Layer.screen);

  return (
    <>
      <div className="step center">
        <BrandMark className="hero-mark" />
        <h1>{L("Tüm oyunların, tek ekranda.", "All your games, on one screen.")}</h1>
        <p className="lead">
          {L(
            "Lumen emülatörleri senin için indirir, ayarlar ve oyunlarını tek bir kütüphanede toplar. Kurulum bir dakikadan kısa sürer.",
            "Lumen downloads and sets up emulators for you and gathers your games in one library. Setup takes less than a minute.",
          )}
        </p>
        <div className="actions">
          <button className="btn primary focused" onClick={onNext}>
            {L("Başla", "Start")}
          </button>
        </div>
      </div>
      <HintBar hints={[{ action: "a", label: L("Başla", "Start") }]} />
    </>
  );
}

function Folder({ onNext, onBack }: { onNext: () => void; onBack: () => void }) {
  const scanning = useStore((s) => s.scanning);

  const [defaultRoot, setDefaultRoot] = useState<string | null>(null);
  useEffect(() => {
    api.defaultRomRoot().then(setDefaultRoot).catch(() => {});
  }, []);

  const useDefault = async () => {
    if (scanning) return;
    sfx.confirm();
    if (await actions.useDefaultRomRoot()) onNext();
  };
  const choose = async () => {
    if (scanning) return;
    sfx.confirm();
    if (await actions.chooseRomRoot()) onNext();
  };

  useInput((a) => {
    if (a === "a") useDefault();
    if (a === "x") choose();
    if (a === "b") {
      sfx.back();
      onBack();
    }
  }, Layer.screen);

  return (
    <>
      <div className="step center">
        <div className="step-count">{L("Adım 1 / 2", "Step 1 / 2")}</div>
        <h1>{L("Oyunların nerede dursun?", "Where should your games live?")}</h1>
        <p className="lead">
          {L(
            <>
              Masaüstünde <b>Lumen Oyunlar</b> klasörünü oluşturacağız. İçinde her sistemin kendi klasörü (<b>ps1</b>,{" "}
              <b>snes</b>, <b>gba</b>…) ve BIOS dosyaları için <b>bios</b> klasörü olacak. Oyun dosyasını ilgili klasöre
              bırakman yeterli; Lumen her açılışta klasörü yeniden tarar.
            </>,
            <>
              We'll create a <b>Lumen Oyunlar</b> folder on your desktop. Inside, each system gets its own folder (
              <b>ps1</b>, <b>snes</b>, <b>gba</b>…) plus a <b>bios</b> folder for BIOS files. Just drop a game file into
              the matching folder; Lumen rescans it every time it starts.
            </>,
          )}
        </p>
        {defaultRoot && <div className="path-chip">{defaultRoot}</div>}
        <div className="actions">
          <button className="btn primary focused" onClick={useDefault} disabled={scanning}>
            {scanning ? L("Klasör hazırlanıyor…", "Preparing folder…") : L("Masaüstünde oluştur", "Create on desktop")}
          </button>
          <button className="btn" onClick={choose} disabled={scanning}>
            {L("Başka klasör seç", "Choose another folder")}
          </button>
        </div>
      </div>
      <HintBar
        hints={[
          { action: "a", label: L("Masaüstünde oluştur", "Create on desktop") },
          { action: "x", label: L("Başka klasör seç", "Choose another folder") },
          { action: "b", label: L("Geri", "Back") },
        ]}
      />
    </>
  );
}

function Systems({ onInstall, onSkip }: { onInstall: (systemIds: string[]) => void; onSkip: () => void }) {
  const overview = useStore((s) => s.overview);
  const counts = useMemo(
    () => new Map(overview?.systems.map((s) => [s.id, s.gameCount]) ?? []),
    [overview],
  );
  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(systems.filter((s) => (counts.get(s.id) ?? 0) > 0).map((s) => s.id)),
  );
  // Index into chips, then the two buttons after them.
  const [focus, setFocus] = useState(systems.length);
  const gridRef = useRef<HTMLDivElement>(null);
  const actionsRef = useRef<HTMLDivElement>(null);
  const COLUMNS = useColumns(gridRef, 6);
  const buttonsStart = systems.length;
  const lastRowStart = Math.floor((systems.length - 1) / COLUMNS) * COLUMNS;

  useEffect(() => {
    const target = focus < buttonsStart ? gridRef.current?.children[focus] : actionsRef.current;
    target?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [focus, buttonsStart]);
  const found = [...counts.values()].reduce((a, b) => a + b, 0);

  const toggle = (id: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const install = async () => {
    if (selected.size === 0) {
      sfx.error();
      return;
    }
    sfx.confirm();
    rumble();
    onInstall([...selected]);
  };

  const move = (to: number) => {
    if (to === focus) return;
    sfx.move();
    setFocus(to);
  };

  useInput((a) => {
    const inGrid = focus < buttonsStart;
    switch (a) {
      case "left":
        if (inGrid && focus % COLUMNS > 0) move(focus - 1);
        else if (!inGrid && focus > buttonsStart) move(focus - 1);
        break;
      case "right":
        if (inGrid && focus % COLUMNS < COLUMNS - 1 && focus + 1 < buttonsStart) move(focus + 1);
        else if (!inGrid && focus < buttonsStart + 1) move(focus + 1);
        break;
      case "up":
        if (inGrid && focus >= COLUMNS) move(focus - COLUMNS);
        else if (!inGrid) move(lastRowStart);
        break;
      case "down":
        if (inGrid) move(focus + COLUMNS < buttonsStart ? focus + COLUMNS : buttonsStart);
        break;
      case "a":
        if (inGrid) {
          sfx.confirm();
          toggle(systems[focus].id);
        } else if (focus === buttonsStart) install();
        else {
          sfx.back();
          onSkip();
        }
        break;
      case "y":
        sfx.confirm();
        setSelected((prev) => (prev.size === systems.length ? new Set() : new Set(systems.map((s) => s.id))));
        break;
    }
  }, Layer.screen);

  return (
    <>
      <div className="step wide">
        <div className="step-count">{L("Adım 2 / 2", "Step 2 / 2")}</div>
        <h1>{L("Hangi sistemleri hazırlayalım?", "Which systems should we set up?")}</h1>
        <p className="lead" style={{ margin: 0 }}>
          {found > 0
            ? L(
                `${found} oyun bulduk; ilgili sistemleri seçtik.`,
                `We found ${found} ${found === 1 ? "game" : "games"} and selected their systems.`,
              )
            : L(
                "Henüz oyun bulunamadı. İstediklerini şimdi seç ya da atla; bir oyunu ilk açtığında emülatörü otomatik kurarız.",
                "No games found yet. Pick the systems you want now or skip; we'll install the emulator automatically the first time you open a game.",
              )}
        </p>

        <div className="system-grid" ref={gridRef}>
          {systems.map((s, i) => (
            <button
              key={s.id}
              className={`system-chip${selected.has(s.id) ? " selected" : ""}${focus === i ? " focused" : ""}`}
              style={{ "--chip-accent": s.accent } as CSSProperties}
              onClick={() => {
                setFocus(i);
                toggle(s.id);
              }}
              tabIndex={-1}
            >
              <span className="check">{selected.has(s.id) ? "✓" : ""}</span>
              <span style={{ minWidth: 0 }}>
                <div className="chip-name">{s.short}</div>
                <div className="chip-meta">{emulatorOf(s.emulator)?.name}</div>
              </span>
              {(counts.get(s.id) ?? 0) > 0 && <span className="chip-count">{counts.get(s.id)}</span>}
            </button>
          ))}
        </div>

        <div className="actions" ref={actionsRef}>
          <button
            className={`btn primary${focus === buttonsStart ? " focused" : ""}`}
            onClick={install}
            disabled={selected.size === 0}
          >
            {L("Kur", "Install")} ({selected.size})
          </button>
          <button className={`btn${focus === buttonsStart + 1 ? " focused" : ""}`} onClick={onSkip}>
            {L("Şimdilik atla", "Skip for now")}
          </button>
        </div>
      </div>
      <HintBar
        hints={[
          { action: "a", label: L("Seç", "Select") },
          { action: "y", label: selected.size === systems.length ? L("Hiçbiri", "None") : L("Hepsi", "All") },
        ]}
      />
    </>
  );
}

const STAGE_LABEL: Record<InstallProgress["stage"], string> = {
  downloading: L("İndiriliyor", "Downloading"),
  extracting: L("Açılıyor", "Extracting"),
  configuring: L("Ayarlanıyor", "Configuring"),
  copying: L("Kopyalanıyor", "Copying"),
  done: L("Hazır", "Ready"),
  error: L("Hata", "Error"),
};

function Install({ finished, onDone }: { finished: boolean; onDone: () => void }) {
  const installs = useStore((s) => s.installs);
  const overview = useStore((s) => s.overview);
  // Installs vanish from the store shortly after finishing; keep our own record.
  const [seen, setSeen] = useState<Record<string, InstallProgress>>({});

  useEffect(() => {
    setSeen((prev) => ({ ...prev, ...installs }));
  }, [installs]);

  const listRef = useRef<HTMLDivElement>(null);

  useInput((a) => {
    if (a === "a" && finished) {
      sfx.confirm();
      onDone();
    }
    if (a === "b" && !finished) {
      sfx.back();
      onDone();
    }
    // The list can outgrow the screen; let the stick/D-pad scroll it.
    if (a === "up" || a === "down") {
      listRef.current?.scrollBy({ top: (a === "up" ? -1 : 1) * 120, behavior: "smooth" });
    }
  }, Layer.screen);

  const rows = Object.values(seen);
  const completed = rows.filter((r) => r.stage === "done" || r.stage === "error").length;
  const active = rows.find((r) => r.stage !== "done" && r.stage !== "error");

  // Keep whatever is downloading in view.
  useEffect(() => {
    if (!active) return;
    const row = listRef.current?.querySelector(`[data-id="${CSS.escape(active.id)}"]`);
    row?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [active?.id]);
  const failed = rows.filter((r) => r.stage === "error").length;
  const missingBios = (overview?.systems ?? [])
    .filter((s) => s.gameCount > 0 && !s.usingFallback)
    .flatMap((s) => s.bios.filter((b) => !b.found && !b.optional).map((b) => ({ system: systemOf(s.id)?.short, ...b })));

  return (
    <>
      <div className="step install-step">
        <h1>{finished
            ? failed
              ? L("Neredeyse hazır", "Almost ready")
              : L("Hazırsın", "You're all set")
            : L("Emülatörler hazırlanıyor", "Setting up emulators")}</h1>
        <p className="lead" style={{ margin: 0 }}>
          {!overview?.platformSupported
            ? L(
                "Bu bir geliştirme önizlemesi; emülatör kurulumu Windows'ta yapılır.",
                "This is a development preview; emulators are installed on Windows.",
              )
            : finished
              ? failed
                ? L(
                    `${failed} kurulum tamamlanamadı. Menü › Emülatörler'den tekrar deneyebilirsin.`,
                    `${failed} ${failed === 1 ? "install" : "installs"} couldn't finish. You can retry from Menu › Emulators.`,
                  )
                : L(
                    "Her şey ayarlandı. Oyun eklemek için sistem klasörlerine dosya koyman yeterli.",
                    "Everything's set up. To add games, just put files in the system folders.",
                  )
              : L(
                  "Resmi kaynaklardan indirip senin için ayarlıyoruz. Bu ekranı kapatabilirsin; arkada devam eder.",
                  "Downloading from official sources and setting them up for you. You can close this screen; it continues in the background.",
                )}
        </p>

        {rows.length > 0 && (
          <div className="install-summary">
            <span>
              {completed} / {rows.length} {L("tamamlandı", "done")}
            </span>
            <ProgressBar value={rows.length ? completed / rows.length : 0} />
          </div>
        )}

        {rows.length > 0 && (
          <div className="install-list" ref={listRef}>
            {rows.map((p) => (
              <div className={`install-row ${p.stage}`} key={p.id} data-id={p.id}>
                <span>{installLabel(p.id)}</span>
                <ProgressBar
                  value={p.stage === "downloading" ? p.progress : 1}
                  indeterminate={p.stage === "downloading" && p.progress === 0}
                />
                <span className="state">{STAGE_LABEL[p.stage]}</span>
              </div>
            ))}
          </div>
        )}

        {finished && missingBios.length > 0 && (
          <div className="callout info" style={{ marginTop: rows.length ? 0 : "1.6rem" }}>
            <span>i</span>
            <div>
              {L("Bazı sistemler BIOS dosyası istiyor:", "Some systems need a BIOS file:")}{" "}
              <b>{missingBios.map((b) => `${b.system}: ${b.label}`).join(" · ")}</b>.{" "}
              {L(
                <>
                  Bu dosyaları kendi cihazından alıp <code>bios</code> klasörüne koyabilirsin.
                </>,
                <>
                  Dump them from your own console and put them in the <code>bios</code> folder.
                </>,
              )}
            </div>
          </div>
        )}

        <div className="actions" style={{ marginTop: "1.6rem" }}>
          <button className={`btn primary${finished ? " focused" : ""}`} onClick={onDone} disabled={!finished}>
            {L("Kütüphaneye git", "Go to library")}
          </button>
          {!finished && (
            <button className="btn" onClick={onDone}>
              {L("Arkada devam etsin", "Continue in background")}
            </button>
          )}
        </div>
      </div>
      <HintBar
        hints={
          finished
            ? [{ action: "a", label: L("Kütüphaneye git", "Go to library") }]
            : [{ action: "b", label: L("Arkada devam etsin", "Continue in background") }]
        }
      />
    </>
  );
}
