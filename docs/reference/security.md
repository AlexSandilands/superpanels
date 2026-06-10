# Security & sandbox

Threat model and the invariants every IPC handler must hold. The validators live in [`crates/superpanels-core/src/ipc/validate.rs`](../../crates/superpanels-core/src/ipc/validate.rs); the daemon socket and Tauri commands share them so neither path can drift.

## Threat model

We treat the webview as **potentially compromised**. A script in the renderer can call any `#[tauri::command]` we expose, in any order, with any payload. Validation lives in Rust; the frontend's clamps and form rules are UX, not enforcement.

**In scope:**

- **Hostile webview.** Anything reachable from `commands.rs` is hostile input.
- **Hostile daemon-socket peer.** The IPC socket lives at `$XDG_RUNTIME_DIR/superpanels/daemon.sock` (or `/tmp/superpanels-<uid>/daemon.sock` when XDG is unavailable, with 0700 enforced on the directory). XDG_RUNTIME_DIR is 0700 by spec, so only the user's own processes can connect — but we still treat socket frames as untrusted (frame-length cap, typed validation, no shell-out on user strings).
- **Hostile image content.** Image bytes from disk are decoded in a worker thread with a 512 MB memory cap (rejects PNG/JPEG bombs).
- **Hostile profile / config TOML.** User-edited config can carry malformed bytes; `Config::load_from` validates before use.

**Out of scope:**

- **Hostile user.** The user owns the box. `set_autostart`, custom backend commands, and library roots run with the user's privileges; we surface clear "runs with your privileges" UX, not a sandbox.
- **Compromised host kernel / compositor.** If Wayland is malicious, no userspace mitigation helps.
- **Network attackers.** No HTTP fetching; nothing listens on a TCP port.

## IPC input invariants

Every `#[tauri::command]` and every daemon-socket handler validates its inputs against the rules below, regardless of which side received them. See [`configuration.md`](./configuration.md) for the full table; key cases:

- `physical_mm`: each component finite, `> 0`, `≤ 10_000` mm.
- `stable_id`, `name`: non-empty, `≤ 256` chars, no control chars / newlines.
- `tag`: non-empty after trim, `≤ 64` chars, no control chars.
- `path` (any library command): canonicalised + verified inside library roots before lookup.
- `Profile.name`: non-empty, `≤ 64` chars post-trim.
- `Profile.body::Span::Slideshow.images`: `≤ 10_000` entries.
- `Profile.body::Span::Slideshow.overrides`: `≤ 1_000` entries, placements + rect finite.
- `Config.profiles`: `≤ 256` entries.
- `Config.monitors`: `≤ 64` entries.
- `offset_px`, `image_size_px`: each component `|v| ≤ 1_000_000`.

Bound choices target *human plausibility, not perf*: the largest production monitor diagonal is ~110″, the longest plausible monitor name is well under 256 chars, and a slideshow with 10 000 images is at the edge of folder-watch sanity. Hitting any cap means the input is malicious or malformed.

Subprocess-bound inputs (custom backend commands, KDE PlasmaShell scripts) follow [`backends.md`](./backends.md): `Command::arg()` per arg, never shell interpolation; KDE script templates JSON-quote paths, never string-concatenate.

## Tauri v2 hardening

The defaults are not safe enough for an app that touches user-supplied paths and runs subprocesses. We lock down four surfaces:

### CSP

`tauri.conf.json` sets `app.security.csp` explicitly:

```
default-src 'self';
script-src  'self';
style-src   'self' 'unsafe-inline';   /* inline style="" for canvas positioning */
img-src     'self' data: blob:;        /* thumbnails arrive as bytes via IPC, become blob: URLs */
connect-src 'self' ipc: http://ipc.localhost;  /* Tauri v2 IPC transport */
object-src  'none';
base-uri    'self';
frame-ancestors 'none';
```

No `'unsafe-eval'`, no `'unsafe-inline'` on `script-src`. `'unsafe-inline'` on `style-src` is the one concession — Svelte's runes-based canvas positioning sets `style="--offset: {x}px"` on elements, which CSP3 governs via `style-src-attr` (falling back to `style-src`).

### `withGlobalTauri: false`

No `window.__TAURI__` global; everything goes through `import { invoke } from '@tauri-apps/api/core'`. Smaller attack surface from page scripts.

### Devtools off in release

The `tauri/devtools` Cargo feature is gated behind `superpanels-gui`'s `dev-tools` feature flag; release builds omit it. Inspect via `just gui-dev` (or `cargo run --features dev-tools`) when needed.

### Capabilities (least-privilege per window)

Plugin permissions are declared in `src-tauri/capabilities/default.json` against the `main` window only. We grant exactly:

- `core:default` (window resize, drag, etc.).
- `fs:scope` *constrained at runtime* to the configured library roots plus `$XDG_CACHE_HOME/superpanels/thumbs/`. No blanket `fs:allow-read-file`.
- `dialog:allow-open` for the file picker — see the [file-picker note](#tauri-file-picker-uses-the-xdg-portal-feature) below.
- `notification:allow-notify` only if the user opted in.
- **No** `shell:`, `http:`, `process:`, or `os:execute` permissions. Subprocesses run in our Rust code with our own subprocess rules, never via the shell plugin.

### No `asset:` protocol for arbitrary paths

Library thumbnails go through `library_thumbnail`, which is constrained to configured roots. Selected/dropped source previews go through `source_thumbnail` — see the exception below.

## `source_thumbnail` — documented exception

`source_thumbnail` is the one IPC command that does **not** check inside library roots. Its job is to render a preview of an image the user has just picked from a system file dialog (or referenced from a saved profile body via an absolute path). Rooting it to library roots would break the file-picker flow.

The trust boundary is *anything reachable through a saved profile body or a live `dialog:allow-open` result*. Concretely, under a hostile webview, a webview script can call `source_thumbnail("/home/user/secret.png")` and receive the decoded image as base64 PNG bytes — **a compromised webview can exfiltrate the contents of any image file the daemon's UID can read.**

We accept this in v1 because:

- The decode-memory cap (512 MB) prevents DoS via image bombs.
- File picks are user-initiated; profile bodies are user-saved. No unbounded discovery primitive (no `list_files`, no glob).
- Anything more restrictive demands a pick-time allowlist persisted across the webview lifecycle — a feature, not hardening.

A future tightening would canonicalise dialog-pick results at pick-time and stash an allowlist in `AppState`, so `source_thumbnail` rejects paths the user hasn't explicitly chosen this session. Tracked in `docs/followups.md`.

## Custom IPC commands

Tauri's capability system gates *plugin* permissions, not custom `#[tauri::command]`s. The IPC-input invariants above are how we mitigate that — every command-handler entry validates its payload against bounded, typed contracts before any FS access, config write, or layout computation. Daemon and in-process handlers share the same `ipc::validate` module.

## Other

- Image decoding happens in a worker thread. The `image` crate is safe Rust but image files come from untrusted sources, so we cap decode memory at a configurable limit (default 512 MB).
- Custom backend commands are user-supplied; we run them as the user. The GUI's custom-command field shows a "runs with your privileges" callout.
- No HTTP fetching; no TCP port listens.
- No `unsafe` Rust in our crates (`#![forbid(unsafe_code)]` on every crate).

## Tauri file picker uses the `xdg-portal` feature

`tauri-plugin-dialog` pulls in `rfd` (the underlying picker). `rfd`'s default backend is GTK3, which produces a bare, non-themed dialog on KDE/Wayland. We turn on `rfd`'s `xdg-portal` feature so the picker goes through `xdg-desktop-portal-kde`/`gtk` and inherits the user's theme. **Never shell out to `kdialog` or `zenity`** — that would land us back in subprocess-validation territory.
