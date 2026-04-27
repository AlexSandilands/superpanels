# Decisions to make early

These need answers before the relevant phase begins. Tracked as GitHub issues once the repo is public.

1. **Licence.** MIT, Apache-2.0/MIT dual, or GPL-3.0? *Recommendation: dual MIT/Apache-2.0 (Rust ecosystem default).*
2. **MSRV (Minimum Supported Rust Version).** *Recommendation: latest stable at start of work; bump freely until v1.0; document in `rust-toolchain.toml`.*
3. **Tauri vs Iced vs egui.** *Recommendation: stick with Tauri v2 (canvas work easier in a web view, Svelte ecosystem). Revisit if WebKitGTK becomes a packaging issue.*
4. **Crate naming.** Flat `superpanels-{core,cli,gui,daemon}` or single binary with subcommands? *Recommendation: workspace of crates internally; single `superpanels` binary externally with subcommand dispatch (best of both — clean architecture, simple packaging).*
5. **EDID hash form.** Full EDID vs (manufacturer + model + serial). *Recommendation: hash of (manufacturer + model + serial). Cable swap shouldn't break per-monitor profile data.*
6. **Thumbnail format.** WebP, AVIF, or PNG. *Recommendation: WebP (decoded by browsers natively, small, decent quality at 80%; AVIF encode is too slow per-image).*
7. **State storage.** SQLite from day one or migrate later? *Recommendation: JSON in Phase 2, SQLite in Phase 4 when tags arrive. Migration code is a known cost we accept.*
