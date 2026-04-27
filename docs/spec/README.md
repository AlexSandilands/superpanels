# SPEC — index

The technical specification, split by section. **Read only the section(s) relevant to your task.**

Section numbers (`§N`) match the file's leading number, so a code comment that says `SPEC.md §6.4` maps to `06-detection.md`.

| § | File | Topic |
|---|---|---|
| 1 | [01-goals.md](./01-goals.md) | Goals & non-goals |
| 2 | [02-personas.md](./02-personas.md) | Personas & user stories |
| 3 | [03-core-concepts.md](./03-core-concepts.md) | `Monitor`, `BezelConfig`, `CropSpec`, `Profile`, `ImageSet` |
| 4 | [04-bezel-math.md](./04-bezel-math.md) | Bezel & layout math (mm-based) |
| 5 | [05-architecture.md](./05-architecture.md) | Process model, IPC, crate layout, threading |
| 6 | [06-detection.md](./06-detection.md) | Display detection, detector contract, `MonitorRef` |
| 7 | [07-library.md](./07-library.md) | Wallpaper sources & library |
| 8 | [08-image-processing.md](./08-image-processing.md) | Image processing & colour |
| 9 | [09-profiles-schedules.md](./09-profiles-schedules.md) | Profiles, schedules & slideshows |
| 10 | [10-backends.md](./10-backends.md) | Backend trait, per-backend specifics, subprocess rules |
| 11 | [11-cli.md](./11-cli.md) | CLI interface (commands, flags, exit codes) |
| 12 | [12-gui.md](./12-gui.md) | Tauri + Svelte GUI, IPC commands, shortcuts |
| 13 | [13-tray.md](./13-tray.md) | System tray |
| 14 | [14-config-state.md](./14-config-state.md) | Config file, runtime state, library DB, migrations |
| 15 | [15-logging.md](./15-logging.md) | Logging & observability |
| 16 | [16-errors.md](./16-errors.md) | Error handling philosophy |
| 17 | [17-security.md](./17-security.md) | Security & sandbox (CSP, Tauri capabilities) |
| 18 | [18-a11y-i18n.md](./18-a11y-i18n.md) | Accessibility & i18n |
| 19 | [19-performance.md](./19-performance.md) | Performance targets |
| 20 | [20-testing.md](./20-testing.md) | Testing strategy |
| 21 | [21-packaging.md](./21-packaging.md) | Packaging & distribution |
| 22 | [22-out-of-scope.md](./22-out-of-scope.md) | Out of scope (v1) |
| 23 | [23-open-questions.md](./23-open-questions.md) | Open questions |

## Common pairings

- Bezel math (§4) almost always also needs the `Monitor` definition (§3.1) and detection rules (§6).
- A new backend (§10) needs the trait and subprocess rules from the same file plus the `Profile` type (§3.4).
- GUI (§12) IPC commands mirror the daemon's (§5.3) and use types from §3.
