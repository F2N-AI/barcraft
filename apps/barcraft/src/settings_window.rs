//! The settings window: appearance, taskbar, widgets, weather place, pinned apps.

use barcraft_core::widgets::WidgetGroup;
use barcraft_core::{
    DockEdge, DockSize, IndicatorStyle, Settings, TaskbarMode, TempUnit, ThemeId, WidgetKind,
};
use egui::{RichText, Ui, ViewportBuilder, ViewportId};

use crate::APP_NAME;
use crate::poller::WeatherState;

/// What changed, so the app knows what to re-apply.
#[derive(Clone, Copy, Debug, Default)]
pub struct Changes {
    pub any: bool,
    pub placement: bool,
    pub taskbar: bool,
    pub weather: bool,
    pub theme: bool,
    pub focus: bool,
    pub import_pins: bool,
    pub autostart: Option<bool>,
    pub quit: bool,
}

/// State that lives only while the window is open.
#[derive(Default)]
pub struct SettingsUi {
    city: Option<String>,
}

const GROUPS: [WidgetGroup; 5] = [
    WidgetGroup::Time,
    WidgetGroup::System,
    WidgetGroup::Media,
    WidgetGroup::Productivity,
    WidgetGroup::Online,
];

pub fn show(
    ctx: &egui::Context,
    settings: &mut Settings,
    state: &mut SettingsUi,
    weather: &WeatherState,
    open: &mut bool,
) -> Changes {
    let mut changes = Changes::default();
    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("barcraft-settings"),
        ViewportBuilder::default()
            .with_title(format!("{APP_NAME} settings"))
            .with_inner_size([560.0, 720.0])
            .with_min_inner_size([420.0, 360.0]),
        |ui, _class| {
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                *open = false;
            }
            egui::CentralPanel::default().show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        contents(ui, settings, state, weather, &mut changes);
                    });
            });
        },
    );
    changes
}

fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(text).heading().strong());
    ui.add_space(2.0);
}

fn contents(
    ui: &mut Ui,
    s: &mut Settings,
    state: &mut SettingsUi,
    weather: &WeatherState,
    c: &mut Changes,
) {
    heading(ui, "Look");
    egui::Grid::new("look")
        .num_columns(2)
        .spacing([16.0, 8.0])
        .show(ui, |ui| {
            ui.label("Theme");
            ui.horizontal_wrapped(|ui| {
                for t in ThemeId::ALL {
                    if ui.selectable_value(&mut s.theme, t, t.label()).changed() {
                        c.any = true;
                        c.theme = true;
                    }
                }
            });
            ui.end_row();

            ui.label("Size");
            ui.horizontal(|ui| {
                for z in DockSize::ALL {
                    if ui.selectable_value(&mut s.size, z, z.label()).changed() {
                        c.any = true;
                        c.placement = true;
                    }
                }
            });
            ui.end_row();

            ui.label("Screen edge");
            ui.horizontal(|ui| {
                for (e, label) in [(DockEdge::Bottom, "Bottom"), (DockEdge::Top, "Top")] {
                    if ui.selectable_value(&mut s.edge, e, label).changed() {
                        c.any = true;
                        c.placement = true;
                    }
                }
            });
            ui.end_row();

            ui.label("Open apps are marked with");
            ui.horizontal(|ui| {
                for i in IndicatorStyle::ALL {
                    c.any |= ui
                        .selectable_value(&mut s.indicator, i, i.label())
                        .changed();
                }
            });
            ui.end_row();

            ui.label("Clock");
            ui.horizontal(|ui| {
                c.any |= ui.checkbox(&mut s.clock_24h, "24-hour").changed();
                c.any |= ui.checkbox(&mut s.show_seconds, "Seconds").changed();
            });
            ui.end_row();
        });

    heading(ui, "Windows taskbar");
    ui.label("While the dock runs, the Windows taskbar should:");
    for m in TaskbarMode::ALL {
        if ui.radio_value(&mut s.taskbar_mode, m, m.label()).changed() {
            c.any = true;
            c.taskbar = true;
        }
    }
    ui.label(
        RichText::new("It always comes back when the dock closes.")
            .small()
            .weak(),
    );
    if ui
        .checkbox(
            &mut s.hide_for_fullscreen,
            "Step aside for full-screen games and videos",
        )
        .changed()
    {
        c.any = true;
    }
    let mut autostart = s.launch_at_startup;
    if ui.checkbox(&mut autostart, "Start with Windows").changed() {
        c.any = true;
        c.autostart = Some(autostart);
    }

    heading(ui, "Widgets");
    ui.label(
        RichText::new("Tick a widget to put it on the dock; the arrows change its place.").weak(),
    );
    for group in GROUPS {
        ui.add_space(4.0);
        ui.label(RichText::new(group.label()).strong());
        for kind in WidgetKind::ALL.into_iter().filter(|k| k.group() == group) {
            ui.horizontal(|ui| {
                let mut on = s.has_widget(kind);
                if ui
                    .checkbox(&mut on, kind.label())
                    .on_hover_text(kind.description())
                    .changed()
                {
                    s.toggle_widget(kind);
                    c.any = true;
                    c.weather |= kind == WidgetKind::Weather;
                }
                if on {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(" › ").on_hover_text("Move right").clicked() {
                            s.move_widget(kind, 1);
                            c.any = true;
                        }
                        if ui.small_button(" ‹ ").on_hover_text("Move left").clicked() {
                            s.move_widget(kind, -1);
                            c.any = true;
                        }
                    });
                }
            });
        }
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Focus timer length");
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

    heading(ui, "Weather");
    let city = state.city.get_or_insert_with(|| s.weather.city.clone());
    ui.horizontal(|ui| {
        ui.label("Place");
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
            }
            c.any = true;
            c.weather = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label("Units");
        for (u, label) in [(TempUnit::Celsius, "°C"), (TempUnit::Fahrenheit, "°F")] {
            if ui.selectable_value(&mut s.weather.unit, u, label).changed() {
                c.any = true;
                c.weather = true;
            }
        }
    });
    let status = match weather {
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
    ui.label(
        RichText::new(barcraft_core::weather::ATTRIBUTION)
            .small()
            .weak(),
    );

    heading(ui, "Pinned apps");
    if s.pinned.is_empty() {
        ui.label(
            RichText::new("Nothing pinned. Right-click an open app on the dock and choose Pin.")
                .weak(),
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
    if ui
        .button("Add the apps pinned to the Windows taskbar")
        .clicked()
    {
        c.any = true;
        c.import_pins = true;
    }

    heading(ui, "About");
    ui.label(format!("{APP_NAME} {}", env!("CARGO_PKG_VERSION")));
    ui.label(
        RichText::new("Open source under the MIT or Apache-2.0 licence. No account, no telemetry: only the weather widget goes online.")
            .weak(),
    );
    ui.add_space(8.0);
    if ui.button(format!("Quit {APP_NAME}")).clicked() {
        c.quit = true;
        c.any = true;
    }
    ui.add_space(12.0);
}
