import { useState } from "react";
import { HintBar } from "../components/HintBar";
import { TextKeyboard } from "../components/TextKeyboard";
import { L } from "../lib/i18n";
import { Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, useStore } from "../lib/store";

/**
 * Links a SteamGridDB account so games libretro has no box art for
 * (homebrew, hacks, loosely named files) get a cover. The key is checked
 * before it's saved, and kept out of backups.
 */
export function ArtLink({ onClose }: { onClose: () => void }) {
  const overview = useStore((s) => s.overview);
  const [typing, setTyping] = useState(false);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const linked = !!overview?.sgdbLinked;

  useInput(
    (action) => {
      if (action === "a") {
        sfx.confirm();
        setTyping(true);
      } else if (action === "x" && linked) {
        sfx.back();
        actions.setSgdbKey("").then(onClose);
      } else if (action === "b") {
        sfx.back();
        onClose();
      }
    },
    Layer.menu,
    !typing,
  );

  if (typing) {
    return (
      <TextKeyboard
        title={L("SteamGridDB API anahtarı", "SteamGridDB API key")}
        placeholder={L("32 karakterlik anahtar", "32-character key")}
        value={key}
        busy={busy}
        onChange={(v) => setKey(v.trim())}
        onDone={async () => {
          if (!key.trim()) return;
          setBusy(true);
          const ok = await actions.setSgdbKey(key);
          setBusy(false);
          if (ok) onClose();
        }}
        onCancel={() => setTyping(false)}
      />
    );
  }

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="menu steam-link">
        <h2>{L("Kapak görselleri", "Cover art")}</h2>
        <p className="menu-sub">
          {L(
            "Lumen kapakları libretro arşivinden alır. Arşivde olmayan oyunlar (homebrew, yamalar, farklı adlandırılmış dosyalar) için SteamGridDB'den kapak indirilebilir.",
            "Lumen gets covers from the libretro archive. For games it doesn't have (homebrew, hacks, oddly named files), covers can be downloaded from SteamGridDB.",
          )}
        </p>
        {linked ? (
          <div className="callout info">
            <span>✓</span>
            <div>
              {L(
                "SteamGridDB bağlı. Eksik kapaklar kütüphane her yenilendiğinde indirilir ve bu bilgisayarda saklanır.",
                "SteamGridDB is linked. Missing covers download every time the library refreshes and are stored on this PC.",
              )}
            </div>
          </div>
        ) : (
          <ol className="steam-steps">
            <li>
              {L(
                <>
                  Telefonundan ya da bilgisayardan <span className="path-chip">steamgriddb.com</span> adresinde ücretsiz
                  bir hesapla (Steam ile) giriş yap.
                </>,
                <>
                  On your phone or a computer, sign in at <span className="path-chip">steamgriddb.com</span> with a free
                  account (via Steam).
                </>,
              )}
            </li>
            <li>
              {L(
                <>
                  <b>Preferences → API</b> sayfasında anahtar oluştur:{" "}
                </>,
                <>
                  Create a key on the <b>Preferences → API</b> page:{" "}
                </>,
              )}
              <span className="path-chip">steamgriddb.com/profile/preferences/api</span>
            </li>
            <li>
              {L(
                "Anahtarı buraya gir. Yalnızca bu bilgisayarda saklanır, yedeklere girmez.",
                "Enter the key here. It's stored only on this PC and never goes into backups.",
              )}
            </li>
          </ol>
        )}
      </aside>
      <HintBar
        hints={[
          { action: "a", label: linked ? L("Anahtarı değiştir", "Change key") : L("Anahtarı gir", "Enter key") },
          ...(linked ? [{ action: "x" as const, label: L("Bağlantıyı kaldır", "Unlink") }] : []),
          { action: "b", label: L("Kapat", "Close") },
        ]}
      />
    </>
  );
}
