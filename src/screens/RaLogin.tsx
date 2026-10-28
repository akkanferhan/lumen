import { useState } from "react";
import { TextKeyboard } from "../components/TextKeyboard";
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
        title="RetroAchievements · kullanıcı adı"
        placeholder="Kullanıcı adı"
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
      title={`RetroAchievements · ${username} için şifre`}
      placeholder="Şifre"
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
