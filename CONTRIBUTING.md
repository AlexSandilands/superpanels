# Contributing to Superpanels

> Read this first. It points you at every other doc you'll need.

This file is the entry point for working on Superpanels. The deeper guides live in [`docs/`](./docs/) and are linked from each section below.

---

## Table of contents

- [What you're building](#what-youre-building)
- [Local dev setup](#local-dev-setup)
- [Pre-commit hooks](#pre-commit-hooks)
- [The doc map](#the-doc-map)
- [Workflow](#workflow)
- [Definition of done for any change](#definition-of-done-for-any-change)
- [Getting help with Rust](#getting-help-with-rust)

---

## What you're building

Superpanels is a Linux wallpaper manager focused on multi-monitor spanning with bezel correction. Topic-scoped reference docs live in [`docs/`](./docs/); start at [the doc map](#the-doc-map) below.

The project is a Cargo workspace (Rust) plus a Tauri shell with a Svelte 5 frontend (TypeScript). The Rust core is the ground truth; CLI, daemon, and GUI are thin wrappers around it.

---

## Local dev setup

You need:

| Tool | Why | Install on Arch / CachyOS |
|---|---|---|
| Rust toolchain | The core language | The `rust-toolchain.toml` file pins the channel; `rustup` installs it on first `cargo` call. Get rustup with `pacman -S rustup` then `rustup default stable`. |
| Node + npm | Frontend tooling (prettier, eslint, Svelte) | `pacman -S nodejs npm` |
| Tauri prerequisites | Building the GUI | `pacman -S webkit2gtk-4.1 base-devel curl wget file openssl librsvg` (Tauri v2 — no GTK3 indicator/appmenu packages needed) |
| pre-commit | Git hook runner | `pacman -S pre-commit` (or `pip install --user pre-commit`) |
| typos | Fast spell-checker (used by pre-commit) | `pacman -S typos` (or `cargo install --locked typos-cli`) |
| cargo-deny | Dep-policy enforcement | `cargo install --locked cargo-deny` |

Optional but recommended:

| Tool | Why |
|---|---|
| `cargo-watch` | Auto-rebuild on save: `cargo install --locked cargo-watch` |
| `cargo-nextest` | Faster, prettier test runner: `cargo install --locked cargo-nextest` |
| `cargo-machete` | Find unused dependencies: `cargo install --locked cargo-machete` |
| `samply` | Sampling profiler for performance work: `cargo install --locked samply` |

> **Always pass `--locked` to `cargo install`.** It tells cargo to use the exact dependency versions in the tool's published `Cargo.lock`, instead of resolving fresh from the registry — which avoids transient dep-resolution failures breaking your install. Some tools (e.g. `cargo-nextest`) refuse to install without it.

After installing tools, install the git hooks once per clone:

```sh
pre-commit install                      # installs pre-commit hooks
pre-commit install --hook-type pre-push # installs the slower pre-push hooks
```

Verify everything works:

```sh
pre-commit run --all-files
cargo check --workspace --all-features
```

---

## Pre-commit hooks

Hooks are configured in [`.pre-commit-config.yaml`](./.pre-commit-config.yaml) and split into two tiers:

- **On `git commit` (fast).** General hygiene (trailing whitespace, no merge markers, no large files), `cargo fmt --check`, `cargo check`, prettier, eslint, typos.
- **On `git push` (slower).** `cargo clippy -- -D warnings`, `cargo test`, `cargo deny check`, `svelte-check`.

If a hook fails on commit:
1. Read the message — most failures are auto-fixable (`cargo fmt`, `prettier --write`).
2. Fix and `git add` the changes.
3. Re-commit.

**Don't bypass hooks** with `--no-verify`. If you genuinely need to (e.g. a hook is broken), open an issue, fix the hook, and commit normally. Bypassing hooks is how broken `main` branches happen.

To run the full pre-push suite manually before pushing:

```sh
pre-commit run --all-files --hook-stage pre-push
```

---

## The doc map

| Doc | When to read it |
|---|---|
| [`docs/reference/layout-math.md`](./docs/reference/layout-math.md) | Anything touching the layout/crop algorithm or the monitor-gap model. |
| [`docs/reference/displays.md`](./docs/reference/displays.md) | Adding a detector or anything keying on monitor identity. |
| [`docs/reference/backends.md`](./docs/reference/backends.md) | Adding/modifying a wallpaper backend or its subprocess plumbing. |
| [`docs/reference/configuration.md`](./docs/reference/configuration.md) | Changing the config / state file shape, the profile schema, or validation bounds. |
| [`docs/reference/security.md`](./docs/reference/security.md) | Anything touching IPC, Tauri capabilities, or input validation. |
| [`docs/contributing/architecture.md`](./docs/contributing/architecture.md) | Before adding/moving a module — workspace map, file-size rules, naming. |
| [`docs/contributing/style-rust.md`](./docs/contributing/style-rust.md) | While writing Rust — idioms, error handling, API design, what to avoid. |
| [`docs/contributing/style-frontend.md`](./docs/contributing/style-frontend.md) | While writing TypeScript or Svelte — strict TS, runes, component conventions. |
| [`docs/contributing/testing.md`](./docs/contributing/testing.md) | When adding tests — unit/integration/property/snapshot conventions. |
| [`docs/release/packaging.md`](./docs/release/packaging.md) | Before tagging a release — AUR, crates.io, GitHub Actions. |
| [`docs/release/stabilisation.md`](./docs/release/stabilisation.md) | What still has to be true before 1.0. |

Deferred workarounds and known follow-up work are tracked as [GitHub issues](https://github.com/AlexSandilands/superpanels/issues), not in a doc.

Memorise the headings, not the contents. When you hit a question, you'll know which doc to grep.

---

## Workflow

1. Pick something to work on — open [GitHub issues](https://github.com/AlexSandilands/superpanels/issues) or the unticked items in `docs/release/stabilisation.md` / `docs/release/packaging.md`.
2. Make a feature branch: `git checkout -b feat/display-rotation-watch`.
3. **Write the test first** for non-trivial logic. See [`docs/contributing/testing.md`](./docs/contributing/testing.md).
4. Implement.
5. Run `cargo fmt && cargo clippy --fix --allow-dirty` before committing.
6. Commit in small, focused chunks. Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) — `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`.
7. Push — pre-push hooks run the full suite. If they pass, open a PR.

---

## Definition of done for any change

Every commit that lands on `main` must satisfy:

- [ ] All pre-commit and pre-push hooks pass.
- [ ] Tests cover the new behaviour. New non-trivial code has at least one test before it lands.
- [ ] No new `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!`, `dbg!`, or `println!` outside tests / `main` / `eprintln!` for user-facing CLI errors.
- [ ] No new `#[allow(...)]` without an inline `// reason: …` comment.
- [ ] No new dependencies added without a one-line justification in the commit message and confirmation that `cargo deny check` passes.
- [ ] Files modified that grew past ~500 lines are split into submodules — see [`docs/contributing/architecture.md`](./docs/contributing/architecture.md#file-and-module-sizing).
- [ ] Public API additions in `superpanels-core` have rustdoc.

---

## Getting help with Rust

If you're new to Rust, these are worth a few hours each:

- [The Rust Book](https://doc.rust-lang.org/book/) — chapters 1–10 cover 80% of what we use.
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/) — for searching "how do I do X".
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/) — *don't* read this; you don't write `unsafe` here.
- [Effective Rust](https://www.lurklurk.org/effective-rust/) — the canonical "writing better Rust" reference; pairs well with [`docs/contributing/style-rust.md`](./docs/contributing/style-rust.md).

When in doubt:
- Run `cargo clippy` and read every warning. Clippy is a great teacher.
- Search `https://docs.rs/<crate>` for whatever you're using.
- Ask: "What would a Rust idiomatic version of this look like?" The compiler is your friend; lean on it.
