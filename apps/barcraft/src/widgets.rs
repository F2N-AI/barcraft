//! The live tiles on the dock. Each sits in its own rounded cell; most are two-line cards with
//! a small label above a larger value.

use std::time::Instant;

use barcraft_core::calendar::{self, WEEKDAY_SHORT};
use barcraft_core::clock::{self, Stopwatch, format_bytes, format_rate, format_stopwatch};
use barcraft_core::cpu::History;
use barcraft_core::focus::{FocusPhase, FocusTimer, format_countdown};
use barcraft_core::usage::format_duration;
use barcraft_core::weather::Sky;
use barcraft_core::{DockEdge, Settings, TempUnit, Theme, WidgetKind};
use barcraft_win32::{
    DiskSpace, MediaAction, MediaStatus, MemoryStatus, PowerStatus, RecycleBinStatus, VolumeStatus,
};
use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike};
use egui::{
    Align2, Color32, FontId, Popup, PopupCloseBehavior, Rect, RectAlign, Response, RichText, Sense,
    Ui, pos2, vec2,
};

use crate::draw::{self, MediaGlyph};
use crate::fonts::{self, Weight};
use crate::poller::WeatherState;

/// Horizontal padding inside a card.
const PAD: f32 = 10.0;
/// Corner radius of a cell: smaller on glass, where the bar's own corners are tighter.
pub fn cell_radius(theme: &Theme) -> f32 {
    if theme.glass { 6.0 } else { 10.0 }
}
/// Space between a cell and the bar's top and bottom edge.
pub const CELL_INSET: f32 = 5.0;

/// What a widget asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    SetVolume(f32),
    SetMuted(bool),
    SetMicrophoneMuted(bool),
    Media(MediaAction),
    ToggleDesktop,
    ShowTray,
    OpenNotifications,
    OpenRecycleBin,
    EmptyRecycleBin,
    OpenSettings,
}

/// The readings widgets show, copied out of the poller each frame.
#[derive(Clone)]
pub struct Readings {
    pub cpu: History,
    pub memory: Option<MemoryStatus>,
    pub power: Option<PowerStatus>,
    pub volume: Option<VolumeStatus>,
    pub microphone: Option<VolumeStatus>,
    pub media: Option<MediaStatus>,
    /// Download speed in bytes per second, newest last.
    pub net_down: Vec<f64>,
    pub net_up: f64,
    pub disk: Option<DiskSpace>,
    pub recycle_bin: Option<RecycleBinStatus>,
    pub usage_total: u64,
    pub usage_top: Vec<(String, u64)>,
    pub weather: WeatherState,
}

pub struct WidgetUi<'a> {
    pub theme: Theme,
    pub settings: &'a Settings,
    pub readings: &'a Readings,
    pub focus: &'a mut FocusTimer,
    pub stopwatch: &'a mut Stopwatch,
    pub calendar: &'a mut Option<(i32, u32)>,
    pub now: DateTime<Local>,
    pub instant: Instant,
    pub height: f32,
    pub actions: Vec<Action>,
    /// Drawing a preview in the settings gallery: no popups, no actions.
    pub preview: bool,
    /// Set by a widget that used the mouse wheel itself (the volume widget), so the widget area
    /// doesn't scroll as well.
    pub wheel_used: bool,
    /// Set by widgets that change faster than once a second (a running stopwatch).
    pub fast_repaint: bool,
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        let mut t: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

impl WidgetUi<'_> {
    fn s(&self) -> f32 {
        self.settings.size.text_size()
    }

    fn label_font(&self) -> FontId {
        fonts::font(Weight::SemiBold, self.s() * 0.66)
    }

    fn value_font(&self) -> FontId {
        fonts::font(Weight::Bold, self.s() * 1.1)
    }

    fn ink(&self) -> Color32 {
        draw::color(self.theme.text)
    }

    fn muted(&self) -> Color32 {
        draw::color(self.theme.muted)
    }

    fn accent(&self) -> Color32 {
        draw::color(self.theme.accent)
    }

    fn warn(&self) -> Color32 {
        draw::color(self.theme.warn)
    }

    fn popup_align(&self) -> RectAlign {
        match self.settings.edge {
            DockEdge::Bottom => RectAlign::TOP,
            DockEdge::Top => RectAlign::BOTTOM,
        }
    }

    fn text_width(&self, ui: &Ui, text: &str, font: FontId) -> f32 {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font, Color32::WHITE)
            .size()
            .x
    }

    /// Reserves a cell `width` wide and paints its background; returns the cell's rectangle.
    fn cell(&self, ui: &mut Ui, width: f32, sense: Sense) -> (Rect, Response) {
        let (rect, resp) = ui.allocate_exact_size(vec2(width, self.height), sense);
        let cell = rect.shrink2(vec2(1.0, CELL_INSET));
        let hot = resp.hovered() && sense.senses_click();
        let fill = if hot {
            self.theme.hover
        } else {
            self.theme.tile
        };
        draw::cell(
            ui.painter(),
            cell,
            cell_radius(&self.theme),
            draw::color(fill),
            &self.theme,
        );
        (cell, resp)
    }

    /// Paints a small upper-case label over a larger value, left-aligned in `area`.
    fn two_lines(
        &self,
        ui: &Ui,
        area: Rect,
        label: &str,
        value: &str,
        label_color: Color32,
        value_color: Color32,
    ) {
        let h = area.height();
        ui.painter().text(
            pos2(area.left() + PAD, area.center().y - h * 0.19),
            Align2::LEFT_CENTER,
            label.to_uppercase(),
            self.label_font(),
            label_color,
        );
        ui.painter().text(
            pos2(area.left() + PAD, area.center().y + h * 0.14),
            Align2::LEFT_CENTER,
            value,
            self.value_font(),
            value_color,
        );
    }

    /// A two-line card with `extra` points of room on its right for an icon or chart.
    /// Returns the cell, the space for the extra, and the response.
    fn card(
        &self,
        ui: &mut Ui,
        label: &str,
        value: &str,
        extra: f32,
        sense: Sense,
    ) -> (Rect, Rect, Response) {
        self.card_with(ui, label, value, None, extra, sense, self.muted())
    }

    /// A card for a value that changes all the time: it is as wide as `widest` would need, so the
    /// bar doesn't shift each time the number does.
    fn card_steady(
        &self,
        ui: &mut Ui,
        label: &str,
        value: &str,
        widest: &str,
        extra: f32,
        sense: Sense,
    ) -> (Rect, Rect, Response) {
        self.card_with(ui, label, value, Some(widest), extra, sense, self.muted())
    }

    /// Like [`Self::card`], with the label in `label_color` and an optional widest value.
    #[allow(clippy::too_many_arguments)]
    fn card_with(
        &self,
        ui: &mut Ui,
        label: &str,
        value: &str,
        widest: Option<&str>,
        extra: f32,
        sense: Sense,
        label_color: Color32,
    ) -> (Rect, Rect, Response) {
        let value_w = self.text_width(ui, value, self.value_font()).max(
            widest
                .map(|w| self.text_width(ui, w, self.value_font()))
                .unwrap_or(0.0),
        );
        let text_w = self
            .text_width(ui, &label.to_uppercase(), self.label_font())
            .max(value_w);
        let width = text_w + 2.0 * PAD + extra;
        let (cell, resp) = self.cell(ui, width, sense);
        let extra_rect = Rect::from_min_max(pos2(cell.right() - extra, cell.top()), cell.max);
        let text_rect = Rect::from_min_max(cell.min, pos2(cell.right() - extra, cell.bottom()));
        self.two_lines(ui, text_rect, label, value, label_color, self.ink());
        (cell, extra_rect, resp)
    }

    /// An icon-only cell.
    fn icon_cell(&self, ui: &mut Ui, sense: Sense) -> (Rect, Response) {
        self.cell(ui, self.height * 0.82, sense)
    }

    /// A thin bar along the bottom of a cell.
    fn bottom_bar(&self, ui: &Ui, cell: Rect, fraction: f32, fill: Color32) {
        let r = Rect::from_min_size(
            pos2(cell.left() + PAD, cell.bottom() - 7.0),
            vec2(cell.width() - 2.0 * PAD, 3.0),
        );
        draw::bar(
            ui.painter(),
            r,
            fraction,
            draw::color(self.theme.hover),
            fill,
        );
    }

    pub fn show(&mut self, ui: &mut Ui, kind: WidgetKind) {
        match kind {
            WidgetKind::Clock => self.clock(ui),
            WidgetKind::AnalogClock => self.analog_clock(ui),
            WidgetKind::Date => self.date(ui),
            WidgetKind::DayProgress => {
                let f = clock::day_fraction(self.now.naive_local());
                self.progress(ui, "Today", f);
            }
            WidgetKind::MonthProgress => {
                let f = clock::month_fraction(self.now.naive_local());
                self.progress(ui, calendar::month_name(self.now.month()), f);
            }
            WidgetKind::YearProgress => {
                let f = clock::year_fraction(self.now.naive_local());
                self.progress(ui, &self.now.year().to_string(), f);
            }
            WidgetKind::Cpu => self.cpu(ui),
            WidgetKind::Memory => self.memory(ui),
            WidgetKind::CpuRing => {
                let load = self.readings.cpu.latest();
                self.ring_tile(ui, "CPU", load, "Processor load".to_owned());
            }
            WidgetKind::MemoryRing => {
                let Some(m) = self.readings.memory else {
                    return;
                };
                let gib = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);
                let tip = format!(
                    "Memory: {:.1} GB of {:.1} GB in use",
                    gib(m.total_bytes.saturating_sub(m.available_bytes)),
                    gib(m.total_bytes)
                );
                self.ring_tile(ui, "RAM", Some(m.load_percent as f32 / 100.0), tip);
            }
            WidgetKind::BatteryTile => self.battery_tile(ui),
            WidgetKind::Network => self.network(ui),
            WidgetKind::Storage => self.storage(ui),
            WidgetKind::Battery => self.battery(ui),
            WidgetKind::SystemTray => self.system_tray(ui),
            WidgetKind::Notifications => self.notifications(ui),
            WidgetKind::RecycleBin => self.recycle_bin(ui),
            WidgetKind::ShowDesktop => self.show_desktop(ui),
            WidgetKind::Volume => self.volume(ui),
            WidgetKind::Media => self.media(ui),
            WidgetKind::Microphone => self.microphone(ui),
            WidgetKind::FocusTimer => self.focus_timer(ui),
            WidgetKind::Stopwatch => self.stopwatch(ui),
            WidgetKind::TimeOnPc => self.time_on_pc(ui),
            WidgetKind::Weather => self.weather(ui),
        }
    }

    fn act(&mut self, action: Action) {
        if !self.preview {
            self.actions.push(action);
        }
    }

    fn clock(&mut self, ui: &mut Ui) {
        let t = self.now;
        let (time, suffix) = match (self.settings.clock_24h, self.settings.show_seconds) {
            (true, false) => (format!("{:02}:{:02}", t.hour(), t.minute()), None),
            (true, true) => (
                format!("{:02}:{:02}:{:02}", t.hour(), t.minute(), t.second()),
                None,
            ),
            (false, secs) => {
                let (pm, h12) = t.hour12();
                let time = if secs {
                    format!("{h12}:{:02}:{:02}", t.minute(), t.second())
                } else {
                    format!("{h12}:{:02}", t.minute())
                };
                (time, Some(if pm { "PM" } else { "AM" }))
            }
        };
        let font = fonts::font(Weight::Bold, self.s() * 1.4);
        let small = self.label_font();
        let suffix_w = suffix
            .map(|s| self.text_width(ui, s, small.clone()) + 3.0)
            .unwrap_or(0.0);
        let time_w = self.text_width(ui, &time, font.clone());
        // The cell is as wide as the widest time of this format, so the bar never shifts.
        let widest = match (self.settings.clock_24h, self.settings.show_seconds) {
            (true, false) | (false, false) => "88:88",
            (true, true) | (false, true) => "88:88:88",
        };
        let widest_w = self.text_width(ui, widest, font.clone()).max(time_w);
        let (cell, resp) = self.cell(ui, widest_w + suffix_w + 2.0 * PAD + 4.0, Sense::hover());
        let left = cell.center().x - (time_w + suffix_w) / 2.0;
        ui.painter().text(
            pos2(left, cell.center().y),
            Align2::LEFT_CENTER,
            time,
            font,
            self.ink(),
        );
        if let Some(sfx) = suffix {
            ui.painter().text(
                pos2(left + time_w + 3.0, cell.center().y + self.s() * 0.25),
                Align2::LEFT_CENTER,
                sfx,
                small,
                self.muted(),
            );
        }
        resp.on_hover_text(self.now.format("%A %-d %B %Y").to_string());
    }

    fn analog_clock(&mut self, ui: &mut Ui) {
        let (cell, resp) = self.cell(ui, self.height * 0.9, Sense::hover());
        let secs = self.settings.show_seconds.then_some(self.now.second());
        draw::analog_clock(
            ui.painter(),
            cell,
            (self.now.hour(), self.now.minute(), secs),
            draw::color(self.theme.hover),
            self.ink(),
            self.accent(),
        );
        resp.on_hover_text(self.now.format("%H:%M · %A %-d %B").to_string());
    }

    fn date(&mut self, ui: &mut Ui) {
        let weekday = self.now.format("%A").to_string();
        let value = self.now.format("%-d %b").to_string();
        // The weekday is the eye-catcher, so it takes the accent colour.
        let (_, _, resp) = self.card_with(
            ui,
            &weekday,
            &value,
            Some("30 Sep"),
            0.0,
            Sense::click(),
            self.accent(),
        );
        let resp = resp.on_hover_text("Show the calendar");
        if self.preview {
            return;
        }
        if resp.clicked() {
            *self.calendar = Some((self.now.year(), self.now.month()));
        }
        let today = (self.now.year(), self.now.month(), self.now.day());
        let theme = self.theme;
        let align = self.popup_align();
        let shown = &mut *self.calendar;
        Popup::from_toggle_button_response(&resp)
            .align(align)
            .gap(8.0)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| month_view(ui, shown, today, theme));
    }

    fn progress(&mut self, ui: &mut Ui, label: &str, fraction: f32) {
        let value = format!("{:.0}%", fraction * 100.0);
        let (cell, _, resp) = self.card_steady(ui, label, &value, "100%", 0.0, Sense::hover());
        self.bottom_bar(ui, cell, fraction, self.accent());
        resp.on_hover_text(format!("{:.1}% of {} has gone by", fraction * 100.0, label));
    }

    fn cpu(&mut self, ui: &mut Ui) {
        let load = self.readings.cpu.latest();
        let value = load
            .map(|l| format!("{:.0}%", l * 100.0))
            .unwrap_or_else(|| "…".into());
        let (_, extra, resp) = self.card_steady(ui, "CPU", &value, "100%", 52.0, Sense::hover());
        let values: Vec<f32> = self.readings.cpu.iter().collect();
        let busy = load.unwrap_or(0.0) > 0.85;
        let line = if busy { self.warn() } else { self.accent() };
        draw::sparkline(
            ui.painter(),
            extra.shrink2(vec2(8.0, extra.height() * 0.24)),
            &values,
            self.readings.cpu.capacity(),
            line,
        );
        resp.on_hover_text("Processor load over the last minute");
    }

    /// A square tile: a ring filled to `fraction`, the percentage inside it and a small label
    /// under the number.
    fn ring_tile(&mut self, ui: &mut Ui, label: &str, fraction: Option<f32>, tip: String) {
        let side = self.height - 2.0 * CELL_INSET;
        let (cell, resp) = self.cell(ui, side + 2.0, Sense::hover());
        let center = cell.center();
        let radius = cell.height() * 0.37;
        let f = fraction.unwrap_or(0.0);
        let fill = if f > 0.85 { self.warn() } else { self.accent() };
        draw::ring(
            ui.painter(),
            center,
            radius,
            f,
            3.2,
            draw::color(self.theme.hover),
            fill,
        );
        let value = fraction
            .map(|f| format!("{:.0}", f * 100.0))
            .unwrap_or_else(|| "…".into());
        ui.painter().text(
            center - vec2(0.0, radius * 0.14),
            Align2::CENTER_CENTER,
            value,
            fonts::font(Weight::Bold, self.s() * 0.98),
            self.ink(),
        );
        ui.painter().text(
            center + vec2(0.0, radius * 0.55),
            Align2::CENTER_CENTER,
            label,
            fonts::font(Weight::SemiBold, self.s() * 0.5),
            self.muted(),
        );
        resp.on_hover_text(match fraction {
            Some(f) => format!("{tip}: {:.0}%", f * 100.0),
            None => tip,
        });
    }

    /// A square tile that fills from the bottom with the charge left.
    fn battery_tile(&mut self, ui: &mut Ui) {
        let Some(p) = self.readings.power.filter(|p| p.has_battery) else {
            return;
        };
        let side = self.height - 2.0 * CELL_INSET;
        let (cell, resp) = self.cell(ui, side + 2.0, Sense::hover());
        let fraction = p.percent.map(|v| f32::from(v) / 100.0).unwrap_or(0.0);
        let low = p.percent.is_some_and(|v| v <= 20) && !p.charging;
        let level = if low { self.warn() } else { self.accent() };
        let inner = cell.shrink(2.0);
        let h = inner.height() * fraction.clamp(0.0, 1.0);
        if h > 0.5 {
            let r = (cell_radius(&self.theme) - 2.0).max(0.0) as u8;
            let top = if fraction > 0.95 { r } else { 2 };
            let fill_rect = Rect::from_min_max(pos2(inner.left(), inner.bottom() - h), inner.max);
            ui.painter().rect_filled(
                fill_rect,
                egui::CornerRadius {
                    nw: top,
                    ne: top,
                    sw: r,
                    se: r,
                },
                level.gamma_multiply(0.42),
            );
            ui.painter().line_segment(
                [
                    pos2(fill_rect.left() + 3.0, fill_rect.top()),
                    pos2(fill_rect.right() - 3.0, fill_rect.top()),
                ],
                egui::Stroke::new(1.5, level),
            );
        }
        let value = p
            .percent
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".into());
        let font = fonts::font(Weight::Bold, self.s() * 1.05);
        let value_w = self.text_width(ui, &value, font.clone());
        let bolt_w = if p.charging { self.s() * 0.62 } else { 0.0 };
        let left = cell.center().x - (value_w + bolt_w) / 2.0;
        if p.charging {
            draw::bolt(
                ui.painter(),
                pos2(left + bolt_w * 0.4, cell.center().y),
                self.s() * 0.95,
                self.ink(),
            );
        }
        ui.painter().text(
            pos2(left + bolt_w, cell.center().y),
            Align2::LEFT_CENTER,
            value,
            font,
            self.ink(),
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

    fn memory(&mut self, ui: &mut Ui) {
        let Some(m) = self.readings.memory else {
            return;
        };
        let value = format!("{}%", m.load_percent);
        let ring = self.height * 0.62;
        let (_, extra, resp) = self.card_steady(ui, "Memory", &value, "100%", ring, Sense::hover());
        let fill = if m.load_percent >= 90 {
            self.warn()
        } else {
            self.accent()
        };
        draw::ring(
            ui.painter(),
            extra.center() - vec2(4.0, 0.0),
            ring * 0.28,
            m.load_percent as f32 / 100.0,
            3.0,
            draw::color(self.theme.hover),
            fill,
        );
        let gib = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);
        resp.on_hover_text(format!(
            "{:.1} GB of {:.1} GB in use",
            gib(m.total_bytes.saturating_sub(m.available_bytes)),
            gib(m.total_bytes)
        ));
    }

    fn network(&mut self, ui: &mut Ui) {
        let down = self.readings.net_down.last().copied().unwrap_or(0.0);
        let up_text = format_rate(self.readings.net_up);
        let down_text = format_rate(down);
        let arrow_w = 12.0;
        let chart = 44.0;
        let text_w = self
            .text_width(ui, &up_text, self.label_font())
            .max(self.text_width(ui, &down_text, self.value_font()));
        let (cell, resp) = self.cell(ui, text_w + arrow_w + 2.0 * PAD + chart, Sense::hover());
        let h = cell.height();
        let x = cell.left() + PAD;
        let up_y = cell.center().y - h * 0.19;
        let down_y = cell.center().y + h * 0.14;
        draw::arrow(ui.painter(), pos2(x + 4.0, up_y), 7.0, true, self.muted());
        draw::arrow(
            ui.painter(),
            pos2(x + 4.0, down_y),
            8.0,
            false,
            self.accent(),
        );
        ui.painter().text(
            pos2(x + arrow_w, up_y),
            Align2::LEFT_CENTER,
            up_text,
            self.label_font(),
            self.muted(),
        );
        ui.painter().text(
            pos2(x + arrow_w, down_y),
            Align2::LEFT_CENTER,
            down_text,
            self.value_font(),
            self.ink(),
        );
        let peak = self
            .readings
            .net_down
            .iter()
            .copied()
            .fold(1.0f64, f64::max);
        let values: Vec<f32> = self
            .readings
            .net_down
            .iter()
            .map(|v| (v / peak) as f32)
            .collect();
        let chart_rect = Rect::from_min_max(pos2(cell.right() - chart, cell.top()), cell.max)
            .shrink2(vec2(6.0, h * 0.24));
        draw::sparkline(ui.painter(), chart_rect, &values, 30, self.accent());
        resp.on_hover_text("Download (below) and upload (above) speed");
    }

    fn storage(&mut self, ui: &mut Ui) {
        let Some(d) = self.readings.disk else {
            return;
        };
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        let value = format!("{} free", format_bytes(d.free));
        let (cell, _, resp) = self.card_steady(
            ui,
            &format!("Disk {drive}"),
            &value,
            "888 GB free",
            0.0,
            Sense::hover(),
        );
        let used = 1.0 - d.free as f32 / d.total.max(1) as f32;
        let fill = if used > 0.9 {
            self.warn()
        } else {
            self.accent()
        };
        self.bottom_bar(ui, cell, used, fill);
        resp.on_hover_text(format!(
            "{} free of {}",
            format_bytes(d.free),
            format_bytes(d.total)
        ));
    }

    fn battery(&mut self, ui: &mut Ui) {
        let Some(p) = self.readings.power.filter(|p| p.has_battery) else {
            return;
        };
        let value = p
            .percent
            .map(|v| format!("{v}%"))
            .unwrap_or_else(|| "?".into());
        let label = if p.charging {
            "Charging"
        } else if p.on_mains {
            "Plugged in"
        } else {
            "Battery"
        };
        let icon = self.height * 0.72;
        let (_, extra, resp) = self.card_steady(ui, label, &value, "100%", icon, Sense::hover());
        let fraction = p.percent.map(|v| f32::from(v) / 100.0).unwrap_or(0.0);
        let low = p.percent.is_some_and(|v| v <= 15) && !p.charging;
        let fill = if low { self.warn() } else { self.accent() };
        draw::battery(
            ui.painter(),
            extra.translate(vec2(-6.0, 0.0)),
            fraction,
            p.charging,
            self.ink(),
            fill,
        );
        resp.on_hover_text(format!("Battery {value}, {}", label.to_lowercase()));
    }

    fn system_tray(&mut self, ui: &mut Ui) {
        let (cell, resp) = self.icon_cell(ui, Sense::click());
        draw::chevron(
            ui.painter(),
            cell,
            self.settings.edge == DockEdge::Bottom,
            self.ink(),
        );
        if resp.on_hover_text("Show the system tray").clicked() {
            self.act(Action::ShowTray);
        }
    }

    fn notifications(&mut self, ui: &mut Ui) {
        let (cell, resp) = self.icon_cell(ui, Sense::click());
        draw::bell(ui.painter(), cell, self.ink());
        if resp.on_hover_text("Open notifications").clicked() {
            self.act(Action::OpenNotifications);
        }
    }

    fn recycle_bin(&mut self, ui: &mut Ui) {
        let bin = self.readings.recycle_bin;
        let full = bin.is_some_and(|b| b.items > 0);
        let (cell, resp) = self.icon_cell(ui, Sense::click());
        draw::bin(ui.painter(), cell, full, self.ink());
        let tip = match bin {
            Some(b) if b.items > 0 => format!(
                "Recycle Bin: {} item{}, {}\nRight-click to empty it",
                b.items,
                if b.items == 1 { "" } else { "s" },
                format_bytes(b.bytes)
            ),
            Some(_) => "The Recycle Bin is empty".to_owned(),
            None => "Recycle Bin".to_owned(),
        };
        let resp = resp.on_hover_text(tip);
        if self.preview {
            return;
        }
        if resp.clicked() {
            self.act(Action::OpenRecycleBin);
        }
        let mut chosen = None;
        Popup::context_menu(&resp).show(|ui| {
            if ui.button("Open the Recycle Bin").clicked() {
                chosen = Some(Action::OpenRecycleBin);
            }
            if ui
                .add_enabled(full, egui::Button::new("Empty the Recycle Bin…"))
                .on_hover_text("Windows asks you to confirm first")
                .clicked()
            {
                chosen = Some(Action::EmptyRecycleBin);
            }
        });
        if let Some(a) = chosen {
            self.act(a);
        }
    }

    fn show_desktop(&mut self, ui: &mut Ui) {
        let (cell, resp) = self.icon_cell(ui, Sense::click());
        draw::desktop(ui.painter(), cell, self.ink());
        if resp.on_hover_text("Show the desktop").clicked() {
            self.act(Action::ToggleDesktop);
        }
    }

    fn volume(&mut self, ui: &mut Ui) {
        let Some(v) = self.readings.volume else {
            return;
        };
        let value = format!("{:.0}%", v.level * 100.0);
        let icon = self.height * 0.62;
        let label = if v.muted { "Muted" } else { "Sound" };
        let (_, extra, resp) = self.card_steady(ui, label, &value, "100%", icon, Sense::click());
        let ink = if v.muted { self.muted() } else { self.ink() };
        draw::speaker(
            ui.painter(),
            extra.translate(vec2(-6.0, 0.0)),
            v.level,
            v.muted,
            ink,
        );
        if resp.hovered() && !self.preview {
            self.wheel_used = true;
            let scroll = ui.input(|i| i.smooth_scroll_delta.y + i.smooth_scroll_delta.x);
            if scroll.abs() > 0.5 {
                let step = (scroll / 50.0).clamp(-0.1, 0.1);
                self.act(Action::SetVolume((v.level + step).clamp(0.0, 1.0)));
            }
        }
        let tip = if v.muted {
            "Muted. Click to unmute."
        } else {
            "Scroll to change the volume, click to mute"
        };
        if resp.on_hover_text(tip).clicked() {
            self.act(Action::SetMuted(!v.muted));
        }
    }

    fn media(&mut self, ui: &mut Ui) {
        let Some(m) = self.readings.media.clone() else {
            let (_, _, resp) = self.card(ui, "Media", "Nothing playing", 0.0, Sense::hover());
            resp.on_hover_text("Play something in a music app or a browser to control it here");
            return;
        };
        let title = truncate(&m.title, 26);
        let artist = truncate(
            if m.artist.is_empty() {
                "Playing"
            } else {
                &m.artist
            },
            30,
        );
        let button = self.height * 0.56;
        let (cell, extra, resp) =
            self.card(ui, &artist, &title, button * 3.0 + 6.0, Sense::hover());
        resp.on_hover_text(format!("{}\n{}", m.title, m.artist));
        let glyphs = [
            (MediaGlyph::Previous, MediaAction::Previous, "Previous"),
            (
                if m.playing {
                    MediaGlyph::Pause
                } else {
                    MediaGlyph::Play
                },
                MediaAction::PlayPause,
                if m.playing { "Pause" } else { "Play" },
            ),
            (MediaGlyph::Next, MediaAction::Next, "Next"),
        ];
        for (i, (glyph, action, tip)) in glyphs.into_iter().enumerate() {
            let r = Rect::from_center_size(
                pos2(extra.left() + button * (i as f32 + 0.5), cell.center().y),
                vec2(button, button),
            );
            let id = ui.scope_id().with(("media-button", i));
            let b = ui.interact(r, id, Sense::click());
            if b.hovered() {
                ui.painter().circle_filled(
                    r.center(),
                    button * 0.48,
                    draw::color(self.theme.hover),
                );
            }
            draw::media_glyph(ui.painter(), r, glyph, self.ink());
            if b.on_hover_text(tip).clicked() {
                self.act(Action::Media(action));
            }
        }
    }

    fn microphone(&mut self, ui: &mut Ui) {
        let (cell, resp) = self.icon_cell(ui, Sense::click());
        match self.readings.microphone {
            Some(mic) => {
                let ink = if mic.muted { self.warn() } else { self.ink() };
                draw::microphone(ui.painter(), cell, mic.muted, ink);
                let tip = if mic.muted {
                    "Microphone muted. Click to unmute."
                } else {
                    "Microphone on. Click to mute."
                };
                if resp.on_hover_text(tip).clicked() {
                    self.act(Action::SetMicrophoneMuted(!mic.muted));
                }
            }
            None => {
                draw::microphone(ui.painter(), cell, false, self.muted());
                resp.on_hover_text("No microphone found");
            }
        }
    }

    fn focus_timer(&mut self, ui: &mut Ui) {
        let phase = self.focus.phase(self.instant);
        let remaining = self.focus.remaining(self.instant);
        let label = match phase {
            FocusPhase::Idle => "Focus",
            FocusPhase::Running => "Focusing",
            FocusPhase::Paused => "Paused",
            FocusPhase::Finished => "Time's up",
        };
        let (cell, _, resp) = self.card_steady(
            ui,
            label,
            &format_countdown(remaining),
            "00:00",
            0.0,
            Sense::click(),
        );
        let total = self.focus.length().as_secs_f32().max(1.0);
        let done = 1.0 - remaining.as_secs_f32() / total;
        self.bottom_bar(ui, cell, done, self.accent());
        if phase == FocusPhase::Finished {
            ui.painter().rect_filled(
                cell,
                cell_radius(&self.theme),
                self.accent().gamma_multiply(0.22),
            );
        }
        let tip = match phase {
            FocusPhase::Idle => "Focus timer. Click to start, right-click to reset.",
            FocusPhase::Running => "Click to pause, right-click to reset",
            FocusPhase::Paused => "Paused. Click to resume, right-click to reset.",
            FocusPhase::Finished => "Time's up. Click to start again.",
        };
        let resp = resp.on_hover_text(tip);
        if self.preview {
            return;
        }
        if resp.clicked() {
            self.focus.toggle(self.instant);
        }
        if resp.secondary_clicked() {
            self.focus.reset();
        }
    }

    fn stopwatch(&mut self, ui: &mut Ui) {
        let elapsed = self.stopwatch.elapsed(self.instant);
        let running = self.stopwatch.is_running();
        let (_, _, resp) = self.card_steady(
            ui,
            "Stopwatch",
            &format_stopwatch(elapsed),
            "88:88.8",
            0.0,
            Sense::click(),
        );
        if running {
            self.fast_repaint = true;
        }
        let resp = resp.on_hover_text("Click to start or pause, right-click to reset");
        if self.preview {
            return;
        }
        if resp.clicked() {
            self.stopwatch.toggle(self.instant);
        }
        if resp.secondary_clicked() {
            self.stopwatch.reset();
        }
    }

    fn time_on_pc(&mut self, ui: &mut Ui) {
        let value = format_duration(self.readings.usage_total);
        let (_, _, resp) =
            self.card_steady(ui, "Today on PC", &value, "18h 88m", 0.0, Sense::click());
        let resp =
            resp.on_hover_text("Time in front of the PC today. Click for the apps you used most.");
        if self.preview {
            return;
        }
        let top = self.readings.usage_top.clone();
        let total = self.readings.usage_total.max(1);
        let theme = self.theme;
        Popup::from_toggle_button_response(&resp)
            .align(self.popup_align())
            .gap(8.0)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(260.0);
                ui.label(RichText::new(format!("Today: {}", format_duration(total))).strong());
                ui.add_space(4.0);
                if top.is_empty() {
                    ui.label(RichText::new("Nothing yet today.").weak());
                }
                for (app, secs) in &top {
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            vec2(130.0, 18.0),
                            egui::Label::new(truncate(app, 18)).truncate(),
                        );
                        let (r, _) = ui.allocate_exact_size(vec2(70.0, 18.0), Sense::hover());
                        draw::bar(
                            ui.painter(),
                            Rect::from_center_size(r.center(), vec2(r.width(), 5.0)),
                            *secs as f32 / total as f32,
                            draw::color(theme.hover),
                            draw::color(theme.accent),
                        );
                        ui.label(RichText::new(format_duration(*secs)).weak());
                    });
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Kept on this PC only. Clear it in Settings → Privacy.")
                        .small()
                        .weak(),
                );
            });
    }

    fn weather(&mut self, ui: &mut Ui) {
        let unit = match self.settings.weather.unit {
            TempUnit::Celsius => "C",
            TempUnit::Fahrenheit => "F",
        };
        let place = self
            .settings
            .weather
            .resolved_name
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .to_owned();
        let (label, value, sky, day) = match &self.readings.weather {
            WeatherState::Ready { current, .. } => (
                if place.is_empty() {
                    "Weather".to_owned()
                } else {
                    truncate(&place, 16)
                },
                format!("{:.0}°", current.temperature),
                current.sky,
                current.is_day,
            ),
            WeatherState::Loading => ("Weather".to_owned(), "…".to_owned(), Sky::Unknown, true),
            WeatherState::Failed(_) => ("Weather".to_owned(), "?".to_owned(), Sky::Unknown, true),
            WeatherState::Off => (
                "Weather".to_owned(),
                "Set place".to_owned(),
                Sky::PartlyCloudy,
                true,
            ),
        };
        let icon = self.height * 0.72;
        let (_, extra, resp) = self.card_steady(ui, &label, &value, "-88°", icon, Sense::click());
        draw::sky(
            ui.painter(),
            extra.translate(vec2(-6.0, 0.0)),
            sky,
            day,
            self.ink(),
            self.accent(),
        );
        let tip = match &self.readings.weather {
            WeatherState::Ready { current, .. } => {
                format!(
                    "{}, {:.1} °{unit}. Click for the next days.",
                    current.sky.label(),
                    current.temperature
                )
            }
            WeatherState::Failed(why) => format!("{why}. Click to check the place in Settings."),
            WeatherState::Loading => "Loading the weather".to_owned(),
            WeatherState::Off => "Choose a place in Settings".to_owned(),
        };
        let resp = resp.on_hover_text(tip);
        if self.preview {
            return;
        }
        let WeatherState::Ready { current, .. } = &self.readings.weather else {
            if resp.clicked() {
                self.act(Action::OpenSettings);
            }
            return;
        };
        let current = current.clone();
        let theme = self.theme;
        let place_name = self.settings.weather.resolved_name.clone();
        Popup::from_toggle_button_response(&resp)
            .align(self.popup_align())
            .gap(8.0)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(250.0);
                ui.label(RichText::new(&place_name).strong());
                ui.label(format!(
                    "{}, {:.0}°{unit}",
                    current.sky.label(),
                    current.temperature
                ));
                if let Some(today) = current.days.first() {
                    ui.label(
                        RichText::new(format!("Today {:.0}° / {:.0}°", today.max, today.min))
                            .weak(),
                    );
                }
                if let (Some(rise), Some(set)) = (&current.sunrise, &current.sunset) {
                    ui.label(RichText::new(format!("Sunrise {rise} · Sunset {set}")).weak());
                }
                ui.add_space(6.0);
                for day in current.days.iter().skip(1) {
                    ui.horizontal(|ui| {
                        let name = NaiveDate::parse_from_str(&day.date, "%Y-%m-%d")
                            .map(|d| d.format("%a %-d").to_string())
                            .unwrap_or_else(|_| day.date.clone());
                        ui.add_sized(vec2(70.0, 22.0), egui::Label::new(name));
                        let (r, _) = ui.allocate_exact_size(vec2(26.0, 22.0), Sense::hover());
                        draw::sky(
                            ui.painter(),
                            r,
                            day.sky,
                            true,
                            draw::color(theme.text),
                            draw::color(theme.accent),
                        );
                        ui.label(format!("{:.0}° / {:.0}°", day.max, day.min));
                    });
                }
                ui.add_space(4.0);
                ui.label(
                    RichText::new(barcraft_core::weather::ATTRIBUTION)
                        .small()
                        .weak(),
                );
            });
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
                                fonts::font(Weight::Regular, 13.0),
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
