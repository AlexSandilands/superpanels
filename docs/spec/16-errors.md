# 16. Error handling philosophy

- **No panics in library code.** `unwrap()` / `expect()` banned outside tests and `main()`.
- All fallible functions return `anyhow::Result<T>` (binary code) or `thiserror`-typed errors (library code, where the error variant is part of the API).
- Error messages are written for the *end user*, not the developer. They say what happened, why, and what to try next.
- Subprocess failures include the command run and its stderr.
- Config parse errors include the file path and the field path.
- Display detection failure is non-fatal — the user can provide manual specs.
- Backend failures revert no state — we set the wallpaper or we don't; we never half-apply.
