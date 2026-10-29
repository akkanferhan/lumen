import type { CSSProperties } from "react";
import { emulatorOf, homebrew, systemOf } from "../lib/catalog";
import { useStore } from "../lib/store";

/** Systems whose emulators are early: most games don't run yet. */
export const EXPERIMENTAL = new Set(["ps4", "ps5", "x360"]);

/** Shown in a system's tab while it has no games: what to put where. */
export function SystemGuide({ systemId, onImport, onStore }: { systemId: string; onImport: () => void; onStore: () => void }) {
  const overview = useStore((s) => s.overview);
  const system = systemOf(systemId);
  if (!system) return null;
  const status = overview?.systems.find((s) => s.id === systemId);
  const root = overview?.settings.romRoot ?? "";
  const slash = root.includes("\\") ? "\\" : "/";
  const folder = root ? `${root}${slash}${system.folders[0]}` : system.folders[0];
  const missing = status?.bios.filter((b) => !b.found && !b.optional) ?? [];
  const emulator = emulatorOf(status?.usingFallback && system.fallback ? system.fallback.emulator : system.emulator);
  const free = homebrew.filter((g) => g.system === systemId).length;

  const format = system.folderMarker
    ? `Oyun klasörü (içinde ${system.launchFile ?? system.folderMarker} olan)`
    : system.extensions
        .filter((e) => e !== "zip" && e !== "7z")
        .map((e) => `.${e}`)
        .join("  ") + (system.extensions.includes("zip") ? "  ya da bunların .zip hali" : "");
  const bios = missing.length
    ? status?.usingFallback
      ? "Gerekmez: yerleşik BIOS ile çalışır"
      : `Gerekli: ${missing.map((b) => b.label).join(", ")}`
    : system.bios?.length
      ? "Bulundu"
      : "Gerekmez";
  const ready = status?.emulatorInstalled && status.coreInstalled;

  return (
    <div className="system-guide" style={{ "--accent": system.accent } as CSSProperties}>
      <div className="eyebrow">
        {system.maker} · {system.year}
        {EXPERIMENTAL.has(systemId) && <span className="pill warn">Deneysel</span>}
      </div>
      <h2>{system.name} için henüz oyun yok</h2>
      <dl className="guide-facts">
        <dt>Oyunları koy</dt>
        <dd>
          <span className="path-chip">{folder}</span>
        </dd>
        <dt>Dosya biçimi</dt>
        <dd>{format}</dd>
        <dt>BIOS</dt>
        <dd className={missing.length && !status?.usingFallback ? "warn" : ""}>{bios}</dd>
        <dt>Emülatör</dt>
        <dd>
          {emulator?.name} · {ready ? "kurulu" : "ilk oyunda kendiliğinden kurulur"}
        </dd>
      </dl>
      {EXPERIMENTAL.has(systemId) && (
        <p className="guide-note">Bu sistemin emülatörü erken aşamada: çoğu oyun henüz açılmayabilir.</p>
      )}
      <div className="actions">
        <button className="btn primary focused" onClick={onImport}>
          Oyunlarını içe aktar
        </button>
        {free > 0 && (
          <button className="btn" onClick={onStore}>
            {free} ücretsiz oyun
          </button>
        )}
      </div>
    </div>
  );
}
