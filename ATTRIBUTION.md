# Attribution

Every asset BarCraft ships or shows, and where it comes from.

| Asset | Author | Source | Licence | Use |
| --- | --- | --- | --- | --- |
| Dock icons (Start grid, show desktop, speaker, battery, weather symbols, letter tiles, rings and bars, tray chevron, bell, bin, microphone, upload/download arrows, media controls, analog clock face) | BarCraft contributors | Drawn in code in `apps/barcraft/src/draw.rs` | MIT OR Apache-2.0 | Dock, widgets and settings previews |
| `assets/fonts/Inter-Regular.ttf`, `Inter-SemiBold.ttf`, `Inter-Bold.ttf` (Inter 4.1) | The Inter Project Authors (Rasmus Andersson) | Copied from [storytold/craft-fonts](https://github.com/storytold/craft-fonts) at commit `cc9269cd8d8258785dac603fba3122e5d2556795`, `fonts/inter/`, which pins [rsms/inter v4.1](https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip) (`extras/ttf/`). SHA-256 match the craft-fonts manifest. | OFL-1.1 (`assets/fonts/OFL.txt`) | Interface text, in three weights |
| Ubuntu-Light font | Dalton Maag, Canonical | Bundled by egui's `default_fonts` feature (egui 0.37) | Ubuntu-font-1.0 | Fallback for characters Inter lacks |
| Hack font | Source Foundry | Bundled by egui's `default_fonts` feature (egui 0.37) | MIT | Monospaced numbers |
| Noto Emoji | Google | Bundled by egui's `default_fonts` feature (egui 0.37) | OFL-1.1 | Symbol fallback |
| emoji-icon-font, and its egui-icons subset | John Slegers ([jslegers/emoji-icon-font](https://github.com/jslegers/emoji-icon-font)) | Bundled by egui's `default_fonts` feature (egui 0.37) | MIT | Symbol fallback; BarCraft draws none of its brand glyphs |

## Data

| Data | Provider | Licence | Use |
| --- | --- | --- | --- |
| Current weather and place search | [Open-Meteo](https://open-meteo.com) | CC-BY-4.0 | Weather widget, only when the user turns it on. Credited in the widget's tooltip and in Settings. |

App icons shown on the dock are read at run time from the programs installed on the user's own
PC; BarCraft does not ship or store them.
