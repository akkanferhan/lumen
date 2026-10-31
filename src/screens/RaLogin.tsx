import { useState } from "react";
import { TextKeyboard } from "../components/TextKeyboard";
import { L } from "../lib/i18n";
import { actions } from "../lib/store";

/** User name, then password; the password is sent once and never stored. */
export function RaLogin({ onClose }: { onClose: () => void }) {
  const [step, setStep] = useState<"user" | "password">("user");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);

  if (step === "user") {
    return (
      <TextKeyboard
        key="user"
        title={L("RetroAchievements · kullanıcı adı", "RetroAchievements · username")}
        placeholder={L("Kullanıcı adı", "Username")}
        value={username}
        onChange={setUsername}
        onDone={() => username.trim() && setStep("password")}
        onCancel={onClose}
      />
    );
  }

  return (
    <TextKeyboard
      key="password"
      title={L(`RetroAchievements · ${username} için şifre`, `RetroAchievements · password for ${username}`)}
      placeholder={L("Şifre", "Password")}
      value={password}
      secret
      busy={busy}
      onChange={setPassword}
      onDone={async () => {
        if (!password) return;
        setBusy(true);
        const ok = await actions.raLogin(username.trim(), password);
        setBusy(false);
        setPassword("");
        if (ok) onClose();
      }}
      onCancel={() => setStep("user")}
    />
  );
}
