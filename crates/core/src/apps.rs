//! How pinned apps and open windows merge into the icons on the dock.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// An app the user keeps on the dock, open or not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedApp {
    /// What to open: a program or a shortcut.
    pub path: PathBuf,
    /// The name shown in tooltips and menus.
    pub name: String,
    /// The program a shortcut points at, used to recognise its open windows.
    #[serde(default)]
    pub target: Option<PathBuf>,
}

impl PinnedApp {
    pub fn new(path: impl Into<PathBuf>, name: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            name: name.into(),
            target: None,
        }
    }

    /// The path open windows are matched against.
    pub fn match_path(&self) -> &Path {
        self.target.as_deref().unwrap_or(&self.path)
    }
}

/// One open window, as the platform layer reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowRef {
    /// The platform's window handle as a number.
    pub id: isize,
    pub title: String,
    /// The program that owns the window, when it could be read.
    pub exe: Option<PathBuf>,
    pub minimized: bool,
}

/// One icon on the dock: a pinned app, a running app, or both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DockItem {
    /// Stable identity, the normalised program path (or the window id for unknown programs).
    pub key: String,
    pub name: String,
    pub pinned: Option<PinnedApp>,
    pub windows: Vec<WindowRef>,
    /// The file to take the icon from.
    pub icon_path: Option<PathBuf>,
    /// The file to open for a new window.
    pub launch_path: Option<PathBuf>,
}

impl DockItem {
    pub fn is_running(&self) -> bool {
        !self.windows.is_empty()
    }
}

/// Lower-case, backslash-separated form of a path, for comparing Windows paths.
pub fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

/// A readable name for a program from its file name: `notepad.exe` becomes `Notepad`.
pub fn display_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut chars = stem.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::from("App"),
    }
}

/// Builds the dock's app icons.
///
/// Pinned apps come first, in the user's order, each with the windows of its program.
/// Programs that are open but not pinned follow, in the order they were first seen;
/// `seen_order` carries that order from one refresh to the next so icons don't jump
/// around when focus changes.
pub fn build_dock_items(
    pinned: &[PinnedApp],
    windows: &[WindowRef],
    seen_order: &mut Vec<String>,
) -> Vec<DockItem> {
    let mut items: Vec<DockItem> = pinned
        .iter()
        .map(|p| DockItem {
            key: normalize_path(p.match_path()),
            name: p.name.clone(),
            pinned: Some(p.clone()),
            windows: Vec::new(),
            icon_path: Some(p.path.clone()),
            launch_path: Some(p.path.clone()),
        })
        .collect();

    let mut loose: Vec<DockItem> = Vec::new();
    for w in windows {
        let key = match &w.exe {
            Some(exe) => normalize_path(exe),
            None => format!("window:{}", w.id),
        };
        if let Some(item) = items.iter_mut().find(|i| i.key == key) {
            item.windows.push(w.clone());
        } else if let Some(item) = loose.iter_mut().find(|i| i.key == key) {
            item.windows.push(w.clone());
        } else {
            loose.push(DockItem {
                key,
                name: w
                    .exe
                    .as_deref()
                    .map(display_name)
                    .unwrap_or_else(|| w.title.clone()),
                pinned: None,
                windows: vec![w.clone()],
                icon_path: w.exe.clone(),
                launch_path: w.exe.clone(),
            });
        }
    }

    for item in &loose {
        if !seen_order.contains(&item.key) {
            seen_order.push(item.key.clone());
        }
    }
    // Forget programs that closed, so the list can't grow without bound.
    seen_order.retain(|k| loose.iter().any(|i| &i.key == k));
    loose.sort_by_key(|i| {
        seen_order
            .iter()
            .position(|k| k == &i.key)
            .unwrap_or(usize::MAX)
    });

    for item in items.iter_mut().chain(loose.iter_mut()) {
        item.windows.sort_by_key(|w| w.id);
    }
    items.extend(loose);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(id: isize, exe: &str) -> WindowRef {
        WindowRef {
            id,
            title: format!("window {id}"),
            exe: Some(PathBuf::from(exe)),
            minimized: false,
        }
    }

    #[test]
    fn windows_join_their_pinned_app() {
        let mut pin = PinnedApp::new("C:\\Links\\Editor.lnk", "Editor");
        pin.target = Some(PathBuf::from("C:\\Apps\\editor.exe"));
        let mut order = Vec::new();
        let items = build_dock_items(&[pin], &[win(2, "c:\\apps\\EDITOR.exe")], &mut order);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].windows.len(), 1);
        assert_eq!(
            items[0].launch_path.as_deref(),
            Some(Path::new("C:\\Links\\Editor.lnk"))
        );
    }

    #[test]
    fn unpinned_programs_keep_first_seen_order() {
        let mut order = Vec::new();
        let a = win(10, "C:\\a.exe");
        let b = win(20, "C:\\b.exe");
        let first = build_dock_items(&[], &[a.clone(), b.clone()], &mut order);
        assert_eq!(first[0].name, "A");
        // Focus changed, so the platform now lists b first.
        let second = build_dock_items(&[], &[b, a], &mut order);
        assert_eq!(second[0].name, "A");
        assert_eq!(second[1].name, "B");
    }

    #[test]
    fn closed_programs_leave_the_order() {
        let mut order = Vec::new();
        build_dock_items(&[], &[win(1, "C:\\a.exe")], &mut order);
        build_dock_items(&[], &[], &mut order);
        assert!(order.is_empty());
    }

    #[test]
    fn windows_of_one_program_group_together() {
        let mut order = Vec::new();
        let items = build_dock_items(
            &[],
            &[
                win(3, "C:\\a.exe"),
                win(1, "C:\\b.exe"),
                win(2, "C:\\a.exe"),
            ],
            &mut order,
        );
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0].windows.iter().map(|w| w.id).collect::<Vec<_>>(),
            vec![2, 3]
        );
    }

    #[test]
    fn window_without_program_stands_alone() {
        let mut order = Vec::new();
        let w = WindowRef {
            id: 7,
            title: "Mystery".into(),
            exe: None,
            minimized: true,
        };
        let items = build_dock_items(&[], &[w], &mut order);
        assert_eq!(items[0].name, "Mystery");
        assert!(items[0].launch_path.is_none());
    }

    #[test]
    fn display_name_capitalises_the_file_stem() {
        assert_eq!(display_name(Path::new("C:\\x\\notepad.exe")), "Notepad");
        assert_eq!(display_name(Path::new("")), "App");
    }
}
