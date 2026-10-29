import { useState } from "react";
import { HintBar } from "./HintBar";
import { TextKeyboard } from "./TextKeyboard";
import { Layer, useInput } from "../lib/input";
import { sfx } from "../lib/sfx";
import { actions, getState, useStore } from "../lib/store";

/** Puts a game in (or takes it out of) the user's collections; new ones are named on the pad keyboard. */
export function CollectionPicker({ gameId, title, onClose }: { gameId: string; title: string; onClose: () => void }) {
  const collections = useStore((s) => s.overview?.collections ?? []);
  const [focus, setFocus] = useState(0);
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState("");
  // Deleting needs a second X on the same row.
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const rows = collections.length + 1; // last row: new collection
  const index = Math.min(focus, rows - 1);
  const current = collections[index];

  const create = async () => {
    if (!name.trim()) return;
    if (await actions.createCollection(name)) {
      // The hook's value is a render behind; the store already has the new list.
      const created = (getState().overview?.collections ?? []).find(
        (c) => c.name.toLowerCase() === name.trim().toLowerCase(),
      );
      if (created) await actions.toggleCollection(created.id, gameId);
      setName("");
      setNaming(false);
    }
  };

  useInput(
    (action) => {
      switch (action) {
        case "up":
        case "down": {
          const next = Math.max(0, Math.min(rows - 1, index + (action === "up" ? -1 : 1)));
          if (next !== index) {
            sfx.move();
            setFocus(next);
            setConfirmDelete(null);
          }
          break;
        }
        case "a":
          sfx.confirm();
          if (current) actions.toggleCollection(current.id, gameId);
          else setNaming(true);
          break;
        case "x":
          if (!current) break;
          if (confirmDelete === current.id) {
            sfx.back();
            actions.deleteCollection(current.id);
            setConfirmDelete(null);
          } else {
            sfx.confirm();
            setConfirmDelete(current.id);
          }
          break;
        case "b":
          sfx.back();
          onClose();
          break;
      }
    },
    Layer.menu,
    !naming,
  );

  return (
    <>
      <div className="scrim" onClick={onClose} />
      <aside className="menu" hidden={naming}>
        <h2>Koleksiyonlar</h2>
        <p className="menu-sub">{title} hangi listelerde olsun? Her koleksiyon kütüphanede kendi sekmesi olur.</p>
        <div className="menu-list">
          {collections.map((c, i) => {
            const inIt = c.games.includes(gameId);
            return (
              <button
                key={c.id}
                className={`menu-item${i === index ? " focused" : ""}`}
                onClick={() => actions.toggleCollection(c.id, gameId)}
                tabIndex={-1}
              >
                <div className="menu-item-main">
                  <div className="menu-item-title">{confirmDelete === c.id ? `“${c.name}” silinsin mi? X ile onayla` : c.name}</div>
                  <div className="menu-item-sub">{c.games.length} oyun</div>
                </div>
                <span className="menu-item-value">{inIt ? <span className="pill ok">✓ Bu listede</span> : "Ekle"}</span>
              </button>
            );
          })}
          <button
            className={`menu-item${index === collections.length ? " focused" : ""}`}
            onClick={() => setNaming(true)}
            tabIndex={-1}
          >
            <div className="menu-item-main">
              <div className="menu-item-title">+ Yeni koleksiyon</div>
              <div className="menu-item-sub">Örneğin “RPG'ler”, “Bitirdiklerim”, “Çocuklar için”</div>
            </div>
          </button>
        </div>
      </aside>
      {naming ? (
        <TextKeyboard
          title="Koleksiyonun adı"
          placeholder="ör. RPG'ler"
          value={name}
          onChange={setName}
          onDone={create}
          onCancel={() => {
            setName("");
            setNaming(false);
          }}
        />
      ) : (
        <HintBar
          hints={[
            { action: "a", label: current ? (current.games.includes(gameId) ? "Listeden çıkar" : "Listeye ekle") : "Oluştur" },
            ...(current ? [{ action: "x" as const, label: confirmDelete === current.id ? "Onayla" : "Sil" }] : []),
            { action: "b", label: "Bitti" },
          ]}
        />
      )}
    </>
  );
}

