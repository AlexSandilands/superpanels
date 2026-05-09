# Superpanels — UI Design Brief

## 1. Project prompt (paste into Claude design)

> Design the desktop GUI for **Superpanels**, a Linux wallpaper-management application focused on **physical-bezel-aware multi-monitor spanning** and folder-driven slideshows.
>
> **What it does.** Takes a wide image (e.g. a 7680×2160 panorama) and spans it across multiple physical monitors, accounting for the *millimetre-wide bezel gaps* between screens so the image stays continuous as the eye sees it across the desk. Also rotates through wallpaper folders on a schedule and supports profiles ("home", "work", "movie", etc.).
>
> **Stack.**
> - Single binary desktop app, **Tauri v2** shell, **Svelte 5 (runes)** frontend, **TypeScript strict**, **Tailwind**.
> - Rust core handles detection, image processing, and applying wallpapers via per-desktop backends (KDE, GNOME, Sway/Hyprland, etc.).
> - Frontend talks to the Rust core via Tauri IPC commands.
> - Optional system-tray companion for quick profile switching.
>
> **Target user / platform.**
> - Primary: Linux power users on Arch / CachyOS, mostly KDE Wayland, often with 2–3 mixed-size, mixed-orientation monitors (e.g. a 34" ultrawide flanked by two 27" 4Ks, one rotated portrait).
> - Secondary: KDE tinkerers who want a polished visual composer.
> - The CLI exists for headless / scripting users — they will never open the GUI, so the GUI doesn't have to cater to them.
>
> **Look & feel.**
> - Default dark theme, KDE-Breeze-adjacent, with optional auto/light/dark from `prefers-color-scheme`.
> - Accent colour follows the system accent on KDE; user-overridable.
> - "Polish is a feature." Smooth canvas updates, ≥60 fps during drag interactions on modest hardware.
> - Honours `prefers-reduced-motion` (apply animations become instant).
> - Full keyboard navigation, ARIA labels, visible high-contrast focus rings; colour is never the only signal.
>
> **Headline UI element.** A live, scaled, **bezel-accurate monitor-preview canvas** that lets the user compose the wallpaper before applying — monitors drawn at correct relative *physical* sizes (a 27" 4K and a 24" 1080p look visibly different), correct relative positions, portrait monitors as rotated rectangles, bezel bars sized proportionally to the configured mm gap.
>
> **Out of scope** (don't design for): Windows/macOS, live/video wallpapers, online wallpaper sources, wallpaper editing beyond canvas-fitting, ICC/colour management, multi-user/multi-seat, mobile.
>
> **Design freely.** The functional surface area below is what the UI must expose; the existing implementation's layout, navigation, and visual choices should *not* constrain you.

---

## 2. Functional surface area the UI must expose

Capabilities, grouped by concern. Each item is a *thing the user must be able to do*, not a screen prescription.

### 2.1 Monitor detection & physical configuration
- See the currently detected monitors: name, resolution, scale, refresh rate, rotation, primary flag, and (when known) physical size in mm.
- Trigger a manual re-detection.
- See **why** a monitor lacks physical-size info, and provide it. Two equivalent input modes:
  - **Diagonal + aspect ratio + orientation** (e.g. 27", 16:9, landscape) → auto-compute mm.
  - **Direct width × height in mm.**
- Edit / re-confirm a monitor's physical size at any time.
- See and act on the "monitors changed since last session" case (e.g. dock re-plug renamed `DP-1`) — prompt to re-confirm assignments.
- Manual override / advanced: pin a layout (`WxH+X+Y[@scale][/rot]`) for headless or test scenarios.

### 2.2 Profiles (the core unit the user thinks in)
- List all saved profiles; see which one is currently active.
- Create a new profile.
- Rename, duplicate, delete a profile.
- Switch the active profile (apply it).
- Mark a profile as the default-on-startup.
- Save the current canvas state as a new profile or overwrite the current one.
- Quick-switch between top profiles via keyboard.

### 2.3 Wallpaper sources within a profile
A profile is one of two *modes*; the UI should make the distinction obvious and the swap painless:
- **Span mode** — one image (or a rotating set) spread across all monitors with bezel correction.
  - Source can be a **single image file**, or a **slideshow** (folder or hand-curated playlist).
- **Per-monitor mode** — pin a specific image to each individual monitor; bezels irrelevant.

For each, the user must be able to:
- Pick a file from the library, file picker, or drag-and-drop.
- See a preview thumbnail of what's selected.
- Swap the source without losing the rest of the profile config.

### 2.4 Bezel configuration
- Set horizontal and vertical bezel gap in **millimetres** (not pixels — this is core to the product).
- Adjust gap interactively and watch the canvas update in real time.
- Optional per-monitor-pair bezel override for asymmetric setups (e.g. thin-bezel ultrawide between two thick-bezel old IPS panels).

### 2.5 The monitor preview canvas (headline element)
The canvas is the heart of the UI. It needs to support:
- **Free-positioning model**: the source image floats freely; monitors are crop windows that hover over it.
- **Pan**: drag inside the image to slide it under the monitor cutouts.
- **Resize**: corner handles on the image, aspect-locked by default with an unlock toggle.
- **Snap-to-cover**: one-click "fit image to cover the union of all monitors" (essential for portrait+landscape combos that share no common axis).
- **Reset transform**: revert offset and free size to the FitMode default.
- **Fit-mode selector**: Fill / Fit (letterbox) / Stretch / Center.
- **Zoom (canvas only, not the apply)**: pinch / scroll zoom for inspecting fine detail; range roughly 0.5×–2.0×.
- **Off-monitor dim toggle**: fade everything outside the monitor cutouts so the user sees what will actually appear on-screen.
- **Per-monitor inspection**:
  - Hover → glow + tooltip showing pixel range and physical-size info.
  - Click → side popout with the exact crop as it will appear on that screen.
- **Apply confirmation animation**: brief flash per monitor (< 400 ms), respecting `prefers-reduced-motion`.
- **Empty / first-run states**:
  - No monitors detected → placeholder + diagnostic hint + "configure manually" affordance.
  - Monitors detected but missing physical-mm config → render the layout but annotate each unconfigured monitor and gate Apply with a clear reason.
  - No profile → "Drop an image here or pick one from the library."

### 2.5b Physical-layout editing on the canvas
Beyond moving the *image*, the user must be able to arrange the *monitors themselves* on the canvas to match (or override) their real-world physical arrangement. This is a separate interaction mode from image positioning — make the mode change obvious.

- **Drag individual monitors** to reposition them relative to one another in physical space (mm).
- **Multi-select** (rubber-band, shift-click) and drag a group together.
- **Live spacing readouts while dragging**: live mm distance to each neighbouring monitor's nearest edge — top, bottom, left, right — rendered as dimension lines between rectangles, the way a CAD tool shows offsets. Should update every frame, not just on release.
- **Snap behaviour**:
  - Snap to edge-alignment with neighbours (top edges, bottom edges, centrelines).
  - Snap to common gap distances (e.g. matching the configured bezel mm, 0 mm, 10 mm).
  - Hold a modifier (e.g. `Alt`) to disable snapping for fine free placement.
- **Precise nudge** with arrow keys (1 mm) and `Shift+arrow` (10 mm) for the selected monitor(s).
- **Numeric entry**: click a monitor to expose `x`, `y` (mm) input fields and an explicit gap-to-neighbour field.
- **Rotate monitors** in place: 90° CW / CCW / 180° via on-canvas rotate handle, context menu, or `[` / `]` keys. Rectangles redraw with the new aspect immediately and the bezel math reflows.
- **Free rotation** (arbitrary angle) is *out of scope* — only the four cardinal rotations the compositor actually supports.
- **Reset to detected layout**: a one-click revert that drops user-edited positions back to whatever the compositor reported.
- **Mark a custom layout as overriding detection**: when the user has manually arranged monitors, persist that arrangement and surface a clear indicator that the canvas no longer matches the compositor's reported positions (with a way to either push the change to the compositor — where supported — or keep it as a Superpanels-only override for bezel math).
- **Mismatch warning**: if dragged positions diverge significantly from detection, surface a non-blocking hint ("Your physical arrangement differs from what KDE reports — apply this to KDE? / keep as Superpanels-only?").
- **Alignment guides**: dynamic guidelines (extending edges of nearby monitors) appear during drag, like Figma's smart guides.
- **Reorder primary**: designate a different monitor as primary from the canvas (right-click → "Set as primary").
- **All edits update the bezel math live**: the image preview, crop windows, and any per-monitor info popouts must reflow as monitors are moved or rotated, without needing an explicit "apply layout" step.

### 2.6 Apply
- One obvious **Apply** action (and `Enter` shortcut).
- Feedback that the apply succeeded, with backend name and how long it took.
- On failure, a non-blocking error surface with the message, an expandable "details" view (full backend stderr), and a "copy details" action — never a blocking modal.

### 2.7 Library
The library is the user's pool of available images, indexed across one or more configured *roots* (folders).
- Browse images as a thumbnail grid.
- Filter by **tag**, **aspect ratio**, **resolution**, **favourite**.
- Search (and a keyboard shortcut to focus the search field).
- Per-image actions:
  - Apply now (sets a single-image span profile on the fly).
  - Set for a specific monitor (creates / updates a per-monitor assignment).
  - Add / remove user tags.
  - Toggle favourite.
  - Reveal in file manager.
- Manage **library roots**: add, remove, toggle recursive scanning, trigger a rescan.
- See scan / index progress for large libraries.
- Drag-and-drop semantics:
  - Drop file on window → set as active profile's source.
  - Drop folder on window → register as a library root.
  - Drop file onto a specific monitor in the canvas → create a per-monitor assignment.

### 2.8 Slideshow
For span-mode profiles whose source is a slideshow:
- Pick the source: **folder** (with optional recursion + filters) or **playlist** (ordered list of files).
- Configure interval (e.g. 30 minutes).
- Sort order: shuffle, alphabetical, date asc/desc, last-shown asc.
- "Don't repeat the last N" recent-history size.
- On-start behaviour: resume / new random / first.
- Pause-when-active toggle (pause timer when user manually changes images).
- Skip-on-unavailable toggle (file vanished → skip rather than error).
- **Smart filters** that let the user prefer images that suit the layout:
  - Min resolution.
  - Aspect ratio bucket (any / wide / standard / custom range).
  - Tag inclusion.
  - Favourites-only shorthand.
- **Manual transport controls** (always reachable): next, previous, pause / resume.
- See current slideshow position (e.g. "image 17 of 42") and recent history.

### 2.9 Schedules
Time-of-day triggers that flip profiles automatically:
- Daily at a specific time → switch to profile X.
- Sunset / sunrise ± offset → switch to profile X (requires lat/long).
- Cron expression escape hatch for power users.
- See, add, edit, delete, enable/disable schedules.
- Schedules can live on a profile *or* in a global list — both forms are valid; the UI should make it possible to express either without confusion.

### 2.10 Backends
- Show the currently active backend (KDE, GNOME, Sway/Hyprland, wlroots, feh, or custom).
- Override: pin a specific backend, or a custom shell command for unsupported environments.
- Show *why* each backend is or isn't available (so the user can debug "why isn't KDE detected?").

### 2.11 App settings
- **Theme**: auto / light / dark.
- **Accent colour**: follow system, or override.
- **Autostart on login** toggle (writes the user's XDG autostart entry).
- **Notifications**: off / errors only / all (success + slideshow advance + errors).
- **Library**: thumbnail size, auto-scan toggle.
- **Backend** selection (cross-link with §2.10).
- **Reduced-motion** override (in addition to the system preference).
- **Locale** override (v1 ships English; design should leave room for other locales).
- Open log file / open config directory / open library DB directory affordances.

### 2.12 System tray (companion surface)
- Tray icon (mono SVG, light/dark variants) with a tooltip showing `Superpanels — <profile> — <current filename>`.
- Left-click: show / hide the main window.
- Right-click menu:
  - Active-profile indicator + click-to-switch list of all profiles.
  - Slideshow next / previous / pause-resume.
  - Open Superpanels (main window).
  - Open Settings.
  - Quit.
- Tray's active-profile tick updates live when the daemon switches profile.

### 2.13 Status / runtime info (always discoverable)
- Active profile.
- Current source (file path or slideshow position).
- Last apply time + backend used.
- Slideshow paused/running state.

### 2.14 Error & event surfacing
- Non-blocking toast surface for backend errors, apply success/failure, and slideshow events.
- Expandable details (full subprocess stderr).
- "Copy details" action.
- Never lose an error — log file should always have it.

### 2.15 Keyboard shortcuts (must all be reachable from focused controls too)
- `Enter` — Apply.
- `Ctrl+N` / `Ctrl+S` / `Ctrl+Shift+S` — New / Save / Save-as profile.
- `Ctrl+1`/`2`/`3` — Switch to top three profiles.
- `Space` — Pause / resume slideshow.
- `→` / `←` — Slideshow next / prev.
- `R` — Reset image transform.
- `D` — Toggle off-monitor dim.
- `F5` — Re-detect monitors.
- `Ctrl+,` — Settings.
- `Ctrl+L` — Focus library search.
- `Esc` — Close modal / return to canvas.

### 2.16 First-run / empty-state flows
- Welcome / "where are your wallpapers?" — register a library root.
- Detect monitors and walk the user through providing physical-mm sizes for each (with the diagonal+aspect prefill described in §2.1).
- Offer to create a starter profile.
- Suggest enabling autostart and choosing a backend if auto-detection found multiple plausible options.

---

## 3. Cross-cutting design considerations

- **The canvas is the centre of gravity.** Whatever surrounding chrome you design, getting the user *to* the canvas with a usable image and an apply-able config in the fewest steps is the win condition.
- **The canvas has two distinct interaction modes** — *image positioning* (move the wallpaper under the monitors) and *physical layout* (move the monitors themselves). The mode the user is in must be unambiguous at a glance, and switching between them must be one obvious action. Consider whether a single unified mode with smart hit-testing can replace explicit modes — but only if it doesn't sacrifice precision.
- **Bezels are mm, not pixels.** Sliders, inputs, and tooltips should use mm consistently. Avoid showing pixel-only bezel values.
- **Gracefully degraded states matter as much as the happy path.** Missing physical sizes, undetected monitors, vanished files, backend failures, dock re-plugs — the design must accommodate them without dead-ends.
- **Power-user / quick-action paths.** Switching profile, advancing the slideshow, and re-applying the current profile should all be ≤ 1 click from the main window *and* from the tray.
- **Polish is a feature.** Smooth canvas, sensible motion, considered empty states, clear error surfacing — these aren't secondary.
