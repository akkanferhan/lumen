import { useEffect, useState } from "react";
import { L } from "../lib/i18n";
import { captureText, Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { Glyph } from "./Glyph";

const LETTERS = ["1234567890", "qwertyuiop", "asdfghjkl-", "zxcvbnm_.@"].map((r) => r.split(""));
const SYMBOLS = ["1234567890", "!#$%&*+=?/", "()[]{}<>:;", "~^'\",|\\_.@"].map((r) => r.split(""));
const SPECIAL = ["Aa", "#+=", "Sil", "Bitti"] as const;
/** What the special keys show (SPECIAL stays the key id). */
const SPECIAL_TEXT: Record<(typeof SPECIAL)[number], string> = {
  Aa: "Aa",
  "#+=": "#+=",
  Sil: L("Sil", "Delete"),
  Bitti: L("Bitti", "Done"),
};

/**
 * Gamepad keyboard for free text (user names, passwords). Case and symbols
 * toggle; a physical keyboard types straight in. `secret` masks the value.
 */
export function TextKeyboard({
  title,
  placeholder,
  value,
  secret = false,
  busy = false,
  onChange,
  onDone,
  onCancel,
}: {
  title: string;
  placeholder: string;
  value: string;
  secret?: boolean;
  busy?: boolean;
  onChange: (value: string) => void;
  onDone: () => void;
  onCancel: () => void;
}) {
  const [row, setRow] = useState(1);
  const [col, setCol] = useState(0);
  const [upper, setUpper] = useState(false);
  const [symbols, setSymbols] = useState(false);
  const rows = symbols ? SYMBOLS : LETTERS;
  const rowLength = (r: number) => (r === rows.length ? SPECIAL.length : rows[r].length);
  const shown = (key: string) => (upper && !symbols ? key.toUpperCase() : key);

  const type = (text: string) => !busy && onChange((value + text).slice(0, 64));
  const erase = () => !busy && onChange(value.slice(0, -1));

  useEffect(() => {
    captureText((text) => (text === "backspace" ? erase() : type(text)));
    return () => captureText(null);
  });

  const move = (r: number, c: number) => {
    const nr = Math.max(0, Math.min(rows.length, r));
    const nc = Math.max(0, Math.min(rowLength(nr) - 1, c));
    if (nr !== row || nc !== col) sfx.move();
    setRow(nr);
    setCol(nc);
  };

  const press = () => {
    sfx.confirm();
    if (row < rows.length) return type(shown(rows[row][col]));
    const key = SPECIAL[col];
    if (key === "Aa") setUpper((u) => !u);
    else if (key === "#+=") setSymbols((s) => !s);
    else if (key === "Sil") erase();
    else if (!busy) onDone();
  };

  useInput((action) => {
    switch (action) {
      case "up":
        return move(row - 1, row === rows.length ? Math.round(col * 3) : col);
      case "down":
        return move(row + 1, row === rows.length - 1 ? Math.min(3, Math.floor(col / 3)) : col);
      case "left":
        return move(row, col - 1);
      case "right":
        return move(row, col + 1);
      case "a":
        return press();
      case "x":
        sfx.back();
        return erase();
      case "y":
        sfx.confirm();
        return setUpper((u) => !u);
      case "b":
        sfx.back();
        return value ? erase() : onCancel();
      case "start":
        sfx.confirm();
        return !busy && onDone();
    }
  }, Layer.running + 5);

  return (
    <>
      <div className="scrim" onClick={onCancel} />
      <section className="search-panel text-panel">
        <h3 className="text-title">{title}</h3>
        <div className="search-field">
          <span className={value ? "" : "placeholder"}>{value ? (secret ? "•".repeat(value.length) : value) : placeholder}</span>
          <span className="caret" />
          <span className="search-count">{busy ? L("Bağlanıyor…", "Connecting…") : ""}</span>
        </div>
        <div className="keys">
          {rows.map((keys, r) => (
            <div className="key-row" key={r}>
              {keys.map((key, c) => (
                <button
                  key={key + c}
                  className={`kb-key${r === row && c === col ? " focused" : ""}`}
                  onClick={() => type(shown(key))}
                  tabIndex={-1}
                >
                  {shown(key)}
                </button>
              ))}
            </div>
          ))}
          <div className="key-row special four">
            {SPECIAL.map((label, c) => (
              <button
                key={label}
                className={`kb-key wide${row === rows.length && c === col ? " focused" : ""}${label === "Bitti" ? " done" : ""}${
                  (label === "Aa" && upper) || (label === "#+=" && symbols) ? " on" : ""
                }`}
                onClick={() => {
                  setRow(rows.length);
                  setCol(c);
                  if (label === "Aa") setUpper((u) => !u);
                  else if (label === "#+=") setSymbols((s) => !s);
                  else if (label === "Sil") erase();
                  else if (!busy) onDone();
                }}
                tabIndex={-1}
              >
                {SPECIAL_TEXT[label]}
              </button>
            ))}
          </div>
        </div>
        <div className="search-hints">
          <span><Glyph action="y" /> {L("Büyük harf", "Caps")}</span>
          <span><Glyph action="x" /> {L("Sil", "Delete")}</span>
          <span><Glyph action="start" /> {L("Bitti", "Done")}</span>
          <span><Glyph action="b" /> {value ? L("Sil", "Delete") : L("İptal", "Cancel")}</span>
        </div>
      </section>
    </>
  );
}
