//! BarCraft's own colour themes.

use serde::{Deserialize, Serialize};

/// A colour as unpremultiplied sRGB bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeId {
    /// Glass that follows Windows: Frost in light mode, Smoke in dark mode.
    #[default]
    FollowWindows,
    Graphite,
    Paper,
    Smoke,
    Harbor,
    Frost,
}

impl ThemeId {
    pub const ALL: [ThemeId; 6] = [
        ThemeId::FollowWindows,
        ThemeId::Graphite,
        ThemeId::Paper,
        ThemeId::Smoke,
        ThemeId::Frost,
        ThemeId::Harbor,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemeId::FollowWindows => "Follow Windows",
            ThemeId::Graphite => "Graphite",
            ThemeId::Paper => "Paper",
            ThemeId::Smoke => "Smoke",
            ThemeId::Harbor => "Harbor",
            ThemeId::Frost => "Frost",
        }
    }

    /// The colours for this choice, given whether Windows is in light mode.
    pub fn resolve(self, windows_light: bool) -> Theme {
        match self {
            ThemeId::FollowWindows if windows_light => ThemeId::Frost.theme(),
            ThemeId::FollowWindows => ThemeId::Smoke.theme(),
            other => other.theme(),
        }
    }

    /// The colours for this choice; "Follow Windows" gives its dark-mode look here, use
    /// [`Self::resolve`] to follow the system.
    pub fn theme(self) -> Theme {
        match self {
            ThemeId::FollowWindows => ThemeId::Smoke.theme(),
            ThemeId::Graphite => Theme {
                dark: true,
                glass: false,
                bar: Rgba(28, 29, 33, 245),
                bar_edge: Rgba(255, 255, 255, 22),
                tile: Rgba(255, 255, 255, 12),
                hover: Rgba(255, 255, 255, 26),
                text: Rgba(236, 237, 240, 255),
                muted: Rgba(150, 154, 163, 255),
                accent: Rgba(98, 182, 255, 255),
                warn: Rgba(255, 170, 70, 255),
                popup: Rgba(34, 35, 40, 252),
            },
            ThemeId::Paper => Theme {
                dark: false,
                glass: false,
                bar: Rgba(246, 246, 243, 245),
                bar_edge: Rgba(0, 0, 0, 30),
                tile: Rgba(0, 0, 0, 10),
                hover: Rgba(0, 0, 0, 22),
                text: Rgba(30, 31, 34, 255),
                muted: Rgba(102, 106, 114, 255),
                accent: Rgba(0, 110, 210, 255),
                warn: Rgba(200, 100, 0, 255),
                popup: Rgba(250, 250, 248, 252),
            },
            // Dark frosted glass: the desktop shows through, blurred.
            ThemeId::Smoke => Theme {
                dark: true,
                glass: true,
                bar: Rgba(16, 16, 20, 118),
                bar_edge: Rgba(255, 255, 255, 48),
                tile: Rgba(255, 255, 255, 20),
                hover: Rgba(255, 255, 255, 40),
                text: Rgba(245, 245, 247, 255),
                muted: Rgba(185, 188, 196, 255),
                accent: Rgba(140, 220, 170, 255),
                warn: Rgba(255, 190, 90, 255),
                popup: Rgba(26, 26, 30, 240),
            },
            ThemeId::Harbor => Theme {
                dark: true,
                glass: false,
                bar: Rgba(14, 33, 48, 240),
                bar_edge: Rgba(120, 200, 230, 50),
                tile: Rgba(120, 200, 230, 16),
                hover: Rgba(120, 200, 230, 34),
                text: Rgba(228, 242, 248, 255),
                muted: Rgba(140, 175, 192, 255),
                accent: Rgba(255, 196, 92, 255),
                warn: Rgba(255, 140, 110, 255),
                popup: Rgba(18, 40, 58, 250),
            },
            // Light frosted glass.
            ThemeId::Frost => Theme {
                dark: false,
                glass: true,
                bar: Rgba(248, 249, 252, 112),
                bar_edge: Rgba(255, 255, 255, 120),
                tile: Rgba(255, 255, 255, 64),
                hover: Rgba(255, 255, 255, 112),
                text: Rgba(20, 22, 27, 255),
                muted: Rgba(64, 68, 77, 255),
                accent: Rgba(0, 98, 196, 255),
                warn: Rgba(184, 76, 0, 255),
                popup: Rgba(246, 247, 250, 250),
            },
        }
    }
}

/// The colours one theme gives the dock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub dark: bool,
    /// The bar is see-through and the desktop behind it is blurred.
    pub glass: bool,
    pub bar: Rgba,
    pub bar_edge: Rgba,
    pub tile: Rgba,
    pub hover: Rgba,
    pub text: Rgba,
    pub muted: Rgba,
    pub accent: Rgba,
    pub warn: Rgba,
    pub popup: Rgba,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative luminance per WCAG 2.x.
    fn luminance(c: Rgba) -> f64 {
        fn ch(v: u8) -> f64 {
            let s = f64::from(v) / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * ch(c.0) + 0.7152 * ch(c.1) + 0.0722 * ch(c.2)
    }

    fn contrast(a: Rgba, b: Rgba) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn text_is_readable_on_every_theme() {
        for id in ThemeId::ALL {
            let t = id.theme();
            assert!(contrast(t.text, t.popup) >= 7.0, "{id:?} text");
            assert!(contrast(t.muted, t.popup) >= 4.5, "{id:?} muted text");
        }
    }
}

#[cfg(test)]
mod follow_tests {
    use super::*;

    #[test]
    fn follow_windows_picks_the_glass_for_the_mode() {
        assert_eq!(ThemeId::FollowWindows.resolve(true), ThemeId::Frost.theme());
        assert_eq!(
            ThemeId::FollowWindows.resolve(false),
            ThemeId::Smoke.theme()
        );
        assert_eq!(ThemeId::Paper.resolve(false), ThemeId::Paper.theme());
        assert!(ThemeId::FollowWindows.resolve(true).glass);
    }
}
