# Guiding principles

- **Build vertically, not horizontally.** Each phase ships something that works end-to-end on at least one real desktop. Half-finished features stay on a branch.
- **Derisk first.** The hard bits (bezel math correctness; KDE D-Bus reliability) get prototyped in Phase 0, before architecture commitments.
- **Pure-Rust core, thin wrappers.** Anything in `superpanels-core` is unit-testable without spawning processes, opening windows, or touching the file system beyond a tempdir. CLI and GUI are dispatchers around the core API.
- **No placeholder code.** If it's merged, it works and is tested. `todo!()` only behind explicit feature flags.
- **No premature abstractions.** Three near-identical backend modules is fine; an abstract "BackendBuilder" framework is not.
- **Small, focused commits.** Diff reviewable in one sitting. The spec is the design doc; commit messages are the changelog.
- **Slick is a requirement.** GUI polish is scoped from Phase 3; it's not a "later" task.

## Phase map

| Phase | Goal | Ship-ready demo | Target version |
|---|---|---|---|
| 0 | Spike & derisk | A throwaway binary that prints correct crops for a 3-monitor KDE setup | — |
| 1 | Core CLI MVP on KDE | `superpanels set pano.jpg` works, with bezel correction | v0.1.0 (CLI only, KDE only) |
| 2 | Multi-backend + slideshow | Works on GNOME, Sway, Hyprland, X11/feh; folder-driven rotation | v0.2.x |
| 3 | Tauri shell + tray | `superpanels gui` window + system tray; profile switching | v0.3.x |
| 4a | Canvas interaction | Drag-to-offset + live bezel sliders; ≥ 60 fps preview | v0.4.x |
| 4b | Library + SQLite | Library grid + thumbnails + tags + favourites | v0.5.x |
| 4c | Polish & accessibility | Onboarding, theming, keyboard shortcuts, a11y audit | v0.6.x |
| 5 | Packaging & v0.7 release | AUR + crates.io + GitHub release artefacts | v0.7.0 |
| 6 | Stabilise toward 1.0 | Schema freeze, docs, perf budget enforced | v0.8 → v1.0.0 |

Rough effort: Phase 0–2 is the unglamorous engine work (the majority of the value). Phase 3–4 is the visible payoff. Phase 5–6 is hardening.

The version trajectory is *intent*, not contract. Pre-1.0 minor bumps may include breaking config changes; the changelog is explicit. The schema is frozen at v1.0; before then we accept the migration cost rather than freezing too early.
