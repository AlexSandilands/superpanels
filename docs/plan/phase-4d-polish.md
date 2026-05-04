# Phase 4d — Polish & accessibility

**Goal.** The first version that's *pleasant* to use. Onboarding, settings, theming, accessibility audit.

**Definition of done.**
- [ ] `prefers-reduced-motion` respected throughout.
- [ ] Five clean screenshots: empty state, single-monitor canvas, three-monitor canvas, library grid, settings.
- [ ] Keyboard-only walk-through of every screen succeeds.
- [ ] Orca screen-reader smoke test passes for the main flows.

## 4d.1 Settings panel
- [ ] General: theme, autostart, notifications, default profile.
- [ ] Library: roots add/remove, recursive toggle, thumbnail size.
- [ ] Backend: prefer dropdown, custom command field with safety callout.
- [ ] Advanced: log level, memory cap, debug pane (raw IPC responses for support).

## 4d.2 Polish pass
- [ ] Tailwind theme tokens for a consistent dark palette.
- [ ] Toasts: bottom-right, dismiss on click, auto-dismiss after 5 s, errors persist until dismissed.
- [ ] Empty states: canvas shows a friendly placeholder with onboarding hint; library shows "no images — add a folder" CTA.
- [ ] Keyboard shortcuts wired per SPEC §12.5; a `?` overlay shows the full list.
- [ ] Loading indicators on long ops (initial library scan, large image apply).
- [ ] Focus outlines preserved (no `outline: none`).

## 4d.3 Accessibility
- [ ] Every interactive control has an `aria-label` or visible label.
- [ ] Tab order audit: keyboard-only walk-through of every screen.
- [ ] Colour-contrast check: text ≥ 4.5:1 against background (WCAG AA).
- [ ] Respect `prefers-reduced-motion`.
- [ ] Screen reader smoke test with Orca on the dev machine.

## 4d.4 Carry-overs from earlier phases
- [ ] **Visual-regression test for canvas layout.** Phase 4a §4a.2 added `serialiseLayout` as the snapshot primitive but did not wire it into a vitest harness. Add a snapshot suite for the layouts that 4a/4c shipped (single-monitor, three-monitor row, 2×2 grid, mixed-orientation) and gate it in CI.
- [ ] **Richer hover tooltip.** Phase 4a §4a.3 left a minimal name+resolution glow; show a floating panel on hover with the source-pixel range that monitor will receive (mirrors the popout's `src ≈ x,y · w×h` line) so users can audit the crop without clicking.
- [ ] **Per-monitor drop-onto-canvas.** Phase 4a §4a.4 listed this as deferred. Translate the Tauri webview drop position through the canvas hit-test and bind it to the `PerMonitor` body's assignments. Coupled with library drag-source from 4b.
- [ ] **Apply animation parity in `ProfileList`.** Phase 4a noted the per-row Apply button skips the canvas flash. Either flash from both paths or document the deliberate split (the row-Apply is a fire-and-forget shortcut from anywhere in the app, the canvas Apply is the explicit save+apply commit).
