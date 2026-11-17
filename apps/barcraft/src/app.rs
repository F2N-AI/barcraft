//! The dock window: where it sits, what it draws, and what clicks on it do.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use barcraft_core::apps::{DockItem, PinnedApp, WindowRef, build_dock_items, normalize_path};
use barcraft_core::clock::Stopwatch;
use barcraft_core::focus::FocusTimer;
use barcraft_core::{DockEdge, DockLayout, DockSize, IndicatorStyle, Settings, Theme, WidgetKind};
use barcraft_win32::{AppBar, Backdrop, Edge, PxRect, WindowId};
use egui::{
    Align2, Color32, CornerRadius, Id, Margin, Popup, PopupCloseBehavior, Pos2, Rect, RectAlign,
    Sense, Shadow, Shape, Stroke, Ui, vec2,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::poller::{Command, DockWatch, Poller};
use crate::settings_window::{self, Changes, SettingsUi};
use crate::widgets::{self, CELL_INSET, Readings, WidgetUi, cell_radius};
use crate::{APP_NAME, draw, save_settings, taskbar};

/// Gap between the bar and the screen edge, in points.
const MARGIN: f32 = 6.0;
/// Inner margin of the bar, left and right, in points.
const BAR_PAD: f32 = 6.0;
/// Corner radius of the bar (glass themes use the radius Windows rounds the blur with).
const BAR_RADIUS: u8 = 16;

/// The bar's corner radius for `theme`: on glass it must match the corners Windows gives the
/// blurred backdrop.
fn bar_radius(theme: &Theme) -> u8 {
    if theme.glass {
        Backdrop::CORNER_RADIUS_POINTS.round() as u8
    } else {
        BAR_RADIUS
    }
}
/// Room above (or below) the bar for menus, tooltips and the calendar, in points.
const POPUP_SPACE: f32 = 460.0;
/// How often to check whether the screen or its scale changed.
const RECHECK: Duration = Duration::from_secs(2);

/// How the app was started.
#[derive(Clone, Copy, Debug, Default)]
pub struct Launch {
    /// Open the settings window straight away.
    pub open_settings: bool,
    /// Draw the bar in an ordinary window instead of docking it: no reserved screen space, no
    /// taskbar changes, no watchdog. For looking at the design next to a running dock.
    pub preview: bool,
}

/// The title of the preview window, so tools can find it.
pub const PREVIEW_TITLE: &str = "BarCraft preview";

pub fn run(launch: Launch) -> eframe::Result {
    let settings = crate::load_settings();
    let viewport = if launch.preview {
        egui::ViewportBuilder::default()
            .with_title(PREVIEW_TITLE)
            .with_app_id("barcraft-preview")
            .with_inner_size([1760.0, 220.0])
            .with_min_inner_size([600.0, 160.0])
    } else {
        egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id("barcraft")
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_taskbar(false)
            .with_always_on_top()
            .with_inner_size([640.0, 120.0])
    };
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(BarApp::new(cc, settings, launch)))),
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
    /// Windows may use the space under the dock (nothing is reserved).
    float: bool,
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
    stopwatch: Stopwatch,
    calendar: Option<(i32, u32)>,
    settings_open: bool,
    settings_ui: SettingsUi,
    styled_for: Option<Theme>,
    /// Whether Windows is in light mode, as the poller last read it.
    windows_light: bool,
    /// Widths measured on the last frame, used to fit the app area and the full-width layout.
    widgets_width: f32,
    apps_scroll: f32,
    widgets_scroll: f32,
    widgets_rect: Rect,
    apps_rect: Rect,
    /// Drawing in an ordinary window, for looking at the design (see [`Launch::preview`]).
    preview: bool,
    /// The blurred backdrop under the bar, for glass themes; made when first needed.
    backdrop: Option<Backdrop>,
    /// Where the backdrop was last put, and when its stacking order was last checked.
    backdrop_at: Option<(PxRect, Instant)>,
    /// The dock window's rectangle on screen, in pixels.
    window_px: Option<PxRect>,
    /// Set once making the backdrop failed, so it isn't retried every frame.
    backdrop_unavailable: bool,
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
    fn new(cc: &eframe::CreationContext<'_>, mut settings: Settings, launch: Launch) -> Self {
        let ctx = cc.egui_ctx.clone();
        crate::fonts::install(&ctx);
        // A preview is an ordinary window: none of the docking below applies to it.
        let window = if launch.preview { None } else { own_window(cc) };
        if let Some(w) = window {
            barcraft_win32::style_dock_window(w);
        }
        // With "windows under the dock" nothing is reserved, so no appbar is registered.
        let appbar = window
            .filter(|_| !settings.float_over_windows)
            .and_then(AppBar::register);
        if appbar.is_none() && !launch.preview && !settings.float_over_windows {
            log::warn!("could not reserve screen space for the dock; windows may cover it");
        }
        if !settings.pins_imported {
            import_taskbar_pins(&mut settings);
            save_settings(&settings);
        }
        settings.launch_at_startup = barcraft_win32::launch_at_startup(APP_NAME);
        if !launch.preview {
            taskbar::apply(&mut settings);
            taskbar::spawn_watchdog();
        }
        let open_settings = launch.open_settings;

        let poller = Poller::start(ctx.clone(), crate::usage_path());
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
            stopwatch: Stopwatch::default(),
            calendar: None,
            settings_open: open_settings,
            settings_ui: SettingsUi::default(),
            styled_for: None,
            windows_light: barcraft_win32::system_uses_light_theme().unwrap_or(false),
            widgets_width: 0.0,
            apps_scroll: 0.0,
            widgets_scroll: 0.0,
            widgets_rect: Rect::NOTHING,
            apps_rect: Rect::NOTHING,
            preview: launch.preview,
            backdrop: None,
            backdrop_at: None,
            window_px: None,
            backdrop_unavailable: false,
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
            backdrop: self
                .backdrop
                .as_ref()
                .filter(|_| self.theme().glass)
                .map(Backdrop::window),
            monitor: self.placement.map(|p| p.monitor),
            hide_for_fullscreen: self.settings.hide_for_fullscreen,
            // A preview never touches the taskbar (a real dock may be running beside it).
            taskbar_mode: if self.preview {
                barcraft_core::TaskbarMode::Untouched
            } else {
                self.settings.taskbar_mode
            },
            widgets: self.settings.widgets.clone(),
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
            float: self.settings.float_over_windows,
        };
        if !force && self.placement == Some(want) {
            return;
        }
        // Reserve the strip, or give it back so maximised windows reach the screen edge and the
        // dock floats over them.
        if want.float {
            self.appbar = None;
        } else if self.appbar.is_none() {
            self.appbar = AppBar::register(win);
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
        self.window_px = Some(rect);
        self.backdrop_at = None;
        self.placement = Some(want);
        self.region = None;
        self.poller.send(Command::Watch(self.watch()));
    }

    /// The colours in use: the chosen theme, with "Follow Windows" resolved for the current mode.
    fn theme(&self) -> Theme {
        self.settings.theme.resolve(self.windows_light)
    }

    /// Keeps the blurred backdrop exactly under the bar (glass themes only), right below the
    /// dock window, and hidden whenever the dock is.
    fn update_backdrop(&mut self, bar: Rect, scale: f32, hidden_for_fullscreen: bool) {
        let (Some(win), Some(window_px)) = (self.window, self.window_px) else {
            return;
        };
        if !self.theme().glass {
            if let Some(b) = self.backdrop.as_mut() {
                b.set_shown(false);
            }
            return;
        }
        if self.backdrop.is_none() && !self.backdrop_unavailable {
            self.backdrop = Backdrop::create();
            if self.backdrop.is_none() {
                self.backdrop_unavailable = true;
                log::info!("Windows offers no blur here; glass themes keep a plain tint");
            }
            self.poller.send(Command::Watch(self.watch()));
        }
        let Some(backdrop) = self.backdrop.as_mut() else {
            return;
        };
        let px = |v: f32| (v * scale).round() as i32;
        let rect = PxRect {
            left: window_px.left + px(bar.left()),
            top: window_px.top + px(bar.top()),
            right: window_px.left + px(bar.right()),
            bottom: window_px.top + px(bar.bottom()),
        };
        if rect.width() <= 0 || rect.height() <= 0 {
            return;
        }
        let now = Instant::now();
        // Re-place when the bar moved or resized, and now and then to keep it under the dock.
        let due = self
            .backdrop_at
            .is_none_or(|(at, t)| at != rect || now.duration_since(t) >= RECHECK);
        if due {
            backdrop.place(win, rect);
            self.backdrop_at = Some((rect, now));
        }
        backdrop.set_shown(!hidden_for_fullscreen);
    }

    fn apply_style(&mut self, ctx: &egui::Context, theme: &Theme) {
        if self.styled_for == Some(*theme) {
            return;
        }
        self.styled_for = Some(*theme);
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
        let (rect, resp) = ui.allocate_exact_size(vec2(icon_side + 16.0, bar_h), Sense::click());
        let is_front = item
            .windows
            .iter()
            .any(|w| Some(WindowId(w.id)) == foreground);
        // Every app sits in its own cell; the one in front and the one under the pointer stand out.
        let fill = if resp.hovered() {
            draw::color(theme.hover)
        } else if is_front {
            draw::color(theme.hover).gamma_multiply(0.7)
        } else {
            draw::color(theme.tile)
        };
        draw::cell(
            ui.painter(),
            rect.shrink2(vec2(1.0, CELL_INSET)),
            cell_radius(theme),
            fill,
            theme,
        );
        // Icons grow a little under the pointer, and shrink a little while pressed.
        let grow = if self.settings.magnify_icons {
            ctx.animate_bool_with_time(resp.id.with("grow"), resp.hovered(), 0.12)
        } else {
            0.0
        };
        let press = if resp.is_pointer_button_down_on() {
            0.92
        } else {
            1.0
        };
        let lift = match self.settings.edge {
            DockEdge::Bottom => -2.0 * grow,
            DockEdge::Top => 2.0 * grow,
        };
        let side = icon_side * (1.0 + 0.14 * grow) * press;
        let icon_rect = Rect::from_center_size(rect.center() + vec2(0.0, lift), vec2(side, side));
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
                DockEdge::Bottom => rect.bottom() - CELL_INSET - 3.5,
                DockEdge::Top => rect.top() + CELL_INSET + 3.5,
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
        if changes.clear_usage {
            self.poller.send(Command::ClearUsage);
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
        if changes.taskbar && !self.preview {
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

/// The bar's background: a soft shadow (not for glass), the theme's fill, a faint light on the
/// upper half and a thin edge. With a glass theme the fill is see-through and the backdrop
/// window underneath blurs the desktop.
fn bar_background(rect: Rect, theme: &Theme) -> Shape {
    let r = bar_radius(theme);
    let radius = CornerRadius::same(r);
    let mut shapes = Vec::with_capacity(4);
    // Glass gets no shadow: it would show through the fill and muddy the blur.
    if !theme.glass {
        let shadow = Shadow {
            offset: [0, 3],
            blur: 14,
            spread: 0,
            color: Color32::from_black_alpha(80),
        }
        .as_shape(rect, radius);
        shapes.push(Shape::Rect(shadow));
    }
    let sheen_alpha = match (theme.glass, theme.dark) {
        (true, _) => 14,
        (false, true) => 9,
        (false, false) => 60,
    };
    let upper = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.center().y));
    let upper_radius = CornerRadius {
        nw: r,
        ne: r,
        sw: 0,
        se: 0,
    };
    shapes.push(Shape::rect_filled(rect, radius, draw::color(theme.bar)));
    shapes.push(Shape::rect_filled(
        upper,
        upper_radius,
        Color32::from_white_alpha(sheen_alpha),
    ));
    shapes.push(Shape::rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, draw::color(theme.bar_edge)),
        egui::StrokeKind::Inside,
    ));
    Shape::Vec(shapes)
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
        if self.preview {
            // A neutral backdrop, so the bar reads as it would over a desktop.
            [0.16, 0.17, 0.19, 1.0]
        } else {
            [0.0, 0.0, 0.0, 0.0]
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.place(ctx.pixels_per_point(), false);
        self.take_updates(&ctx);
        let theme = self.theme();
        self.apply_style(&ctx, &theme);

        let (windows, pids, foreground, readings, hidden_for_fullscreen) = {
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
                microphone: snap.microphone,
                media: snap.media.clone(),
                net_down: snap.net_down.iter().copied().collect(),
                net_up: snap.net_up,
                disk: snap.disk,
                recycle_bin: snap.recycle_bin,
                usage_total: snap.usage.total(),
                usage_top: snap
                    .usage
                    .top(6)
                    .into_iter()
                    .map(|(app, secs)| (app.to_owned(), secs))
                    .collect(),
                weather: snap.weather.clone(),
                windows_light: snap.windows_light,
            };
            (
                windows,
                pids,
                snap.foreground,
                readings,
                snap.hidden_for_fullscreen,
            )
        };
        self.windows_light = readings.windows_light;
        let items = build_dock_items(&self.settings.pinned, &windows, &mut self.seen_order);

        let size = self.settings.size;
        let bar_h = size.bar_height();
        let (anchor, offset) = match (self.preview, self.settings.edge) {
            (true, _) => (Align2::CENTER_CENTER, vec2(0.0, 0.0)),
            (false, DockEdge::Bottom) => (Align2::CENTER_BOTTOM, vec2(0.0, -MARGIN)),
            (false, DockEdge::Top) => (Align2::CENTER_TOP, vec2(0.0, MARGIN)),
        };
        let now = chrono::Local::now();
        let instant = Instant::now();
        let mut app_actions = Vec::new();
        let mut widget_actions = Vec::new();
        let mut open_menu_from_start = false;
        let mut fast_repaint = false;

        // How wide the bar may get, and how much of that the apps may use.
        let gap = 2.0;
        let screen_w = ctx.content_rect().width();
        let max_inner = (screen_w - 2.0 * MARGIN - 2.0 * BAR_PAD - 2.0).max(200.0);
        let start_w = size.icon_size() + 16.0;
        let separator_w = if self.settings.widgets.is_empty() {
            0.0
        } else {
            9.0
        };
        let app_cell = size.icon_size() + 16.0 + gap;
        let min_apps = app_cell * 3.0;
        let widgets_max = (max_inner - start_w - separator_w - min_apps - 4.0 * gap).max(120.0);
        // Widgets get the room they asked for last frame (up to their maximum); apps get the rest.
        let widgets_w = self.widgets_width.min(widgets_max);
        let apps_max = (max_inner - start_w - separator_w - widgets_w - 4.0 * gap).max(60.0);
        let apps_w = (items.len() as f32 * app_cell).min(apps_max);
        // An anchored area only offers last frame's size to its contents, so a scroll area inside
        // could never grow; asking for the whole width up front lets the bar widen in one frame.
        let wanted_inner = (start_w + apps_w + separator_w + widgets_w + 4.0 * gap).min(max_inner);
        // Scroll the app and widget areas with the mouse wheel when they don't all fit.
        let wheel = ctx.input(|i| i.smooth_scroll_delta.y + i.smooth_scroll_delta.x);
        let pointer = ctx.pointer_hover_pos();
        if pointer.is_some_and(|p| self.apps_rect.contains(p)) {
            self.apps_scroll = (self.apps_scroll - wheel).max(0.0);
        }
        let over_widgets = pointer.is_some_and(|p| self.widgets_rect.contains(p));
        let mut widgets_wheel_used = false;

        let bar = egui::Area::new(Id::unique("barcraft-bar"))
            .anchor(anchor, offset)
            .movable(false)
            .sense(Sense::click_and_drag())
            .show(&ctx, |ui| {
                // The bar's background is painted after its contents are measured, underneath them.
                let background = ui.painter().add(Shape::Noop);
                let frame = egui::Frame::new()
                    .inner_margin(Margin::symmetric(BAR_PAD as i8, 0))
                    .show(ui, |ui| {
                        if self.settings.layout == DockLayout::FullWidth {
                            ui.set_min_width(max_inner);
                        } else {
                            ui.set_min_width(wanted_inner);
                        }
                        ui.set_height(bar_h);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = vec2(gap, 0.0);
                            // Start menu.
                            let (rect, resp) =
                                ui.allocate_exact_size(vec2(start_w, bar_h), Sense::click());
                            let fill = if resp.hovered() {
                                theme.hover
                            } else {
                                theme.tile
                            };
                            draw::cell(
                                ui.painter(),
                                rect.shrink2(vec2(1.0, CELL_INSET)),
                                cell_radius(&theme),
                                draw::color(fill),
                                &theme,
                            );
                            draw::launcher(ui.painter(), rect, draw::color(theme.text));
                            let resp =
                                resp.on_hover_text("Start (right-click for the dock's menu)");
                            if resp.clicked() {
                                barcraft_win32::open_start_menu();
                            }
                            if resp.secondary_clicked() {
                                open_menu_from_start = true;
                            }

                            // Apps, scrolling sideways when there are too many to fit.
                            let scroll = egui::ScrollArea::horizontal()
                                .id_salt("barcraft-apps")
                                .max_width(apps_max)
                                .max_height(bar_h)
                                .content_margin(0)
                                .horizontal_scroll_offset(self.apps_scroll)
                                .scroll_bar_visibility(
                                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                )
                                .show(ui, |ui| {
                                    // Top-aligned: each cell is exactly as tall as the bar, while the
                                    // scroll area's own height is open-ended, so centring would push
                                    // the icons down.
                                    ui.horizontal_top(|ui| {
                                        ui.spacing_mut().item_spacing = vec2(gap, 0.0);
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
                                    });
                                });
                            self.apps_scroll = scroll.state.clamped_offset().x;
                            self.apps_rect = scroll.inner_rect;

                            if self.settings.widgets.is_empty() {
                                self.widgets_width = 0.0;
                                return;
                            }
                            if self.settings.layout == DockLayout::FullWidth {
                                // Push the widgets to the far end.
                                let used = ui.min_rect().width();
                                let spare = max_inner - used - separator_w - widgets_w - 2.0 * gap;
                                ui.add_space(spare.max(0.0));
                            }
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(separator_w, bar_h), Sense::hover());
                            ui.painter().line_segment(
                                [
                                    Pos2::new(rect.center().x, rect.top() + bar_h * 0.3),
                                    Pos2::new(rect.center().x, rect.bottom() - bar_h * 0.3),
                                ],
                                Stroke::new(1.0, draw::color(theme.bar_edge)),
                            );
                            // Widgets, also scrolling sideways when they don't all fit; the apps
                            // keep room for at least a few icons.
                            let widget_list = self.settings.widgets.clone();
                            let settings = &self.settings;
                            let focus = &mut self.focus;
                            let stopwatch = &mut self.stopwatch;
                            let calendar = &mut self.calendar;
                            let wscroll = egui::ScrollArea::horizontal()
                                .id_salt("barcraft-widgets")
                                .max_width(widgets_max)
                                .max_height(bar_h)
                                .content_margin(0)
                                .horizontal_scroll_offset(self.widgets_scroll)
                                .scroll_bar_visibility(
                                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                )
                                .show(ui, |ui| {
                                    ui.horizontal_top(|ui| {
                                        ui.spacing_mut().item_spacing = vec2(gap, 0.0);
                                        let mut wui = WidgetUi {
                                            theme,
                                            settings,
                                            readings: &readings,
                                            focus,
                                            stopwatch,
                                            calendar,
                                            now,
                                            instant,
                                            height: bar_h,
                                            actions: Vec::new(),
                                            preview: false,
                                            fast_repaint: false,
                                            wheel_used: false,
                                        };
                                        for kind in widget_list {
                                            wui.show(ui, kind);
                                        }
                                        (wui.actions, wui.fast_repaint, wui.wheel_used)
                                    })
                                    .inner
                                });
                            let (actions, fast, wheel_used) = wscroll.inner;
                            widget_actions = actions;
                            fast_repaint = fast;
                            widgets_wheel_used = wheel_used;
                            self.widgets_scroll = wscroll.state.clamped_offset().x;
                            self.widgets_rect = wscroll.inner_rect;
                            // The widgets' full width, scrolled or not, so next frame can make room.
                            self.widgets_width = wscroll.content_size.x;
                        });
                    });
                ui.painter()
                    .set(background, bar_background(frame.response.rect, &theme));
            });
        self.update_backdrop(
            bar.response.rect,
            ctx.pixels_per_point(),
            hidden_for_fullscreen,
        );
        // The wheel scrolls the widgets unless a widget used it (the volume widget does).
        if over_widgets && !widgets_wheel_used && wheel != 0.0 {
            self.widgets_scroll = (self.widgets_scroll - wheel).max(0.0);
            ctx.request_repaint();
        }

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
                widgets::Action::SetMicrophoneMuted(m) => {
                    self.poller.send(Command::SetMicrophoneMuted(m));
                }
                widgets::Action::Media(m) => self.poller.send(Command::Media(m)),
                widgets::Action::ToggleDesktop => barcraft_win32::toggle_desktop(),
                widgets::Action::ShowTray => self.poller.send(Command::ShowTray),
                widgets::Action::OpenNotifications => barcraft_win32::open_notification_center(),
                widgets::Action::OpenRecycleBin => {
                    barcraft_win32::open_recycle_bin();
                }
                widgets::Action::EmptyRecycleBin => self.poller.send(Command::EmptyRecycleBin),
                widgets::Action::OpenSettings => self.settings_open = true,
            }
        }

        if self.settings_open {
            let mut open = true;
            let changes = settings_window::show(
                &ctx,
                &mut self.settings,
                &mut self.settings_ui,
                &readings,
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

        let ms = if fast_repaint {
            100
        } else {
            1000u64
                .saturating_sub(u64::from(now.timestamp_subsec_millis()))
                .max(20)
        };
        ctx.request_repaint_after(Duration::from_millis(ms));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.appbar = None;
        self.backdrop = None;
        if !self.preview {
            taskbar::restore(&mut self.settings);
        }
        save_settings(&self.settings);
    }
}
