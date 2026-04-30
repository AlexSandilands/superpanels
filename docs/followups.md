# Follow-ups

Loose ends that aren't blocking, but should be revisited. Add an entry when
deferring a workaround or "we'll do this later" item; remove it when done.

## WebKitGTK DMABUF renderer workaround

`WEBKIT_DISABLE_DMABUF_RENDERER=1` is set in three places to dodge a WebKitGTK
crash (`Gdk-Message: Error 71 (Protocol error)`) on Wayland under common
NVIDIA / Mesa + KDE Plasma 6 stacks:

- `.cargo/config.toml` — for `cargo run` / `cargo test`.
- `justfile` (`gui` recipe) — for direct release-binary invocation.
- `crates/superpanels-gui/src/autostart.rs` (`DESKTOP_BODY`) — for the
  installed `~/.config/autostart/superpanels.desktop` entry.

**Revisit when:** WebKitGTK ships a fix for the DMABUF renderer crash on the
affected stacks (track upstream WebKit / `webkit2gtk` Arch package).

**Action when fixed:** drop the env var from all three sites and the
`set_enabled_at_true_writes_desktop_file` /
`desktop_body_includes_webkit_dmabuf_workaround` test assertions in
`autostart.rs`.

## Phase 3 review nitpicks (advisory)

Logged by the agent-team review on 2026-04-30 after Phase 3 landed. None
blocked; all are small enough to defer.

**Code dedup**

- `crates/superpanels-cli/src/ipc_client.rs` and
  `crates/superpanels-gui/src/ipc_client.rs` are near-duplicates (frame format,
  timeouts, `MAX_FRAME_BYTES`, error enum). When either is next touched, hoist
  into `superpanels-core::ipc::client` (sync, no tokio).
- `bezel_mm_to_f32` / `bezel_h_to_f32` are duplicated between
  `crates/superpanels-daemon/src/server/handlers.rs` and
  `crates/superpanels-gui/src/commands/in_process.rs`. Move to a shared
  helper in `superpanels-core` next time bezel parsing changes.

**IPC validation consistency**

- `cmd_library_tag` (`server/handlers/library.rs`) does an exact-`PathBuf`
  match against existing entries; semi-equivalent paths
  (`/walls/./x.png`, symlinks, trailing slash) silently miss. Apply the
  existing `canonicalise_inside_roots` helper so all path-bearing IPC
  commands canonicalise uniformly.
- `canonicalise_inside_roots` silently skips a configured root whose own
  canonicalisation fails (`is_ok_and(...)` short-circuits). Behaviour is
  fail-deny and correct — add a one-line comment so a future reader doesn't
  "improve" it into fail-open.

**Tray polling**

- `crates/superpanels-gui/src/tray.rs::spawn_poller` is an unbounded
  `loop { sleep; … }` thread with no shutdown signal — teardown leaks the
  thread. Wire a `tokio::sync::Notify` or an `AtomicBool` shutdown flag.
- The poller refetches `list_profiles` every tick regardless of whether
  anything changed; cache the response signature and skip the call when
  unchanged to keep daemon-idle CPU under SPEC §19's 0.1 % budget.

**Library thumbnail cache**

- `cmd_library_thumbnail` decodes + resizes on every call. Add an LRU keyed
  on `(canonicalised_path, mtime)` (cap at ~64 entries / ~16 MiB) once the
  GUI starts hitting it at scale. Phase 4a / 4b territory.

**Benches missing for new hot paths**

- No `criterion` bench covers `read_dimensions`, `library_thumbnail` at
  4K source, `library_list` at 5K entries, or the `temp::save_temp_in`
  fast-PNG change. Add them in the Phase 4a PR that wires the canvas drag
  so SPEC §19's 5 ms / call canvas budget and 200 ms / 4K thumbnail target
  become tracked baselines.

**GUI hygiene**

- `crates/superpanels-gui/src/lib.rs` declares every submodule `pub mod`.
  Tighten to `pub(crate)` for the modules that aren't part of the
  `tauri::generate_handler!` consumer surface (`autostart`, `bridge`,
  `notifications`, `state`, `tray`, `window_state`, `ipc_client`).
- `ui/eslint.config.js` allows `console.warn` and `console.error` via
  `no-console: ['error', { allow: ['warn', 'error'] }]`. The frontend
  style guide disallows `console.error` in committed code too — drop the
  allow-list.
- `crates/superpanels-gui/src/commands.rs` is at 274 LoC, nearing the 400
  soft limit. If non-§12.4 commands keep accreting, peel
  `commands/autostart.rs` off (the `commands/in_process.rs` precedent
  establishes the directory form).

**Test gaps**

- `apply_tag` no-op match arms (`(true, Some(_))`, `(false, None)`, and
  the `favourite=false` path) aren't exercised in
  `library_tag_toggles_tag_and_favourite`.
- `library_list` multi-roots case isn't exercised in either the daemon or
  in-process tests.
- `tray::handle_menu_event`'s profile-prefix parsing (`name != "empty"`
  guard) has no unit coverage. Tauri-bound code is hard to drive in unit
  tests; consider extracting the pure-string parsing into a helper.

**Misc small things**

- `crates/superpanels-gui/src/commands/in_process.rs` `apply_profile`
  validates `params.name` only to discard it (`_name`) and unconditionally
  return an error. Either remove the dead validation or wire the
  in-process apply path properly.
- `crates/superpanels-gui/src/commands.rs` calls
  `serde_json::to_value(&args).unwrap_or(Value::Null)` for typed inputs
  that should never fail to serialise — surface the error instead.
- 3 ts-rs-generated TS files (`IpcError.ts`, `LibraryFilter.ts`,
  `PreviewArgs.ts`) trip Prettier; the generated output doesn't match the
  project's formatter. Add a Prettier-ignore for `ui/src/lib/types/*.ts`
  or post-process the ts-rs output through `prettier --write` in `build.rs`.

**Supply chain**

- `cargo deny check` flags advisories on Tauri's transitive deps —
  `RUSTSEC-2025-0098` (`unic-ucd-version`), `RUSTSEC-2024-0413` (`gtk`),
  among others. Not introduced by Phase 3 but newly visible in our lockfile
  because adding `image` runtime to daemon expanded the resolved graph.
  Track upstream Tauri / `gtk-rs` releases and bump when they ship clean.
