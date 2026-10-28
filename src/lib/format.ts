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
  if (seconds < 60) return seconds > 0 ? "1 dakikadan az" : "Hiç oynanmadı";
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  if (hours === 0) return `${minutes} dk`;
  return minutes % 60 ? `${hours} sa ${minutes % 60} dk` : `${hours} sa`;
}

export function formatLastPlayed(unixSeconds: number | null): string {
  if (!unixSeconds) return "Hiç";
  const then = new Date(unixSeconds * 1000);
  const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((startOfDay(new Date()) - startOfDay(then)) / 86_400_000);
  if (days <= 0) return "Bugün";
  if (days === 1) return "Dün";
  if (days < 7) return `${days} gün önce`;
  return then.toLocaleDateString("tr-TR", { day: "numeric", month: "long", year: "numeric" });
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
