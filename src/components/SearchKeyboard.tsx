import { useEffect, useState } from "react";
import { L } from "../lib/i18n";
import { captureText, Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { Glyph } from "./Glyph";

const ROWS = ["ABCDEFGHIJ", "KLMNOPQRST", "UVWXYZ0123", "456789'-:."].map((r) => r.split(""));
const SPECIAL = ["Boşluk", "Sil", "Bitti"] as const;
/** What the special keys show (SPECIAL stays the key id). */
const SPECIAL_TEXT: Record<(typeof SPECIAL)[number], string> = {
  Boşluk: L("Boşluk", "Space"),
  Sil: L("Sil", "Delete"),
  Bitti: L("Bitti", "Done"),
};

/**
 * Gamepad on-screen keyboard. A physical keyboard types straight into it.
 * `onDone` leaves the keyboard but keeps the query; `onCancel` clears it.
 */
export function SearchKeyboard({
  query,
  results,
  onChange,
  onDone,
  onCancel,
}: {
  query: string;
  results: number;
  onChange: (query: string) => void;
  onDone: () => void;
  onCancel: () => void;
}) {
  // Row index ROWS.length is the special row.
  const [row, setRow] = useState(0);
  const [col, setCol] = useState(0);
  const rowLength = (r: number) => (r === ROWS.length ? SPECIAL.length : ROWS[r].length);

  const type = (text: string) => onChange((query + text).slice(0, 40));
  const erase = () => onChange(query.slice(0, -1));

  useEffect(() => {
    captureText((text) => (text === "backspace" ? erase() : type(text)));
    return () => captureText(null);
  });

  const move = (r: number, c: number) => {
    const nr = Math.max(0, Math.min(ROWS.length, r));
    const nc = Math.max(0, Math.min(rowLength(nr) - 1, c));
    if (nr !== row || nc !== col) sfx.move();
    setRow(nr);
    setCol(nc);
  };

  const press = () => {
    sfx.confirm();
    if (row < ROWS.length) return type(ROWS[row][col].toLowerCase());
    if (SPECIAL[col] === "Boşluk") type(" ");
    else if (SPECIAL[col] === "Sil") erase();
    else onDone();
  };

  useInput((action) => {
    switch (action) {
      case "up":
        // Special row is shorter; map columns proportionally.
        return move(row - 1, row === ROWS.length ? col * 4 : col);
      case "down":
        return move(row + 1, row === ROWS.length - 1 ? Math.min(2, Math.floor(col / 4)) : col);
      case "left":
        return move(row, col - 1);
      case "right":
        return move(row, col + 1);
      case "a":
        return press();
      case "y":
        sfx.confirm();
        return type(" ");
      case "x":
        sfx.back();
        return erase();
      case "b":
        sfx.back();
        return query ? erase() : onCancel();
      case "start":
        sfx.confirm();
        return onDone();
    }
  }, Layer.sheet);

  return (
    <>
      <div className="scrim" onClick={onDone} />
      <section className="search-panel">
        <div className="search-field">
          <span className="search-icon">⌕</span>
          <span className={query ? "" : "placeholder"}>{query || L("Oyun ara", "Search games")}</span>
          <span className="caret" />
          <span className="search-count">{query ? L(`${results} sonuç`, `${results} ${results === 1 ? "result" : "results"}`) : ""}</span>
        </div>
        <div className="keys">
          {ROWS.map((keys, r) => (
            <div className="key-row" key={r}>
              {keys.map((key, c) => (
                <button
                  key={key}
                  className={`kb-key${r === row && c === col ? " focused" : ""}`}
                  onClick={() => type(key.toLowerCase())}
                  tabIndex={-1}
                >
                  {key}
                </button>
              ))}
            </div>
          ))}
          <div className="key-row special">
            {SPECIAL.map((label, c) => (
              <button
                key={label}
                className={`kb-key wide${row === ROWS.length && c === col ? " focused" : ""}${label === "Bitti" ? " done" : ""}`}
                onClick={() => (label === "Boşluk" ? type(" ") : label === "Sil" ? erase() : onDone())}
                tabIndex={-1}
              >
                {SPECIAL_TEXT[label]}
              </button>
            ))}
          </div>
        </div>
        <div className="search-hints">
          <span><Glyph action="x" /> {L("Sil", "Delete")}</span>
          <span><Glyph action="y" /> {L("Boşluk", "Space")}</span>
          <span><Glyph action="start" /> {L("Bitti", "Done")}</span>
          <span><Glyph action="b" /> {query ? L("Sil", "Delete") : L("Kapat", "Close")}</span>
        </div>
      </section>
    </>
  );
}
