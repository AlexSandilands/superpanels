# 2. Personas & user stories

## 2.1 Personas

**The triple-monitor power user (primary).** Three screens: a 34" ultrawide flanked by two 27" 4Ks, one rotated portrait. Wants a single panorama to span the whole desk, with bezel correction, and a folder of curated panoramas to rotate through every couple of hours.

**The KDE tinkerer.** Comfortable in the terminal, but reaches for a GUI when designing things visually. Wants the canvas to *look* like the desk so they can compose without applying first.

**The minimalist Sway/Hyprland user.** Lives in the CLI. Will never open the GUI. Needs scripting hooks, a `--dry-run`, and predictable JSON output for `detect`.

## 2.2 Headline user stories

1. *"Drop a 7680×2160 panorama into Superpanels and have it span my three monitors with the bezel gap accounted for."* — see §4, §7, §10.
2. *"Point Superpanels at `~/Pictures/walls/panoramas/` and have it rotate through them every 30 minutes, never repeating the last 10."* — see §9.
3. *"Open the GUI, drag the image around the canvas to choose what bit lands on which monitor, then click Apply."* — see §12.
4. *"Have a 'work' profile (calm panoramas) and a 'home' profile (game art per monitor) that I can switch between from the tray."* — see §9, §13.
5. *"Run `superpanels set my.jpg` over SSH on a headless gaming PC to set the wallpaper before I sit down."* — see §11.
6. *"Tell Superpanels my monitor is portrait-rotated and have it carve out the right slice of a wide image to land on it correctly."* — see §3, §4.
7. *"The slideshow should pick images that suit my monitor layout — skip the 1024×768 squares and prefer ultrawides for spanning."* — see §7.
