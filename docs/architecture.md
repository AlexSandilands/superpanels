# Architecture & code organization

> Where things go, how big they should be, and what to call them.

This doc is for *structural* decisions. For Rust *style* (errors, idioms, API design) see [`style-rust.md`](./style-rust.md). For frontend style see [`style-frontend.md`](./style-frontend.md).

---

## Table of contents

- [Workspace layout](#workspace-layout)
- [Crate responsibilities](#crate-responsibilities)
- [Dependency direction](#dependency-direction)
- [Module organization](#module-organization)
- [File and module sizing](#file-and-module-sizing)
- [Naming conventions](#naming-conventions)
- [Configuration ownership](#configuration-ownership)
- [Workspace lints (the canonical list)](#workspace-lints-the-canonical-list)
- [Adding a new module](#adding-a-new-module)
- [Adding a new dependency](#adding-a-new-dependency)

---

## Workspace layout

```
superpanels/
├── Cargo.toml                       ← workspace root, [workspace.lints]
├── Cargo.lock                       ← committed (we ship binaries)
├── rust-toolchain.toml
├── rustfmt.toml
├── clippy.toml
├── deny.toml
├── typos.toml
├── .pre-commit-config.yaml
├── .editorconfig
├── .gitignore
├── SPEC.md
├── PLAN.md
├── CONTRIBUTING.md
├── README.md
├── LICENSE / LICENSE-MIT / LICENSE-APACHE
├── crates/
│   ├── superpanels-core/            ← pure-Rust library, no UI, no IPC
│   ├── superpanels-cli/             ← clap CLI binary
│   ├── superpanels-daemon/          ← background daemon binary
│   └── superpanels-gui/             ← Tauri shell binary (feature-gated)
├── ui/                              ← Svelte 5 frontend (its own package.json)
├── docs/                            ← architecture, style, testing
├── packaging/                       ← PKGBUILDs, flatpak manifest, .desktop files
└── .github/workflows/               ← CI definitions (Phase 1+)
```

The single distributable binary `superpanels` is produced by the GUI crate when built with `--features gui`, or by the CLI crate when built without. Subcommand dispatch unifies them externally; internally, they are separate Cargo targets so building the CLI alone doesn't pull WebKitGTK.

---

## Crate responsibilities

Read this carefully — drift between intent and reality is a code smell.

### `superpanels-core`
- **Pure logic.** Bezel math, image processing, config parsing, library indexing, slideshow logic, schedule evaluation, backend trait + implementations, display-detector trait + implementations.
- **No I/O against the running system except through trait boundaries** (e.g. `WallpaperBackend::apply` is a trait; concrete backends live here but are exercised through the trait).
- **No UI, no IPC, no Tauri, no clap, no tokio runtime construction.** The core can be driven sync or async; it doesn't decide.
- **Fully testable without a desktop session.**

### `superpanels-cli`
- **Argument parsing** with clap.
- **Subcommand dispatch** to core functions.
- **Output formatting** (human, JSON).
- **Logging setup** (tracing-subscriber for terminal).
- Talks to the daemon over IPC if one is running; otherwise calls core directly.

### `superpanels-daemon`
- **Tokio runtime.**
- **IPC server** over Unix socket.
- **Slideshow timer** and **schedule evaluator**.
- **FS watcher** for library roots.
- Holds runtime state in memory; persists to disk on relevant events.

### `superpanels-gui`
- **Tauri shell.**
- **Tauri command wrappers** — each is a 3–5 line bridge to a core function.
- **System tray.**
- The Svelte frontend in `ui/` is the actual UI; this crate only hosts it.

---

## Dependency direction

```
                    ┌──────────────────┐
                    │ superpanels-core │
                    └────────▲─────────┘
                             │
            ┌────────────────┼────────────────┐
            │                │                │
   ┌────────┴───────┐ ┌──────┴───────┐ ┌──────┴───────┐
   │ superpanels-   │ │ superpanels- │ │ superpanels- │
   │ cli            │ │ daemon       │ │ gui          │
   └────────────────┘ └──────────────┘ └──────────────┘
```

**Rules:**
- `core` depends on no other crate in this workspace.
- `cli`, `daemon`, `gui` all depend on `core`.
- `cli`, `daemon`, `gui` **do not depend on each other**. The IPC protocol crate (if it grows enough to need one) lives in `core` or in a small shared `superpanels-ipc` crate, not as a dep between binaries.

If you find yourself wanting `cli` to import from `daemon`, stop and refactor — the shared piece belongs in `core`.

---

## Module organization

Inside any crate, organise by *concept*, not by *layer*. Avoid generic dumping grounds:

- ❌ `utils.rs`, `helpers.rs`, `common.rs`, `misc.rs`
- ❌ `types.rs` for the whole crate (per-module `types.rs` *is* fine)
- ✅ `display/`, `layout.rs`, `image.rs`, `library.rs` — each named for what it owns

A module in Rust is a directory or a file. Use the **module-as-directory** form (`module.rs` + `module/submodule.rs`) introduced in Rust 2018 — not the older `mod.rs` form. So:

```
✅                              ❌
display.rs                      display/mod.rs
display/
  kscreen.rs                    display/kscreen.rs
  xrandr.rs                     display/xrandr.rs
```

Why: searching `git log -- display.rs` finds the module's main file directly; `mod.rs` files all share the same name and confuse history tools.

Sub-modules are private (`mod foo;`) by default. Re-export only what the rest of the crate needs via the parent module.

---

## File and module sizing

| | Soft limit | Hard limit |
|---|---|---|
| Production `.rs` files | **400 lines** | **600 lines** |
| Test modules (`#[cfg(test)] mod tests`) | no limit | no limit |
| Svelte component files | **200 lines** | **350 lines** |
| TS files | **300 lines** | **500 lines** |
| Generated code | n/a | n/a |

"Lines" means total lines including comments and blanks — what `wc -l` reports — because *that's the cost of reading the file*. We're not optimising the metric; we're optimising readability.

**Soft vs hard:** a single file at 450 lines is fine if splitting it would be artificial. A single file at 700 lines is a code smell that needs to be addressed before merging. The pre-commit hooks don't enforce this — humans do, in code review.

**When you hit the limit, split by responsibility, not by line count.** If `library.rs` is 550 lines, look at what it does:
- File scanning?
- FS watching?
- SQLite I/O?
- Filtering?

Each is a candidate for its own submodule. The wrong split is "library_part_2.rs" — that's just the same file with worse navigation.

**Common splits that work well:**
- `module.rs` (public API + structs) + `module/types.rs` (private types) + `module/parser.rs` (one specific algorithm)
- `module.rs` (orchestrator) + `module/<concrete>.rs` per implementation (the detector pattern)

---

## Naming conventions

### Rust (enforced by clippy / rustfmt)

| Thing | Convention | Example |
|---|---|---|
| Modules / files | `snake_case` | `display`, `layout.rs` |
| Types (struct, enum, trait, type alias) | `PascalCase` | `Monitor`, `BezelConfig`, `WallpaperBackend` |
| Enum variants | `PascalCase` | `Rotation::Left`, `FitMode::Fill` |
| Functions / methods / variables | `snake_case` | `compute_crop_specs`, `monitor_id` |
| Constants / statics | `SCREAMING_SNAKE_CASE` | `DEFAULT_BEZEL_MM`, `MAX_DECODE_BYTES` |
| Lifetime parameters | short, lowercase | `'a`, `'src` |
| Type parameters | single letter or `PascalCase` | `T`, `Monitor`, `BackendImpl` |
| Crate names | `kebab-case` | `superpanels-core` (becomes `superpanels_core` in code) |

### Frontend

| Thing | Convention | Example |
|---|---|---|
| Svelte components | `PascalCase.svelte` | `MonitorCanvas.svelte` |
| TS modules | `kebab-case.ts` | `canvas-render.ts` |
| Stores | `kebab-case.ts` | `profile-store.ts` |
| Test files | `<name>.test.ts` | `bezel.test.ts` |

### General

- **No abbreviations** unless the long form is genuinely awkward. `monitor`, not `mon`. `configuration` is acceptable as `config` because that's the universal short form.
- **No Hungarian notation.** `image_path: PathBuf` not `path_image`. Type information lives in the type system.
- **Verbs for functions, nouns for types.** `compute_crop_specs()`, not `crop_spec_computer()`.
- **Boolean-returning functions read like predicates.** `is_available()`, `has_rotation()`, `should_skip()`.

---

## Configuration ownership

| File | Owner | Purpose |
|---|---|---|
| `Cargo.toml` (root) | workspace | Members, `[workspace.lints]`, shared dep versions |
| `Cargo.toml` (per crate) | that crate | Crate metadata, deps, features |
| `rustfmt.toml` | repo root | Code formatting |
| `clippy.toml` | repo root | Clippy thresholds (levels live in `Cargo.toml`) |
| `rust-toolchain.toml` | repo root | Rustc channel + components |
| `deny.toml` | repo root | Licence / advisory policy |
| `typos.toml` | repo root | Spell-check config |
| `.editorconfig` | repo root | Editor-agnostic indent/charset |
| `.prettierrc.json` | repo root | Frontend formatting |
| `eslint.config.js` | `ui/` | Frontend linting (created in Phase 3) |
| `tsconfig.json` | `ui/` | TS compiler config (created in Phase 3) |
| `tauri.conf.json` | `crates/superpanels-gui/` | Tauri build config |

Don't duplicate config across crates. If you need a per-crate override, document why in the crate's `Cargo.toml`.

---

## Workspace lints (the canonical list)

This block goes into the root `Cargo.toml` once the workspace is scaffolded (Phase 1.1):

```toml
[workspace.lints.rust]
unsafe_code        = "forbid"        # we forbid unsafe — `forbid` is stronger than `deny`
missing_docs       = "warn"          # public items in libraries should be documented
unused             = "warn"
nonstandard_style  = "warn"
rust_2018_idioms   = "warn"
unreachable_pub    = "warn"
trivial_casts      = "warn"
trivial_numeric_casts = "warn"

[workspace.lints.clippy]
# Tier 1: catch real bugs and style issues.
all      = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }

# Things we want to be loud about:
unwrap_used     = "warn"
expect_used     = "warn"
panic           = "warn"
todo            = "warn"
unimplemented   = "warn"
dbg_macro       = "warn"
print_stdout    = "warn"
print_stderr    = "warn"   # use `tracing` or `eprintln!` only in CLI top-level
mod_module_files = "warn"  # enforce module.rs over mod.rs

# Pedantic lints that are noisy without being useful — selectively allow.
module_name_repetitions = "allow"
must_use_candidate      = "allow"
missing_errors_doc      = "allow"
missing_panics_doc      = "allow"
return_self_not_must_use = "allow"
```

Per-crate Cargo.toml inherits with:

```toml
[lints]
workspace = true
```

Each crate may override individual lints if it has a documented reason (e.g. the CLI may `allow(print_stdout)` because that *is* its job).

---

## Adding a new module

1. Decide where it goes. If it's general logic, it goes in `superpanels-core`. If it's CLI-only or daemon-only, in those crates.
2. Pick a name that describes what it owns, not what kind of thing it is. `library.rs` not `library_manager.rs`.
3. Write the public API as a sketch first — types, function signatures, doc comments. *Don't* fill in bodies yet.
4. Write at least one test that calls the public API as a future user would. The test will fail to compile; that's fine.
5. Implement.
6. If the file passes 400 lines, plan a split *before* it passes 600.

---

## Adding a new dependency

Dependencies are debt. Each one is something we have to keep up with.

Before adding:

1. **Could a few lines of our own do the job?** If yes, just write it.
2. **Is the crate maintained?** Check last release date and open PR turnaround. Anything > 1 year stale needs justification.
3. **What's its dep tree?** A leaf crate with 0 deps is essentially free. A crate that pulls in 50 deps is not.
4. **Licence?** Must be in our `deny.toml` allow-list.
5. **Run `cargo deny check`** before committing.

In the commit message, justify the addition in one line: `feat(library): add notify@7 for FS-watch — used by daemon root scanner`.

If you're not sure, ask in the PR rather than just adding it.
