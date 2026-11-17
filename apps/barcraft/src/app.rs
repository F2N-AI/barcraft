//! The dock window: where it sits, what it draws, and what clicks on it do.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use barcraft_core::apps::{DockItem, PinnedApp, WindowRef, build_dock_items, normalize_path};
use barcraft_core::focus::FocusTimer;
use barcraft_core::{DockEdge, DockSize, IndicatorStyle, Settings, Theme, ThemeId, WidgetKind};
use barcraft_win32::{AppBar, Edge, PxRect, WindowId};
use egui::{
    Align2, Color32, CornerRadius, Id, Margin, Popup, PopupCloseBehavior, Pos2, Rect, RectAlign,
    Sense, Shadow, Stroke, Ui, vec2,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::poller::{Command, DockWatch, Poller};
use crate::settings_window::{self, Changes, SettingsUi};
use crate::widgets::{self, Readings, WidgetUi};
use crate::{APP_NAME, draw, save_settings, taskbar};

/// Gap between the bar and the screen edge, in points.
const MARGIN: f32 = 6.0;
/// Room above (or below) the bar for menus, tooltips and the calendar, in points.
const POPUP_SPACE: f32 = 460.0;
/// How often to check whether the screen or its scale changed.
const RECHECK: Duration = Duration::from_secs(2);

pub fn run() -> eframe::Result {
    let settings = crate::load_settings();
    let viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_app_id("barcraft")
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_taskbar(false)
        .with_always_on_top()
        .with_inner_size([640.0, 120.0]);
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(BarApp::new(cc, settings)))),
    )
}

enum IconTex {
    Pending,
    Missing,
    Ready(egui::TextureHandle),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Placement {
    monitor: PxRect,
    edge: DockEdge,
    size: DockSize,
    scale: f32,
}

/// Something a click on an app asked for, applied after drawing.
enum AppAction {
    Launch(PathBuf),
    Activate(WindowId),
    Minimize(WindowId),
    Close(WindowId),
    EndTask(u32),
    Pin(PinnedApp),
    Unpin(PathBuf),
    MovePin(PathBuf, isize),
}

struct BarApp {
    settings: Settings,
    poller: Poller,
    window: Option<WindowId>,
    appbar: Option<AppBar>,
    placement: Option<Placement>,
    last_check: Option<Instant>,
    region: Option<Option<PxRect>>,
    icons: HashMap<String, IconTex>,
    seen_order: Vec<String>,
    focus: FocusTimer,
    calendar: Option<(i32, u32)>,
    settings_open: bool,
    settings_ui: SettingsUi,
    styled_for: Option<ThemeId>,
}

fn own_window(cc: &eframe::CreationContext<'_>) -> Option<WindowId> {
    let handle = cc.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(WindowId(h.hwnd.get())),
        _ => None,
    }
}

fn import_taskbar_pins(settings: &mut Settings) {
    for pin in barcraft_win32::taskbar_pins() {
        let mut app = PinnedApp::new(pin.link, pin.name);
        app.target = pin.target;
        settings.pin(app);
    }
    settings.pins_imported = true;
}

fn to_px(r: Rect, scale: f32) -> PxRect {
    PxRect {
        left: (r.left() * scale).floor() as i32,
        top: (r.top() * scale).floor() as i32,
        right: (r.right() * scale).ceil() as i32,
        bottom: (r.bottom() * scale).ceil() as i32,
    }
}

impl BarApp {
    fn new(cc: &eframe::CreationContext<'_>, mut settings: Settings) -> Self {
        let ctx = cc.egui_ctx.clone();
        let window = own_window(cc);
        if let Some(w) = window {
            barcraft_win32::style_dock_window(w);
        }
        let appbar = window.and_then(AppBar::register);
        if appbar.is_none() {
            log::warn!("could not reserve screen space for the dock; windows may cover it");
        }
        if !settings.pins_imported {
            import_taskbar_pins(&mut settings);
            save_settings(&settings);
        }
        settings.launch_at_startup = barcraft_win32::launch_at_startup(APP_NAME);
        taskbar::apply(&mut settings);
        taskbar::spawn_watchdog();

        let poller = Poller::start(ctx.clone());
        let focus = FocusTimer::new(Duration::from_secs(u64::from(settings.focus_minutes) * 60));
        let mut app = BarApp {
            settings,
            poller,
            window,
            appbar,
            placement: None,
            last_check: None,
            region: None,
            icons: HashMap::new(),
            seen_order: Vec::new(),
            focus,
            calendar: None,
            settings_open: false,
            settings_ui: SettingsUi::default(),
            styled_for: None,
        };
        app.send_weather();
        // Place the window before its first frame so it never shows in the wrong spot.
        let scale = window.map(barcraft_win32::dpi_scale).unwrap_or(1.0);
        app.place(scale, true);
        ctx.request_repaint();
        app
    }

    fn send_weather(&self) {
        let w = if self.settings.has_widget(WidgetKind::Weather) {
            self.settings.weather.clone()
        } else {
            Default::default()
        };
        self.poller.send(Command::Weather(w));
    }

    fn watch(&self) -> DockWatch {
        DockWatch {
            window: self.window,
            monitor: self.placement.map(|p| p.monitor),
            hide_for_fullscreen: self.settings.hide_for_fullscreen,
            taskbar_mode: self.settings.taskbar_mode,
        }
    }

    /// Reserves the screen strip and moves the window over it, when anything changed.
    fn place(&mut self, scale: f32, force: bool) {
        let Some(win) = self.window else {
            return;
        };
        let now = Instant::now();
        if !force
            && self
                .last_check
                .is_some_and(|t| now.duration_since(t) < RECHECK)
        {
            return;
        }
        self.last_check = Some(now);
        let Some(monitor) = barcraft_win32::primary_monitor() else {
            return;
        };
        let scale = if scale.is_finite() && scale > 0.25 {
            scale
        } else {
            1.0
        };
        let want = Placement {
            monitor,
            edge: self.settings.edge,
            size: self.settings.size,
            scale,
        };
        if !force && self.placement == Some(want) {
            return;
        }
        let reserve = ((self.settings.size.bar_height() + 2.0 * MARGIN) * scale).round() as i32;
        let popup = (POPUP_SPACE * scale).round() as i32;
        let edge = match self.settings.edge {
            DockEdge::Top => Edge::Top,
            DockEdge::Bottom => Edge::Bottom,
        };
        let strip = match &self.appbar {
            Some(bar) => bar.reserve(edge, monitor, reserve),
            None => match edge {
                Edge::Top => PxRect {
                    bottom: monitor.top + reserve,
                    ..monitor
                },
                Edge::Bottom => PxRect {
                    top: monitor.bottom - reserve,
                    ..monitor
                },
            },
        };
        let rect = match edge {
            Edge::Top => PxRect {
                top: strip.top,
                bottom: strip.top.saturating_add(reserve + popup),
                ..strip
            },
            Edge::Bottom => PxRect {
                top: strip.bottom.saturating_sub(reserve + popup),
                bottom: strip.bottom,
                ..strip
            },
        };
        barcraft_win32::place_window(win, rect);
        self.placement = Some(want);
        self.region = None;
        self.poller.send(Command::Watch(self.watch()));
    }

    fn apply_style(&mut self, ctx: &egui::Context, theme: &Theme) {
        if self.styled_for == Some(self.settings.theme) {
            return;
        }
        self.styled_for = Some(self.settings.theme);
        let mut v = if theme.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.window_fill = draw::color(theme.popup);
        v.panel_fill = draw::color(theme.popup);
        v.override_text_color = Some(draw::color(theme.text));
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(10);
        v.selection.bg_fill = draw::color(theme.accent).gamma_multiply(0.6);
        v.hyperlink_color = draw::color(theme.accent);
        ctx.set_visuals(v);
    }

    fn icon(&mut self, ctx: &egui::Context, item: &DockItem) -> Option<egui::TextureId> {
        let key = match &item.icon_path {
            Some(p) => normalize_path(p),
            None => item.key.clone(),
        };
        match self.icons.get(&key) {
            Some(IconTex::Ready(t)) => Some(t.id()),
            Some(IconTex::Missing) | Some(IconTex::Pending) => None,
            None => {
                self.icons.insert(key.clone(), IconTex::Pending);
                self.poller.send(Command::LoadIcon {
                    key,
                    file: item.icon_path.clone(),
                    window: item.windows.first().map(|w| WindowId(w.id)),
                });
                let _ = ctx;
                None
            }
        }
    }

    fn take_updates(&mut self, ctx: &egui::Context) {
        let (icons, place) = {
            let mut snap = self.poller.snapshot();
            (std::mem::take(&mut snap.icons), snap.resolved_place.take())
        };
        for (key, icon) in icons {
            let tex = match icon {
                Some(i) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [i.width as usize, i.height as usize],
                        &i.rgba,
                    );
                    IconTex::Ready(ctx.load_texture(
                        format!("icon:{key}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    ))
                }
                None => IconTex::Missing,
            };
            self.icons.insert(key, tex);
        }
        if let Some(p) = place {
            self.settings.weather.latitude = Some(p.latitude);
            self.settings.weather.longitude = Some(p.longitude);
            self.settings.weather.resolved_name = p.name;
            save_settings(&self.settings);
        }
    }

    fn quit(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// The dock's own menu: settings, edge, quit.
    fn dock_menu(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        if ui.button("Settings and widgets…").clicked() {
            self.settings_open = true;
        }
        let (label, edge) = match self.settings.edge {
            DockEdge::Bottom => ("Move the dock to the top", DockEdge::Top),
            DockEdge::Top => ("Move the dock to the bottom", DockEdge::Bottom),
        };
        if ui.button(label).clicked() {
            self.settings.edge = edge;
            save_settings(&self.settings);
            self.place(ctx.pixels_per_point(), true);
        }
        ui.separator();
        if ui.button(format!("Quit {APP_NAME}")).clicked() {
            self.quit(ctx);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn app_button(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        item: &DockItem,
        foreground: Option<WindowId>,
        pids: &HashMap<isize, u32>,
        theme: &Theme,
        actions: &mut Vec<AppAction>,
    ) {
        let size = self.settings.size;
        let bar_h = size.bar_height();
        let icon_side = size.icon_size();
        let (rect, resp) = ui.allocate_exact_size(vec2(icon_side + 14.0, bar_h), Sense::click());
        let is_front = item
            .windows
            .iter()
            .any(|w| Some(WindowId(w.id)) == foreground);
        if resp.hovered() || is_front {
            let fill = if resp.hovered() {
                theme.hover
            } else {
                theme.tile
            };
            ui.painter()
                .rect_filled(rect.shrink2(vec2(1.0, 4.0)), 9.0, draw::color(fill));
        }
        let icon_rect = Rect::from_center_size(rect.center(), vec2(icon_side, icon_side));
        let press = if resp.is_pointer_button_down_on() {
            0.92
        } else {
            1.0
        };
        let icon_rect = Rect::from_center_size(icon_rect.center(), icon_rect.size() * press);
        match self.icon(ctx, item) {
            Some(tex) => {
                ui.painter().image(
                    tex,
                    icon_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            None => draw::letter_tile(
                ui.painter(),
                icon_rect,
                &item.name,
                draw::color(theme.text),
                draw::color(theme.hover),
            ),
        }
        if item.is_running() {
            let ink = draw::color(if is_front { theme.accent } else { theme.muted });
            let y = match self.settings.edge {
                DockEdge::Bottom => rect.bottom() - 4.0,
                DockEdge::Top => rect.top() + 4.0,
            };
            let x = rect.center().x;
            match self.settings.indicator {
                IndicatorStyle::Dot => {
                    ui.painter().circle_filled(Pos2::new(x, y), 2.0, ink);
                }
                IndicatorStyle::LargeDot => {
                    ui.painter().circle_filled(Pos2::new(x, y), 3.0, ink);
                }
                IndicatorStyle::Line => {
                    let w = if is_front { 18.0 } else { 10.0 };
                    ui.painter().rect_filled(
                        Rect::from_center_size(Pos2::new(x, y), vec2(w, 3.0)),
                        1.5,
                        ink,
                    );
                }
            }
            if item.windows.len() > 1 {
                let badge = Pos2::new(rect.right() - 7.0, rect.top() + 9.0);
                ui.painter()
                    .circle_filled(badge, 6.5, draw::color(theme.bar));
                ui.painter().text(
                    badge,
                    Align2::CENTER_CENTER,
                    item.windows.len().min(9).to_string(),
                    egui::FontId::proportional(9.5),
                    draw::color(theme.text),
                );
            }
        }

        let tip = match item.windows.as_slice() {
            [only] => format!("{}\n{}", item.name, only.title),
            _ => item.name.clone(),
        };
        let resp = resp.on_hover_text(tip);

        let align = match self.settings.edge {
            DockEdge::Bottom => RectAlign::TOP,
            DockEdge::Top => RectAlign::BOTTOM,
        };
        let list_id = Id::unique(("barcraft-window-list", &item.key));
        if resp.clicked() {
            match item.windows.as_slice() {
                [] => {
                    if let Some(p) = &item.launch_path {
                        actions.push(AppAction::Launch(p.clone()));
                    }
                }
                [only] => {
                    let id = WindowId(only.id);
                    if foreground == Some(id) && !only.minimized {
                        actions.push(AppAction::Minimize(id));
                    } else {
                        actions.push(AppAction::Activate(id));
                    }
                }
                _ => Popup::toggle_id(ctx, list_id),
            }
        }
        if resp.middle_clicked()
            && let Some(p) = &item.launch_path
        {
            actions.push(AppAction::Launch(p.clone()));
        }

        Popup::from_response(&resp)
            .id(list_id)
            .open_memory(None)
            .align(align)
            .gap(8.0)
            .close_behavior(PopupCloseBehavior::CloseOnClick)
            .show(|ui| {
                ui.set_max_width(320.0);
                for w in &item.windows {
                    if ui.button(window_title(w)).clicked() {
                        actions.push(AppAction::Activate(WindowId(w.id)));
                    }
                }
            });

        // The app's menu opens above (or below) its icon, not under the pointer, so it never
        // covers the dock. A left click on the icon closes it.
        let menu_cmd = if resp.secondary_clicked() {
            Some(egui::SetOpenCommand::Bool(true))
        } else if resp.clicked() {
            Some(egui::SetOpenCommand::Bool(false))
        } else {
            None
        };
        Popup::menu(&resp)
            .id(Id::unique(("barcraft-app-menu", &item.key)))
            .open_memory(menu_cmd)
            .align(align)
            .gap(8.0)
            .show(|ui| {
                ui.set_min_width(200.0);
                ui.label(egui::RichText::new(&item.name).strong());
                if item.windows.len() > 1 {
                    for w in &item.windows {
                        if ui.button(window_title(w)).clicked() {
                            actions.push(AppAction::Activate(WindowId(w.id)));
                        }
                    }
                }
                ui.separator();
                if let Some(p) = &item.launch_path
                    && ui
                        .button(if item.is_running() {
                            "Open a new window"
                        } else {
                            "Open"
                        })
                        .clicked()
                {
                    actions.push(AppAction::Launch(p.clone()));
                }
                match &item.pinned {
                    Some(pin) => {
                        if ui.button("Unpin from the dock").clicked() {
                            actions.push(AppAction::Unpin(pin.path.clone()));
                        }
                        ui.horizontal(|ui| {
                            if ui.button("‹ Move left").clicked() {
                                actions.push(AppAction::MovePin(pin.path.clone(), -1));
                            }
                            if ui.button("Move right ›").clicked() {
                                actions.push(AppAction::MovePin(pin.path.clone(), 1));
                            }
                        });
                    }
                    None => {
                        if let Some(exe) = &item.launch_path
                            && ui.button("Pin to the dock").clicked()
                        {
                            actions.push(AppAction::Pin(PinnedApp::new(
                                exe.clone(),
                                item.name.clone(),
                            )));
                        }
                    }
                }
                if item.is_running() {
                    ui.separator();
                    let label = if item.windows.len() > 1 {
                        "Close all windows"
                    } else {
                        "Close window"
                    };
                    if ui.button(label).clicked() {
                        for w in &item.windows {
                            actions.push(AppAction::Close(WindowId(w.id)));
                        }
                    }
                    if ui
                        .button("End task")
                        .on_hover_text("Stops the program at once; unsaved work is lost")
                        .clicked()
                    {
                        let mut ended: Vec<u32> = Vec::new();
                        for w in &item.windows {
                            if let Some(&pid) = pids.get(&w.id)
                                && !ended.contains(&pid)
                            {
                                ended.push(pid);
                                actions.push(AppAction::EndTask(pid));
                            }
                        }
                    }
                }
            });
    }

    fn apply_app_actions(&mut self, actions: Vec<AppAction>) {
        let mut pins_changed = false;
        for a in actions {
            match a {
                AppAction::Launch(p) => {
                    if !barcraft_win32::launch(&p) {
                        log::warn!("could not open {}", p.display());
                    }
                }
                AppAction::Activate(w) => barcraft_win32::activate_window(w),
                AppAction::Minimize(w) => barcraft_win32::minimize_window(w),
                AppAction::Close(w) => barcraft_win32::close_window(w),
                AppAction::EndTask(pid) => {
                    barcraft_win32::end_process(pid);
                }
                AppAction::Pin(app) => {
                    self.settings.pin(app);
                    pins_changed = true;
                }
                AppAction::Unpin(p) => {
                    self.settings.unpin(&p);
                    pins_changed = true;
                }
                AppAction::MovePin(p, d) => {
                    self.settings.move_pin(&p, d);
                    pins_changed = true;
                }
            }
        }
        if pins_changed {
            save_settings(&self.settings);
        }
    }

    fn apply_settings_changes(&mut self, ctx: &egui::Context, changes: Changes) {
        if !changes.any {
            return;
        }
        if changes.import_pins {
            import_taskbar_pins(&mut self.settings);
        }
        if let Some(on) = changes.autostart {
            let command = std::env::current_exe()
                .ok()
                .map(|p| format!("\"{}\"", p.display()));
            let ok = barcraft_win32::set_launch_at_startup(
                APP_NAME,
                if on { command.as_deref() } else { None },
            );
            if !ok {
                log::warn!("could not change the start-with-Windows entry");
            }
            self.settings.launch_at_startup = barcraft_win32::launch_at_startup(APP_NAME);
        }
        if changes.taskbar {
            taskbar::apply(&mut self.settings);
        }
        if changes.weather {
            self.send_weather();
        }
        if changes.focus {
            self.focus.set_length(Duration::from_secs(
                u64::from(self.settings.focus_minutes) * 60,
            ));
        }
        save_settings(&self.settings);
        self.place(ctx.pixels_per_point(), changes.placement);
        self.poller.send(Command::Watch(self.watch()));
    }
}

fn window_title(w: &WindowRef) -> String {
    let mut t: String = w.title.chars().take(60).collect();
    if w.title.chars().count() > 60 {
        t.push('…');
    }
    if w.minimized {
        t.push_str("  (minimised)");
    }
    t
}

impl eframe::App for BarApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.place(ctx.pixels_per_point(), false);
        self.take_updates(&ctx);
        let theme = self.settings.theme.theme();
        self.apply_style(&ctx, &theme);

        let (windows, pids, foreground, readings) = {
            let snap = self.poller.snapshot();
            let windows: Vec<WindowRef> = snap
                .windows
                .iter()
                .map(|w| WindowRef {
                    id: w.id.0,
                    title: w.title.clone(),
                    exe: w.exe.clone(),
                    minimized: w.minimized,
                })
                .collect();
            let pids: HashMap<isize, u32> = snap.windows.iter().map(|w| (w.id.0, w.pid)).collect();
            let readings = Readings {
                cpu: snap.cpu.clone(),
                memory: snap.memory,
                power: snap.power,
                volume: snap.volume,
                weather: snap.weather.clone(),
            };
            (windows, pids, snap.foreground, readings)
        };
        let items = build_dock_items(&self.settings.pinned, &windows, &mut self.seen_order);

        let size = self.settings.size;
        let bar_h = size.bar_height();
        let (anchor, offset) = match self.settings.edge {
            DockEdge::Bottom => (Align2::CENTER_BOTTOM, vec2(0.0, -MARGIN)),
            DockEdge::Top => (Align2::CENTER_TOP, vec2(0.0, MARGIN)),
        };
        let now = chrono::Local::now();
        let instant = Instant::now();
        let mut app_actions = Vec::new();
        let mut widget_actions = Vec::new();
        let mut open_menu_from_start = false;

        let bar = egui::Area::new(Id::unique("barcraft-bar"))
            .anchor(anchor, offset)
            .movable(false)
            .sense(Sense::click_and_drag())
            .show(&ctx, |ui| {
                egui::Frame::new()
                    .fill(draw::color(theme.bar))
                    .stroke(Stroke::new(1.0, draw::color(theme.bar_edge)))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::symmetric(6, 0))
                    .shadow(Shadow {
                        offset: [0, 2],
                        blur: 10,
                        spread: 0,
                        color: Color32::from_black_alpha(70),
                    })
                    .show(ui, |ui| {
                        ui.set_height(bar_h);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                            // Start menu.
                            let (rect, resp) = ui.allocate_exact_size(
                                vec2(size.icon_size() + 12.0, bar_h),
                                Sense::click(),
                            );
                            if resp.hovered() {
                                ui.painter().rect_filled(
                                    rect.shrink2(vec2(1.0, 4.0)),
                                    9.0,
                                    draw::color(theme.hover),
                                );
                            }
                            draw::launcher(ui.painter(), rect, draw::color(theme.text));
                            let resp = resp.on_hover_text("Start");
                            if resp.clicked() {
                                barcraft_win32::open_start_menu();
                            }
                            if resp.secondary_clicked() {
                                open_menu_from_start = true;
                            }

                            for item in &items {
                                self.app_button(
                                    ui,
                                    &ctx,
                                    item,
                                    foreground,
                                    &pids,
                                    &theme,
                                    &mut app_actions,
                                );
                            }

                            if !self.settings.widgets.is_empty() {
                                let (rect, _) =
                                    ui.allocate_exact_size(vec2(9.0, bar_h), Sense::hover());
                                ui.painter().line_segment(
                                    [
                                        Pos2::new(rect.center().x, rect.top() + bar_h * 0.25),
                                        Pos2::new(rect.center().x, rect.bottom() - bar_h * 0.25),
                                    ],
                                    Stroke::new(1.0, draw::color(theme.bar_edge)),
                                );
                                let widget_list = self.settings.widgets.clone();
                                let mut wui = WidgetUi {
                                    theme,
                                    settings: &self.settings,
                                    readings: &readings,
                                    focus: &mut self.focus,
                                    calendar: &mut self.calendar,
                                    now,
                                    instant,
                                    height: bar_h,
                                    actions: Vec::new(),
                                };
                                for kind in widget_list {
                                    wui.show(ui, kind);
                                }
                                widget_actions = wui.actions;
                            }
                        });
                    });
            });

        // The dock's own menu, from a right-click on the bar or on the Start button.
        let menu_id = Id::unique("barcraft-dock-menu");
        if bar.response.secondary_clicked() || open_menu_from_start {
            Popup::open_id(&ctx, menu_id);
        }
        let menu_align = match self.settings.edge {
            DockEdge::Bottom => RectAlign::TOP_START,
            DockEdge::Top => RectAlign::BOTTOM_START,
        };
        Popup::menu(&bar.response)
            .id(menu_id)
            .open_memory(None)
            .align(menu_align)
            .gap(6.0)
            .show(|ui| self.dock_menu(ui, &ctx));

        // Drag the bar to the other half of the screen to move it to that edge.
        if bar.response.drag_stopped()
            && let (Some((_, y)), Some(p)) = (barcraft_win32::cursor_pos(), self.placement)
        {
            let middle = p.monitor.top + p.monitor.height() / 2;
            let edge = if y < middle {
                DockEdge::Top
            } else {
                DockEdge::Bottom
            };
            if edge != self.settings.edge {
                self.settings.edge = edge;
                save_settings(&self.settings);
                self.place(ctx.pixels_per_point(), true);
            }
        }

        self.apply_app_actions(app_actions);
        for a in widget_actions {
            match a {
                widgets::Action::SetVolume(v) => self.poller.send(Command::SetVolume(v)),
                widgets::Action::SetMuted(m) => self.poller.send(Command::SetMuted(m)),
                widgets::Action::ToggleDesktop => barcraft_win32::toggle_desktop(),
                widgets::Action::OpenSettings => self.settings_open = true,
            }
        }

        if self.settings_open {
            let weather_state = self.poller.snapshot().weather.clone();
            let mut open = true;
            let changes = settings_window::show(
                &ctx,
                &mut self.settings,
                &mut self.settings_ui,
                &weather_state,
                &mut open,
            );
            if !open {
                self.settings_open = false;
            }
            if changes.quit {
                self.quit(&ctx);
            }
            self.apply_settings_changes(&ctx, changes);
            if changes.theme {
                self.styled_for = None;
            }
        }

        // Only the bar takes clicks, unless a menu, tooltip or drag needs the rest of the window.
        if let Some(win) = self.window {
            let scale = ctx.pixels_per_point();
            let bar_rect = bar.response.rect;
            let hovering = ctx
                .pointer_hover_pos()
                .is_some_and(|p| bar_rect.expand(4.0).contains(p));
            let full = hovering
                || ctx.any_popup_open()
                || Popup::is_any_open(&ctx)
                || bar.response.dragged();
            let region = if full {
                None
            } else {
                Some(to_px(bar_rect.expand(8.0), scale))
            };
            if self.region != Some(region) {
                barcraft_win32::set_window_region(win, region);
                self.region = Some(region);
                ctx.request_repaint();
            }
        }

        let ms = 1000u64
            .saturating_sub(u64::from(now.timestamp_subsec_millis()))
            .max(20);
        ctx.request_repaint_after(Duration::from_millis(ms));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.appbar = None;
        taskbar::restore(&mut self.settings);
        save_settings(&self.settings);
    }
}
