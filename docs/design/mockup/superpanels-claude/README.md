# UI mockup — Superpanels Claude (profiles redesign)

Self-contained React/Babel mockup. Open `index.html` in a browser to view.

**Authoritative for the profiles redesign only.** Other surfaces in this mockup (canvas, bezel handling, library, generic settings, tool dock, source dock, tweaks panel) reflect a stale earlier brief and should be ignored — current design is in `docs/spec/`.

## What's authoritative

| Surface | File:line |
|---|---|
| Title bar additions: Save-as-new icon, Profile-manager icon, profile pill behaviour | `chrome.jsx:4` |
| Tray selector (search, schedule hint, disabled-state rows, footer) | `chrome.jsx:124` |
| Profile manager window (list, detail pane, mini-topology preview) | `profiles.jsx:171` |
| Save / new-profile dialog (name + 12-swatch palette + description) | `profiles.jsx:66` |
| Color popover | `profiles.jsx:427` |
| Confirm dialog (reusable for delete) | `profiles.jsx:148` |
| Curated 12-swatch palette | `profiles.jsx:6` |
| Schedules settings section (rules list, editor, conflict detection) | `overlays.jsx:430`, `:496`, `:524` |
| Tray popover (system tray right-click menu) | `overlays.jsx:650` |
| Recency formatting helper | `profiles.jsx:25` |

## What to ignore (stale)

- `BezelDock` (`chrome.jsx:317`) — bezels are gone in the current design; gaps derive from authored monitor positions.
- `ToolDock`, `SourceDock`, canvas-level interactions, `MonitorInspector`, library modal, non-Schedules settings sections, `tweaks-panel.jsx`, `app.jsx` integration scaffolding.
