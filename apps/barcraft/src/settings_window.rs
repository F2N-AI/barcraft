//! The settings window: a sidebar of sections, and a widget gallery with live previews.

use std::time::Instant;

use barcraft_core::clock::Stopwatch;
use barcraft_core::focus::FocusTimer;
use barcraft_core::widgets::WidgetGroup;
use barcraft_core::{
    DockEdge, DockLayout, DockSize, IndicatorStyle, Settings, TaskbarMode, TempUnit, ThemeId,
    WidgetKind,
};
use egui::{Color32, RichText, Sense, Ui, ViewportBuilder, ViewportId, vec2};

use crate::APP_NAME;
use crate::draw;
use crate::poller::WeatherState;
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

pub fn show(
    ctx: &egui::Context,
    settings: &mut Settings,
    state: &mut SettingsUi,
    readings: &Readings,
    open: &mut bool,
) -> Changes {
    let mut changes = Changes::default();
    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("barcraft-settings"),
        ViewportBuilder::default()
            .with_title(format!("{APP_NAME} settings"))
            .with_inner_size([860.0, 680.0])
            .with_min_inner_size([620.0, 420.0]),
        |ui, _class| {
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                *open = false;
            }
            // The dock's window is see-through; this one must not be, or what's behind shows.
            let fill = ui.visuals().panel_fill.to_opaque();
            let side_fill = ui.visuals().extreme_bg_color.to_opaque();
            egui::Panel::left("barcraft-settings-nav")
                .exact_size(190.0)
                .resizable(false)
                .frame(egui::Frame::new().fill(side_fill).inner_margin(10))
                .show(ui, |ui| {
                    ui.add_space(14.0);
                    ui.label(RichText::new(APP_NAME).heading().strong());
                    ui.add_space(12.0);
                    for s in Section::ALL {
                        let selected = state.section == s;
                        let button =
                            egui::Button::selectable(selected, RichText::new(s.label()).size(14.5))
                                .min_size(vec2(170.0, 30.0));
                        if ui.add(button).clicked() {
                            state.section = s;
                        }
                    }
                });
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(fill)
                        .inner_margin(egui::Margin::symmetric(18, 8)),
                )
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_space(8.0);
                            ui.label(RichText::new(state.section.label()).heading().strong());
                            ui.add_space(6.0);
                            match state.section {
                                Section::General => general(ui, settings, &mut changes),
                                Section::Look => look(ui, settings, &mut changes),
                                Section::Taskbar => taskbar(ui, settings, &mut changes),
                                Section::Apps => apps(ui, settings, &mut changes),
                                Section::Widgets => {
                                    widgets(ui, settings, state, readings, &mut changes)
                                }
                                Section::Weather => {
                                    weather(ui, settings, state, readings, &mut changes)
                                }
                                Section::Privacy => {
                                    privacy(ui, settings, state, readings, &mut changes)
                                }
                                Section::About => about(ui, &mut changes),
                            }
                            ui.add_space(16.0);
                        });
                });
        },
    );
    changes
}

fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(vec2(190.0, 24.0), egui::Label::new(label));
        add(ui);
    });
    ui.add_space(4.0);
}

fn note(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().weak());
}

fn general(ui: &mut Ui, s: &mut Settings, c: &mut Changes) {
    let mut autostart = s.launch_at_startup;
    if ui.checkbox(&mut autostart, "Start with Windows").changed() {
        c.any = true;
        c.autostart = Some(autostart);
    }
    if ui
        .checkbox(
            &mut s.hide_for_fullscreen,
            "Step aside for full-screen games and videos",
        )
        .changed()
    {
        c.any = true;
    }
    if ui
        .checkbox(
            &mut s.float_over_windows,
            "Let windows use the space under the dock",
        )
        .on_hover_text(
            "Maximised windows reach the edge of the screen and the dock floats over them, as on a Mac. With a glass theme you then see them, blurred, through the bar.",
        )
        .changed()
    {
        c.any = true;
        c.placement = true;
    }
    ui.add_space(10.0);
    row(ui, "Clock", |ui| {
        c.any |= ui.checkbox(&mut s.clock_24h, "24-hour").changed();
        c.any |= ui.checkbox(&mut s.show_seconds, "Seconds").changed();
    });
    row(ui, "Focus timer length", |ui| {
        let mut minutes = s.focus_minutes;
        if ui
            .add(
                egui::DragValue::new(&mut minutes)
                    .range(1..=180)
                    .suffix(" min"),
            )
            .changed()
        {
            s.focus_minutes = minutes;
            c.any = true;
            c.focus = true;
        }
    });
}

fn swatch(ui: &mut Ui, id: ThemeId) {
    if id == ThemeId::FollowWindows {
        // Both looks it switches between: light, then dark.
        swatch_of(ui, id.resolve(true));
        swatch_of(ui, id.resolve(false));
    } else {
        swatch_of(ui, id.theme());
    }
}

fn swatch_of(ui: &mut Ui, t: barcraft_core::Theme) {
    let (r, _) = ui.allocate_exact_size(vec2(46.0, 18.0), Sense::hover());
    ui.painter().rect_filled(r, 9.0, draw::color(t.bar));
    ui.painter().rect_stroke(
        r,
        9.0,
        egui::Stroke::new(1.0, draw::color(t.bar_edge)),
        egui::StrokeKind::Inside,
    );
    ui.painter().circle_filled(
        r.left_center() + vec2(12.0, 0.0),
        4.5,
        draw::color(t.accent),
    );
    ui.painter()
        .circle_filled(r.left_center() + vec2(24.0, 0.0), 4.5, draw::color(t.text));
    ui.painter()
        .circle_filled(r.left_center() + vec2(36.0, 0.0), 4.5, draw::color(t.muted));
}

fn look(ui: &mut Ui, s: &mut Settings, c: &mut Changes) {
    row(ui, "Theme", |ui| {
        ui.vertical(|ui| {
            for t in ThemeId::ALL {
                ui.horizontal(|ui| {
                    swatch(ui, t);
                    if ui.radio_value(&mut s.theme, t, t.label()).changed() {
                        c.any = true;
                        c.theme = true;
                    }
                });
            }
        });
    });
    row(ui, "Size", |ui| {
        for z in DockSize::ALL {
            if ui.selectable_value(&mut s.size, z, z.label()).changed() {
                c.any = true;
                c.placement = true;
            }
        }
    });
    row(ui, "Layout", |ui| {
        for l in DockLayout::ALL {
            c.any |= ui.selectable_value(&mut s.layout, l, l.label()).changed();
        }
    });
    row(ui, "Screen edge", |ui| {
        for (e, label) in [(DockEdge::Bottom, "Bottom"), (DockEdge::Top, "Top")] {
            if ui.selectable_value(&mut s.edge, e, label).changed() {
                c.any = true;
                c.placement = true;
            }
        }
    });
    row(ui, "Open apps are marked with", |ui| {
        for i in IndicatorStyle::ALL {
            c.any |= ui
                .selectable_value(&mut s.indicator, i, i.label())
                .changed();
        }
    });
    row(ui, "Icons", |ui| {
        c.any |= ui
            .checkbox(&mut s.magnify_icons, "Grow a little under the pointer")
            .changed();
    });
    note(
        ui,
        "Tip: you can also drag the dock to the other half of the screen to move it there.",
    );
}

fn taskbar(ui: &mut Ui, s: &mut Settings, c: &mut Changes) {
    ui.label("While the dock runs, the Windows taskbar should:");
    for m in TaskbarMode::ALL {
        if ui.radio_value(&mut s.taskbar_mode, m, m.label()).changed() {
            c.any = true;
            c.taskbar = true;
        }
    }
    ui.add_space(6.0);
    note(
        ui,
        "It always comes back when the dock closes, even if the dock crashes. Other screens keep their own taskbar.",
    );
    note(
        ui,
        "While it is hidden, the System tray widget brings it up with its tray icons; it hides again once you click elsewhere.",
    );
}

fn apps(ui: &mut Ui, s: &mut Settings, c: &mut Changes) {
    if s.pinned.is_empty() {
        note(
            ui,
            "Nothing pinned. Right-click an open app on the dock and choose Pin.",
        );
    }
    let mut remove = None;
    let mut shift = None;
    for (i, p) in s.pinned.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(&p.name)
                .on_hover_text(p.path.display().to_string());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
                if ui.small_button(" › ").on_hover_text("Move right").clicked() {
                    shift = Some((p.path.clone(), 1));
                }
                if ui.small_button(" ‹ ").on_hover_text("Move left").clicked() {
                    shift = Some((p.path.clone(), -1));
                }
            });
        });
    }
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
    ui.add_space(8.0);
    if ui
        .button("Add the apps pinned to the Windows taskbar")
        .clicked()
    {
        c.any = true;
        c.import_pins = true;
    }
}

fn widgets(
    ui: &mut Ui,
    s: &mut Settings,
    state: &mut SettingsUi,
    readings: &Readings,
    c: &mut Changes,
) {
    // What's on the dock, in order.
    ui.label(RichText::new("On the dock").strong());
    ui.add_space(2.0);
    if s.widgets.is_empty() {
        note(ui, "No widgets yet. Add some from the gallery below.");
    }
    let mut action: Option<(WidgetKind, isize)> = None;
    let count = s.widgets.len();
    egui::Grid::new("barcraft-on-dock")
        .num_columns(4)
        .spacing(vec2(8.0, 2.0))
        .striped(true)
        .show(ui, |ui| {
            for (i, kind) in s.widgets.iter().enumerate() {
                ui.label(format!("{}.", i + 1));
                ui.label(kind.label());
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(i > 0, egui::Button::new(" ‹ ").small())
                        .on_hover_text("Move left on the dock")
                        .clicked()
                    {
                        action = Some((*kind, -1));
                    }
                    if ui
                        .add_enabled(i + 1 < count, egui::Button::new(" › ").small())
                        .on_hover_text("Move right on the dock")
                        .clicked()
                    {
                        action = Some((*kind, 1));
                    }
                });
                if ui.small_button("Remove").clicked() {
                    action = Some((*kind, 0));
                }
                ui.end_row();
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

    ui.add_space(14.0);
    ui.label(RichText::new("Gallery").strong());
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.search)
                .hint_text("Search widgets")
                .desired_width(200.0),
        );
        if ui.selectable_label(state.group.is_none(), "All").clicked() {
            state.group = None;
        }
        for g in WidgetGroup::ALL {
            if ui
                .selectable_label(state.group == Some(g), g.label())
                .clicked()
            {
                state.group = Some(g);
            }
        }
    });
    ui.add_space(6.0);

    // Previews draw with a copy of the settings and their own timers, so nothing on the dock moves.
    let preview_settings = Settings {
        size: DockSize::Small,
        ..s.clone()
    };
    let theme = s.theme.resolve(readings.windows_light);
    let now = chrono::Local::now();
    let mut focus = FocusTimer::new(std::time::Duration::from_secs(
        u64::from(s.focus_minutes) * 60,
    ));
    let mut stopwatch = Stopwatch::default();
    let mut calendar = None;
    let mut toggled = None;

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
        ui.add_space(6.0);
        ui.label(RichText::new(group.label()).weak().strong());
        ui.add_space(4.0);
        // Cards are laid out in as many columns as fit: a frame's size isn't known before it is
        // drawn, so egui's own wrapping can't move one to the next row in time.
        const CARD: f32 = 250.0;
        const SPACING: f32 = 10.0;
        let outer = CARD + 2.0 * 10.0 + 2.0;
        let cols = ((ui.available_width() + SPACING) / (outer + SPACING))
            .floor()
            .max(1.0) as usize;
        for row in kinds.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(SPACING, SPACING);
                for &kind in row {
                    let on = s.has_widget(kind);
                    egui::Frame::new()
                        .fill(ui.visuals().faint_bg_color)
                        .stroke(egui::Stroke::new(
                            1.0,
                            if on {
                                draw::color(theme.accent)
                            } else {
                                ui.visuals().widgets.noninteractive.bg_stroke.color
                            },
                        ))
                        .corner_radius(12)
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.set_width(250.0);
                            ui.set_height(150.0);
                            ui.vertical(|ui| {
                                // The live preview, on a piece of bar.
                                let bar_h = preview_settings.size.bar_height();
                                let (slot, _) = ui
                                    .allocate_exact_size(vec2(250.0, bar_h + 8.0), Sense::hover());
                                ui.painter().rect_filled(slot, 12.0, draw::color(theme.bar));
                                let mut child = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(slot.shrink2(vec2(6.0, 4.0)))
                                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                                );
                                child.set_clip_rect(slot.shrink(2.0));
                                let mut wui = WidgetUi {
                                    theme,
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
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(kind.label()).strong());
                                    if kind.goes_online() {
                                        ui.label(RichText::new("online").small().weak())
                                            .on_hover_text(
                                                "This widget contacts a service on the internet",
                                            );
                                    }
                                    if kind.records_activity() {
                                        ui.label(RichText::new("private").small().weak())
                                        .on_hover_text(
                                            "Keeps a record on this PC only; clear it in Privacy",
                                        );
                                    }
                                });
                                ui.label(RichText::new(kind.description()).small().weak());
                                ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
                                    let label = if on {
                                        "Remove from the dock"
                                    } else {
                                        "Add to the dock"
                                    };
                                    let button = if on {
                                        egui::Button::new(label)
                                    } else {
                                        egui::Button::new(
                                            RichText::new(label).color(Color32::WHITE),
                                        )
                                        .fill(draw::color(theme.accent).gamma_multiply(0.85))
                                    };
                                    if ui.add(button).clicked() {
                                        toggled = Some(kind);
                                    }
                                });
                            });
                        });
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
    s: &mut Settings,
    state: &mut SettingsUi,
    readings: &Readings,
    c: &mut Changes,
) {
    let city = state.city.get_or_insert_with(|| s.weather.city.clone());
    row(ui, "Place", |ui| {
        let edit = ui.add(
            egui::TextEdit::singleline(city)
                .hint_text("City name, e.g. Bologna")
                .desired_width(220.0),
        );
        let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button("Find").clicked() || enter {
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
    });
    row(ui, "Units", |ui| {
        for (u, label) in [(TempUnit::Celsius, "°C"), (TempUnit::Fahrenheit, "°F")] {
            if ui.selectable_value(&mut s.weather.unit, u, label).changed() {
                c.any = true;
                c.weather = true;
            }
        }
    });
    let status = match &readings.weather {
        WeatherState::Off => "No place chosen.".to_owned(),
        WeatherState::Loading => "Looking it up…".to_owned(),
        WeatherState::Ready { current, .. } => format!(
            "{}: {}, {:.0}°",
            s.weather.resolved_name,
            current.sky.label(),
            current.temperature
        ),
        WeatherState::Failed(why) => format!("{why}."),
    };
    ui.label(RichText::new(status).weak());
    ui.add_space(6.0);
    note(ui, barcraft_core::weather::ATTRIBUTION);
}

fn privacy(
    ui: &mut Ui,
    s: &mut Settings,
    state: &mut SettingsUi,
    readings: &Readings,
    c: &mut Changes,
) {
    ui.label("BarCraft has no account and sends no telemetry.");
    ui.add_space(4.0);
    let online: Vec<&str> = s
        .widgets
        .iter()
        .filter(|w| w.goes_online())
        .map(|w| w.label())
        .collect();
    if online.is_empty() {
        note(ui, "Nothing on your dock goes online right now.");
    } else {
        note(
            ui,
            &format!(
                "Goes online: {}. The weather widget sends only the place you chose to Open-Meteo.",
                online.join(", ")
            ),
        );
    }
    ui.add_space(12.0);
    ui.label(RichText::new("Time on PC").strong());
    note(
        ui,
        "When the Time on PC widget is on the dock, BarCraft notes which app is in front, once every half second, while you are at the PC. Today's totals stay in %APPDATA%\\BarCraft\\usage.json and start over each day.",
    );
    ui.label(format!(
        "Recorded today: {}",
        barcraft_core::usage::format_duration(readings.usage_total)
    ));
    if state.confirm_clear {
        ui.horizontal(|ui| {
            ui.label("Clear today's record?");
            if ui.button("Clear").clicked() {
                c.any = true;
                c.clear_usage = true;
                state.confirm_clear = false;
            }
            if ui.button("Cancel").clicked() {
                state.confirm_clear = false;
            }
        });
    } else if ui.button("Clear the record…").clicked() {
        state.confirm_clear = true;
    }
    ui.add_space(12.0);
    ui.label(RichText::new("Settings file").strong());
    note(ui, &crate::settings_path().display().to_string());
}

fn about(ui: &mut Ui, c: &mut Changes) {
    ui.label(RichText::new(format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION"))).strong());
    ui.label("An open-source dock with live widgets for Windows, made by F2N.");
    ui.add_space(4.0);
    note(
        ui,
        "Open source under the MIT or Apache-2.0 licence. Written from scratch in Rust.",
    );
    ui.add_space(8.0);
    if ui.button("Open the project page").clicked() {
        barcraft_win32::open_url("https://github.com/F2N-AI/barcraft");
    }
    ui.add_space(16.0);
    if ui.button(format!("Quit {APP_NAME}")).clicked() {
        c.quit = true;
        c.any = true;
    }
}
