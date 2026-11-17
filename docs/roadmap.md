# Roadmap

Where BarCraft stands against what a full widget dock offers, and what comes next. The list of
features comes from public product pages and documentation of docks of this kind, rewritten in
our own words; names, looks and artwork are our own (see [AGENTS.md](../AGENTS.md)).

## Ground rules for new widgets

- **Local first.** A widget that needs the user's account, API key or sign-in session (revenue
  dashboards, social follower counts, AI plan usage, mail) is out of scope for now. AI plan usage
  in particular would mean reading the user's sign-in sessions; we won't.
- **No typing in the dock.** The dock never takes keyboard focus. Anything that needs typing
  (notes, to-dos, search, a launcher) opens in its own window, like Settings.
- **Private by default.** Data a widget collects about the user (for example how long each app
  is used) stays in `%APPDATA%\BarCraft`, is never sent anywhere, and can be cleared in Settings.
- **Our own names.** Every new label goes through ArtCraft's naming rules
  (`storytold/craft-contrib`, `standards/naming.md`). For example the app-usage widget is called
  "Time on PC", not another company's feature name.

## Now (0.2)

A tick means implemented; the pull request for each change says what was tried by hand.

Dock
- [x] Clicking the dock never takes focus from the app in use (needed for click-to-minimise).
- [x] Reach the system tray while the taskbar is hidden: a tile that brings the Windows taskbar
      up with its tray, and lets it hide again once focus moves elsewhere.
- [x] Never run off the screen: the app area scrolls sideways when it doesn't fit.
- [x] Full-width layout option (apps on the left, widgets on the right) besides the centred bar.
- [x] A cell behind every item, two-line cards for widgets, a soft glassy bar (gradient, edge
      light, shadow), icons that grow a little under the pointer.

Settings
- [x] Sidebar sections (General, Look, Taskbar, Apps, Widgets, Weather, Privacy, About).
- [x] A widget gallery: categories, search, a live preview of each widget, add or remove with a
      click, a mark on the ones already on the dock.

Widgets (no account, no typing)
- [x] Media: what's playing, with play/pause, next and previous.
- [x] Network: download and upload speed.
- [x] Storage: free space on the system drive.
- [x] Time on PC: today's total and the apps used most (local, clearable).
- [x] Microphone: mute and unmute.
- [x] Stopwatch.
- [x] Day, month and year progress.
- [x] Analog clock.
- [x] Recycle Bin: how much is in it, open it, empty it (with the system's own confirmation).
- [x] Notifications: open the notification centre.
- [x] System tray (see Dock).
- [x] Weather: today's high and low, the next days, sunrise and sunset.

## Next

- Busiest apps (processor and memory), world clock, keyboard layout, light/dark switch.
- More looks for widgets (gauges, dot grids); the first ones are in: processor and memory rings,
  battery tile.
- Drag to reorder and to pin; app groups; drag a file onto an app.
- Auto-hiding dock; a dock on every screen.
- Notes, to-dos and a launcher with search, each in its own window.
- Window previews on hover; jump lists.
- Real background blur behind the bar: the biggest visual difference left. It needs the bar's
  backdrop in a window of its own, since the dock window also holds the menus.
- An installer (MSI) and signed releases with checksums, for the ArtCraft launcher.

## Not planned

- Widgets that read the user's sign-in sessions or need their API keys.
- Copying another product's look, themes, artwork or names.
