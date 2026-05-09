# Frontend style guide

> TypeScript and Svelte 5 conventions for the `ui/` package, as the codebase is shaped today.

For Rust style see [`style-rust.md`](./style-rust.md). For where things go in the workspace see [`architecture.md`](./architecture.md). For tests see [`testing.md`](./testing.md).

---

## Table of contents

- [TypeScript: strict mode, no exceptions](#typescript-strict-mode-no-exceptions)
- [Type-only imports](#type-only-imports)
- [Svelte 5 runes](#svelte-5-runes)
- [Where things live](#where-things-live)
- [Component conventions](#component-conventions)
- [Stores](#stores)
- [Tauri IPC bindings](#tauri-ipc-bindings)
- [Styling](#styling)
- [File-size caps](#file-size-caps)
- [Naming](#naming)
- [Forbidden patterns](#forbidden-patterns)

---

## TypeScript: strict mode, no exceptions

The `tsconfig.json` enables every safety flag we care about: `strict`, `noUncheckedIndexedAccess`, `noImplicitOverride`, `noFallthroughCasesInSwitch`, `exactOptionalPropertyTypes`, `verbatimModuleSyntax`, `isolatedModules`. If a type error looks impossible to satisfy, the answer is _almost never_ `any`. Use `unknown`, narrow it, or fix the source.

### `noUncheckedIndexedAccess` matters

```ts
const monitors: Monitor[] = await api.detectMonitors();

// ❌ implicitly possibly undefined
const first: Monitor = monitors[0];

// ✅
const first = monitors[0];
if (!first) throw new Error('no monitors');
```

This catches a real class of bug — accessing a nonexistent index — that JavaScript silently returns `undefined` for.

---

## Type-only imports

```ts
✅ import type { Monitor } from '$lib/api';
   import { api } from '$lib/api';

❌ import { Monitor, api } from '$lib/api';  // Monitor is type-only
```

`verbatimModuleSyntax: true` makes mixing them an error. ESLint's `consistent-type-imports` enforces it.

---

## Svelte 5 runes

We're on Svelte 5 — use **runes** (`$state` / `$derived` / `$effect` / `$props`), not the legacy `let`-as-reactive syntax. No `export let`.

```svelte
<script lang="ts">
  import type { Monitor } from '$lib/api';

  type Props = {
    monitors: Monitor[];
    onApply: (name: string) => void;
  };
  let { monitors, onApply }: Props = $props();

  let hovered = $state<string | null>(null);
  const totalPx = $derived(monitors.reduce((a, m) => a + m.resolution[0], 0));

  $effect(() => {
    if (monitors.length === 0) hovered = null;
  });
</script>
```

`$derived` is referentially transparent — no side effects. Use `$derived.by(() => { ... })` for multi-statement derivations. Use `$effect` _sparingly_; most "I want X to update when Y changes" cases are `$derived`.

Runes also work in plain TypeScript modules with the `.svelte.ts` extension — see [Stores](#stores) below.

---

## Where things live

```
ui/src/
├── App.svelte                          ← root composition (≤ 350 lines)
├── app.css                             ← theme tokens + global styles
├── main.ts                             ← Svelte mount
├── api.ts                              ← typed Tauri IPC wrapper
├── profile-swatch.ts                   ← deterministic colour helper
├── components/
│   ├── canvas/                         ← preview canvas + grid overlays
│   │   ├── PreviewCanvas.svelte
│   │   ├── CanvasGrid.svelte
│   │   └── DimensionLines.svelte
│   ├── chrome/                         ← window/canvas chrome (TitleBar,
│   │   │                                  BezelDock, ToolDock,
│   │   │                                  MonitorInspector, …)
│   ├── widgets/                        ← reusable primitives (Backdrop,
│   │   │                                  Select, StepperInput, Toast,
│   │   │                                  Icon)
│   └── overlays/                       ← modal flows (LibraryModal,
│       │                                  SettingsModal, TrayPopover,
│       │                                  LibraryGrid, LibraryPinMenu)
│       └── settings/                   ← the per-section panes inside
│                                          SettingsModal
└── lib/
    ├── api.ts re-exports
    ├── actions.ts                      ← cross-store orchestrations
    ├── keymap.ts                       ← keyboard shortcut dispatcher
    ├── slideshow-controller.svelte.ts  ← slideshow state + transport
    ├── canvas/
    │   ├── drag.svelte.ts              ← drag state machine
    │   ├── select.ts                   ← selection-scoped mutations
    │   ├── transform-actions.ts        ← gap/cover/reset orchestrations
    │   ├── preview-layout.ts           ← orchestrator + geometry
    │   └── preview-layout/
    │       ├── projection.ts           ← positions + cover-fit rect
    │       └── gaps.ts                 ← neighbour detection +
    │                                      normalisation
    ├── events/window.ts                ← Tauri tray + drag-drop wiring
    ├── library/                        ← image-cache + thumb-cache
    ├── stores/                         ← module-scoped state stores
    │   ├── canvas-view.svelte.ts       (zoom, pan, hover, overrides)
    │   ├── image-transform.svelte.ts   (image rect + source effect)
    │   ├── library.svelte.ts           (paginated index + filters)
    │   ├── monitors.svelte.ts          (detect + refresh)
    │   ├── profile.svelte.ts           (list + draft + save)
    │   ├── runtime.svelte.ts           (last-applied meta)
    │   ├── toast.svelte.ts             (notifications)
    │   └── ui.svelte.ts                (theme/accent/density/blur)
    └── types/                          ← hand-written + ts-rs-ish IPC
                                           shapes
```

The split between `chrome/` and `widgets/` is intentional: anything in `widgets/` is meant to be used in two or more places (e.g. `Select` shows up in General settings _and_ in BezelDock). `chrome/` components belong to one specific layout slot.

---

## Component conventions

### One component per file, `PascalCase.svelte`

```
✅ components/chrome/TitleBar.svelte
❌ components/chrome/title-bar.svelte
❌ components/chrome/index.svelte
```

### File structure (advisory)

A useful default order — _not_ an enforced grammar. Current code interleaves where it's clearer to do so.

1. Imports (types first, then runtime).
2. `$props()` destructure.
3. `$state` declarations.
4. `$derived` declarations.
5. `$effect`s.
6. Functions.
7. Markup.
8. `<style>`.

If a `$derived` belongs visually next to the `$state` it consumes, group them. The point is "you shouldn't have to scroll to find what feeds what", not a checklist.

### No logic in markup

Markup expressions should be one identifier or a trivial accessor. Anything more goes in a `$derived`. Bad:

```text
{monitors.filter(m => m.primary).map(m => m.name).join(', ')}
```

Good:

```text
<script>
  const primaries = $derived(
    monitors.filter(m => m.primary).map(m => m.name).join(', ')
  );
</script>

{primaries}
```

---

## Stores

Stores are plain `.svelte.ts` modules with module-scoped `let x = $state(...)` declarations exposed through a frozen object literal of getters and methods. **No classes**, no `writable`/`readable`.

```ts
// ui/src/lib/stores/runtime.svelte.ts

export type ApplyMeta = { backend: string; elapsedMs: number; at: number /* … */ };

let last = $state<ApplyMeta | null>(null);

export const runtime = {
  get last() {
    return last;
  },
  recordApply(meta: ApplyMeta) {
    last = meta;
  },
};
```

Why an object-with-getters and not just exporting `last` directly? Because consumers reading `runtime.last.elapsedMs` get reactive subscription; consumers reading a bare `let` export would snapshot once.

### One store per concept, _not_ per variable

A "concept" is a coherent UI/preferences cluster. `ui.svelte.ts` owns theme + accent + density + blur because they're a single setting bundle that's persisted together. `library.svelte.ts` owns the entries + filters + roots because they all flow from the same fetch. The rule we're avoiding is the `appStore`-grab-bag — one giant store with everything in it. "One store per useEffect-style hook" is too fine-grained.

### Stores own their refresh logic

Components don't fetch and write into a store from outside. The store is the boundary:

```ts
// ✅
async function init() {
  await libraryStore.refresh();
}

// ❌
const list = await api.libraryList(...);
libraryStore.entries = list;
```

### Effects in stores

Module-scope `$effect` doesn't fire — runes only run inside a component or function-call from one. If a store needs to react to other-store changes (e.g. `imageTransform` reseeding overrides when monitors change), expose a function (`useSourceImage`, `seedOverridesFromMonitors`) that the App component calls during setup. The function declares `$effect` inside, where it's properly mounted.

---

## Tauri IPC bindings

All `invoke` calls go through `lib/api.ts` — there is no `invoke` use elsewhere. The actual shape is a hand-written `api` object literal with one method per command and a generic `call<T>` wrapper:

```ts
// ui/src/lib/api.ts
import { invoke } from '@tauri-apps/api/core';

async function call<T>(name: string, args: Record<string, unknown> = {}): Promise<T> {
  return invoke<T>(name, args);
}

export const api = {
  detectMonitors: () => call<Monitor[]>('detect_monitors'),
  applyProfile: (name: string) => call<AppliedReport>('apply_profile', { name }),
  // …
};
```

Some types come from `lib/types/` via `ts-rs` (`IpcError`, `LibraryFilter`, `PreviewArgs`, `Profile`); others (`Monitor`, `LibraryEntry`, `RuntimeState`, `AppliedReport`) are hand-written and explicitly tagged with a "until ts-rs covers them" comment in `api.ts`. Don't promise compile-time guarantees the code doesn't deliver — adding a method is a one-line addition with no codegen step today.

When ts-rs grows to cover the remaining shapes, the hand-written types in `api.ts` should disappear.

---

## Styling

- **Tailwind utility classes** for layout/spacing/colour where they fit.
- **CSS variables** for theme tokens — defined in `app.css`, referenced everywhere.
- **Component `<style>` blocks** for component-specific CSS. Default-scoped.
- **Inline `style:`** for _dynamic_ values only (`style:left="{x}px"`). No static inline styles.
- **`prefers-reduced-motion`** on every animation longer than a flash.

### Theme tokens

`app.css` keys themes off attribute selectors so the runtime can swap them without re-renders:

```css
:root[data-theme='dark']        { --bg: …; --text: …; … }
:root[data-theme='light']       { --bg: …; --text: …; … }
:root[data-density='compact']   { --row-h: 24px; --pad: 6px; }
:root[data-density='regular']   { --row-h: 28px; --pad: 8px; }
:root[data-blur='on'] .panel    { backdrop-filter: blur(12px); }
```

The `ui` store's `applyDocumentTokens()` writes `data-theme` / `data-density` / `data-blur` on `<html>` and sets `--accent` as an inline custom property. Components reference tokens by name; theme changes touch one file.

---

## File-size caps

|                        | Soft | Hard    |
| ---------------------- | ---- | ------- |
| Svelte component       | 200  | **350** |
| TypeScript module      | 300  | **500** |
| Rust file (production) | 400  | 600     |

These target _monolithic_ files (mixed responsibility), not raw line counts. A 220-line component that does one thing well is fine. A 180-line component that does three things is already too much.

**Data-only files earn an inline `reason:` exemption.** A long sprite map, an enum-like dispatch table, a catalogue of magic numbers — none of these benefit from a split, and the cap exists to fight cognitive load, not file length. Tag them with a single-line marker:

```svelte
<!-- reason: monolithic SVG sprite map, no logic — exempt from the 350-line cap -->
<script lang="ts">
  // …
</script>
```

`Icon.svelte` is the canonical example. Use the same pattern for any TS file that's pure data.

When you do split, split by responsibility:

- Big component → child components for the chunks that own state separately (e.g. `LibraryModal` → `LibraryGrid` + `LibraryPinMenu`).
- Big TS module → `module.ts` orchestrator + `module/<concept>.ts` submodules (`previewLayout.ts` + `previewLayout/projection.ts` + `previewLayout/gaps.ts`).
- Big Svelte component with extractable logic → sibling `.svelte.ts` (drag state machine, override-seeding effect).

---

## Naming

| Thing                   | Convention                  | Example                                                       |
| ----------------------- | --------------------------- | ------------------------------------------------------------- |
| Components              | `PascalCase.svelte`         | `MonitorInspector.svelte`                                     |
| TS modules              | `kebab-case.ts`             | `preview-layout.ts`, `source-image.ts`, `profile-swatch.ts`   |
| Stores                  | `kebab-case.svelte.ts`      | `canvas-view.svelte.ts`                                       |
| Test files              | `<name>.test.ts`            | `gaps.test.ts`                                                |
| Bench files             | `<name>.bench.ts`           | `gaps.bench.ts`                                               |
| Identifiers (exports)   | `camelCase`                 | `export const slideshowController`, `export function profileSwatch` |
| Local component state   | `camelCase`                 | `let hoverId = $state(null)`                                  |
| Props / event callbacks | `camelCase` / `onCamelCase` | `onApply`, `onMonitorDrop`                                    |

File names are kebab-case; the identifiers they export stay camelCase. The `lib/types/` files (`IpcError.ts`, `LibraryFilter.ts`, `PreviewArgs.ts`) are an exception — they're generated by `ts-rs` and match the Rust struct names, so leave them as-is.

No abbreviations unless the long form is genuinely awkward. `monitor`, not `mon`. No Hungarian. Verbs for functions, nouns for types, predicates read like predicates (`hasOverrides`, `isPrimary`).

---

## Forbidden patterns

- **No `any`.** `@typescript-eslint/no-explicit-any` is `error`. Use `unknown` and narrow.
- **No `// @ts-ignore`.** Use `// @ts-expect-error reason: …` only when truly necessary; the comment is required.
- **No `console.*` in committed code** (`log`, `error`, `warn`, `info`, `debug`, `dir`). Use the toast store for user-facing messages and `tracing` (forwarded from Rust) for diagnostics.
  Acceptable carve-out: a deliberate dev-diagnostic `console.warn` with an inline `// reason: …` justification. None exist today.
- **No `setInterval` / `setTimeout` outside `$effect` cleanup paths or `onMount`.** Effects manage their own teardown; ad-hoc timers leak.
- **No untyped `fetch`.** All IPC goes through `api`. There should be no `fetch` calls — it's all Tauri.
- **No `as` casts to silence type errors.** A cast that hides a real type mismatch creates a runtime bug.
- **No mutating props.** Props are read-only; communicate up via callback props (`onChange`, `onApply`).
- **No DOM access in `<script>` top level.** Wait for an effect or `onMount` — `document` may not exist in some test/SSR contexts.
- **No `git add .` / `git add -A`.** Stage explicit paths; the working tree may carry unrelated user work.
