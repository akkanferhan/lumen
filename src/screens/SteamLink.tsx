import { useState } from "react";
import { HintBar } from "../components/HintBar";
import { TextKeyboard } from "../components/TextKeyboard";
import { Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, useStore } from "../lib/store";

/**
 * Links the Steam account signed in on this PC with a Steam Web API key, so
 * owned games that aren't installed show in the Steam tab. The key is
 * checked before it's saved, and kept out of backups.
 */
export function SteamLink({ onClose }: { onClose: () => void }) {
  const overview = useStore((s) => s.overview);
  const [typing, setTyping] = useState(false);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const linked = !!overview?.steamLinked;
  const account = !!overview?.steamAccount;

  useInput(
    (action) => {
      if (action === "a" && account) {
        sfx.confirm();
        setTyping(true);
      } else if (action === "x" && linked) {
        sfx.back();
        actions.setSteamKey("").then(onClose);
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
        title="Steam Web API anahtarı"
        placeholder="32 karakterlik anahtar"
        value={key}
        busy={busy}
        onChange={(v) => setKey(v.toUpperCase())}
        onDone={async () => {
          if (!key.trim()) return;
          setBusy(true);
          const ok = await actions.setSteamKey(key);
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
        <h2>Steam kütüphanesi</h2>
        <p className="menu-sub">
          Steam'de sahip olduğun ama yüklü olmayan oyunlar Steam sekmesinde görünür; birini seçince Steam kurulum
          penceresi açılır.
        </p>
        {!account ? (
          <div className="callout">
            <span>!</span>
            <div>Bu bilgisayarda Steam'e giriş yapılmış bir hesap bulunamadı. Önce Steam uygulamasında oturum aç.</div>
          </div>
        ) : linked ? (
          <div className="callout info">
            <span>✓</span>
            <div>Steam hesabın bağlı. Liste her açılışta güncellenir.</div>
          </div>
        ) : (
          <ol className="steam-steps">
            <li>
              Telefonundan ya da bilgisayardan <span className="path-chip">steamcommunity.com/dev/apikey</span> adresini aç
              ve Steam hesabınla giriş yap.
            </li>
            <li>Alan adı olarak herhangi bir şey yaz (ör. <b>lumen</b>) ve anahtarı oluştur.</li>
            <li>Gösterilen 32 karakterlik anahtarı buraya gir. Anahtar yalnızca bu bilgisayarda saklanır, yedeklere girmez.</li>
            <li>
              Liste boş gelirse Steam profilinde <b>Gizlilik → Oyun ayrıntıları</b>'nı "Herkese açık" yap.
            </li>
          </ol>
        )}
      </aside>
      <HintBar
        hints={[
          ...(account ? [{ action: "a" as const, label: linked ? "Anahtarı değiştir" : "Anahtarı gir" }] : []),
          ...(linked ? [{ action: "x" as const, label: "Bağlantıyı kaldır" }] : []),
          { action: "b", label: "Kapat" },
        ]}
      />
    </>
  );
}
