// Themes change layout, type and navigation, not just color. Each theme
// picks a library layout; its look lives in styles.css under [data-theme].

import { L } from "./i18n";

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
    name: L("Işık", "Light"),
    description: L("Kapak ızgarası, karanlık ve parlak. Varsayılan.", "Cover grid, dark and bright. The default."),
    layout: "grid",
  },
  {
    id: "galeri",
    name: L("Galeri", "Gallery"),
    description: L("Tam ekran oyun görseli, serif başlıklar, altta kapak şeridi. Bir sanat kitabı gibi.", "Full-screen game art, serif titles, a cover strip below. Like an art book."),
    layout: "rail",
  },
  {
    id: "sade",
    name: L("Sade", "Plain"),
    description: L("Açık renk, metin listesi ve büyük önizleme. Büyük kütüphanelerde en hızlısı.", "Light colors, a text list and a big preview. Fastest for large libraries."),
    layout: "list",
  },
  {
    id: "kartus",
    name: L("Kartuş", "Cartridge"),
    description: L("Kartuş kartlar, piksel yazılar ve CRT tarama çizgileri. Retro ruhu.", "Cartridge cards, pixel fonts and CRT scanlines. Pure retro."),
    layout: "cartridge",
  },
  {
    id: "raf",
    name: L("Raf", "Shelves"),
    description: L("Her sistem bir raf: raflar arasında yukarı-aşağı, raf içinde sağa-sola.", "One shelf per system: up and down between shelves, left and right along one."),
    layout: "shelves",
  },
  {
    id: "neon",
    name: "Neon",
    description: L("3B kapak akışı, neon ışıklar ve perspektif ızgara. Atari salonu gecesi.", "3D cover flow, neon lights and a perspective grid. Arcade night."),
    layout: "coverflow",
  },
  {
    id: "kagit",
    name: L("Kâğıt", "Paper"),
    description: L("Siyah-beyaz, daktilo yazısı; kapaklar seçilince renklenir. Fanzin sayfası gibi.", "Black and white, typewriter text; covers turn color when selected. Like a fanzine page."),
    layout: "grid",
  },
  {
    id: "gece",
    name: L("Gece", "Night"),
    description: L("Saf siyah zemin, ince çizgiler, renk yalnızca kapaklarda. OLED ekranlarda pil dostu.", "Pure black, thin lines, color only on the covers. Battery-friendly on OLED screens."),
    layout: "grid",
  },
  {
    id: "pastel",
    name: "Pastel",
    description: L("Açık lavanta ve şeftali tonları, yumuşak köşeler, yuvarlak yazı. Gündüz oyunları için.", "Light lavender and peach tones, soft corners, rounded type. For daytime play."),
    layout: "grid",
  },
  {
    id: "terminal",
    name: "Terminal",
    description: L("Yeşil fosforlu monitör: tek renk, monospace yazı ve hızlı liste görünümü.", "Green phosphor monitor: one color, monospace text and a fast list view."),
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
