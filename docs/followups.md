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

**Benches missing for new hot paths**

- No `criterion` bench covers `read_dimensions`, `library_thumbnail` at
  4K source, `library_list` at 5K entries, or the `temp::save_temp_in`
  fast-PNG change. Add them so SPEC §19's 5 ms / call canvas budget and
  200 ms / 4K thumbnail target become tracked baselines. Phase 6
  stabilisation is the natural home.

**Supply chain**

- `cargo deny check` flags advisories on Tauri's transitive deps —
  `RUSTSEC-2025-0098` (`unic-ucd-version`), `RUSTSEC-2024-0413` (`gtk`),
  among others. Not introduced by Phase 3 but newly visible in our lockfile
  because adding `image` runtime to daemon expanded the resolved graph.
  Track upstream Tauri / `gtk-rs` releases and bump when they ship clean.
