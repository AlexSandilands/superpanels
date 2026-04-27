# 11. CLI interface

```
superpanels [OPTIONS] <COMMAND>

Commands:
  set         Set wallpaper immediately
  next        Advance the slideshow (or apply the next entry of the active profile's source)
  prev        Step back in the slideshow
  pause       Pause the slideshow timer
  resume      Resume the slideshow timer
  profile     Manage profiles
  library     Manage the wallpaper library
  detect      Print detected monitor layout
  daemon      Run the background daemon
  gui         Launch the graphical interface
  config      Print the resolved config (debug aid)

Global options:
  -v, --verbose    Enable debug logging (-vv for trace)
  --quiet          Suppress non-error output
  --json           Machine-readable output where supported
  --config <PATH>  Use alternate config file
  --no-daemon     Do not contact the running daemon; run in-process
```

## 11.1 `set`

```
superpanels set <IMAGE> [<IMAGE>...]
  Set wallpaper from one or more image paths.
  - One image:        spanned across all monitors with bezel compensation.
  - Multiple images:  one per monitor, left-to-right (or pin with --monitor).

Options:
  --bezel-h <MM>      Horizontal gap between monitors (mm)
  --bezel-v <MM>      Vertical gap between monitors (mm)
  --fit <MODE>        fill | fit | stretch | center  [default: fill]
  --offset <X,Y>      Image offset within the canvas (px, signed)
  --backend <NAME>    Override backend detection
  --monitors <SPEC>   Manual monitor spec (see §6.2)
  --monitor DP-1=path Pin a specific image to a specific monitor (repeatable)
  --dry-run           Process image but don't apply; print what would happen
  --save-as <NAME>    Save the resolved invocation as a profile and apply it
```

## 11.2 `profile`

```
superpanels profile list [--json]
superpanels profile show <NAME> [--json]
superpanels profile apply <NAME>
superpanels profile create <NAME> [...same options as `set`]
superpanels profile edit <NAME>      # opens $EDITOR on the profile TOML block
superpanels profile delete <NAME>
superpanels profile rename <OLD> <NEW>
superpanels profile export <NAME> [-o FILE]   # print/write a portable profile bundle
superpanels profile import <FILE>             # merge a bundle into config
```

## 11.3 `library`

```
superpanels library scan                       # rescan all configured roots
superpanels library list [--tag T] [--json]
superpanels library tag <PATH> <TAG>...
superpanels library untag <PATH> <TAG>...
superpanels library favourite <PATH>
superpanels library unfavourite <PATH>
superpanels library roots add <PATH>           # register a folder root
superpanels library roots remove <PATH>
```

## 11.4 `detect`

```
superpanels detect [--json] [--debug]

# Plain output:
# Monitor 0: DP-1     2560x1440 at (0,0)      609x343mm  108 PPI  scale 1.0
# Monitor 1: HDMI-1   1920x1080 at (2560,0)   527x296mm   83 PPI  scale 1.0  rotation: portrait
# Bezel (0→1): 8mm horizontal  (configured)

# --json: Vec<Monitor> serialised, suitable for scripting.
# --debug: also prints which detectors were tried, their stderr, and the parser output.
```

## 11.5 `daemon`

```
superpanels daemon [--foreground] [--socket PATH]
  Start the background daemon. Default is to fork to background with logs going to
  $XDG_STATE_HOME/superpanels/superpanels.log. --foreground keeps it attached
  (useful for systemd user units).
```

## 11.6 Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Generic failure (wallpaper not applied) |
| 2 | Bad arguments |
| 3 | Config error (invalid TOML, etc.) |
| 4 | No backend available |
| 5 | Display detection failure |
| 6 | Image processing failure (bad file, unsupported format) |
| 7 | IPC/daemon error |
