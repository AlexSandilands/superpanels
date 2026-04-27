# 15. Logging & observability

## 15.1 Logging

- `tracing` + `tracing-subscriber`.
- Default level: `info` for app code, `warn` for dependencies.
- Console output is human-friendly with colour (when stdout is a TTY).
- File output is JSON-lines, written to `$XDG_STATE_HOME/superpanels/superpanels.log`, rotated daily, kept for 7 days.
- `-v` / `--verbose` raises level to `debug`; `-vv` to `trace`.
- A redaction layer scrubs anything that looks like a home path from JSON output (`/home/alex/...` → `~/...`) for shareable logs.

## 15.2 Crash diagnostics

`color-eyre` for panics in user-facing binaries, `human-panic` for end-user-friendly crash messages. A panic dumps a structured report to `$XDG_STATE_HOME/superpanels/crash-<ts>.txt` and prints a path to it.

## 15.3 Telemetry

None. Superpanels does not phone home. There is no analytics, no usage reporting, no crash uploads. (We may add an opt-in `--report-crash` flag in a later release that the user runs explicitly to attach a crash report to a GitHub issue.)
