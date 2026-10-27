import { useEffect, useState } from "react";
import { captureText, Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { Glyph } from "./Glyph";

const ROWS = ["ABCDEFGHIJ", "KLMNOPQRST", "UVWXYZ0123", "456789'-:."].map((r) => r.split(""));
const SPECIAL = ["Boşluk", "Sil", "Bitti"] as const;

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
          <span className={query ? "" : "placeholder"}>{query || "Oyun ara"}</span>
          <span className="caret" />
          <span className="search-count">{query ? `${results} sonuç` : ""}</span>
        </div>
        <div className="keys">
          {ROWS.map((keys, r) => (
            <div className="key-row" key={r}>
              {keys.map((key, c) => (
                <button
                  key={key}
                  className={`key${r === row && c === col ? " focused" : ""}`}
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
                className={`key wide${row === ROWS.length && c === col ? " focused" : ""}${label === "Bitti" ? " done" : ""}`}
                onClick={() => (label === "Boşluk" ? type(" ") : label === "Sil" ? erase() : onDone())}
                tabIndex={-1}
              >
                {label}
              </button>
            ))}
          </div>
        </div>
        <div className="search-hints">
          <span><Glyph action="x" /> Sil</span>
          <span><Glyph action="y" /> Boşluk</span>
          <span><Glyph action="start" /> Bitti</span>
          <span><Glyph action="b" /> {query ? "Sil" : "Kapat"}</span>
        </div>
      </section>
    </>
  );
}
