import { L, locale } from "./i18n";

/** Same rules as `clean_title` in library.rs. */
export function cleanTitle(stem: string): string {
  const base = stem
    .replace(/\s*[([][^)\]]*[)\]]/g, "")
    .replace(/_/g, " ")
    .split(/\s+/)
    .filter(Boolean)
    .join(" ");
  const title = base.replace(/^(.+?), (The|A|An)( - .*)?$/, "$2 $1$3");
  return title || stem;
}

export function formatPlaytime(seconds: number): string {
  if (seconds < 60) return seconds > 0 ? L("1 dakikadan az", "Less than a minute") : L("Hiç oynanmadı", "Never played");
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  if (hours === 0) return L(`${minutes} dk`, `${minutes} min`);
  return minutes % 60
    ? L(`${hours} sa ${minutes % 60} dk`, `${hours} h ${minutes % 60} min`)
    : L(`${hours} sa`, `${hours} h`);
}

export function formatLastPlayed(unixSeconds: number | null): string {
  if (!unixSeconds) return L("Hiç", "Never");
  const then = new Date(unixSeconds * 1000);
  const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((startOfDay(new Date()) - startOfDay(then)) / 86_400_000);
  if (days <= 0) return L("Bugün", "Today");
  if (days === 1) return L("Dün", "Yesterday");
  if (days < 7) return L(`${days} gün önce`, `${days} days ago`);
  return then.toLocaleDateString(locale, { day: "numeric", month: "long", year: "numeric" });
}

/** Case/diacritic-insensitive form for search and letter grouping ("Pokémon" → "pokemon"). */
export function fold(text: string): string {
  return text
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/ı/g, "i")
    .toLowerCase();
}

/** Library group letter: A–Z, or "#" for anything else. */
export function groupLetter(title: string): string {
  const first = fold(title.replace(/^(the|a|an) /i, "")).charAt(0).toUpperCase();
  return first >= "A" && first <= "Z" ? first : "#";
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

/** Unix seconds of today's local midnight. */
export function todayStart(): number {
  const now = new Date();
  return Math.floor(new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime() / 1000);
}
