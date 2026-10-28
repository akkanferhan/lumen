/** The Lumen mark: a glowing orb with a play-shaped cut (same as app-icon.svg). */
export function BrandMark({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 1024 1024" aria-hidden>
      <defs>
        <linearGradient id="bm-tile" x1="0.15" y1="0" x2="0.85" y2="1">
          <stop offset="0" stopColor="#252a4a" />
          <stop offset="0.55" stopColor="#12142a" />
          <stop offset="1" stopColor="#0a0b16" />
        </linearGradient>
        <radialGradient id="bm-halo" cx="0.5" cy="0.47" r="0.5">
          <stop offset="0" stopColor="#ffcf7a" stopOpacity="0.55" />
          <stop offset="0.5" stopColor="#ff7a45" stopOpacity="0.16" />
          <stop offset="1" stopColor="#ff7a45" stopOpacity="0" />
        </radialGradient>
        <radialGradient id="bm-orb" cx="0.38" cy="0.32" r="0.75">
          <stop offset="0" stopColor="#fff6dc" />
          <stop offset="0.35" stopColor="#ffd27e" />
          <stop offset="0.75" stopColor="#ff9a4d" />
          <stop offset="1" stopColor="#e2582e" />
        </radialGradient>
        <linearGradient id="bm-cut" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#1b1f3b" />
          <stop offset="1" stopColor="#0c0d1c" />
        </linearGradient>
      </defs>
      <rect x="64" y="64" width="896" height="896" rx="214" fill="url(#bm-tile)" />
      <circle cx="512" cy="492" r="400" fill="url(#bm-halo)" />
      <circle cx="512" cy="492" r="292" fill="none" stroke="#ffb35c" strokeOpacity="0.16" strokeWidth="10" />
      <circle cx="512" cy="492" r="232" fill="url(#bm-orb)" />
      <path d="M468 392 Q448 380 448 403 V581 Q448 604 468 592 L612 504 Q632 492 612 480 Z" fill="url(#bm-cut)" />
      <ellipse cx="438" cy="380" rx="70" ry="38" fill="#ffffff" fillOpacity="0.35" transform="rotate(-28 438 380)" />
    </svg>
  );
}
