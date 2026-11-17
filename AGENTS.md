# AGENTS.md: rules for every contributor, human or agent

BarCraft is a clean-room, open-source Rust dock for Windows. It follows the house rules of the
ArtCraft Craft apps (`storytold/craft-contrib`, `standards/house-rules-core.md` and
`standards/house-rules-rust.md`).

## Non-negotiables

- **Clean room.** Learn from visible behaviour and public documentation only. Never unpack,
  disassemble or read the files of another dock or taskbar product, and never copy its code,
  strings, artwork, themes or names. Icons are drawn in code (`apps/barcraft/src/draw.rs`) or
  come from an openly licensed set recorded in `ATTRIBUTION.md`.
- **Never crash.** Production code does not panic: no `unwrap`, `expect`, `panic!`, `todo!`,
  `unimplemented!` or `unreachable!` outside tests (the crate roots deny them). Return errors or
  fall back.
- **`unsafe` lives only in `crates/win32`.** Every block carries a `// SAFETY:` comment and sits
  behind a safe function. `crates/core` and `apps/barcraft` have `#![forbid(unsafe_code)]`.
- **Rust only.** No JavaScript, TypeScript, webview, Tauri or Electron.
- **Time goes through `chrono`.** No hand-rolled calendar maths.
- **The taskbar always comes back.** Any change to how the dock hides the Windows taskbar keeps
  all three recovery paths in `apps/barcraft/src/taskbar.rs` working.
- **Licences.** Dependencies and assets use only MIT, Apache-2.0, BSD-2/3-Clause, ISC, OFL-1.1,
  Ubuntu-font-1.0, Bitstream-Vera, CC0-1.0, CC-BY-4.0 or CC-BY-SA-4.0.

## Working

- `cargo test` runs every test; `cargo clippy --all-targets` must be clean.
- Keep changes narrow, add a test that fails without the fix, and say what you ran.
