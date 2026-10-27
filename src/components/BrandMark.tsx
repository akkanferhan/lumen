export function BrandMark({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 1024 1024" aria-hidden>
      <defs>
        <linearGradient id="bm-bg" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#1b2140" />
          <stop offset="1" stopColor="#0b0d12" />
        </linearGradient>
        <radialGradient id="bm-glow" cx="0.5" cy="0.5" r="0.5">
          <stop offset="0" stopColor="#ffd27a" stopOpacity="0.9" />
          <stop offset="0.45" stopColor="#ff8a4c" stopOpacity="0.35" />
          <stop offset="1" stopColor="#ff8a4c" stopOpacity="0" />
        </radialGradient>
        <linearGradient id="bm-mark" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#fff4d6" />
          <stop offset="1" stopColor="#ffb35c" />
        </linearGradient>
      </defs>
      <rect x="64" y="64" width="896" height="896" rx="208" fill="url(#bm-bg)" />
      <circle cx="512" cy="512" r="360" fill="url(#bm-glow)" />
      <path d="M432 330 L700 512 L432 694 Z" fill="url(#bm-mark)" stroke="url(#bm-mark)" strokeWidth="40" strokeLinejoin="round" />
    </svg>
  );
}
