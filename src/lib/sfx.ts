// Tiny synthesized UI sounds; no audio assets to ship.

let ctx: AudioContext | null = null;
let enabled = true;

export function setSoundsEnabled(on: boolean) {
  enabled = on;
}

function tone(freq: number, duration: number, gain: number, type: OscillatorType = "sine", slideTo?: number) {
  if (!enabled) return;
  try {
    ctx ??= new AudioContext();
    const t = ctx.currentTime;
    const osc = ctx.createOscillator();
    const amp = ctx.createGain();
    osc.type = type;
    osc.frequency.setValueAtTime(freq, t);
    if (slideTo) osc.frequency.exponentialRampToValueAtTime(slideTo, t + duration);
    amp.gain.setValueAtTime(0, t);
    amp.gain.linearRampToValueAtTime(gain, t + 0.006);
    amp.gain.exponentialRampToValueAtTime(0.0001, t + duration);
    osc.connect(amp).connect(ctx.destination);
    osc.start(t);
    osc.stop(t + duration + 0.02);
  } catch {
    /* audio unavailable */
  }
}

export const sfx = {
  move: () => tone(1250, 0.035, 0.035, "triangle"),
  tab: () => tone(820, 0.06, 0.05, "triangle", 1100),
  confirm: () => {
    tone(660, 0.09, 0.06, "sine", 990);
    setTimeout(() => tone(1320, 0.12, 0.04), 55);
  },
  back: () => tone(520, 0.08, 0.05, "sine", 360),
  error: () => tone(190, 0.18, 0.07, "square"),
  launch: () => {
    [523, 659, 784, 1047].forEach((f, i) => setTimeout(() => tone(f, 0.22, 0.045), i * 60));
  },
};
