import { installLabel } from "../lib/catalog";
import { L } from "../lib/i18n";
import { useStore } from "../lib/store";
import type { InstallStage } from "../lib/types";

const STAGE: Record<InstallStage, string> = {
  downloading: L("İndiriliyor", "Downloading"),
  extracting: L("Açılıyor", "Extracting"),
  configuring: L("Ayarlanıyor", "Configuring"),
  copying: L("Kopyalanıyor", "Copying"),
  done: L("Hazır", "Ready"),
  error: L("Hata", "Error"),
};

export function Toasts({ hideInstalls = false }: { hideInstalls?: boolean }) {
  const notices = useStore((s) => s.notices);
  const installs = useStore((s) => s.installs);

  return (
    <div className="toasts" aria-live="polite">
      {!hideInstalls &&
        Object.values(installs).map((p) => (
          <div className={`toast${p.stage === "done" ? " success" : p.stage === "error" ? " error" : ""}`} key={p.id}>
            <div className="toast-head">
              <span>{installLabel(p.id)}</span>
              <span>
                {STAGE[p.stage]}
                {(p.stage === "downloading" || p.stage === "copying") && p.progress > 0
                  ? L(` · %${Math.round(p.progress * 100)}`, ` · ${Math.round(p.progress * 100)}%`)
                  : ""}
              </span>
            </div>
            <ProgressBar
              value={p.stage === "downloading" || p.stage === "copying" ? p.progress : 1}
              indeterminate={(p.stage === "downloading" || p.stage === "copying") && p.progress === 0}
            />
          </div>
        ))}
      {notices.map((n) => (
        <div className={`toast ${n.tone}`} key={n.id}>
          {n.text}
        </div>
      ))}
    </div>
  );
}

export function ProgressBar({ value, indeterminate = false }: { value: number; indeterminate?: boolean }) {
  return (
    <div className={`bar${indeterminate ? " indeterminate" : ""}`}>
      <div className="bar-fill" style={{ width: `${Math.min(1, Math.max(0, value)) * 100}%` }} />
    </div>
  );
}
