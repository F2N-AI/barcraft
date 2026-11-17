//! The live tiles on the right of the dock.

use std::time::Instant;

use barcraft_core::calendar::{self, WEEKDAY_SHORT};
use barcraft_core::cpu::History;
use barcraft_core::focus::{FocusPhase, FocusTimer, format_countdown};
use barcraft_core::weather::Sky;
use barcraft_core::{DockEdge, Settings, TempUnit, Theme, WidgetKind};
use barcraft_win32::{MemoryStatus, PowerStatus, VolumeStatus};
use chrono::{DateTime, Datelike, Local, Timelike};
use egui::{
    Align2, Color32, FontId, Popup, PopupCloseBehavior, RectAlign, Response, RichText, Sense, Ui,
    vec2,
};

use crate::draw;
use crate::poller::WeatherState;

/// What a widget asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    SetVolume(f32),
    SetMuted(bool),
    ToggleDesktop,
    OpenSettings,
}

/// The readings widgets show, copied out of the poller each frame.
pub struct Readings {
    pub cpu: History,
    pub memory: Option<MemoryStatus>,
    pub power: Option<PowerStatus>,
    pub volume: Option<VolumeStatus>,
    pub weather: WeatherState,
}

pub struct WidgetUi<'a> {
    pub theme: Theme,
    pub settings: &'a Settings,
    pub readings: &'a Readings,
    pub focus: &'a mut FocusTimer,
    pub calendar: &'a mut Option<(i32, u32)>,
    pub now: DateTime<Local>,
    pub instant: Instant,
    pub height: f32,
    pub actions: Vec<Action>,
}

impl WidgetUi<'_> {
    fn text_size(&self) -> f32 {
        self.settings.size.text_size()
    }

    fn popup_align(&self) -> RectAlign {
        match self.settings.edge {
            DockEdge::Bottom => RectAlign::TOP,
            DockEdge::Top => RectAlign::BOTTOM,
        }
    }

    /// Reserves a tile `width` wide, draws its hover background and returns it.
    fn tile(&self, ui: &mut Ui, width: f32, sense: Sense) -> (egui::Rect, Response) {
        let (rect, resp) = ui.allocate_exact_size(vec2(width, self.height), sense);
        if resp.hovered() {
            ui.painter().rect_filled(
                rect.shrink2(vec2(0.0, 4.0)),
                8.0,
                draw::color(self.theme.hover),
            );
        }
        (rect, resp)
    }

    fn text_width(&self, ui: &Ui, text: &str, size: f32) -> f32 {
        ui.painter()
            .layout_no_wrap(text.to_owned(), FontId::proportional(size), Color32::WHITE)
            .size()
            .x
    }

    pub fn show(&mut self, ui: &mut Ui, kind: WidgetKind) {
        match kind {
            WidgetKind::Clock => self.clock(ui),
            WidgetKind::Date => self.date(ui),
            WidgetKind::Cpu => self.cpu(ui),
            WidgetKind::Memory => self.memory(ui),
            WidgetKind::Battery => self.battery(ui),
            WidgetKind::Volume => self.volume(ui),
            WidgetKind::Weather => self.weather(ui),
            WidgetKind::FocusTimer => self.focus_timer(ui),
            WidgetKind::ShowDesktop => self.show_desktop(ui),
        }
    }

    fn clock(&mut self, ui: &mut Ui) {
        let t = self.now;
        let text = match (self.settings.clock_24h, self.settings.show_seconds) {
            (true, false) => format!("{:02}:{:02}", t.hour(), t.minute()),
            (true, true) => format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second()),
            (false, secs) => {
                let (pm, h12) = t.hour12();
                let suffix = if pm { "PM" } else { "AM" };
                if secs {
                    format!("{h12}:{:02}:{:02} {suffix}", t.minute(), t.second())
                } else {
                    format!("{h12}:{:02} {suffix}", t.minute())
                }
            }
        };
        let size = self.text_size() * 1.25;
        let w = self.text_width(ui, &text, size) + 18.0;
        let (rect, resp) = self.tile(ui, w, Sense::hover());
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(size),
            draw::color(self.theme.text),
        );
        resp.on_hover_text(self.now.format("%A %-d %B %Y").to_string());
    }

    fn date(&mut self, ui: &mut Ui) {
        let text = self.now.format("%a %-d %b").to_string();
        let size = self.text_size();
        let w = self.text_width(ui, &text, size) + 18.0;
        let (rect, resp) = self.tile(ui, w, Sense::click());
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(size),
            draw::color(self.theme.text),
        );
        let resp = resp.on_hover_text("Show the calendar");
        if resp.clicked() {
            *self.calendar = Some((self.now.year(), self.now.month()));
        }
        let today = (self.now.year(), self.now.month(), self.now.day());
        let theme = self.theme;
        let align = self.popup_align();
        let calendar = &mut *self.calendar;
        Popup::from_toggle_button_response(&resp)
            .align(align)
            .gap(8.0)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| month_view(ui, calendar, today, theme));
    }

    fn cpu(&mut self, ui: &mut Ui) {
        let load = self.readings.cpu.latest();
        let text = match load {
            Some(l) => format!("CPU {:>3.0}%", l * 100.0),
            None => "CPU  …".to_owned(),
        };
        let size = self.text_size();
        let w = self.text_width(ui, "CPU 100%", size) + 22.0;
        let (rect, resp) = self.tile(ui, w, Sense::hover());
        let chart = rect.shrink2(vec2(6.0, self.height * 0.2));
        let values: Vec<f32> = self.readings.cpu.iter().collect();
        let busy = load.unwrap_or(0.0) > 0.85;
        let line = draw::color(if busy {
            self.theme.warn
        } else {
            self.theme.accent
        });
        draw::sparkline(
            ui.painter(),
            chart,
            &values,
            self.readings.cpu.capacity(),
            line,
        );
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::monospace(size * 0.92),
            draw::color(self.theme.text),
        );
        resp.on_hover_text("Processor load over the last minute");
    }

    fn memory(&mut self, ui: &mut Ui) {
        let Some(m) = self.readings.memory else {
            return;
        };
        let size = self.text_size();
        let text = format!("RAM {:>3}%", m.load_percent);
        let w = self.text_width(ui, "RAM 100%", size) + 22.0;
        let (rect, resp) = self.tile(ui, w, Sense::hover());
        let bar = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 8.0, rect.bottom() - self.height * 0.24),
            vec2(rect.width() - 16.0, 3.0),
        );
        ui.painter()
            .rect_filled(bar, 1.5, draw::color(self.theme.tile));
        let used = egui::Rect::from_min_size(
            bar.min,
            vec2(bar.width() * m.load_percent as f32 / 100.0, bar.height()),
        );
        let fill = if m.load_percent >= 90 {
            self.theme.warn
        } else {
            self.theme.accent
        };
        ui.painter().rect_filled(used, 1.5, draw::color(fill));
        ui.painter().text(
            rect.center() - vec2(0.0, 2.0),
            Align2::CENTER_CENTER,
            text,
            FontId::monospace(size * 0.92),
            draw::color(self.theme.text),
        );
        let gib = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);
        resp.on_hover_text(format!(
            "{:.1} GB of {:.1} GB in use",
            gib(m.total_bytes.saturating_sub(m.available_bytes)),
            gib(m.total_bytes)
        ));
    }

    fn battery(&mut self, ui: &mut Ui) {
        let Some(p) = self.readings.power.filter(|p| p.has_battery) else {
            return;
        };
        let size = self.text_size();
        let pct = p
            .percent
            .map(|v| format!("{v}%"))
            .unwrap_or_else(|| "?".into());
        let w = self.height * 0.9 + self.text_width(ui, &pct, size) + 14.0;
        let (rect, resp) = self.tile(ui, w, Sense::hover());
        let icon = egui::Rect::from_min_size(
            rect.min + vec2(4.0, 0.0),
            vec2(self.height * 0.9, rect.height()),
        );
        let fraction = p.percent.map(|v| f32::from(v) / 100.0).unwrap_or(0.0);
        let low = p.percent.is_some_and(|v| v <= 15) && !p.charging;
        let fill = draw::color(if low {
            self.theme.warn
        } else {
            self.theme.accent
        });
        draw::battery(
            ui.painter(),
            icon,
            fraction,
            p.charging,
            draw::color(self.theme.text),
            fill,
        );
        ui.painter().text(
            egui::pos2(icon.right() + 2.0, rect.center().y),
            Align2::LEFT_CENTER,
            pct,
            FontId::proportional(size),
            draw::color(self.theme.text),
        );
        let state = if p.charging {
            "charging"
        } else if p.on_mains {
            "plugged in"
        } else {
            "on battery"
        };
        resp.on_hover_text(format!(
            "Battery {}, {state}",
            p.percent.map(|v| format!("{v}%")).unwrap_or_default()
        ));
    }

    fn volume(&mut self, ui: &mut Ui) {
        let Some(v) = self.readings.volume else {
            return;
        };
        let size = self.text_size();
        let pct = format!("{:.0}%", v.level * 100.0);
        let w = self.height * 0.8 + self.text_width(ui, "100%", size) + 12.0;
        let (rect, resp) = self.tile(ui, w, Sense::click());
        let icon = egui::Rect::from_min_size(
            rect.min + vec2(2.0, 0.0),
            vec2(self.height * 0.8, rect.height()),
        );
        let ink = draw::color(if v.muted {
            self.theme.muted
        } else {
            self.theme.text
        });
        draw::speaker(ui.painter(), icon, v.level, v.muted, ink);
        ui.painter().text(
            egui::pos2(icon.right(), rect.center().y),
            Align2::LEFT_CENTER,
            pct,
            FontId::proportional(size),
            ink,
        );
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y + i.smooth_scroll_delta.x);
            if scroll.abs() > 0.5 {
                let step = (scroll / 50.0).clamp(-0.1, 0.1);
                self.actions
                    .push(Action::SetVolume((v.level + step).clamp(0.0, 1.0)));
            }
        }
        if resp.clicked() {
            self.actions.push(Action::SetMuted(!v.muted));
        }
        let tip = if v.muted {
            "Muted. Click to unmute."
        } else {
            "Scroll to change the volume, click to mute"
        };
        resp.on_hover_text(tip);
    }

    fn weather(&mut self, ui: &mut Ui) {
        let size = self.text_size();
        let unit = match self.settings.weather.unit {
            TempUnit::Celsius => "C",
            TempUnit::Fahrenheit => "F",
        };
        let (label, sky, day, tip) = match &self.readings.weather {
            WeatherState::Ready { current, .. } => (
                format!("{:.0}°", current.temperature),
                current.sky,
                current.is_day,
                format!(
                    "{}: {}, {:.1} °{unit}\n{}",
                    self.settings.weather.resolved_name,
                    current.sky.label(),
                    current.temperature,
                    barcraft_core::weather::ATTRIBUTION
                ),
            ),
            WeatherState::Loading => (
                "…".to_owned(),
                Sky::Unknown,
                true,
                "Loading the weather".to_owned(),
            ),
            WeatherState::Failed(why) => (
                "?".to_owned(),
                Sky::Unknown,
                true,
                format!("{why}. Click to check the place in Settings."),
            ),
            WeatherState::Off => (
                "Set place".to_owned(),
                Sky::PartlyCloudy,
                true,
                "Choose a place in Settings".to_owned(),
            ),
        };
        let w = self.height * 0.85 + self.text_width(ui, &label, size) + 12.0;
        let (rect, resp) = self.tile(ui, w, Sense::click());
        let icon = egui::Rect::from_min_size(
            rect.min + vec2(2.0, 0.0),
            vec2(self.height * 0.85, rect.height()),
        );
        draw::sky(
            ui.painter(),
            icon,
            sky,
            day,
            draw::color(self.theme.text),
            draw::color(self.theme.accent),
        );
        ui.painter().text(
            egui::pos2(icon.right(), rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(size),
            draw::color(self.theme.text),
        );
        let resp = resp.on_hover_text(tip);
        if resp.clicked() && !matches!(self.readings.weather, WeatherState::Ready { .. }) {
            self.actions.push(Action::OpenSettings);
        }
    }

    fn focus_timer(&mut self, ui: &mut Ui) {
        let size = self.text_size();
        let phase = self.focus.phase(self.instant);
        let remaining = self.focus.remaining(self.instant);
        let text = format_countdown(remaining);
        let w = self.text_width(ui, "00:00", size * 1.05) + 30.0;
        let (rect, resp) = self.tile(ui, w, Sense::click());
        let total = self.focus.length().as_secs_f32().max(1.0);
        let done = 1.0 - remaining.as_secs_f32() / total;
        let track = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 8.0, rect.bottom() - self.height * 0.24),
            vec2(rect.width() - 16.0, 3.0),
        );
        ui.painter()
            .rect_filled(track, 1.5, draw::color(self.theme.tile));
        let accent = draw::color(self.theme.accent);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(track.min, vec2(track.width() * done.clamp(0.0, 1.0), 3.0)),
            1.5,
            accent,
        );
        if phase == FocusPhase::Finished {
            ui.painter().rect_filled(
                rect.shrink2(vec2(0.0, 4.0)),
                8.0,
                accent.gamma_multiply(0.25),
            );
        }
        let ink = draw::color(if phase == FocusPhase::Paused {
            self.theme.muted
        } else {
            self.theme.text
        });
        ui.painter().text(
            rect.center() - vec2(0.0, 2.0),
            Align2::CENTER_CENTER,
            text,
            FontId::monospace(size * 1.05),
            ink,
        );
        if resp.clicked() {
            self.focus.toggle(self.instant);
        }
        if resp.secondary_clicked() {
            self.focus.reset();
        }
        let tip = match phase {
            FocusPhase::Idle => "Focus timer. Click to start, right-click to reset.",
            FocusPhase::Running => "Click to pause, right-click to reset",
            FocusPhase::Paused => "Paused. Click to resume, right-click to reset.",
            FocusPhase::Finished => "Time's up. Click to start again.",
        };
        resp.on_hover_text(tip);
    }

    fn show_desktop(&mut self, ui: &mut Ui) {
        let (rect, resp) = self.tile(ui, self.height * 0.8, Sense::click());
        draw::desktop(ui.painter(), rect, draw::color(self.theme.text));
        if resp.on_hover_text("Show the desktop").clicked() {
            self.actions.push(Action::ToggleDesktop);
        }
    }
}

/// The calendar popup: a month grid with today marked and arrows to move between months.
fn month_view(ui: &mut Ui, shown: &mut Option<(i32, u32)>, today: (i32, u32, u32), theme: Theme) {
    let (year, month) = shown.unwrap_or((today.0, today.1));
    // Seven 32-point columns with 2-point gaps; everything else is sized to match.
    let width = 7.0 * 32.0 + 6.0 * 2.0;
    ui.set_width(width);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let arrow = vec2(28.0, 24.0);
        if ui
            .add_sized(arrow, egui::Button::new("‹"))
            .on_hover_text("Previous month")
            .clicked()
        {
            *shown = calendar::shift_month(year, month, -1).or(*shown);
        }
        let title = format!("{} {year}", calendar::month_name(month));
        let label = egui::Label::new(RichText::new(title).strong()).sense(Sense::click());
        if ui
            .add_sized(vec2(width - 2.0 * arrow.x, arrow.y), label)
            .on_hover_text("Back to today")
            .clicked()
        {
            *shown = Some((today.0, today.1));
        }
        if ui
            .add_sized(arrow, egui::Button::new("›"))
            .on_hover_text("Next month")
            .clicked()
        {
            *shown = calendar::shift_month(year, month, 1).or(*shown);
        }
    });
    ui.add_space(4.0);
    let Some(grid) = calendar::month_grid(year, month) else {
        return;
    };
    egui::Grid::new("month")
        .spacing(vec2(2.0, 2.0))
        .min_col_width(32.0)
        .show(ui, |ui| {
            for d in WEEKDAY_SHORT {
                ui.add_sized(
                    vec2(32.0, 18.0),
                    egui::Label::new(RichText::new(d).small().color(draw::color(theme.muted))),
                );
            }
            ui.end_row();
            for week in &grid.weeks {
                for day in week {
                    match day {
                        Some(d) => {
                            let is_today = (year, month, *d) == today;
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(32.0, 26.0), Sense::hover());
                            if is_today {
                                ui.painter().circle_filled(
                                    rect.center(),
                                    12.0,
                                    draw::color(theme.accent),
                                );
                            }
                            let ink = if is_today {
                                if theme.dark {
                                    Color32::BLACK
                                } else {
                                    Color32::WHITE
                                }
                            } else {
                                draw::color(theme.text)
                            };
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                d.to_string(),
                                FontId::proportional(13.0),
                                ink,
                            );
                        }
                        None => {
                            ui.allocate_exact_size(vec2(32.0, 26.0), Sense::hover());
                        }
                    }
                }
                ui.end_row();
            }
        });
}
