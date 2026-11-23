# BarCraft

**An open-source dock with live widgets for Windows 10 and 11.** BarCraft sits on the top or
bottom edge of the screen, holds your apps and a row of live tiles (what's playing, network
speed, processor and memory load, disk space, battery, volume, weather, timers and more), and
stands in for the Windows taskbar while it runs. Close it and the taskbar comes back as it was.

BarCraft is made by [F2N](https://github.com/F2N-AI) and written from scratch in Rust with
[egui](https://github.com/emilk/egui), in the spirit of the [ArtCraft](https://getartcraft.com)
Craft apps: free, local, no account, no telemetry.

> **Status: early alpha.** Windows only. Tried so far on one Windows 11 PC with two screens;
> not yet tried on Windows 10. Known gaps:
>
> - The tray icons aren't on the dock itself: the System tray widget brings the Windows taskbar
>   up with them, and it hides again once you click elsewhere.
> - The dock sits on the main screen only; other screens keep their own taskbar.
> - Only pinned shortcuts (`.lnk`) are imported from the Windows taskbar; pinned Store apps are
>   not yet.
> - No installer yet.

## What it does

- **Your apps.** The apps pinned to your Windows taskbar are imported on first start; every open
  window shows up too, grouped by program.
  - Click: open the app, bring its window forward, or minimise it if it is already in front.
    With several windows, click to choose one.
  - Middle-click: open a new window.
  - Right-click: the app's windows, pin or unpin, move, close, end task.
- **25 widgets, no account needed.** Each sits in its own card, most with two lines in Inter;
  processor and memory also come as square rings, and the battery card or tile fills with liquid.
  - Time: clock, analog clock, date with a month calendar, day, month and year progress.
  - System: processor load with a one-minute chart, memory, network speed, disk space,
    battery, system tray, notifications, Recycle Bin, show desktop.
  - Sound and media: what's playing with play/pause/next/previous, volume, microphone mute.
  - Productivity: focus timer, stopwatch, Time on PC (today's total and top apps, kept on
    your PC only).
  - Online: weather for a place you choose, with the next days, sunrise and sunset.
- **A widget gallery.** Settings show every widget with a live preview, by category, with search.
- **Top or bottom, centred or full width.** Drag the bar to the other half of the screen, or pick
  the edge in Settings. BarCraft reserves its strip so maximised windows stop above (or below)
  it. When there are more apps than fit, the app area scrolls.
- **Out of the way.** Clicking the dock never takes focus from your app, and the dock steps aside
  when a game or a video goes full screen.
- **Themes and sizes.** Six themes of our own and three sizes. App icons sit free on the bar and
  magnify under the pointer, pushing their neighbours aside, with the app's name above; how much
  is up to you. Two are glass: Smoke (dark) and Frost (light) let what's behind show through the bar,
  blurred by Windows itself (rounded corners need Windows 11; Windows 10 keeps them square).
  The default, Follow Windows, switches between them with Windows' light and dark mode.
- **Windows under the dock, if you like.** By default the dock keeps its strip of the screen to
  itself; turn on "Let windows use the space under the dock" and maximised windows reach the
  edge with the dock floating over them, as on a Mac. Or let it hide: it slides out of sight
  when you leave it and comes back when the pointer touches the screen edge.

## The Windows taskbar always comes back

BarCraft hides the taskbar only while it runs (or just switches on auto-hide, or leaves it alone:
your choice). Three things make sure it returns:

1. A normal exit restores it.
2. A small watchdog copy of BarCraft waits for the dock to end, however it ends (crash, End task),
   and restores it.
3. The taskbar's original state is saved before anything changes, so even after a power cut the
   next start restores it. You can also run `barcraft --restore-taskbar` at any time.

## Privacy

Nothing leaves your PC except the weather widget's requests to
[Open-Meteo](https://open-meteo.com), and only if you turn that widget on. Settings live in
`%APPDATA%\BarCraft\settings.json`. If you add the Time on PC widget, today's per-app totals are
kept in `usage.json` next to it, start over each day, and can be cleared in Settings → Privacy.

For trying things out, `BARCRAFT_DATA_DIR` points BarCraft at another folder for its settings,
`barcraft --open-settings` starts it with the settings window open, and `barcraft --preview`
draws the bar in an ordinary window, without docking it or touching the taskbar, so it can run
next to your real dock; add `--hover N` to see it as if the pointer rested on app N.

## Build from source

You need Rust (stable, 1.95 or newer) and, on Windows, the Visual Studio Build Tools with the
C++ workload. Keep the checkout in a short path (or set `CARGO_TARGET_DIR` to one): the MSVC
linker can't open files whose full path is longer than 260 characters.

```bash
cargo run --release -p barcraft
```

```bash
cargo test
```

## How it is built

| Crate | What it holds |
| --- | --- |
| `crates/core` | Settings, themes, the widget catalogue, how windows and pins merge into dock icons, the calendar, CPU maths, the focus timer, weather parsing. No operating-system calls; fully tested. |
| `crates/win32` | The only crate that calls Win32. Every `unsafe` block lives here, each with a `// SAFETY:` note, behind a safe API that never panics. |
| `apps/barcraft` | The egui app: the dock window, widgets, the settings window, and a background thread that reads the system. |

## Clean room

BarCraft is written from public documentation and from how docks and taskbars visibly behave. It
contains no code, artwork, text or other material from any other product. Its icons are drawn in
code. See [docs/behaviour.md](docs/behaviour.md) for the behaviour it implements and
[ATTRIBUTION.md](ATTRIBUTION.md) for the third-party material it uses.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
