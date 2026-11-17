# What BarCraft does

The behaviour BarCraft implements, written in our own words from how docks and taskbars visibly
work and from public documentation of that kind of product. Nothing here is copied from another
product's code, text or artwork.

## Placement

- The dock is a bar on the bottom or top edge of the main screen, centred, with a small gap to
  the edge.
- It registers as an application desktop toolbar ("appbar"), so the shell keeps maximised windows
  out of its strip.
- Drag the bar into the other half of the screen to move it to that edge. The choice is saved.
- It stays above other windows, never takes keyboard focus from the app in use, and doesn't
  appear in Alt+Tab.
- When the focused window covers the whole screen (a game, a full-screen video), the dock hides.
  It comes back when that window leaves full screen. This can be turned off.

## The Windows taskbar

- Three choices: hide it while the dock runs (default), only switch on its auto-hide, or leave it
  alone. Hiding applies to the main screen, where the dock is; other screens keep their taskbar
  (with auto-hide on).
- Before changing anything, BarCraft saves the taskbar's auto-hide setting. On exit it shows the
  taskbar again and restores that setting.
- A watchdog process restores it if the dock ends any other way. `barcraft --restore-taskbar`
  restores it by hand.
- If the shell shows the taskbar again while it should be hidden, the dock hides it again within
  a second.

## Apps

- On first start the dock imports the shortcuts pinned to the Windows taskbar. After that the
  dock keeps its own list.
- Open windows that a taskbar would show (visible, not tool windows, not owned, not cloaked)
  appear on the dock, grouped by the program that owns them. Windows of Store apps are grouped by
  the app's own process, not the frame host.
- Pinned apps come first in the user's order. Other open programs follow in the order they were
  first seen, so icons don't move when focus changes.
- Under an open app: a mark (dot, large dot or line). The app in front gets the accent colour;
  apps with several windows show a count.
- Click: no window → open the app; one window → bring it forward, or minimise it if it is already
  in front; several → a list to choose from.
- Middle-click opens a new window.
- Right-click menu: the app's windows, open a new window, pin or unpin, move left or right, close
  window(s), end task.
- An app whose icon can't be read shows a tile with its first letter.

## Widgets

- Every widget sits in its own rounded cell; most are two-line cards (a small upper-case label
  over a larger value), some are a single icon.
- Time: clock (12/24-hour, optional seconds), analog clock, date (click for a month calendar,
  with arrows and a jump back to today), day, month and year progress.
- System: processor load (with a one-minute chart), memory in use (ring), network speed
  (download, upload and a short chart), free space on the system drive, battery (hidden on PCs
  without one), system tray (brings the hidden Windows taskbar up with its tray until the pointer
  leaves it), notifications (opens the notification centre), Recycle Bin (click to open; its menu
  empties it after Windows' own confirmation), show desktop.
- Sound and media: what's playing (from the system media session) with previous, play/pause and
  next; volume (scroll to change, click to mute); microphone (click to mute or unmute).
- Productivity: focus timer (click to start or pause, right-click to reset), stopwatch (the
  same), Time on PC (today's time with someone at the PC, by app in front; local only, starts
  over each day, clearable).
- Online: weather for a chosen place (Open-Meteo, no account), with today's high and low, the
  next three days, sunrise and sunset in its pop-up.
- Settings has a gallery of all widgets with live previews, categories and search; the dock's
  own list there reorders and removes them.

## Settings

- Theme (five of our own; Smoke and Frost are glass: a separate, never-focused window under the
  bar asks Windows to blur the desktop behind it, with corners rounded by Windows 11), size
  (three), layout, screen edge, open-app mark, icon growth, clock format.
- Taskbar choice, full-screen behaviour, start with Windows.
- Widgets, focus timer length, weather place and units.
- Pinned apps: reorder, remove, import again from the Windows taskbar.

## Not yet

Multiple screens, window previews, jump lists, notification badges, and the system tray icons
are not implemented yet.
