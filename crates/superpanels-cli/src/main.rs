#![forbid(unsafe_code)]

//! Superpanels CLI entry point.

use superpanels_core::VERSION;

fn main() {
    // reason: the CLI top-level may print to stderr for friendly user output
    // and errors — see docs/architecture.md "Workspace lints" and CLAUDE.md.
    #[allow(clippy::print_stderr)]
    {
        eprintln!("superpanels v{VERSION} — see PLAN.md for status");
    }
}
