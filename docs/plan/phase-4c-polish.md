# Phase 4c — Polish & accessibility

**Goal.** The first version that's *pleasant* to use. Onboarding, settings, theming, accessibility audit.

**Definition of done.**
- [ ] `prefers-reduced-motion` respected throughout.
- [ ] Five clean screenshots: empty state, single-monitor canvas, three-monitor canvas, library grid, settings.
- [ ] Keyboard-only walk-through of every screen succeeds.
- [ ] Orca screen-reader smoke test passes for the main flows.

## 4c.1 Settings panel
- [ ] General: theme, autostart, notifications, default profile.
- [ ] Library: roots add/remove, recursive toggle, thumbnail size.
- [ ] Backend: prefer dropdown, custom command field with safety callout.
- [ ] Advanced: log level, memory cap, debug pane (raw IPC responses for support).

## 4c.2 Polish pass
- [ ] Tailwind theme tokens for a consistent dark palette.
- [ ] Toasts: bottom-right, dismiss on click, auto-dismiss after 5 s, errors persist until dismissed.
- [ ] Empty states: canvas shows a friendly placeholder with onboarding hint; library shows "no images — add a folder" CTA.
- [ ] Keyboard shortcuts wired per SPEC §12.5; a `?` overlay shows the full list.
- [ ] Loading indicators on long ops (initial library scan, large image apply).
- [ ] Focus outlines preserved (no `outline: none`).

## 4c.3 Accessibility
- [ ] Every interactive control has an `aria-label` or visible label.
- [ ] Tab order audit: keyboard-only walk-through of every screen.
- [ ] Colour-contrast check: text ≥ 4.5:1 against background (WCAG AA).
- [ ] Respect `prefers-reduced-motion`.
- [ ] Screen reader smoke test with Orca on the dev machine.
