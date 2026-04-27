# 17. Security & sandbox considerations

- Image decoding happens in a worker thread. The `image` crate is safe Rust but image files come from untrusted sources (downloads), so we cap decode memory at a configurable limit (default 512 MB; rejects pathological PNG bombs).
- Custom backend commands are user-supplied; we run them as the user. The config doc warns clearly; the GUI's custom-command field shows a "this runs with your privileges" callout.
- No HTTP fetching in v1.
- **Tauri v2 hardening.** The defaults are not safe enough for an app that touches user-supplied paths and runs subprocesses. We lock down four surfaces:
  - **CSP.** `tauri.conf.json` sets `app.security.csp` explicitly:
    ```
    default-src 'self';
    script-src  'self';
    style-src   'self' 'unsafe-inline';   /* inline style="" attrs for canvas positioning */
    img-src     'self' data: blob:;       /* thumbnails arrive as bytes via IPC, become blob: URLs */
    connect-src 'self' ipc: http://ipc.localhost;  /* Tauri v2 IPC transport */
    object-src  'none';
    base-uri    'self';
    frame-ancestors 'none';
    ```
    No `'unsafe-eval'`, no `'unsafe-inline'` on `script-src`. `'unsafe-inline'` on `style-src` is the one concession — Svelte's runes-based canvas positioning sets `style="--offset: {x}px"` on elements, which CSP3 governs via `style-src-attr` (falling back to `style-src`). If we eliminate inline-attr styles in Phase 4a we drop this too.
  - **`withGlobalTauri: false`.** No `window.__TAURI__` global; everything goes through `import { invoke } from '@tauri-apps/api/core'`. Smaller attack surface from page scripts.
  - **Capabilities (least-privilege per window).** Plugin permissions are declared in `src-tauri/capabilities/default.json` against the `main` window only. We grant exactly:
    - `core:default` (window resize, drag, etc.).
    - `fs:scope` *constrained at runtime* to the configured library roots plus `$XDG_CACHE_HOME/superpanels/thumbs/`. No blanket `fs:allow-read-file`.
    - `dialog:allow-open` for the file picker.
    - `notification:allow-notify` only if the user opted in.
    - **No** `shell:`, `http:`, `process:`, or `os:execute` permissions. Subprocesses run in our Rust code with our own subprocess rules (§10.3), never via the shell plugin.
  - **No `asset:` protocol for arbitrary paths.** Thumbnails and previews go through a dedicated IPC command (`library_thumbnail`) that returns bytes; the frontend wraps them as `blob:` URLs. This means a webview script can't read arbitrary files via `asset://` even if CSP is bypassed.
- Custom IPC commands (the ones in §12.4) are app-level handlers — Tauri's capability system gates *plugin* permissions, not custom `#[tauri::command]`s. We mitigate that by validating every command's input as if it were untrusted: paths are canonicalised and verified to be inside an allowed root before any FS access; profile names are matched against the on-disk profile list, never used as path components.
- No `unsafe` Rust in our crates. Allowed via `#![forbid(unsafe_code)]`.
