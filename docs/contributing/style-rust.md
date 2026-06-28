# Rust style guide

> Idioms, error handling, API design. Opinions with reasons.

This is the doc for "how should I write *this* Rust" decisions. For "where does this code go" decisions, see [`architecture.md`](./architecture.md).

If you're new to Rust, these patterns will feel like a lot at first. They're not arbitrary — each one heads off a specific class of bug or maintenance pain. The compiler and clippy will nudge you toward most of them automatically.

---

## Table of contents

- [The forbidden list](#the-forbidden-list)
- [Error handling](#error-handling)
- [Ownership and borrowing](#ownership-and-borrowing)
- [API design](#api-design)
- [Newtypes for IDs](#newtypes-for-ids)
- [Strings, paths, and slices](#strings-paths-and-slices)
- [Concurrency](#concurrency)
- [Tracing and logging](#tracing-and-logging)
- [Comments and documentation](#comments-and-documentation)
- [Imports](#imports)
- [Common smells](#common-smells)

---

## The forbidden list

These are blanket rules. The pre-commit hooks catch most of them via clippy.

- **No `unsafe`.** `#![forbid(unsafe_code)]` on every crate. If you genuinely need `unsafe`, the answer is "find a crate that already encapsulates it" or "open an issue first".
- **No `unwrap()` / `expect()` outside tests and `main()`.** They turn handleable errors into panics. Use `?` or pattern-match.
- **No `panic!()`, `todo!()`, `unimplemented!()` in committed code.** Library code never panics. `todo!` is fine in a WIP branch; not on `main`.
- **No `println!()` / `print!()` / `dbg!()` in committed code.** Use `tracing::info!()` etc. The CLI's top-level may `eprintln!` for friendly user errors.
- **No `#[allow(...)]` without a reason comment.** Format: `#[allow(clippy::large_enum_variant)] // reason: hot path; size doesn't matter`.
- **No `as` casts that can lose data.** Use `try_into()` / `try_from()` and handle the error. `usize as u32` is a bug waiting to happen on 64-bit systems.
- **No `String` / `Vec<T>` parameters when `&str` / `&[T]` will do.** Forces an allocation on the caller. See [API design](#api-design).
- **No public functions returning `Vec<T>` when a slice would do.** (Internal functions are fine.)

---

## Error handling

Two crates, two roles, two rules.

### `thiserror` for libraries (`superpanels-core`)

The core crate exposes typed errors. Every fallible function returns `Result<T, ModuleError>` where `ModuleError` is a `thiserror`-derived enum:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("monitor list cannot be empty")]
    EmptyMonitorList,

    #[error("image too small for canvas: {image_w}x{image_h} vs canvas {canvas_w}x{canvas_h}")]
    ImageTooSmall {
        image_w: u32,
        image_h: u32,
        canvas_w: u32,
        canvas_h: u32,
    },

    #[error("monitor {0} has invalid physical size (zero in one or both dimensions)")]
    InvalidPhysicalSize(String),
}
```

**Why typed errors:** callers can `match` on the variant and react differently to each. The CLI may want to print "image too small" with a hint to use `--fit fit`; the GUI may want to surface it as a specific toast colour. With `anyhow::Error` the caller can only print the string.

**Conversion between error types** uses `#[from]` to make `?` propagate cleanly:

```rust
#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Layout(#[from] LayoutError),

    #[error(transparent)]
    Image(#[from] ImageError),

    #[error("config: {0}")]
    Config(#[from] ConfigError),
}
```

### `anyhow` for binaries (`-cli`, `-daemon`, `-gui`)

In binary crates, `anyhow::Result<T>` is fine — the binary doesn't expose an API to anyone. Use `.context("doing X")` to add layers as the error bubbles up:

```rust
use anyhow::{Context, Result};

fn run() -> Result<()> {
    let config = load_config().context("loading config")?;
    apply_profile(&config, "home").context("applying profile 'home'")?;
    Ok(())
}
```

### Error message style

Error messages are written for the *end user*, not the developer.

- ✅ `"could not detect monitor layout. Try --monitors to specify manually."`
- ❌ `"DetectorError::AllFailed"` — useless to a user.

- ✅ `"failed to write wallpaper to /tmp/superpanels/0.png: permission denied"`
- ❌ `"io error"` — what file? what op?

Always include:
- *what* was being attempted,
- *why* it failed (the underlying message),
- if relevant, *what to try next*.

---

## Ownership and borrowing

You'll fight the borrow checker for a week and then it disappears. Some rules of thumb that shorten that week:

### Take references in parameters; return owned in returns

```rust
✅ fn compute(monitors: &[Monitor], image: &Path) -> Result<Vec<CropSpec>>
❌ fn compute(monitors: Vec<Monitor>, image: PathBuf) -> Result<Vec<CropSpec>>
```

The caller might still want to use `monitors` after the call. Forcing them to give it up means they `clone()` to keep it. References are free; clones are not.

### `Cow<str>` when "maybe-allocate"

```rust
fn normalize(s: &str) -> Cow<'_, str> {
    if s.contains('\n') {
        Cow::Owned(s.replace('\n', " "))
    } else {
        Cow::Borrowed(s)
    }
}
```

### Don't fight: clone deliberately

If you've spent 10 minutes wrestling lifetimes, a `clone()` with a comment explaining why is usually fine. Profile if it actually matters.

### `Arc<T>` for shared immutable data

If multiple parts of the system need to read the same monitor list, give them `Arc<Vec<Monitor>>` rather than each holding their own clone. Cheap to share.

### `Arc<Mutex<T>>` is a smell

It usually means you're sharing mutable state across threads. Often the better answer is:

- A channel (`tokio::sync::mpsc` or `crossbeam::channel`) with one writer and one reader.
- An `Arc<T>` of an immutable snapshot, replaced atomically (`arc-swap` crate).
- A single owning task that other tasks talk to via messages.

Use `Mutex` only when the lock is held briefly and contention is genuinely low.

---

## API design

### Take traits, not concrete types

```rust
✅ fn read_config(path: impl AsRef<Path>) -> Result<Config>   // canonical
✅ fn read_config<P: AsRef<Path>>(path: P) -> Result<Config>  // older form, equivalent
❌ fn read_config(path: PathBuf) -> Result<Config>
```

Prefer `impl Trait` in argument position — it reads top-down and avoids introducing a type parameter the caller doesn't care about. The `<P: AsRef<Path>>` form is fine when you reuse `P` elsewhere in the signature; otherwise it's noise. Either way, callers can pass `&str`, `&Path`, `String`, `PathBuf`. Lifetime cost: zero.

### Builder pattern for > 3 arguments

```rust
let crops = LayoutBuilder::new(&monitors)
    .bezels(BezelConfig::uniform(8.0, 5.0))
    .fit(FitMode::Fill)
    .reference_ppi(108.0)
    .compute(image_size)?;
```

vs.

```rust
let crops = compute_crop_specs(&monitors, &bezels, FitMode::Fill, Some(108.0), image_size)?;
```

Builder wins for legibility once you cross 3–4 args. Don't introduce a builder for a 2-arg fn.

### Return enums, not booleans, when there's meaning

```rust
✅ enum DetectionOutcome { Found(Vec<Monitor>), NotInstalled, Failed(String) }
❌ -> Option<Vec<Monitor>>  // what does None mean? Crashed? No monitors? Tool missing?
```

### Never return `()` from a fallible function

```rust
✅ fn apply(...) -> Result<AppliedReport>
❌ fn apply(...) -> Result<()>  // what happened? what was set?
```

Even if you don't use the return today, you might want to surface "applied to N monitors" later, and the API change is then a breaking one.

(Exception: `()` is fine when there is genuinely nothing to report — a `pause()` function for example.)

### Make illegal states unrepresentable

```rust
❌ struct Profile { name: String, image_path: Option<PathBuf>, image_paths: Option<Vec<PathBuf>> }
✅ enum ProfileBody { Standard(StandardProfile), Slideshow(SlideshowProfile) }
   struct Profile { name: String, body: ProfileBody }
```

The first lets you express "both Some" or "both None" — neither valid. The second can only express valid states.

---

## Newtypes for IDs

Don't pass `u32` everywhere — wrap IDs in newtypes:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(pub u32);
```

Now `fn apply(profile: ProfileId, monitor: MonitorId)` can't be called with the args swapped — the compiler stops you. With raw `u32`s, swapping compiles and silently does the wrong thing.

Newtypes have zero runtime cost. Use them whenever a value is "an ID of a specific kind".

---

## Strings, paths, and slices

| You have | Use |
|---|---|
| Some text in a struct field | `String` |
| A text parameter | `&str` |
| Some path in a struct field | `PathBuf` |
| A path parameter | `&Path` (or `impl AsRef<Path>` for max ergonomics) |
| A list in a struct field | `Vec<T>` |
| A list parameter | `&[T]` |

For OS-passed strings (subprocess args, env vars):

- `OsStr` / `OsString` for things you'll hand back to the OS unmodified.
- Convert to `String` with `.to_string_lossy()` only at display boundaries (logs, errors).

**Never interpolate a path into a shell command.** All subprocess args are passed as separate `OsStr` arguments via `Command::arg()`. See [`docs/reference/backends.md`](../reference/backends.md) (subprocess rules).

---

## Concurrency

### Default to single-threaded synchronous

Most of `superpanels-core` is sync. The compiler keeps it correct without you thinking about it. Async is a tax — pay it only where the gains are real.

### Async only at the I/O boundary

The daemon needs `tokio` because it has long-lived I/O (IPC sockets, FS watchers, timers). The core's image processing does not — it's CPU work. Run CPU work on `rayon` thread pools, or just call sync code from `spawn_blocking` in the daemon.

### Don't hold a lock across `.await`

```rust
❌ let guard = state.lock().await;
   let result = do_async_thing(&guard).await;  // lock held during await — deadlock risk

✅ let snapshot = { let guard = state.lock().await; guard.clone() };
   let result = do_async_thing(&snapshot).await;
```

### Channels over locks

If two tasks need to coordinate, prefer message-passing. Locks invite deadlocks; channels make ordering explicit.

---

## Tracing and logging

We use `tracing`, not `log` and not `println!`.

```rust
use tracing::{debug, info, warn, error, instrument};

#[instrument(skip(image), fields(monitors = monitors.len()))]
pub fn compute_crop_specs(
    monitors: &[Monitor],
    image: &DynamicImage,
    bezels: &BezelConfig,
) -> Result<Vec<CropSpec>, LayoutError> {
    info!("computing crop specs");
    // ...
}
```

### Levels

| Level | When | Example |
|---|---|---|
| `error!` | Something failed and the user needs to know | "failed to apply wallpaper: …" |
| `warn!` | Recoverable, but unusual | "monitor reported zero physical size; falling back to 96 PPI" |
| `info!` | Top-level state changes | "applied profile 'home'" |
| `debug!` | Internal flow useful for debugging | "computed 3 crop specs" |
| `trace!` | Per-iteration / hot path | inside a tight loop |

### Structured fields, not interpolation

```rust
✅ info!(monitor = %name, "applied wallpaper");
❌ info!("applied wallpaper for {}", name);
```

Structured fields make logs filterable. The JSON file output uses them as proper fields.

### `#[instrument]` on public functions

It auto-creates a span with arg names + values. `skip` for big arguments (images, file contents).

---

## Comments and documentation

**Default to no comment.** A comment is justified only when the *why* is non-obvious — a hidden constraint, an external-API quirk, a workaround, or a spec cross-ref for math the reader would otherwise get wrong (e.g. mm vs. pixels). If a future reader could derive the information from the signature, the type, or the name, the comment is noise. Token-cost matters: every line of fluff is a line every agent reads on every pass.

### Rustdoc on public items

Every `pub` item in `superpanels-core` gets a **one-line** doc summary. Add more only when behavior is surprising.

```rust
✅ /// Computes one [`CropSpec`] per monitor; the image is mapped onto the
   /// physical desktop plane in mm including bezels (`SPEC §4`).
   pub fn compute_crop_specs(/* ... */) -> Result<Vec<CropSpec>, LayoutError>
```

Rules:

- **No field or enum-variant docs that restate the name.** Skip them entirely; the name and type already say it. Add a doc only when the field's meaning is genuinely ambiguous (units, sentinel values, ordering invariants).
- **`# Errors`**: list a variant only when its trigger isn't obvious from its name. `EmptyMonitorList` doesn't need an `# Errors` line — the name is the doc. `ImageTooSmall` *might*, because the threshold isn't obvious.
- **`# Examples`**: only when the usage has a non-obvious edge case worth pinning with a doctest. Do *not* write tutorial examples ("construct a default and read a field") — they cost tokens and prove nothing the type system doesn't already.
- **Module headers**: one line. No design essays — those belong in `docs/`.

### Inline comments

Explain *why*, never *what*.

```rust
✅ // Plasma 6 has no `outputName` on Desktop; iterate assignments by connector
   // and resolve via `screenForConnector` instead.
   for (connector, image) in assignments { /* ... */ }

❌ // Loop over the assignments.
   for (connector, image) in assignments { /* ... */ }

❌ // Set reference_ppi to the maximum PPI of all monitors.
   let reference_ppi = monitors.iter().map(|m| m.ppi).fold(0.0, f64::max);
```

If the *what* needs explaining, rename the binding. If the *why* is obvious from local context, omit the comment.

### Don't comment-out code; delete it

Git remembers. Commented-out code rots and confuses readers.

---

## Imports

Group imports into three blocks separated by blank lines:

1. `std`
2. external crates
3. internal (`crate::`, `super::`, `self::`)

```rust
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::display::Monitor;
use crate::layout::CropSpec;
```

Stable rustfmt won't reorder across these groups for you (the option is nightly), so do it by hand. Within a group, sort alphabetically.

### No glob imports

```rust
❌ use crate::display::*;
✅ use crate::display::{Monitor, MonitorId, Rotation};
```

Exception: `use super::*;` inside `mod tests` — that's idiomatic.

---

## Common smells

### Premature traits

> "I might want to swap the implementation later."

You won't. And if you do, refactoring concrete code to a trait takes 5 minutes. Premature traits add indirection now for hypothetical flexibility later. Resist.

The exceptions are `WallpaperBackend` (see [`docs/reference/backends.md`](../reference/backends.md)) and `DisplayDetector` (see [`docs/reference/displays.md`](../reference/displays.md)) — both have multiple implementations from day one.

### Module-level state

```rust
❌ static CURRENT_PROFILE: Mutex<Option<Profile>> = ...;
```

Pass state in. Globals make tests interfere with each other and obscure data flow.

### Massive `impl` blocks

If a struct's `impl` is 300+ lines, group methods into multiple `impl` blocks by theme, or split into a sibling type that delegates back. The reader can scan headings.

### `match` with one meaningful branch

```rust
❌ match optional {
       Some(v) => use(v),
       None    => {},
   }

✅ if let Some(v) = optional {
       use(v);
   }
```

### Re-exporting everything from `lib.rs`

The crate root is the *public API*. Be deliberate. Don't `pub use crate::module::*` — it makes refactoring across versions painful and the rustdoc a soup.

### `Result<Result<T, E1>, E2>`

Always a sign of an error type that needs a `#[from]` variant. Flatten.

### Boolean parameters

```rust
❌ fn save(profile: &Profile, force: bool, write_backup: bool)
✅ fn save(profile: &Profile, options: SaveOptions)
   #[derive(Default)] struct SaveOptions { force: bool, write_backup: bool }
```

Or a builder. At a call site, `save(&p, true, false)` is unreadable.
