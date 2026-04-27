# 13. System tray

## 13.1 Tray icon

Monochrome SVG, light and dark variants. Selected automatically based on system theme. Falls back to a 22×22 PNG on environments without SVG tray support.

## 13.2 Menu

Left click: show/hide the main window. Right click:

```
┌──────────────────────────────────┐
│ ✓ home                           │
│   work                           │
│   movie                          │
│ ─────                            │
│   ▶ Next                         │
│   ◀ Previous                     │
│   ⏸ Pause slideshow             │
│ ─────                            │
│   Open Superpanels               │
│   Settings…                      │
│ ─────                            │
│   Quit                           │
└──────────────────────────────────┘
```

The tick mark next to the active profile updates live when the daemon switches profiles.

## 13.3 Tooltip

Hovering the tray icon shows: `Superpanels — <profile name> — <current filename>`.

## 13.4 Notifications

Optional desktop notifications (off by default, opt-in in settings) on:
- Apply success (briefly).
- Apply failure (always, even when notifications-on-success is off).
- Slideshow advanced (off by default).

Uses `notify-rust` / `org.freedesktop.Notifications`.
