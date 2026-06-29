# Superpanels

Linux wallpaper manager focused on **physical-bezel-aware multi-monitor spanning** and folder-driven slideshows. Single binary; Rust core, Tauri v2 + Svelte 5 GUI. Primary target: Arch / CachyOS on KDE Wayland.

## Doc map — read ONLY what's relevant to the current task

| Working on | Read |
|---|---|
| Layout / monitor-gap math | `docs/reference/layout-math.md` |
| Display detection / monitor identity | `docs/reference/displays.md` |
| Wallpaper backends, subprocess rules | `docs/reference/backends.md` |
| Config schema, state files, validation | `docs/reference/configuration.md` |
| Threat model, IPC invariants, Tauri hardening | `docs/reference/security.md` |
| Adding/moving modules, file sizes, naming, deps | `docs/contributing/architecture.md` |
| Writing Rust (errors, idioms, API design) | `docs/contributing/style-rust.md` |
| Writing TypeScript / Svelte | `docs/contributing/style-frontend.md` |
| Writing or running tests | `docs/contributing/testing.md` |
| Releasing — packaging, AUR, crates.io | `packaging/README.md` |
| Deferred follow-ups / known workarounds | GitHub issues (see below) |
| Local setup, tools, hooks | `CONTRIBUTING.md` |

## Follow-ups live in GitHub issues

Deferred work, known workarounds, and "we'll do this later" items are tracked as **GitHub issues** (`gh issue list`), not in a doc. The old `docs/followups.md` has been retired.

**Confirm before logging.** Never create, edit, or close a GitHub issue without explicit confirmation from the user first — show the proposed title and body and wait for a go-ahead. The user will ask you to log follow-ups there; treat that as the trigger, not a standing licence to file issues unprompted.

## MCP context tools — required for impact analysis

At session start, confirm the `jcodemunch` and `jdocmunch` MCP servers are connected (they appear in the deferred-tools list). When available, they are the **default** for repo context — they return targeted slices instead of whole files, keeping the working set small.

- **`jcodemunch`** — code: structure, symbols, references, importers, call hierarchies, blast radius.
- **`jdocmunch`** — docs in `docs/`: search by topic, "is this already documented?", and finding which docs need updating after a code change.

**MCP is required (not optional) before any change that:**
- modifies the body or signature of a function/method called from outside its file → run `find_references` / `get_call_hierarchy` to confirm every caller is fine;
- spans multiple files, or you're unsure how many it spans → run `get_blast_radius` or `find_importers`;
- touches a public type, trait, module re-export, or anything in `crates/superpanels-core` → its callers live in cli/daemon/gui and `grep` won't catch them all reliably.

**`grep` / `Read` is fine for:** reading a known path, a single-string lookup with a single expected hit (e.g. "where is `const TIMEOUT` defined"), or quick existence checks.

**Don't rationalise that a query is "too narrow."** If you've run 2+ greps to triangulate a symbol or its callers, you should have started with `jcodemunch`. The cost of an MCP call is much lower than the cost of missing a caller and shipping a broken change.

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
- **Comments earn their place.** Default to none. A comment is justified only when the *why* is non-obvious — a hidden constraint, an external-API quirk, a workaround, or a cross-ref to `docs/` for math the reader would otherwise get wrong. Do **not** write comments that restate the type, name, or signature.
  - Module headers: one line. No design essays — those belong in `docs/`.
  - Public items: one-line summary. Add detail only when behavior is surprising.
  - Field / enum-variant docs: omit unless the name is genuinely ambiguous.
  - `# Errors`: list a variant only if its trigger isn't obvious from its name.
  - `# Examples`: only for non-obvious usage (edge cases, gotchas). Skip "construct and read default" demos — `cargo test --doc` doesn't need filler.
  - Inline: explain *why*, never *what*. If naming makes the *what* clear, the comment is noise.
  - See `docs/contributing/style-rust.md` §Comments for examples of good vs. bad.

## Testing

- Unit tests in same file: `#[cfg(test)] mod tests { ... }`. Integration tests in `crates/<x>/tests/`.
- `insta` for parser snapshots; `proptest` for layout-math invariants; doc tests on every public-API example.
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

- **Layout math is in physical millimetres**, not pixels. The image maps onto the physical desktop plane *including* the monitor gap (bezel + air-gap) between panels; pixel-only thinking gives wrong results. See `docs/reference/layout-math.md`.
- **Monitor identity** uses `MonitorRef { stable_id, name }`. `stable_id` is the KDE per-output UUID on KDE, or a hash of `manufacturer+model+serial` on other compositors. Names like `DP-1` are unstable across reboots / dock plugs — don't key persistent data on them.
- **Monitor physical mm comes from config, not detection.** `kscreen-doctor` doesn't expose it. `Monitor.physical_size_mm: Option<…>` is `None` until the user has filled in a `[[monitor]]` block (or used the GUI's first-run flow). `compute_crop_specs` returns `LayoutError::PhysicalSizeMissing` when any monitor lacks one.
- **The CLI runs without the daemon** (in-process fallback). Don't assume a daemon is present for one-shot operations.
- **Logging uses `tracing` with structured fields**, never `format!` into the message: `info!(monitor = %name, "applied")`, not `info!("applied {name}")`.
- **KDE backend** calls `org.kde.PlasmaShell.evaluateScript` via zbus — image paths are JSON-quoted into the script template, never string-concatenated. See `docs/reference/backends.md` (KDE section).
