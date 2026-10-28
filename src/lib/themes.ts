// Themes change layout, type and navigation, not just color. Each theme
// picks a library layout; its look lives in styles.css under [data-theme].

export type LibraryLayout = "grid" | "rail" | "list" | "cartridge";

export interface Theme {
  id: string;
  name: string;
  description: string;
  layout: LibraryLayout;
}

export const THEMES: Theme[] = [
  {
    id: "lumen",
    name: "Işık",
    description: "Kapak ızgarası, karanlık ve parlak. Varsayılan.",
    layout: "grid",
  },
  {
    id: "galeri",
    name: "Galeri",
    description: "Tam ekran oyun görseli, serif başlıklar, altta kapak şeridi. Bir sanat kitabı gibi.",
    layout: "rail",
  },
  {
    id: "sade",
    name: "Sade",
    description: "Açık renk, metin listesi ve büyük önizleme. Büyük kütüphanelerde en hızlısı.",
    layout: "list",
  },
  {
    id: "kartus",
    name: "Kartuş",
    description: "Kartuş kartlar, piksel yazılar ve CRT tarama çizgileri. Retro ruhu.",
    layout: "cartridge",
  },
];

export function themeOf(id: string | undefined): Theme {
  return THEMES.find((t) => t.id === id) ?? THEMES[0];
}

/** Applies the theme's CSS (data-theme on <html>). */
export function applyTheme(id: string | undefined) {
  document.documentElement.dataset.theme = themeOf(id).id;
}
