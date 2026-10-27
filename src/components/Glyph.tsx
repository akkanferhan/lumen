import { useEffect, useState } from "react";
import { onInputSource, type Action } from "../lib/input";

const PAD: Partial<Record<Action, string>> = {
  a: "A", b: "B", x: "X", y: "Y", lb: "LB", rb: "RB", lt: "LT", rt: "RT", start: "☰", select: "⧉",
};
const KEYS: Partial<Record<Action, string>> = {
  a: "Enter", b: "Esc", x: "X", y: "F", lb: "Q", rb: "E", lt: "PgUp", rt: "PgDn", start: "Tab", select: "S",
};

let current: "pad" | "keys" = "keys";

export function useInputSource() {
  const [source, setSource] = useState(current);
  useEffect(
    () =>
      onInputSource((s) => {
        current = s;
        setSource(s);
      }),
    [],
  );
  return source;
}

// Hide the cursor while the gamepad is in use.
onInputSource((s) => document.body.classList.toggle("pad-mode", s === "pad"));

export function Glyph({ action }: { action: Action }) {
  const source = useInputSource();
  if (source === "keys") return <span className="glyph key">{KEYS[action]}</span>;
  const face = action === "a" || action === "b" || action === "x" || action === "y";
  return <span className={`glyph ${face ? action : "pill"}`}>{PAD[action]}</span>;
}
