// Themes change layout, type and navigation, not just color. Each theme
// picks a library layout; its look lives in styles.css under [data-theme].

export type LibraryLayout = "grid" | "rail" | "list" | "cartridge" | "shelves" | "coverflow";

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
  {
    id: "raf",
    name: "Raf",
    description: "Her sistem bir raf: raflar arasında yukarı-aşağı, raf içinde sağa-sola.",
    layout: "shelves",
  },
  {
    id: "neon",
    name: "Neon",
    description: "3B kapak akışı, neon ışıklar ve perspektif ızgara. Atari salonu gecesi.",
    layout: "coverflow",
  },
  {
    id: "kagit",
    name: "Kâğıt",
    description: "Siyah-beyaz, daktilo yazısı; kapaklar seçilince renklenir. Fanzin sayfası gibi.",
    layout: "grid",
  },
  {
    id: "gece",
    name: "Gece",
    description: "Saf siyah zemin, ince çizgiler, renk yalnızca kapaklarda. OLED ekranlarda pil dostu.",
    layout: "grid",
  },
  {
    id: "pastel",
    name: "Pastel",
    description: "Açık lavanta ve şeftali tonları, yumuşak köşeler, yuvarlak yazı. Gündüz oyunları için.",
    layout: "grid",
  },
  {
    id: "terminal",
    name: "Terminal",
    description: "Yeşil fosforlu monitör: tek renk, monospace yazı ve hızlı liste görünümü.",
    layout: "list",
  },
];

export function themeOf(id: string | undefined): Theme {
  return THEMES.find((t) => t.id === id) ?? THEMES[0];
}

/** Applies the theme's CSS (data-theme on <html>). */
export function applyTheme(id: string | undefined) {
  document.documentElement.dataset.theme = themeOf(id).id;
}
