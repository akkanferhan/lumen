import { useState } from "react";
import { HintBar } from "../components/HintBar";
import { TextKeyboard } from "../components/TextKeyboard";
import { L } from "../lib/i18n";
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
        title={L("Steam Web API anahtarı", "Steam Web API key")}
        placeholder={L("32 karakterlik anahtar", "32-character key")}
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
        <h2>{L("Steam kütüphanesi", "Steam library")}</h2>
        <p className="menu-sub">
          {L(
            "Steam'de sahip olduğun ama yüklü olmayan oyunlar Steam sekmesinde görünür; birini seçince Steam kurulum penceresi açılır.",
            "Games you own on Steam but haven't installed show up in the Steam tab; pick one to open Steam's install window.",
          )}
        </p>
        {!account ? (
          <div className="callout">
            <span>!</span>
            <div>
              {L(
                "Bu bilgisayarda Steam'e giriş yapılmış bir hesap bulunamadı. Önce Steam uygulamasında oturum aç.",
                "No Steam account is signed in on this PC. Sign in to the Steam app first.",
              )}
            </div>
          </div>
        ) : linked ? (
          <div className="callout info">
            <span>✓</span>
            <div>{L("Steam hesabın bağlı. Liste her açılışta güncellenir.", "Your Steam account is linked. The list updates every time Lumen opens.")}</div>
          </div>
        ) : (
          <ol className="steam-steps">
            <li>
              {L(
                <>
                  Telefonundan ya da bilgisayardan <span className="path-chip">steamcommunity.com/dev/apikey</span>{" "}
                  adresini aç ve Steam hesabınla giriş yap.
                </>,
                <>
                  On your phone or a computer, open <span className="path-chip">steamcommunity.com/dev/apikey</span> and
                  sign in with your Steam account.
                </>,
              )}
            </li>
            <li>
              {L(
                <>Alan adı olarak herhangi bir şey yaz (ör. <b>lumen</b>) ve anahtarı oluştur.</>,
                <>Enter anything as the domain name (e.g. <b>lumen</b>) and create the key.</>,
              )}
            </li>
            <li>
              {L(
                "Gösterilen 32 karakterlik anahtarı buraya gir. Anahtar yalnızca bu bilgisayarda saklanır, yedeklere girmez.",
                "Enter the 32-character key shown here. It's stored only on this PC and never goes into backups.",
              )}
            </li>
            <li>
              {L(
                <>Liste boş gelirse Steam profilinde <b>Gizlilik → Oyun ayrıntıları</b>'nı "Herkese açık" yap.</>,
                <>If the list comes up empty, set <b>Privacy → Game details</b> to "Public" in your Steam profile.</>,
              )}
            </li>
          </ol>
        )}
      </aside>
      <HintBar
        hints={[
          ...(account ? [{ action: "a" as const, label: linked ? L("Anahtarı değiştir", "Change key") : L("Anahtarı gir", "Enter key") }] : []),
          ...(linked ? [{ action: "x" as const, label: L("Bağlantıyı kaldır", "Unlink") }] : []),
          { action: "b", label: L("Kapat", "Close") },
        ]}
      />
    </>
  );
}
