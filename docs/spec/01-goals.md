# 1. Goals & non-goals

## 1.1 Primary goals
- **Bezel-aware spanning.** Take a wide or panoramic image and span it across multiple monitors so the image content remains continuous *as the eye sees it across the desk* — bezels included. The image accounts for the physical gap between screens, so content isn't shifted, squashed, or duplicated.
- **Multi-monitor first.** The interesting case is two-or-more displays of mixed size, mixed PPI, and mixed orientation. Single-monitor is supported but is not the design centre.
- **Folder-driven slideshows.** Point at a folder of wallpapers and have the app rotate through them on a schedule, with sane filtering and history so the same image doesn't appear twice in a row.
- **Lightweight & static.** Trivially installable. `pacman -S superpanels` or `cargo install superpanels`. No Python, no virtualenvs, no system library drift.
- **Slick optional GUI.** Tauri + Svelte 5. The headline UI element is a live, scaled, bezel-accurate monitor-preview canvas that lets you compose the wallpaper before applying.
- **CLI-first.** Every feature reachable from the GUI is reachable from the CLI for scripting and automation. The GUI calls the same library the CLI does.
- **Extensible backends.** Each desktop environment is an isolated, testable module behind a single trait. Adding a backend should not require touching unrelated code.

## 1.2 Quality goals
- **Looks great.** Polish is a feature, not garnish. Default theme is dark, KDE-Breeze-adjacent, with smooth canvas updates.
- **Fast.** Apply a wallpaper in under 500 ms on a typical setup (excluding the time the compositor takes to redraw). GUI canvas redraws stay above 60 fps during drag interactions on a Ryzen 5600 / integrated graphics.
- **Predictable.** A given config + image + monitor layout always produces the same result. No hidden state.
- **Recoverable.** Bad config or backend failure never leaves the desktop in a broken state; it returns an error and the previous wallpaper remains.

## 1.3 Non-goals
- Cross-platform (Windows/macOS) — not in v1; the backend trait is shaped to allow it later.
- Live wallpaper / video / shader wallpapers.
- Online wallpaper sources (wallhaven, unsplash) — possibly post-v1, not core.
- Per-monitor colour calibration / ICC profile management.
- Perspective correction (Superpaper-style angled-monitor warping).
- Wallpaper editing (cropping, colour adjustment) beyond what's needed to fit the canvas.
