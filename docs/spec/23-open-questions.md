# 23. Open questions

These need resolution before or during early implementation. Tracked as GitHub issues once the repo is public.

1. **Hyprland integration.** Should we support `swww` on Hyprland in addition to `hyprpaper`? Many Hyprland users prefer it. Probably yes — list both, pick the one running.
2. **GNOME multi-monitor span.** GNOME's `picture-uri` is one image per workspace, stretched. Our composite-to-one approach works, but GNOME users with very large total resolutions (24-megapixel composite for a 6K + 4K + 4K trio) will see a memory spike. Acceptable? Cap at 8K composite and downscale?
3. **Edid hashing.** Should we hash the full EDID or just `manufacturer + model + serial`? Latter is more stable (cable swap doesn't change the hash); former is more unique.
4. **Tauri v2 vs Iced.** Tauri brings WebKitGTK as a dep. Iced is pure Rust, smaller binaries, but the canvas work is more code. Decision: stick with Tauri for v1 (web tech for the canvas is hard to beat); revisit if WebKitGTK is a deal-breaker for a packager.
5. **Slideshow during sleep.** Should the slideshow timer pause when the screen is locked / the system is asleep? Answer: yes, listen on `org.freedesktop.login1` for `PrepareForSleep` and `LockedHint`.
6. **Schema for per-monitor profiles.** When a profile pins images per monitor, how do we refer to monitors in a way that survives re-plugs? `MonitorRef { stable_id?, name? }` (§6.4) is the resolved design — `stable_id` is the KDE per-output UUID where available, an EDID-derived hash otherwise. Needs a real-world test on non-KDE compositors.
