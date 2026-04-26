# Frontend style guide

> TypeScript and Svelte 5 conventions for the `ui/` package.

For Rust style see [`style-rust.md`](./style-rust.md). For where things go see [`architecture.md`](./architecture.md).

---

## Table of contents

- [TypeScript: strict mode, no exceptions](#typescript-strict-mode-no-exceptions)
- [Type-only imports](#type-only-imports)
- [Branded types](#branded-types)
- [Svelte 5 runes](#svelte-5-runes)
- [Component conventions](#component-conventions)
- [Stores](#stores)
- [Tauri IPC bindings](#tauri-ipc-bindings)
- [Styling](#styling)
- [Forbidden patterns](#forbidden-patterns)

---

## TypeScript: strict mode, no exceptions

The `tsconfig.json` (created in Phase 3) turns on every safety flag:

```jsonc
{
  "compilerOptions": {
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "noImplicitOverride": true,
    "noFallthroughCasesInSwitch": true,
    "noPropertyAccessFromIndexSignature": true,
    "exactOptionalPropertyTypes": true,

    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "isolatedModules": true,

    "skipLibCheck": true,
    "esModuleInterop": true,
    "resolveJsonModule": true,
    "useDefineForClassFields": true,

    "lib": ["ES2022", "DOM", "DOM.Iterable"]
  }
}
```

If a type error looks impossible to satisfy, the answer is *almost never* `any`. Use `unknown`, narrow it, or fix the source.

### `noUncheckedIndexedAccess` matters

```ts
const monitors: Monitor[] = await detect();

❌ const first: Monitor = monitors[0];
   //  ^ implicitly possibly undefined

✅ const first = monitors[0];
   if (!first) throw new Error('no monitors');
   // first is now Monitor
```

This catches a real class of bug — accessing a nonexistent index — that JavaScript silently returns `undefined` for.

---

## Type-only imports

```ts
✅ import type { Monitor } from './types';
   import { detect } from './api';

❌ import { Monitor, detect } from './api';  // Monitor is type-only
```

Type-only imports compile away to nothing. With `verbatimModuleSyntax: true`, mixing them is an error. ESLint's `consistent-type-imports` enforces it.

---

## Branded types

Match the Rust newtype pattern from [`style-rust.md`](./style-rust.md#newtypes-for-ids):

```ts
type Brand<K, T> = K & { readonly __brand: T };

export type MonitorId = Brand<number, 'MonitorId'>;
export type ProfileId = Brand<number, 'ProfileId'>;

// Construction is intentional:
export const monitorId = (n: number): MonitorId => n as MonitorId;
```

Now `applyProfile(monitor, profile)` won't compile if you swap the args.

For types shared with Rust, generate them via `ts-rs` from the Rust types — single source of truth.

---

## Svelte 5 runes

We're on Svelte 5 — use **runes**, not the legacy `let`-as-reactive syntax.

### `$state` for reactive variables

```svelte
<script lang="ts">
  let count = $state(0);
  let monitors = $state<Monitor[]>([]);
</script>
```

### `$derived` for computed values

```svelte
<script lang="ts">
  let monitors = $state<Monitor[]>([]);
  let canvasWidth = $derived(monitors.reduce((acc, m) => acc + m.physicalSizeMm[0], 0));
</script>
```

`$derived` is referentially transparent — no side effects, just compute. Use `$derived.by(() => { ... })` for multi-statement derivations.

### `$effect` for side effects

```svelte
<script lang="ts">
  let activeProfile = $state<Profile | null>(null);

  $effect(() => {
    if (activeProfile) {
      document.title = `Superpanels — ${activeProfile.name}`;
    }
  });
</script>
```

Use `$effect` *sparingly*. Most "I want X to update when Y changes" cases are actually `$derived`.

### `$props` for component props

```svelte
<script lang="ts">
  type Props = {
    monitors: Monitor[];
    onApply: (profile: Profile) => void;
  };
  let { monitors, onApply }: Props = $props();
</script>
```

No `export let` — that's the Svelte 4 way.

---

## Component conventions

### One component per file, named `PascalCase.svelte`

```
ui/src/lib/canvas/MonitorCanvas.svelte    ✅
ui/src/lib/canvas/monitor-canvas.svelte   ❌
ui/src/lib/canvas/index.svelte            ❌
```

### Component file structure

```svelte
<script lang="ts">
  // 1. Imports (types first, then runtime)
  import type { Monitor } from '$lib/types';
  import { detect } from '$lib/api';

  // 2. Props
  type Props = { monitors: Monitor[] };
  let { monitors }: Props = $props();

  // 3. State
  let hovered = $state<number | null>(null);

  // 4. Derived
  let totalWidth = $derived(monitors.reduce((a, m) => a + m.resolution[0], 0));

  // 5. Effects
  $effect(() => { /* ... */ });

  // 6. Functions
  function handleClick(id: number) { /* ... */ }
</script>

<!-- 7. Markup -->
<canvas onclick={() => handleClick(0)} />

<!-- 8. Styles (scoped by default) -->
<style>
  canvas { display: block; }
</style>
```

### File length

Components stay under 200 lines comfortably; 350 is the hard limit. If you're past that, extract child components or pull logic into a `.ts` sibling file (`MonitorCanvas.svelte` + `monitor-canvas.logic.ts`).

### No logic in markup

```svelte
❌ {monitors.filter(m => m.primary && m.resolution[0] > 1920).map(m => m.name).join(', ')}
✅ <script>
     let primaryWideNames = $derived(
       monitors.filter(m => m.primary && m.resolution[0] > 1920).map(m => m.name).join(', ')
     );
   </script>
   {primaryWideNames}
```

Markup expressions should be one identifier or a trivial accessor. Anything more goes in `$derived`.

---

## Stores

We use `$state`-backed stores in plain `.ts` files, not the legacy `writable`/`readable` API.

```ts
// ui/src/lib/stores/profile.ts
import type { Profile } from '$lib/types';

class ProfileStore {
  active = $state<Profile | null>(null);
  list = $state<Profile[]>([]);

  async refresh() {
    this.list = await invoke('list_profiles');
  }

  async apply(name: string) {
    await invoke('apply_profile', { name });
    this.active = this.list.find(p => p.name === name) ?? null;
  }
}

export const profileStore = new ProfileStore();
```

**One store per concept.** `profileStore`, `monitorStore`, `libraryStore`, `toastStore`. Don't make a single "appStore" with everything.

**Stores own their refresh logic** — don't have components fetch data and write into a store from outside. The store is the boundary.

---

## Tauri IPC bindings

All `invoke()` calls go through a typed wrapper, never directly:

```ts
// ui/src/lib/api.ts
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type { Monitor, Profile, CropSpec, BezelConfig, FitMode } from './types';

type ApiMap = {
  detect_monitors: { args: void; ret: Monitor[] };
  list_profiles: { args: void; ret: Profile[] };
  apply_profile: { args: { name: string }; ret: void };
  preview_crop: {
    args: { image: string; offsetPx: [number, number]; bezels: BezelConfig; fit: FitMode };
    ret: CropSpec[];
  };
  // ...
};

export async function invoke<K extends keyof ApiMap>(
  cmd: K,
  ...args: ApiMap[K]['args'] extends void ? [] : [ApiMap[K]['args']]
): Promise<ApiMap[K]['ret']> {
  return tauriInvoke(cmd, args[0] as Record<string, unknown> | undefined);
}
```

Now misspelt commands or missing args fail at compile time.

The `types.ts` file is generated from Rust via `ts-rs`. Don't write it by hand.

---

## Styling

- **Tailwind utility classes** for layout and spacing.
- **CSS variables** for theme tokens (colours, fonts, spacing scale) defined in `app.css`.
- **Component `<style>` blocks** for component-specific styles. Default-scoped, no leakage.
- **No inline `style` attribute** except for dynamic values (`style="--offset: {x}px"`).
- **Honour `prefers-reduced-motion`** on every animation.

### i18n

UI strings flow through a small `t(key, args?)` helper backed by [`@nubolab-ffwd/svelte-fluent`](https://github.com/nubolab-ffwd/svelte-fluent) (or an equivalent thin wrapper around `@fluent/bundle`). It's a runtime function call, not a Rust-style macro — Svelte's compiler doesn't have macros, so don't write `t!(…)` in `.svelte` files.

```svelte
<script lang="ts">
  import { t } from '$lib/i18n';
</script>
<button>{t('apply.button')}</button>
```

Rust-side strings (CLI human-readable messages) use a real `t!` macro backed by `fluent` directly. The two locales live side by side in `ui/locales/en.ftl` and `crates/superpanels-cli/locales/en.ftl`.

```svelte
<style>
  .canvas {
    border: 1px solid var(--color-border);
    transition: opacity 200ms ease;
  }
  @media (prefers-reduced-motion: reduce) {
    .canvas { transition: none; }
  }
</style>
```

### Theme tokens

```css
/* ui/src/app.css */
:root {
  --color-bg: #18181b;
  --color-fg: #f4f4f5;
  --color-accent: #3b82f6;
  --color-border: #3f3f46;
  /* ... */
}

@media (prefers-color-scheme: light) {
  :root {
    --color-bg: #fafafa;
    /* ... */
  }
}
```

Components reference tokens by name, not by hex. Theme changes touch one file.

---

## Forbidden patterns

- **No `any`.** ESLint rule `@typescript-eslint/no-explicit-any` is `error`. Use `unknown` and narrow.
- **No `// @ts-ignore`.** Use `// @ts-expect-error reason: …` if absolutely necessary; ESLint requires the comment.
- **No `console.log` / `console.error` in committed code.** Use the toast store for user-facing messages and `tracing` (forwarded from Rust) for diagnostics.
- **No `setInterval` / `setTimeout` outside `$effect` cleanup paths.** Effects manage their own teardown; ad-hoc timers leak.
- **No untyped `fetch`.** All IPC goes through the typed `invoke` wrapper. There should be no `fetch` calls — it's all Tauri.
- **No `as` casts to silence type errors.** A cast that hides a real type mismatch creates a runtime bug.
- **No mutating props.** Props are read-only; communicate up via callback props (`onChange`, `onApply`).
- **No DOM access in `<script>` top-level** — wait for an effect or the component is mounted. `document` may not exist during SSR (we don't currently SSR, but the discipline is cheap).
