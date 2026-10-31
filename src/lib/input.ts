// Unified input: gamepad (polled), keyboard and a layered handler stack.
// Only the topmost active layer receives actions, so an open overlay never
// leaks presses to the screen beneath it.

import { useEffect, useRef } from "react";

export type Action =
  | "up"
  | "down"
  | "left"
  | "right"
  | "a"
  | "b"
  | "x"
  | "y"
  | "lb"
  | "rb"
  | "lt"
  | "rt"
  | "start"
  | "select";

/** Higher layers win. */
export const Layer = { screen: 0, sheet: 10, menu: 20, running: 30, loading: 40 } as const;

type Handler = (action: Action) => void;
interface Entry {
  layer: number;
  order: number;
  handler: { current: Handler };
}

const entries: Entry[] = [];
let order = 0;
const listeners = new Set<(source: "pad" | "keys") => void>();

function dispatch(action: Action) {
  let top: Entry | undefined;
  for (const e of entries) {
    if (!top || e.layer > top.layer || (e.layer === top.layer && e.order > top.order)) top = e;
  }
  top?.handler.current(action);
}

export function useInput(handler: Handler, layer: number, enabled = true) {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(() => {
    if (!enabled) return;
    const entry: Entry = { layer, order: ++order, handler: ref };
    entries.push(entry);
    return () => {
      entries.splice(entries.indexOf(entry), 1);
    };
  }, [layer, enabled]);
}

/** Notified whenever the active input device changes (used to swap glyphs / hide cursor). */
export function onInputSource(cb: (source: "pad" | "keys") => void): () => void {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}

let source: "pad" | "keys" = "keys";
function setSource(next: "pad" | "keys") {
  if (next === source) return;
  source = next;
  listeners.forEach((cb) => cb(next));
}

// ---- Gamepad (standard mapping; the ROG Ally reports as an Xbox controller) ----

const BUTTONS: Partial<Record<number, Action>> = {
  0: "a", 1: "b", 2: "x", 3: "y", 4: "lb", 5: "rb", 6: "lt", 7: "rt", 8: "select", 9: "start",
  12: "up", 13: "down", 14: "left", 15: "right",
};
const REPEATABLE = new Set<Action>(["up", "down", "left", "right"]);
const FIRST_REPEAT_MS = 380;
const REPEAT_MS = 85;
const STICK_DEADZONE = 0.55;

const held = new Map<Action, number>(); // action → time of next repeat

function poll(now: number) {
  const pressed = new Set<Action>();
  if (document.hasFocus()) {
    for (const pad of navigator.getGamepads?.() ?? []) {
      if (!pad) continue;
      pad.buttons.forEach((b, i) => {
        const action = BUTTONS[i];
        if (action && b.pressed) pressed.add(action);
      });
      const [x = 0, y = 0] = pad.axes;
      if (x < -STICK_DEADZONE) pressed.add("left");
      if (x > STICK_DEADZONE) pressed.add("right");
      if (y < -STICK_DEADZONE) pressed.add("up");
      if (y > STICK_DEADZONE) pressed.add("down");
    }
  }

  for (const action of pressed) {
    const next = held.get(action);
    if (next === undefined) {
      held.set(action, now + FIRST_REPEAT_MS);
      setSource("pad");
      dispatch(action);
    } else if (REPEATABLE.has(action) && now >= next) {
      held.set(action, now + REPEAT_MS);
      dispatch(action);
    }
  }
  for (const action of held.keys()) if (!pressed.has(action)) held.delete(action);

  requestAnimationFrame(poll);
}
requestAnimationFrame(poll);

/** Short rumble on supported pads; silently ignored elsewhere. */
export function rumble(strength = 0.35, duration = 28) {
  for (const pad of navigator.getGamepads?.() ?? []) {
    const actuator = (pad as (Gamepad & { vibrationActuator?: GamepadHapticActuator }) | null)?.vibrationActuator;
    actuator?.playEffect?.("dual-rumble", { duration, strongMagnitude: 0, weakMagnitude: strength }).catch(() => {});
  }
}

// ---- Keyboard (development and handhelds with a keyboard attached) ----

const KEYS: Record<string, Action> = {
  ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right",
  Enter: "a", " ": "a", Escape: "b", Backspace: "b",
  x: "x", y: "y", f: "y", q: "lb", e: "rb", PageUp: "lt", PageDown: "rt", Tab: "start", m: "start", s: "select",
};

/** While set, typed characters go here instead of being mapped to actions (search box). */
let textCapture: ((text: string | "backspace") => void) | null = null;
export function captureText(fn: typeof textCapture) {
  textCapture = fn;
}

window.addEventListener("keydown", (e) => {
  if (textCapture && !e.metaKey && !e.ctrlKey && !e.altKey) {
    if (e.key === "Backspace") {
      e.preventDefault();
      textCapture("backspace");
      return;
    }
    if (e.key.length === 1) {
      e.preventDefault();
      textCapture(e.key);
      return;
    }
  }
  const action = KEYS[e.key] ?? KEYS[e.key.toLowerCase()];
  if (!action || e.metaKey || e.ctrlKey || e.altKey) return;
  e.preventDefault();
  if (e.repeat && !REPEATABLE.has(action)) return;
  setSource("keys");
  dispatch(action);
});
