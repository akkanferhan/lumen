import type { ReactNode } from "react";
import type { Action } from "../lib/input";
import { Glyph } from "./Glyph";

export interface Hint {
  action: Action;
  label: string;
}

export function HintBar({ hints, left }: { hints: Hint[]; left?: ReactNode }) {
  return (
    <footer className="hintbar">
      <div className="hintbar-left">{left}</div>
      {hints.map((h) => (
        <span className="hint" key={h.action + h.label}>
          <Glyph action={h.action} />
          {h.label}
        </span>
      ))}
    </footer>
  );
}
