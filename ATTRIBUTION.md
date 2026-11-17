# Attribution

Every asset BarCraft ships or shows, and where it comes from.

| Asset | Author | Source | Licence | Use |
| --- | --- | --- | --- | --- |
| Dock icons (Start grid, show desktop, speaker, battery, weather symbols, letter tiles) | BarCraft contributors | Drawn in code in `apps/barcraft/src/draw.rs` | MIT OR Apache-2.0 | Dock and widgets |
| Ubuntu-Light font | Dalton Maag, Canonical | Bundled by egui's `default_fonts` feature (egui 0.37) | Ubuntu-font-1.0 | Interface text |
| Hack font | Source Foundry | Bundled by egui's `default_fonts` feature (egui 0.37) | MIT | Monospaced numbers |
| Noto Emoji | Google | Bundled by egui's `default_fonts` feature (egui 0.37) | OFL-1.1 | Symbol fallback |
| emoji-icon-font, and its egui-icons subset | John Slegers ([jslegers/emoji-icon-font](https://github.com/jslegers/emoji-icon-font)) | Bundled by egui's `default_fonts` feature (egui 0.37) | MIT | Symbol fallback; BarCraft draws none of its brand glyphs |

## Data

| Data | Provider | Licence | Use |
| --- | --- | --- | --- |
| Current weather and place search | [Open-Meteo](https://open-meteo.com) | CC-BY-4.0 | Weather widget, only when the user turns it on. Credited in the widget's tooltip and in Settings. |

App icons shown on the dock are read at run time from the programs installed on the user's own
PC; BarCraft does not ship or store them.
