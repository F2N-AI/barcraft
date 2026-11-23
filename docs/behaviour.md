# What BarCraft does

The behaviour BarCraft implements, written in our own words from how docks and taskbars visibly
work and from public documentation of that kind of product. Nothing here is copied from another
product's code, text or artwork.

## Placement

- The dock is a bar on the bottom or top edge of the main screen, centred, with a small gap to
  the edge.
- It registers as an application desktop toolbar ("appbar"), so the shell keeps maximised windows
  out of its strip. With "Let windows use the space under the dock" it registers nothing: maximised
  windows reach the screen edge and the dock floats over them (a maximised window is never taken
  for a full-screen one).
- Drag the bar into the other half of the screen to move it to that edge. The choice is saved.
- It stays above other windows, never takes keyboard focus from the app in use, and doesn't
  appear in Alt+Tab.
- When the focused window covers the whole screen (a game, a full-screen video), the dock hides.
  It comes back when that window leaves full screen. This can be turned off.
- Auto-hide (optional): 0.7 s after the pointer leaves the dock, with no menu open, the bar
  slides past the screen edge; touching that edge brings it back. While hidden the dock keeps
  only a 2-point strip along the edge, reserves no space, and hides its glass backdrop.

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
- Under an open app: a quiet mark (dot, large dot or line) in the text colour, a little brighter
  for the app in front; one dot per window, up to three.
- Click: no window → open the app; one window → bring it forward, or minimise it if it is already
  in front; several → a list to choose from.
- Middle-click opens a new window.
- Right-click menu: the app's windows, open a new window, pin or unpin, move left or right, close
  window(s), end task.
- Apps go by their own names: a Store app by its Start menu name ("Terminal", "WhatsApp"),
  any other program by the description in its program file, or its product name when the
  description only names what it is built with ("Electron"), and by its file name when there is
  nothing better. Pinned apps keep the name of their shortcut.
- Store apps take their icon from the Start menu, since many carry only a generic one in their
  program file.
- An app whose icon can't be read shows its first letter on a coloured tile; the colour comes
  from the name, so each app keeps its own.
- An app opened from the dock bounces until its first window shows (three hops at most). A
  pressed icon darkens.
- Start is a tile of its own in the accent colour.
- App icons have no cell behind them. Magnification (on by default, 10 to 100 %): the icon under
  the pointer grows the most and its neighbours less the further away they are, growing away
  from the screen edge and pushing the rest of the row aside. The point under the pointer never
  moves, so a click lands on the icon drawn there. The bar widens by the same amount wherever the
  pointer is along the row, only while the magnification fades in or out. Past the ends of the
  row the end icon stays the largest. With magnification off, a soft patch marks the icon under
  the pointer.
- The name of the app under the pointer shows in a small label above its icon (below it on a
  top dock), with a small pointer at the icon; it fades in at once and follows the pointer.
- The bar is lit from above (a light gradient and a glint along its top edge) with a hairline
  rim. On glass the blur sits 1.5 points inside the bar's edge, so the tighter corners Windows
  gives it stay inside the bar's own, and Windows draws a soft shadow under it. When the dock
  starts it rises from the screen edge and settles; auto-hide eases in and out.

## Widgets

- Every widget sits on a soft rounded platter whose corners run parallel to the bar's, which
  brightens under the pointer; most have two lines (a small label over a larger value), some are
  a single icon. The clock sits on the bar itself. Volume is a slider: drag or click along it,
  scroll over it, click the speaker to mute. The battery card shows the percentage beside a
  battery shape filled with liquid. The date is a small calendar page. Media shows a cover with
  bars that move while something plays.
- Time: clock (12/24-hour, optional seconds), analog clock, date (click for a month calendar,
  with arrows and a jump back to today), day, month and year progress.
- System: processor load (with a one-minute chart), memory in use (ring), network speed
  (download, upload and a short chart), free space on the system drive, battery (hidden on PCs
  without one; liquid fills the card or the tile to the charge left, its surface waving while
  the battery charges or the pointer is on it and still otherwise), system tray (brings the hidden Windows taskbar up with its tray until the pointer
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

- A window of its own, in the dock's light or dark colours: a sidebar of sections, each with a
  coloured icon, and pages of rows grouped on rounded panels, with switches, segmented choices,
  a slider, a stepper and pill buttons. Themes are picked from pictures of the bar on a piece of
  desktop. The widget gallery shows each widget live on a strip of bar, with a tick on those
  already on the dock. `barcraft --open-settings look` opens it at a section.
- Theme (six of our own; the default, Follow Windows, uses Frost in light mode and Smoke in dark
  mode, following Windows as it changes; Smoke and Frost are glass: a separate, never-focused window under the
  bar asks Windows to blur the desktop behind it, with corners rounded by Windows 11), size
  (three), layout, screen edge, open-app mark, magnification and how much, clock format.
- Taskbar choice, full-screen behaviour, start with Windows.
- Widgets, focus timer length, weather place and units.
- Pinned apps: reorder, remove, import again from the Windows taskbar.

## Not yet

Multiple screens, window previews, jump lists, notification badges, and the system tray icons
are not implemented yet.
