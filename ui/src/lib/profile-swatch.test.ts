import { describe, it, expect } from 'vitest';
import { profileSwatch } from './profile-swatch';

describe('profileSwatch', () => {
  it('returns_same_swatch_for_same_name', () => {
    expect(profileSwatch('work')).toBe(profileSwatch('work'));
    expect(profileSwatch('long profile name with spaces')).toBe(
      profileSwatch('long profile name with spaces'),
    );
  });

  it('different_names_likely_produce_different_swatches', () => {
    const samples = ['work', 'home', 'gaming', 'photo', 'movies', 'reading'];
    const swatches = samples.map(profileSwatch);
    const unique = new Set(swatches);
    // Six distinct seeds should not all collide on (h1, h2, h3) modulo 360.
    expect(unique.size).toBeGreaterThan(samples.length - 2);
  });

  it('handles_empty_name_via_untitled_fallback', () => {
    expect(profileSwatch('')).toBe(profileSwatch('untitled'));
  });

  it('produces_a_linear_gradient_string', () => {
    expect(profileSwatch('any')).toMatch(/^linear-gradient\(90deg, oklch\(/);
  });
});
