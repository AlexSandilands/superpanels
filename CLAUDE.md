# Superpanels

Linux wallpaper manager focused on **physical-bezel-aware multi-monitor spanning** and folder-driven slideshows. Single binary; Rust core, Tauri v2 + Svelte 5 GUI. Primary target: Arch / CachyOS on KDE Wayland.

**Pre-code.** Only specs / docs / configs exist — no `Cargo.toml` yet. Workspace scaffold is `PLAN.md` Phase 1.1.

## Doc map — read what's relevant to the work

| Working on | Read |
|---|---|
| Feature design / what we're building | `SPEC.md` |
| Picking next task / phase progress | `PLAN.md` |
| Adding/moving modules, file sizes, naming, deps | `docs/architecture.md` |
| Writing Rust (errors, idioms, API design) | `docs/style-rust.md` |
| Writing TypeScript / Svelte | `docs/style-frontend.md` |
| Writing or running tests | `docs/testing.md` |
| Local setup, tools, hooks | `CONTRIBUTING.md` |

## Stack

Rust workspace: `crates/superpanels-{core,cli,daemon,gui}`. **Core is pure logic** — no UI, no IPC, no runtime construction. CLI / daemon / GUI are thin wrappers around core.
Frontend: `ui/` — Svelte 5 with **runes** (`$state`/`$derived`/`$effect`/`$props`), TypeScript `strict: true`, Tailwind.
Tooling: `cargo`, `rustfmt`, `clippy`, `cargo-deny`, `typos`, `pre-commit`, `prettier`, `eslint`, `svelte-check`. Configs at repo root.

## Hard rules (clippy + pre-push enforce most via `-D warnings`)

- `#![forbid(unsafe_code)]` on every crate. **Stable Rust only — no nightly features.**
- **No `unwrap` / `expect` / `panic!` / `todo!` / `dbg!` / `println!`** outside `#[cfg(test)]` and `main()`. Use `?` + `thiserror` (libraries) or `anyhow::Context` (binaries).
- **No lossy `as` casts.** Use `try_from` / `try_into` and handle the error.
- **Parameters take `&str` / `&Path` / `&[T]`**, not owned types, unless ownership is genuinely needed.
- **TypeScript:** no `any` (eslint `error`); no `console.log`; `import type` for types.
- **File size:** Rust 400 lines soft / 600 hard; Svelte 200 / 350; TS 300 / 500. Split by responsibility, not line count.
- **Modules:** `module.rs` + `module/sub.rs` — never `module/mod.rs`.
- **No `#[allow(...)]`** without an inline `// reason: ...` comment.
- **Subprocesses:** `Command::arg()` per arg, never shell interpolation. Always with timeout + stderr capture.

## Testing

- Unit tests in same file: `#[cfg(test)] mod tests { ... }`. Integration tests in `crates/<x>/tests/`.
- `insta` for parser snapshots; `proptest` for bezel-math invariants; doc tests on every public-API example.
- Backends are tested via `MockBackend` — never touch a real desktop in tests.
- `tempfile::tempdir()` for FS work, never raw `/tmp`. Tests must be deterministic.
- Naming: `<scenario>_<expected_outcome>`. Structure: AAA (Arrange / Act / Assert).

## Commands

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features        # or: cargo nextest run
cargo deny check
pre-commit run --all-files --hook-stage pre-push
```

Don't bypass hooks (`--no-verify`) — fix the issue. Commit messages follow Conventional Commits (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`).

## Gotchas

- **Bezel math is in physical millimetres**, not pixels. The image maps onto the physical desktop plane *including* bezel gaps; pixel-only thinking gives wrong results. See `SPEC.md` §4.
- **Monitor identity** uses `MonitorRef { stable_id, name }`. `stable_id` is the KDE per-output UUID on KDE, or a hash of `manufacturer+model+serial` on other compositors. Names like `DP-1` are unstable across reboots / dock plugs — don't key persistent data on them.
- **Monitor physical mm comes from config, not detection.** `kscreen-doctor` doesn't expose it. `Monitor.physical_size_mm: Option<…>` is `None` until the user has filled in a `[[monitor]]` block (or used the GUI's first-run flow). `compute_crop_specs` returns `LayoutError::PhysicalSizeMissing` when any monitor lacks one.
- **The CLI runs without the daemon** (in-process fallback). Don't assume a daemon is present for one-shot operations.
- **Logging uses `tracing` with structured fields**, never `format!` into the message: `info!(monitor = %name, "applied")`, not `info!("applied {name}")`.
- **KDE backend** calls `org.kde.PlasmaShell.evaluateScript` via zbus — image paths are JSON-quoted into the script template, never string-concatenated. See `SPEC.md` §10.4.
