# UI polish punch list

Active list of UI/UX issues to work through. Tick off as we land them.
Unlike `docs/followups.md`, these aren't "revisit later" items — they're
the next batch of polish work.

## 1. "Set physical size" warning Fix button → wrong settings tab ✅

- [x] When monitors haven't been set up, an orange warning appears at the
      top of the canvas asking the user to set the physical size, with a
      "Fix" button.
- [x] The Fix button currently navigates to **Settings → General**.
- [x] It should navigate to **Settings → Monitors** instead.

## 2. Settings → Monitors can't set physical size ✅

- [x] The Monitors page doesn't expose a control for entering the
      physical width/height (mm). Add it. This is the canonical place
      a user would expect to fix the warning from item 1.

## 3. Settings → General Select dropdowns look ugly ✅

- [x] Restyle the native `<select>` elements in Settings → General to
      match the rest of the UI. Likely a custom Select component or
      consistent `appearance: none` styling + caret + theming.

## 4. Monitor inspector popup overflows horizontally ✅

- [x] After clicking a monitor on the canvas, the right-hand inspector
      popup has a horizontal scrollbar.
- [x] Cause appears to be the **X / Y position** input boxes — they're
      too wide.
- [x] Shrink those boxes and align their visual treatment with similar
      input boxes used elsewhere (consistent width, border, padding).
- [x] Refresh-rate (Hz) display is far too long — round to **2 decimal
      places**.

## 5. Bezel gap adjustor in the bottom panel doesn't drive anything, what is it actually for? Key question - does the bezel gap settings apply to the profile, or the monitor system settings

- [ ] The bezel gap slider/input in the bottom panel currently has no
      effect.
- [ ] Decide on the interaction model. Proposed: applies a single gap
      value across **all** monitor pairs and **overrides** any gaps the
      user set manually by dragging. Confirm with a quick UX call before
      implementing.


## 6 When there are no profiles saved, click the profile selector up the top left is awkward, can't click out of it, have to spam/double click the canvas
