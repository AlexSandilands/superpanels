_default:
    @just --list

# Build the Svelte frontend bundle into ui/dist (consumed by the GUI).
ui-build:
    npm --prefix ui run build

# Build release binaries for the whole workspace (includes frontend bundle).
build: ui-build
    cargo build --release --workspace

# Build debug binaries for fast iteration (includes frontend bundle).
build-debug: ui-build
    cargo build --workspace

# Run the daemon in the foreground (kills any existing instance first).
# `--foreground` keeps it attached to this terminal (default is to re-exec
# into the background); `-v` enables DEBUG-level tracing.
daemon: build
    - pkill -f superpanels-daemon
    ./target/release/superpanels-daemon --foreground -v

# Run the GUI (kills any existing daemon-less GUI instance first).
# WEBKIT_DISABLE_DMABUF_RENDERER mirrors .cargo/config.toml; cargo's [env]
# only applies to `cargo run`, not direct binary invocation.
gui: build
    -pkill -x superpanels-gui
    WEBKIT_DISABLE_DMABUF_RENDERER=1 ./target/release/superpanels-gui

# Run the GUI with Tauri devtools enabled (Ctrl+Shift+I / right-click →
# Inspect). Debug build via `cargo run` so the [env] block applies.
gui-dev: ui-build
    -pkill -x superpanels-gui
    cargo run -p superpanels-gui --features dev-tools

# Pass arguments through to the CLI: `just cli set --bezel-h 20 path/to.jpg`.
cli *ARGS: build
    ./target/release/superpanels {{ARGS}}

# Format every crate.
fmt:
    cargo fmt --all

# Lint with the workspace's `-D warnings` policy.
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run the workspace test suite.
test:
    cargo test --workspace --all-features

# Full pre-push gate: format, lint, test, deny.
check: fmt lint test
    cargo deny check

# Run pre-commit's pre-push hooks against every file.
hooks:
    pre-commit run --all-files --hook-stage pre-push

# Wipe build artifacts. Add `:cache` to also drop the wallpaper temp dir.
clean:
    cargo clean

clean-cache:
    rm -rf ~/.cache/superpanels/temp

# Reset local data to simulate a first-run experience. Backs up to
# ~/superpanels-backup-<timestamp>/ first. Pass `--no-backup` or
# `--autostart` to forward to the script.
wipe *ARGS:
    ./scripts/reset-local-data.sh -y {{ARGS}}
