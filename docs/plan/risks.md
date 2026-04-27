# Risk register

| ID | Risk | Likelihood | Impact | Mitigation | Phase |
|---|---|---|---|---|---|
| R1 | KDE D-Bus call fails or is rate-limited | Med | High | Backoff + retry; `evaluateScript` has been stable for years; capture stderr | 1 |
| R2 | ~~EDID-derived physical mm missing on some monitors~~ — *resolved during Phase 0 spike*: kscreen-doctor doesn't expose physical mm at all, so the design now sources it from per-monitor config (§14.1). What was a risk became a deliberate design decision. | — | — | n/a | — |
| R3 | Hyprland JSON shape changes between minor versions | Med | Med | Pin lower bound; capture multiple-version fixtures; quick parser updates | 2 |
| R4 | GNOME composite memory spike on huge canvases | Med | Med | Cap at 8K long-edge; downscale below that; document in troubleshooting | 2 |
| R5 | Tauri tray UX differences across compositors | High | Med | Detect `StatusNotifierItem` host; degrade gracefully (no tray, window still works); document | 3 |
| R6 | IPC roundtrip in canvas drag too slow | Low | Med | Profile early; if > 5 ms/frame, port crop math to TS for live preview, IPC on release | 4 |
| R7 | Thumbnail cache grows unbounded | Med | Low | Bounded cache (500 MB default), LRU eviction | 4 |
| R8 | AUR review rejection | Low | Low | Follow style guide; budget revision time | 5 |
| R9 | Static linking on glibc systems is brittle | Med | Low | Document glibc requirement; offer dynamic build; provide Flatpak | 5 |
