// Turkish and English. The language follows Windows' display language
// unless the menu pins one (kept in localStorage, read once at start; a
// change reloads the page). Text is written in both languages where it's
// used — L("Oyna", "Play") — so nothing can go missing from a dictionary.

export type Language = "auto" | "tr" | "en";

const KEY = "lumen.language";

/** The menu's choice ("auto" when none). */
export function languageSetting(): Language {
  try {
    const saved = localStorage.getItem(KEY);
    return saved === "tr" || saved === "en" ? saved : "auto";
  } catch {
    return "auto";
  }
}

function resolve(setting: Language): "tr" | "en" {
  if (setting !== "auto") return setting;
  const preferred = navigator.languages?.[0] ?? navigator.language ?? "tr";
  return preferred.toLowerCase().startsWith("tr") ? "tr" : "en";
}

/** The language the UI is shown in. */
export const lang: "tr" | "en" = resolve(languageSetting());
export const english = lang === "en";
/** For dates and numbers: "tr-TR" or "en-US". */
export const locale = english ? "en-US" : "tr-TR";

if (typeof document !== "undefined") document.documentElement.lang = lang;

/** Turkish or English: text, or any JSX. */
export function L<T>(tr: T, en: T): T {
  return english ? en : tr;
}

/** Saves the menu's choice and reloads the UI in it. */
export function setLanguageSetting(next: Language) {
  try {
    if (next === "auto") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, next);
  } catch {
    // Can't remember it: the reload shows the automatic language.
  }
  location.reload();
}
