// Builds src-tauri/homebrew.json from Homebrew Hub (hh.gbdev.io).
// Only games whose license explicitly allows redistribution are included;
// entries without a license are left out. Run: node scripts/homebrew-catalog.mjs

import { writeFileSync } from "node:fs";

const API = "https://hh3.gbdev.io/api/search";
const STATIC = "https://hh3.gbdev.io/static";
const HEADERS = { "User-Agent": "Lumen (+https://github.com/akkanferhan/lumen)" };

// SPDX-ish ids that permit sharing the unmodified game (NC/ND variants allow
// non-commercial verbatim copies, which is all Lumen does).
const REDISTRIBUTABLE =
  /^(MIT|0BSD|BSD-[23]-Clause|Apache-2\.0|Zlib|Unlicense|WTFPL(-2\.0)?|MPL-2\.0|L?GPL-[23]\.[01](-only|-or-later)?|CC0-1\.0|CC-BY(-SA|-NC|-NC-SA|-NC-ND|-ND)?[- ]?[0-9.]*|FSFAP|MIT \/ CC-BY-4\.0 \(Assets\))$/i;

// Lumen system by file extension (some GB entries ship .gbc files).
const SYSTEM_BY_EXT = { gb: "gb", gbc: "gbc", gba: "gba", nes: "nes" };

async function fetchAll() {
  const entries = [];
  for (let page = 1, total = 1; page <= total; page++) {
    const res = await fetch(`${API}?page_size=100&page=${page}`, { headers: HEADERS });
    if (!res.ok) throw new Error(`Homebrew Hub ${res.status} on page ${page}`);
    const body = await res.json();
    total = body.page_total;
    entries.push(...body.entries);
  }
  return entries;
}

const clean = (text = "") => text.replace(/\s+/g, " ").trim();

const entries = await fetchAll();
const catalog = [];
// The API's pagination can repeat entries across pages.
const seen = new Set();
for (const e of entries) {
  if (seen.has(e.slug)) continue;
  seen.add(e.slug);
  const license = clean(e.gameLicense || e.license || "");
  if (!license || !REDISTRIBUTABLE.test(license) || e.typetag !== "game") continue;

  const playable = (e.files || []).filter((f) => f.playable);
  const file = playable.find((f) => f.default) || playable[0];
  if (!file) continue;
  const fileName = file.filename.split("/").pop();
  const system = SYSTEM_BY_EXT[fileName.split(".").pop().toLowerCase()];
  if (!system) continue;

  const base = `${STATIC}/${e.basepath}/entries/${e.slug}`;
  const cover = (e.screenshots || [])[0];
  const description = clean(e.description);
  catalog.push({
    slug: e.slug,
    title: clean(e.title),
    system,
    // `developer` is a string, an object or a list of either, depending on the entry.
    developer: [e.developer ?? []]
      .flat()
      .map((d) => (typeof d === "string" ? d : d?.name))
      .filter(Boolean)
      .join(", "),
    year: /^(19[89]\d|20\d\d)$/.test((e.date || "").slice(0, 4)) ? e.date.slice(0, 4) : null,
    license,
    description: description.length > 400 ? `${description.slice(0, 397)}…` : description,
    url: `${base}/${file.filename}`,
    fileName,
    cover: cover ? `${base}/${cover}` : null,
    source: e.repository || e.website || e.gameWebsite || null,
  });
}

catalog.sort((a, b) => a.title.localeCompare(b.title, "en"));
writeFileSync(new URL("../src-tauri/homebrew.json", import.meta.url), JSON.stringify(catalog, null, 2) + "\n");
console.log(`${catalog.length} redistributable games from ${entries.length} entries`);
