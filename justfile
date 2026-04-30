_default:
    @just --list

# Build release binaries for the whole workspace.
build:
    cargo build --release --workspace

# Build debug binaries for fast iteration.
build-debug:
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
