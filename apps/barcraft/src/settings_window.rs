//! The settings window: a sidebar of sections with coloured icons, and pages of rows grouped on
//! rounded panels, with switches, segmented controls and a widget gallery with live previews.

use std::time::Instant;

use barcraft_core::clock::Stopwatch;
use barcraft_core::focus::FocusTimer;
use barcraft_core::widgets::WidgetGroup;
use barcraft_core::{
    DockEdge, DockLayout, DockSize, IndicatorStyle, Settings, TaskbarMode, TempUnit, Theme,
    ThemeId, WidgetKind,
};
use egui::{
    Color32, Rect, Sense, Shape, Stroke, StrokeKind, Ui, ViewportBuilder, ViewportId, pos2, vec2,
};

use crate::APP_NAME;
use crate::draw;
use crate::fonts::Weight;
use crate::poller::WeatherState;
use crate::settings_kit::{
    self as kit, Glyph, Kind, Palette, SectionIcon, button, caption, group, rich, title,
};
use crate::widgets::{Readings, WidgetUi};

/// What changed, so the app knows what to re-apply.
#[derive(Clone, Copy, Debug, Default)]
pub struct Changes {
    pub any: bool,
    pub placement: bool,
    pub taskbar: bool,
    pub weather: bool,
    pub theme: bool,
    pub focus: bool,
    pub widgets: bool,
    pub import_pins: bool,
    pub clear_usage: bool,
    pub autostart: Option<bool>,
    pub quit: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Section {
    General,
    Look,
    Taskbar,
    Apps,
    #[default]
    Widgets,
    Weather,
    Privacy,
    About,
}

impl Section {
    const ALL: [Section; 8] = [
        Section::General,
        Section::Look,
        Section::Taskbar,
        Section::Apps,
        Section::Widgets,
        Section::Weather,
        Section::Privacy,
        Section::About,
    ];

    fn label(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Look => "Look",
            Section::Taskbar => "Windows taskbar",
            Section::Apps => "Pinned apps",
            Section::Widgets => "Widgets",
            Section::Weather => "Weather",
            Section::Privacy => "Privacy",
            Section::About => "About",
        }
    }

    /// The section's tile in the sidebar: its colour and picture.
    fn icon(self) -> (Color32, SectionIcon) {
        match self {
            Section::General => (Color32::from_rgb(132, 136, 146), SectionIcon::Gear),
            Section::Look => (Color32::from_rgb(112, 98, 232), SectionIcon::Appearance),
            Section::Taskbar => (Color32::from_rgb(52, 128, 246), SectionIcon::Taskbar),
            Section::Apps => (Color32::from_rgb(248, 138, 52), SectionIcon::Pin),
            Section::Widgets => (Color32::from_rgb(38, 182, 150), SectionIcon::Widgets),
            Section::Weather => (Color32::from_rgb(64, 168, 236), SectionIcon::Cloud),
            Section::Privacy => (Color32::from_rgb(84, 102, 234), SectionIcon::Lock),
            Section::About => (Color32::from_rgb(132, 136, 146), SectionIcon::Info),
        }
    }
}

/// State that lives only while the window is open.
#[derive(Default)]
pub struct SettingsUi {
    section: Section,
    city: Option<String>,
    search: String,
    group: Option<WidgetGroup>,
    confirm_clear: bool,
}

impl SettingsUi {
    /// Opens at the section whose name starts with `name` (any case), else at the default one.
    pub fn at(name: Option<&str>) -> SettingsUi {
        let section = name
            .map(str::to_lowercase)
            .and_then(|n| {
                Section::ALL
                    .into_iter()
                    .find(|s| !n.is_empty() && s.label().to_lowercase().starts_with(&n))
            })
            .unwrap_or_default();
        SettingsUi {
            section,
            ..Default::default()
        }
    }
}

pub fn show(
    ctx: &egui::Context,
    settings: &mut Settings,
    state: &mut SettingsUi,
    readings: &Readings,
    open: &mut bool,
) -> Changes {
    let mut changes = Changes::default();
    let theme = settings.theme.resolve(readings.windows_light);
    let pal = Palette::new(&theme);
    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("barcraft-settings"),
        ViewportBuilder::default()
            .with_title(format!("{APP_NAME} settings"))
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([680.0, 460.0]),
        |ui, _class| {
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                *open = false;
            }
            // The dock's window is see-through; this one must not be, or what's behind shows.
            egui::Panel::left("barcraft-settings-nav")
                .exact_size(224.0)
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(pal.sidebar)
                        .inner_margin(egui::Margin::symmetric(12, 16)),
                )
                .show(ui, |ui| sidebar(ui, &pal, state));
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(pal.window)
                        .inner_margin(egui::Margin::symmetric(30, 0)),
                )
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            // Pages read best at a comfortable width, even in a wide window.
                            let width = ui.available_width().min(720.0);
                            ui.set_max_width(width);
                            ui.add_space(26.0);
                            title(ui, &pal, state.section.label());
                            match state.section {
                                Section::General => general(ui, &pal, settings, &mut changes),
                                Section::Look => look(ui, &pal, settings, &mut changes),
                                Section::Taskbar => taskbar(ui, &pal, settings, &mut changes),
                                Section::Apps => apps(ui, &pal, settings, &mut changes),
                                Section::Widgets => widgets(
                                    ui,
                                    &pal,
                                    &theme,
                                    settings,
                                    state,
                                    readings,
                                    &mut changes,
                                ),
                                Section::Weather => {
                                    weather(ui, &pal, settings, state, readings, &mut changes)
                                }
                                Section::Privacy => {
                                    privacy(ui, &pal, settings, state, readings, &mut changes)
                                }
                                Section::About => about(ui, &pal, &theme, &mut changes),
                            }
                            ui.add_space(30.0);
                        });
                });
        },
    );
    changes
}

fn sidebar(ui: &mut Ui, pal: &Palette, state: &mut SettingsUi) {
    ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
    // The app's own tile and name.
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        let (tile, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
        draw::start_tile(ui.painter(), tile, pal.accent, 0.0, false);
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.label(rich(APP_NAME, 16.0, Weight::SemiBold, pal.text));
            ui.label(rich("Settings", 12.0, Weight::Regular, pal.muted));
        });
    });
    ui.add_space(18.0);
    for s in Section::ALL {
        let (colour, icon) = s.icon();
        if kit::nav_item(ui, pal, s.label(), colour, icon, state.section == s).clicked() {
            state.section = s;
        }
    }
}

fn general(ui: &mut Ui, pal: &Palette, s: &mut Settings, c: &mut Changes) {
    group(ui, pal, None, |rows| {
        let mut autostart = s.launch_at_startup;
        if rows
            .row("Start with Windows", None, |ui| {
                kit::toggle(ui, pal, &mut autostart)
            })
            .changed()
        {
            c.any = true;
            c.autostart = Some(autostart);
        }
        if rows
            .row(
                "Step aside for full-screen apps",
                Some("Games and videos get the whole screen."),
                |ui| kit::toggle(ui, pal, &mut s.hide_for_fullscreen),
            )
            .changed()
        {
            c.any = true;
        }
    });
    group(ui, pal, Some("Dock"), |rows| {
        if rows
            .row(
                "Hide the dock automatically",
                Some("It slides away when you leave it and comes back at the screen edge."),
                |ui| kit::toggle(ui, pal, &mut s.auto_hide),
            )
            .changed()
        {
            c.any = true;
            c.placement = true;
        }
        let auto_hide = s.auto_hide;
        if rows
            .row(
                "Let windows use the space under the dock",
                Some("Maximised windows reach the screen edge and the dock floats over them."),
                |ui| {
                    ui.add_enabled_ui(!auto_hide, |ui| {
                        kit::toggle(ui, pal, &mut s.float_over_windows)
                    })
                    .inner
                },
            )
            .changed()
        {
            c.any = true;
            c.placement = true;
        }
    });
    group(ui, pal, Some("Clock"), |rows| {
        c.any |= rows
            .row("24-hour time", None, |ui| {
                kit::toggle(ui, pal, &mut s.clock_24h)
            })
            .changed();
        c.any |= rows
            .row("Show seconds", None, |ui| {
                kit::toggle(ui, pal, &mut s.show_seconds)
            })
            .changed();
    });
    group(ui, pal, Some("Focus timer"), |rows| {
        let mut minutes = s.focus_minutes;
        if rows.row("Length", None, |ui| {
            kit::stepper(ui, pal, &mut minutes, 1..=180, 5, " min")
        }) {
            s.focus_minutes = minutes;
            c.any = true;
            c.focus = true;
        }
    });
}

/// A small picture of a theme: a piece of desktop with the bar on it.
fn theme_picture(p: &egui::Painter, rect: Rect, t: &Theme) {
    let bar = Rect::from_center_size(
        pos2(rect.center().x, rect.bottom() - 18.0),
        vec2(rect.width() - 34.0, 22.0),
    );
    p.rect_filled(bar, 7, draw::color(t.bar));
    p.rect_stroke(
        bar,
        7,
        Stroke::new(1.0, draw::color(t.bar_edge)),
        StrokeKind::Inside,
    );
    let colours = [
        draw::color(t.accent),
        Color32::from_rgb(255, 128, 92),
        Color32::from_rgb(64, 196, 140),
        Color32::from_rgb(250, 250, 252),
    ];
    for (i, col) in colours.into_iter().enumerate() {
        let r = Rect::from_center_size(
            pos2(bar.left() + 14.0 + i as f32 * 15.0, bar.center().y),
            vec2(11.0, 11.0),
        );
        p.rect_filled(r, 3, col);
    }
    let chip = Rect::from_min_max(
        pos2(bar.right() - 42.0, bar.top() + 4.0),
        pos2(bar.right() - 5.0, bar.bottom() - 4.0),
    );
    p.rect_filled(chip, 4, draw::color(t.tile));
    p.rect_filled(
        Rect::from_center_size(chip.center(), vec2(chip.width() - 14.0, 3.0)),
        1,
        draw::color(t.text),
    );
}

/// A theme to pick: its picture, with a ring around it when chosen, and its name.
fn theme_card(ui: &mut Ui, pal: &Palette, id: ThemeId, chosen: bool) -> bool {
    let size = vec2(158.0, 128.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let picture = Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(size.x - 8.0, 92.0));
    let p = ui.painter();
    // A piece of desktop, the same for every theme.
    p.add(draw::gradient_fill(
        &draw::rounded_points(picture, 10.0),
        Color32::from_rgb(66, 74, 150),
        Color32::from_rgb(206, 118, 92),
    ));
    if id == ThemeId::FollowWindows {
        // Light on the left, dark on the right: the two it switches between.
        let half = picture.center().x;
        let left = Rect::from_min_max(picture.min, pos2(half, picture.bottom()));
        let right = Rect::from_min_max(pos2(half, picture.top()), picture.max);
        theme_picture(&p.with_clip_rect(left), picture, &id.resolve(true));
        theme_picture(&p.with_clip_rect(right), picture, &id.resolve(false));
    } else {
        theme_picture(p, picture, &id.theme());
    }
    let hot = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("hot"), resp.hovered(), 0.12);
    if chosen {
        p.rect_stroke(
            picture.expand(3.0),
            13,
            Stroke::new(2.5, pal.accent),
            StrokeKind::Middle,
        );
    } else if hot > 0.0 {
        p.rect_stroke(
            picture.expand(3.0),
            13,
            Stroke::new(2.0, pal.line.gamma_multiply(hot * 2.0)),
            StrokeKind::Middle,
        );
    }
    let weight = if chosen {
        Weight::SemiBold
    } else {
        Weight::Regular
    };
    p.text(
        pos2(rect.center().x, picture.bottom() + 18.0),
        egui::Align2::CENTER_CENTER,
        id.label(),
        crate::fonts::font(weight, 13.0),
        if chosen { pal.accent } else { pal.text },
    );
    resp.clicked()
}

fn look(ui: &mut Ui, pal: &Palette, s: &mut Settings, c: &mut Changes) {
    ui.add_space(18.0);
    ui.label(rich("Theme", 12.5, Weight::SemiBold, pal.muted));
    ui.add_space(8.0);
    let per_row = ((ui.available_width() + 12.0) / (158.0 + 12.0))
        .floor()
        .max(1.0) as usize;
    for chunk in ThemeId::ALL.chunks(per_row) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(12.0, 8.0);
            for &t in chunk {
                if theme_card(ui, pal, t, s.theme == t) && s.theme != t {
                    s.theme = t;
                    c.any = true;
                    c.theme = true;
                }
            }
        });
    }
    group(ui, pal, Some("Dock"), |rows| {
        let sizes: Vec<(DockSize, &str)> = DockSize::ALL.iter().map(|z| (*z, z.label())).collect();
        if rows.row("Size", None, |ui| {
            kit::segmented(ui, pal, &mut s.size, &sizes)
        }) {
            c.any = true;
            c.placement = true;
        }
        let layouts: Vec<(DockLayout, &str)> =
            DockLayout::ALL.iter().map(|l| (*l, l.label())).collect();
        c.any |= rows.row("Layout", None, |ui| {
            kit::segmented(ui, pal, &mut s.layout, &layouts)
        });
        let edges = [(DockEdge::Bottom, "Bottom"), (DockEdge::Top, "Top")];
        if rows.row(
            "Screen edge",
            Some("You can also drag the dock to the other half of the screen."),
            |ui| kit::segmented(ui, pal, &mut s.edge, &edges),
        ) {
            c.any = true;
            c.placement = true;
        }
        let marks: Vec<(IndicatorStyle, &str)> = IndicatorStyle::ALL
            .iter()
            .map(|i| (*i, i.label()))
            .collect();
        c.any |= rows.row("Open apps are marked with", None, |ui| {
            kit::segmented(ui, pal, &mut s.indicator, &marks)
        });
    });
    group(ui, pal, Some("Icons"), |rows| {
        c.any |= rows
            .row(
                "Magnify under the pointer",
                Some("The icon under the pointer grows and its neighbours make room."),
                |ui| kit::toggle(ui, pal, &mut s.magnify_icons),
            )
            .changed();
        if s.magnify_icons {
            c.any |= rows.row("Magnification", None, |ui| {
                kit::slider(ui, pal, &mut s.magnify_percent, 10..=100, " %")
            });
        }
    });
}

fn taskbar(ui: &mut Ui, pal: &Palette, s: &mut Settings, c: &mut Changes) {
    group(
        ui,
        pal,
        Some("While the dock runs, the Windows taskbar should"),
        |rows| {
            for m in TaskbarMode::ALL {
                let sub = match m {
                    TaskbarMode::Hidden => "Out of sight; the System tray widget brings it up.",
                    TaskbarMode::AutoHide => "It slides in when the pointer reaches its edge.",
                    TaskbarMode::Untouched => "It stays as Windows shows it.",
                };
                if rows.choice(m.label(), Some(sub), s.taskbar_mode == m) && s.taskbar_mode != m {
                    s.taskbar_mode = m;
                    c.any = true;
                    c.taskbar = true;
                }
            }
        },
    );
    caption(
        ui,
        pal,
        "It always comes back when the dock closes, even if the dock crashes. Other screens keep their own taskbar.",
    );
}

fn apps(ui: &mut Ui, pal: &Palette, s: &mut Settings, c: &mut Changes) {
    let mut remove = None;
    let mut shift = None;
    let count = s.pinned.len();
    group(ui, pal, Some("On the dock, in this order"), |rows| {
        if s.pinned.is_empty() {
            rows.custom(|ui| {
                ui.label(rich(
                    "Nothing pinned yet. Right-click an open app on the dock and choose Pin.",
                    13.0,
                    Weight::Regular,
                    pal.muted,
                ));
            });
        }
        for (i, p) in s.pinned.iter().enumerate() {
            let path = p.path.display().to_string();
            let short: String = if path.chars().count() > 48 {
                let tail: String = path
                    .chars()
                    .rev()
                    .take(46)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                format!("…{tail}")
            } else {
                path
            };
            rows.row(&p.name, Some(&short), |ui| {
                // Right to left: remove, then move right, then move left.
                if kit::round_button(ui, pal, Glyph::Remove, true)
                    .on_hover_text("Unpin")
                    .clicked()
                {
                    remove = Some(i);
                }
                ui.add_space(4.0);
                if kit::round_button(ui, pal, Glyph::Right, i + 1 < count)
                    .on_hover_text("Move right")
                    .clicked()
                    && i + 1 < count
                {
                    shift = Some((p.path.clone(), 1));
                }
                if kit::round_button(ui, pal, Glyph::Left, i > 0)
                    .on_hover_text("Move left")
                    .clicked()
                    && i > 0
                {
                    shift = Some((p.path.clone(), -1));
                }
            });
        }
    });
    if let Some(i) = remove
        && i < s.pinned.len()
    {
        s.pinned.remove(i);
        c.any = true;
    }
    if let Some((path, d)) = shift {
        s.move_pin(&path, d);
        c.any = true;
    }
    ui.add_space(14.0);
    if button(
        ui,
        pal,
        "Add the apps pinned to the Windows taskbar",
        Kind::Plain,
    )
    .clicked()
    {
        c.any = true;
        c.import_pins = true;
    }
}

/// A piece of desktop with a strip of the bar on it, for a widget's preview.
fn preview_strip(ui: &Ui, rect: Rect, theme: &Theme) {
    let p = ui.painter();
    p.add(draw::gradient_fill(
        &draw::rounded_points(rect, 10.0),
        Color32::from_rgb(70, 78, 150),
        Color32::from_rgb(170, 104, 120),
    ));
    let bar = rect.shrink2(vec2(10.0, 9.0));
    p.rect_filled(bar, 10, draw::color(theme.bar));
    p.rect_stroke(
        bar,
        10,
        Stroke::new(1.0, draw::color(theme.bar_edge)),
        StrokeKind::Inside,
    );
}

#[allow(clippy::too_many_arguments)]
fn widgets(
    ui: &mut Ui,
    pal: &Palette,
    theme: &Theme,
    s: &mut Settings,
    state: &mut SettingsUi,
    readings: &Readings,
    c: &mut Changes,
) {
    // What's on the dock, in order.
    let mut action: Option<(WidgetKind, isize)> = None;
    let count = s.widgets.len();
    group(ui, pal, Some("On the dock, left to right"), |rows| {
        if s.widgets.is_empty() {
            rows.custom(|ui| {
                ui.label(rich(
                    "No widgets yet. Add some from the gallery below.",
                    13.0,
                    Weight::Regular,
                    pal.muted,
                ));
            });
        }
        for (i, kind) in s.widgets.iter().enumerate() {
            rows.row(kind.label(), Some(kind.group().label()), |ui| {
                if kit::round_button(ui, pal, Glyph::Remove, true)
                    .on_hover_text("Remove from the dock")
                    .clicked()
                {
                    action = Some((*kind, 0));
                }
                ui.add_space(4.0);
                if kit::round_button(ui, pal, Glyph::Right, i + 1 < count)
                    .on_hover_text("Move right")
                    .clicked()
                    && i + 1 < count
                {
                    action = Some((*kind, 1));
                }
                if kit::round_button(ui, pal, Glyph::Left, i > 0)
                    .on_hover_text("Move left")
                    .clicked()
                    && i > 0
                {
                    action = Some((*kind, -1));
                }
            });
        }
    });
    if let Some((kind, d)) = action {
        if d == 0 {
            s.toggle_widget(kind);
        } else {
            s.move_widget(kind, d);
        }
        c.any = true;
        c.widgets = true;
        c.weather |= kind == WidgetKind::Weather;
    }

    ui.add_space(24.0);
    ui.label(rich("Gallery", 18.0, Weight::SemiBold, pal.text));
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        kit::text_field(ui, pal, &mut state.search, "Search widgets", 190.0);
        ui.add_space(6.0);
        if kit::chip(ui, pal, "All", state.group.is_none()).clicked() {
            state.group = None;
        }
        for g in WidgetGroup::ALL {
            if kit::chip(ui, pal, g.label(), state.group == Some(g)).clicked() {
                state.group = Some(g);
            }
        }
    });

    // Previews draw with a copy of the settings and their own timers, so nothing on the dock moves.
    let preview_settings = Settings {
        size: DockSize::Small,
        ..s.clone()
    };
    let now = chrono::Local::now();
    let mut focus = FocusTimer::new(std::time::Duration::from_secs(
        u64::from(s.focus_minutes) * 60,
    ));
    let mut stopwatch = Stopwatch::default();
    let mut calendar = None;
    let mut toggled = None;

    const SPACING: f32 = 14.0;
    for group in WidgetGroup::ALL {
        if state.group.is_some_and(|g| g != group) {
            continue;
        }
        let kinds: Vec<WidgetKind> = WidgetKind::ALL
            .into_iter()
            .filter(|k| k.group() == group && k.matches(&state.search))
            .collect();
        if kinds.is_empty() {
            continue;
        }
        ui.add_space(18.0);
        ui.label(rich(group.label(), 12.5, Weight::SemiBold, pal.muted));
        ui.add_space(8.0);
        // Cards are laid out in as many columns as fit: a frame's size isn't known before it is
        // drawn, so egui's own wrapping can't move one to the next row in time.
        let cols = ((ui.available_width() + SPACING) / (300.0 + SPACING))
            .floor()
            .max(1.0) as usize;
        let card_w = ((ui.available_width() + SPACING) / cols as f32 - SPACING).floor();
        for row in kinds.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(SPACING, SPACING);
                for &kind in row {
                    let on = s.has_widget(kind);
                    // The frame adds its margin and a 1-point stroke on each side.
                    let inner_w = card_w - 30.0;
                    let card = egui::Frame::new()
                        .fill(pal.panel)
                        .stroke(Stroke::new(1.0, pal.line))
                        .corner_radius(14)
                        .inner_margin(14)
                        .show(ui, |ui| {
                            ui.set_width(inner_w);
                            ui.set_height(176.0);
                            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                            ui.vertical(|ui| {
                                // The live preview, on a strip of bar over a piece of desktop.
                                let bar_h = preview_settings.size.bar_height();
                                let (slot, _) = ui.allocate_exact_size(
                                    vec2(inner_w, bar_h + 22.0),
                                    Sense::hover(),
                                );
                                preview_strip(ui, slot, theme);
                                let inside = slot.shrink2(vec2(16.0, 11.0));
                                let mut child = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(inside)
                                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                                );
                                child.set_clip_rect(slot.shrink(10.0));
                                let mut wui = WidgetUi {
                                    theme: *theme,
                                    settings: &preview_settings,
                                    readings,
                                    focus: &mut focus,
                                    stopwatch: &mut stopwatch,
                                    calendar: &mut calendar,
                                    now,
                                    instant: Instant::now(),
                                    height: bar_h,
                                    actions: Vec::new(),
                                    preview: true,
                                    fast_repaint: false,
                                    wheel_used: false,
                                };
                                wui.show(&mut child, kind);
                                if on {
                                    // A check in the corner: already on the dock.
                                    let badge = pos2(slot.right() - 4.0, slot.top() + 4.0);
                                    ui.painter().circle_filled(badge, 9.0, pal.accent);
                                    ui.painter().circle_stroke(
                                        badge,
                                        9.0,
                                        Stroke::new(2.0, pal.panel),
                                    );
                                    ui.painter().add(Shape::line(
                                        vec![
                                            badge + vec2(-3.8, 0.2),
                                            badge + vec2(-1.0, 3.0),
                                            badge + vec2(4.0, -2.6),
                                        ],
                                        Stroke::new(1.8, Color32::WHITE),
                                    ));
                                }
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    ui.label(rich(kind.label(), 14.0, Weight::SemiBold, pal.text));
                                    if kind.goes_online() {
                                        kit::tag(ui, pal, "ONLINE").on_hover_text(
                                            "This widget contacts a service on the internet",
                                        );
                                    }
                                    if kind.records_activity() {
                                        kit::tag(ui, pal, "PRIVATE").on_hover_text(
                                            "Keeps a record on this PC only; clear it in Privacy",
                                        );
                                    }
                                });
                                ui.add(
                                    egui::Label::new(rich(
                                        kind.description(),
                                        12.0,
                                        Weight::Regular,
                                        pal.muted,
                                    ))
                                    .wrap(),
                                );
                                ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
                                    let (label, style) = if on {
                                        ("Remove", Kind::Plain)
                                    } else {
                                        ("Add to the dock", Kind::Primary)
                                    };
                                    if button(ui, pal, label, style).clicked() {
                                        toggled = Some(kind);
                                    }
                                });
                            });
                        });
                    let _ = card;
                }
            });
            ui.add_space(SPACING);
        }
    }
    if let Some(kind) = toggled {
        s.toggle_widget(kind);
        c.any = true;
        c.widgets = true;
        c.weather |= kind == WidgetKind::Weather;
    }
}

fn weather(
    ui: &mut Ui,
    pal: &Palette,
    s: &mut Settings,
    state: &mut SettingsUi,
    readings: &Readings,
    c: &mut Changes,
) {
    let city = state.city.get_or_insert_with(|| s.weather.city.clone());
    let mut find = false;
    group(ui, pal, None, |rows| {
        rows.row("Place", None, |ui| {
            // Right to left: the button, then the field before it.
            find |= button(ui, pal, "Find", Kind::Primary).clicked();
            let edit = kit::text_field(ui, pal, city, "City name, e.g. Bologna", 200.0);
            find |= edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        });
        let units = [(TempUnit::Celsius, "°C"), (TempUnit::Fahrenheit, "°F")];
        if rows.row("Units", None, |ui| {
            kit::segmented(ui, pal, &mut s.weather.unit, &units)
        }) {
            c.any = true;
            c.weather = true;
        }
        let status = match &readings.weather {
            WeatherState::Off => "No place chosen".to_owned(),
            WeatherState::Loading => "Looking it up…".to_owned(),
            WeatherState::Ready { current, .. } => {
                format!("{}, {:.0}°", current.sky.label(), current.temperature)
            }
            WeatherState::Failed(why) => why.clone(),
        };
        let found = if s.weather.resolved_name.is_empty() {
            "Current weather".to_owned()
        } else {
            s.weather.resolved_name.clone()
        };
        rows.row(&found, None, |ui| {
            ui.label(rich(&status, 13.5, Weight::Regular, pal.muted));
        });
    });
    if find {
        s.weather.city = city.trim().to_owned();
        s.weather.latitude = None;
        s.weather.longitude = None;
        s.weather.resolved_name.clear();
        if !s.has_widget(WidgetKind::Weather) && !s.weather.city.is_empty() {
            s.toggle_widget(WidgetKind::Weather);
            c.widgets = true;
        }
        c.any = true;
        c.weather = true;
    }
    caption(ui, pal, barcraft_core::weather::ATTRIBUTION);
}

fn privacy(
    ui: &mut Ui,
    pal: &Palette,
    s: &mut Settings,
    state: &mut SettingsUi,
    readings: &Readings,
    c: &mut Changes,
) {
    let online: Vec<&str> = s
        .widgets
        .iter()
        .filter(|w| w.goes_online())
        .map(|w| w.label())
        .collect();
    group(ui, pal, None, |rows| {
        rows.row("Account and telemetry", None, |ui| {
            ui.label(rich("None", 13.5, Weight::Regular, pal.muted));
        });
        let goes = if online.is_empty() {
            "Nothing".to_owned()
        } else {
            online.join(", ")
        };
        rows.row(
            "Goes online",
            Some("The weather widget sends only the place you chose to Open-Meteo."),
            |ui| {
                ui.label(rich(&goes, 13.5, Weight::Regular, pal.muted));
            },
        );
    });
    group(ui, pal, Some("Time on PC"), |rows| {
        let today = barcraft_core::usage::format_duration(readings.usage_total);
        rows.row("Recorded today", None, |ui| {
            ui.label(rich(&today, 13.5, Weight::Regular, pal.muted));
        });
        rows.row("Today's record", None, |ui| {
            if state.confirm_clear {
                // Right to left: cancel, then the clear button before it.
                if button(ui, pal, "Cancel", Kind::Plain).clicked() {
                    state.confirm_clear = false;
                }
                if button(ui, pal, "Clear", Kind::Danger).clicked() {
                    c.any = true;
                    c.clear_usage = true;
                    state.confirm_clear = false;
                }
            } else if button(ui, pal, "Clear…", Kind::Plain).clicked() {
                state.confirm_clear = true;
            }
        });
    });
    caption(
        ui,
        pal,
        "When the Time on PC widget is on the dock, BarCraft notes which app is in front, every half second, while you are at the PC. Today's totals stay in usage.json next to the settings and start over each day.",
    );
    group(ui, pal, Some("Settings file"), |rows| {
        rows.custom(|ui| {
            ui.add(
                egui::Label::new(rich(
                    &crate::settings_path().display().to_string(),
                    12.5,
                    Weight::Regular,
                    pal.muted,
                ))
                .wrap(),
            );
        });
    });
}

fn about(ui: &mut Ui, pal: &Palette, theme: &Theme, c: &mut Changes) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        let (tile, _) = ui.allocate_exact_size(vec2(84.0, 84.0), Sense::hover());
        draw::start_tile(ui.painter(), tile, draw::color(theme.accent), 0.0, false);
        ui.add_space(12.0);
        ui.label(rich(APP_NAME, 22.0, Weight::SemiBold, pal.text));
        ui.label(rich(
            &format!("Version {}", env!("CARGO_PKG_VERSION")),
            13.0,
            Weight::Regular,
            pal.muted,
        ));
        ui.add_space(10.0);
        ui.label(rich(
            "An open-source dock with live widgets for Windows, made by F2N.",
            14.0,
            Weight::Regular,
            pal.text,
        ));
        ui.label(rich(
            "MIT or Apache-2.0. Written from scratch in Rust.",
            12.5,
            Weight::Regular,
            pal.muted,
        ));
    });
    group(ui, pal, None, |rows| {
        rows.row(
            "Project page",
            Some("Source, issues and releases on GitHub."),
            |ui| {
                if button(ui, pal, "Open", Kind::Plain).clicked() {
                    barcraft_win32::open_url("https://github.com/F2N-AI/barcraft");
                }
            },
        );
        rows.row(
            &format!("Quit {APP_NAME}"),
            Some("The Windows taskbar comes back."),
            |ui| {
                if button(ui, pal, "Quit", Kind::Danger).clicked() {
                    c.quit = true;
                    c.any = true;
                }
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_open_at_a_section_named_by_its_start() {
        assert_eq!(SettingsUi::at(Some("look")).section, Section::Look);
        assert_eq!(SettingsUi::at(Some("WIN")).section, Section::Taskbar);
        assert_eq!(SettingsUi::at(Some("pinned")).section, Section::Apps);
        assert_eq!(SettingsUi::at(Some("")).section, Section::default());
        assert_eq!(SettingsUi::at(Some("nonsense")).section, Section::default());
        assert_eq!(SettingsUi::at(None).section, Section::default());
    }
}
