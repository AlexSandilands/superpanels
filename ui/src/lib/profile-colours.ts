// Shared swatch palette for profile colour pickers (manager modal, popover,
// dialogs). The eventual gradient palette migration replaces just this file.

import type { ProfileColour } from './types/ProfileColour';

export const PROFILE_COLOURS: ProfileColour[] = [
  'slate',
  'stone',
  'red',
  'orange',
  'amber',
  'yellow',
  'lime',
  'emerald',
  'teal',
  'sky',
  'indigo',
  'violet',
];

const COLOUR_CSS: Record<ProfileColour, string> = {
  slate: 'oklch(0.65 0.04 250)',
  stone: 'oklch(0.7 0.02 80)',
  red: 'oklch(0.62 0.2 25)',
  orange: 'oklch(0.7 0.18 50)',
  amber: 'oklch(0.78 0.17 80)',
  yellow: 'oklch(0.85 0.18 100)',
  lime: 'oklch(0.78 0.2 130)',
  emerald: 'oklch(0.7 0.18 160)',
  teal: 'oklch(0.7 0.13 200)',
  sky: 'oklch(0.7 0.15 235)',
  indigo: 'oklch(0.55 0.2 270)',
  violet: 'oklch(0.6 0.22 300)',
};

export function profileColourCss(c: ProfileColour): string {
  return COLOUR_CSS[c];
}
