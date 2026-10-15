// Tiny synthesized sound effects (no audio files to ship).
let ctx: AudioContext | null = null;

function beep(freq: number, ms: number, type: OscillatorType = 'sine', gain = 0.08) {
  try {
    ctx ??= new AudioContext();
    const o = ctx.createOscillator();
    const g = ctx.createGain();
    o.type = type;
    o.frequency.value = freq;
    g.gain.value = gain;
    g.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + ms / 1000);
    o.connect(g).connect(ctx.destination);
    o.start();
    o.stop(ctx.currentTime + ms / 1000);
  } catch {
    // audio unavailable (autoplay policy, old browser): stay silent
  }
}

export const sounds = {
  yourTurn: () => {
    beep(660, 120);
    setTimeout(() => beep(880, 160), 130);
  },
  deal: () => beep(320, 60, 'triangle', 0.05),
  chips: () => beep(1200, 50, 'square', 0.03),
  win: () => [523, 659, 784].forEach((f, i) => setTimeout(() => beep(f, 160), i * 110)),
  splat: () => beep(140, 180, 'sawtooth', 0.06),
};
