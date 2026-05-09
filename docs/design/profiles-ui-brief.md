# Profiles redesign — UI brief

Concise list of functionality, UI surfaces, and workflows for the profile-management redesign. Implementation-agnostic; this brief was the input to a UI design tool.

**Mockup output is now in `docs/design/mockup/superpanels-claude/`** — that's the visual source of truth for the profile-related surfaces below. See the mockup's `README.md` for which components are authoritative and which are stale (only the profile, schedules, manager, tray, and save-dialog parts are current).

---

## Concept summary

A **profile** is a mode the user is in. It bundles:
- An image (single file or a slideshow set).
- An image transform (position + size on the canvas).
- A physical monitor arrangement (each monitor's position in mm, rotation).
- A colour swatch and optional description.
- Provenance (created, last applied, etc.).

Exactly one profile is **active** at a time. Switching profiles swaps everything atomically — image, crop, and monitor layout. The canvas is a live view of the active profile.

A **schedule** is a separate, top-level rule: "at time T, switch to profile X". Schedules preempt manual choice unless globally paused.

---

## Functionality changes

### Behaviour
- Switching profiles changes the image **and** the on-canvas monitor arrangement (positions, rotations) at the same time.
- Any change made on the canvas while a profile is active **auto-saves** to that profile (drag a monitor, rotate it, move/resize the image, swap the image).
- A profile can be **disabled** when it can't be applied to the current setup. Reasons:
  - Connected monitor set or rotation differs from what the profile was authored against.
  - The referenced image file is missing.
  - The slideshow folder is missing or empty.
  - A referenced monitor is no longer connected.
  - A monitor's physical size hasn't been entered yet.
- Disabled profiles are visible everywhere but greyed out and won't auto-apply. Clicking one opens a **repair flow**: the canvas opens with the current monitor layout pre-populated and the image reset to its default fit; the user re-positions and saves.
- Schedules live at the top level (not inside profiles). They can be paused globally.
- Two schedules at the same minute is **not allowed** — the settings UI blocks save with an inline error.

### Concepts removed
- "Bezel size" is gone as a separate concept. Gaps between monitors are derived from how the user has positioned them in mm.
- "Save current as new" no longer lives inside the tray selector dropdown.

---

## UI surfaces

### 1. Tray selector (top-nav menu bar)

Purely for **switching profiles**. No CRUD.

Contents:
- Search/filter (when many profiles).
- List of profiles, sorted by most recently used. Each row shows:
  - Colour swatch (small dot or bar).
  - Profile name (truncated with ellipsis if long; full name in tooltip).
  - Active indicator on the current one.
  - Disabled rows are greyed out with a small warning icon.
- "Auto: switching to *<profile>* at HH:MM" hint when a schedule is active.
- Menu items at the bottom:
  - Open profile manager…
  - Pause schedules / Resume schedules (toggle).

Behaviour:
- Opens on click; **closes when clicking anywhere outside** (this is currently broken — it must work like a normal dropdown).
- Empty state when no profiles exist: a single CTA "No profiles yet — create one in the profile manager" that opens the manager window.

### 2. Top-nav "Save as new profile" button

A small save icon **next to the existing Apply button** in the title bar.

- Click opens a small dialog:
  - **Name** (required, must be unique).
  - **Colour swatch** (required, curated palette of ~12 colours).
  - **Description** (optional, single line).
  - Default name suggestion: `"<active-profile-name>-copy"` or `"untitled-N"`.
- Confirm → creates a new profile from the current canvas state and makes it active.
- Cancel → closes, no change.
- Disabled (with tooltip "no image on canvas") when there's nothing to capture.

### 3. Profile manager (new window)

Opened from a top-nav button and from the tray selector ("Open profile manager…"). Sized and shaped similarly to the existing library window (left rail / list / detail).

Contents:
- Header: search field + "New profile" button + "Import…" button.
- List of all profiles. Each row:
  - Thumbnail (the image used by the profile).
  - Colour swatch.
  - Name.
  - Last-used recency ("just now", "2 days ago", etc.).
  - Validity badge: OK / Disabled (with reason on hover).
  - "Authored for a different setup" chip when the profile's expected topology doesn't match the current setup.
- Detail pane on row click:
  - Larger preview of the image and the monitor arrangement.
  - Editable name (inline).
  - Editable colour swatch (palette popover).
  - Editable description.
  - "Open file in file manager" / "Open folder in file manager" link.
  - Action buttons: **Apply**, **Duplicate**, **Export**, **Delete** (with confirm dialog).
  - For disabled profiles: prominent **Repair** button that triggers the repair flow.
- Empty state (no profiles): a centred CTA "Create your first profile" with a New profile button.

### 4. Settings → Schedules tab

Currently empty. Populate with:

- Master toggle: **Pause all schedules** (mirrored in tray).
- Location field (latitude / longitude). Required only if any sunset/sunrise rule exists.
- List of schedule rules. Each row:
  - Enabled toggle.
  - Trigger summary: "Daily at 18:00", "Sunset −30 min", or a cron expression.
  - Target profile (with its colour swatch).
  - "Next fires at HH:MM" hint.
  - Edit / Delete buttons.
- "Add schedule" button → form (modal or inline):
  - Trigger type: **Daily** | **Sunset/Sunrise** | **Cron**.
  - Daily: hour + minute pickers.
  - Sunset/Sunrise: which event + offset in minutes.
  - Cron: expression input with validation.
  - Target profile dropdown (existing profiles only; disabled profiles greyed out).
  - Optional display name.
  - Save / Cancel.
- **Conflict prevention:** if a new or edited rule would fire at the same minute as another enabled rule, the form blocks save and shows an inline error pointing at the conflicting rule.

---

## Workflows

### Switching profile
1. Click the profile selector in the top-nav.
2. Click a profile.
3. Canvas updates: image swap, monitor placements re-arrange, transform applied.

### Creating a new profile from the current canvas
1. User has positioned an image / monitors how they like.
2. Click the save icon next to Apply in the top-nav.
3. Dialog: enter name + pick colour (+ optional description).
4. Confirm → new profile is created and becomes active. The previous profile is unchanged.

### Creating a blank profile
1. Open profile manager.
2. Click "New profile".
3. Pick name, colour, image source. Save.

### Duplicating a profile
1. Open profile manager.
2. Click a profile, then "Duplicate" in the detail pane.
3. New profile appears with name `"<original>-copy"`; user can rename inline.

### Editing a profile
- Tweaking on canvas while a profile is active: changes auto-save. No explicit action.
- Renaming / changing colour / changing description: in profile manager detail pane.

### Repairing a disabled profile
1. User clicks a disabled (greyed) profile in the selector or manager.
2. Canvas opens with the current monitor layout pre-populated and the image reset to default fit.
3. User re-positions image / monitors as desired.
4. Profile is saved against the current setup and becomes Ok.

### Deleting a profile
1. Profile manager → select profile → Delete.
2. Confirm dialog.
3. Any schedules pointing at the deleted profile are flagged or removed (TBD by UI design).

### Adding a schedule
1. Settings → Schedules → "Add schedule".
2. Pick trigger type, fill parameters, pick target profile.
3. If the rule would conflict at the same minute as another, form shows an inline error and won't save.
4. Save → rule appears in the list with its "next fires at" hint.

### Pausing schedules
- Tray selector → "Pause schedules" toggle, or
- Settings → Schedules → master toggle.
- While paused, schedules don't fire; manual switching is unaffected.

---

## Key visual primitives

- **Colour swatch.** Curated palette of ~12 colours. Used in: profile rows, profile selector, schedule rule rows.
- **Validity badge / icon.** Two states (Ok, Disabled) with reason text on hover.
- **Topology chip.** Small inline tag — "Authored for a different setup" — shown only when relevant.
- **Recency text.** Relative time ("2 days ago"), used in profile list.
- **Disabled (greyed) state.** Applies to whole rows in selector and manager; clicking still works but routes into the repair flow.
- **Save dialog.** Reusable for "Save as new" and "New profile" — name + colour + optional description.
- **Confirm dialog.** Reusable for destructive actions (Delete profile, Delete schedule).
