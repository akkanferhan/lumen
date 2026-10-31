import { useState } from "react";
import { HintBar } from "../components/HintBar";
import { TextKeyboard } from "../components/TextKeyboard";
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
        title="SteamGridDB API anahtarı"
        placeholder="32 karakterlik anahtar"
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
        <h2>Kapak görselleri</h2>
        <p className="menu-sub">
          Lumen kapakları libretro arşivinden alır. Arşivde olmayan oyunlar (homebrew, yamalar, farklı adlandırılmış
          dosyalar) için SteamGridDB'den kapak indirilebilir.
        </p>
        {linked ? (
          <div className="callout info">
            <span>✓</span>
            <div>SteamGridDB bağlı. Eksik kapaklar kütüphane her yenilendiğinde indirilir ve bu bilgisayarda saklanır.</div>
          </div>
        ) : (
          <ol className="steam-steps">
            <li>
              Telefonundan ya da bilgisayardan <span className="path-chip">steamgriddb.com</span> adresinde ücretsiz bir
              hesapla (Steam ile) giriş yap.
            </li>
            <li>
              <b>Preferences → API</b> sayfasında anahtar oluştur:{" "}
              <span className="path-chip">steamgriddb.com/profile/preferences/api</span>
            </li>
            <li>Anahtarı buraya gir. Yalnızca bu bilgisayarda saklanır, yedeklere girmez.</li>
          </ol>
        )}
      </aside>
      <HintBar
        hints={[
          { action: "a", label: linked ? "Anahtarı değiştir" : "Anahtarı gir" },
          ...(linked ? [{ action: "x" as const, label: "Bağlantıyı kaldır" }] : []),
          { action: "b", label: "Kapat" },
        ]}
      />
    </>
  );
}
