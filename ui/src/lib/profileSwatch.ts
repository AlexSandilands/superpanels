// Deterministic colour swatch for a profile, derived from its name. Used in
// the profile menu, profile list, and tray popover so each profile has a
// stable, distinguishable identity without persisting a colour.

function hash(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i += 1) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

export function profileSwatch(name: string): string {
  const seed = hash(name || 'untitled');
  const h1 = seed % 360;
  const h2 = (seed >> 8) % 360;
  const h3 = (seed >> 16) % 360;
  return `linear-gradient(90deg, oklch(0.45 0.15 ${h1}), oklch(0.55 0.18 ${h2}), oklch(0.7 0.16 ${h3}))`;
}
