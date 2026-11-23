//! The dock window: where it sits, what it draws, and what clicks on it do.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use barcraft_core::apps::{DockItem, PinnedApp, WindowRef, build_dock_items, normalize_path};
use barcraft_core::clock::Stopwatch;
use barcraft_core::focus::FocusTimer;
use barcraft_core::{
    DockEdge, DockLayout, DockSize, IndicatorStyle, Lens, Settings, Theme, WidgetKind,
};
use barcraft_win32::{AppBar, Backdrop, Edge, PxRect, WindowId};
use egui::{
    Align2, Color32, CornerRadius, Id, Margin, Popup, PopupCloseBehavior, Pos2, Rect, RectAlign,
    Sense, Shadow, Shape, Stroke, Ui, pos2, vec2,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::poller::{Command, DockWatch, Poller};
use crate::settings_window::{self, Changes, SettingsUi};
use crate::widgets::{self, Readings, WidgetUi};
use crate::{APP_NAME, draw, save_settings, taskbar};

/// Gap between the bar and the screen edge, in points.
const MARGIN: f32 = 6.0;
/// Inner margin of the bar, left and right, in points.
const BAR_PAD: f32 = 8.0;
/// Corner radius of an opaque bar, in points.
const BAR_RADIUS: u8 = 18;
/// Corner radius of a glass bar. Windows rounds the blur behind it less, so the blur is set in
/// by [`GLASS_INSET`]; with these numbers its corners stay inside the bar's.
const GLASS_RADIUS: u8 = 13;
/// How far the blurred backdrop sits inside the bar's edge, in points.
const GLASS_INSET: f32 = 1.5;
/// Space between widgets, in points.
const WIDGET_GAP: f32 = 6.0;

/// The bar's corner radius for `theme`.
fn bar_radius(theme: &Theme) -> u8 {
    if theme.glass {
        GLASS_RADIUS
    } else {
        BAR_RADIUS
    }
}

/// The blur's own corners (rounded by Windows) must stay inside the bar's: an inner corner of
/// radius r, set in by d, fits an outer one of radius R when sqrt(2)(R - d - r) + r <= R.
const _: () = {
    let (big, inset, blur) = (
        GLASS_RADIUS as f32,
        GLASS_INSET,
        Backdrop::CORNER_RADIUS_POINTS,
    );
    let gap = big - inset - blur;
    assert!(std::f32::consts::SQRT_2 * gap + blur <= big + 0.01);
};
/// Room above (or below) the bar for menus, tooltips and the calendar, in points.
const POPUP_SPACE: f32 = 460.0;
/// Height of the strip along the screen edge that brings a hidden dock back, in points.
const EDGE_STRIP: f32 = 2.0;
/// How long an auto-hiding dock stays out after the pointer leaves it.
const HIDE_DELAY: Duration = Duration::from_millis(700);
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
    /// In a preview, draw the bar as if the pointer rested on this app (counted from 0), to
    /// look at magnification and labels without touching the mouse.
    pub hover: Option<usize>,
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
    /// Auto-hide: whether the dock is out, when the pointer was last on it, and where the bar
    /// was drawn last frame.
    revealed: bool,
    last_inside: Instant,
    last_bar_rect: Rect,
    /// Magnification: where along the row the pointer last was, and the area where it keeps
    /// the icons magnified (the app row, plus the room the grown icons take up).
    lens_x: f32,
    lens_zone: Rect,
    /// Apps opened from the dock and when, so their icons bounce until a window shows.
    launching: HashMap<String, Instant>,
    /// See [`Launch::hover`]; only used in a preview.
    preview_hover: Option<usize>,
    /// When the dock started, for its entrance.
    started: Instant,
    /// Where each app's cell was last frame.
    last_cells: Vec<Rect>,
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
            .filter(|_| !settings.windows_under_dock())
            .and_then(AppBar::register);
        if appbar.is_none() && !launch.preview && !settings.windows_under_dock() {
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
            revealed: true,
            last_inside: Instant::now(),
            last_bar_rect: Rect::NOTHING,
            lens_x: 0.0,
            lens_zone: Rect::NOTHING,
            launching: HashMap::new(),
            preview_hover: launch.hover.filter(|_| launch.preview),
            started: Instant::now(),
            last_cells: Vec::new(),
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
            float: self.settings.windows_under_dock(),
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

    /// How far out the dock is, from 0 (out of sight) to 1, animating between the two. Without
    /// auto-hide it is always 1.
    fn reveal_amount(&mut self, ctx: &egui::Context, auto_hide: bool) -> f32 {
        if !auto_hide {
            self.revealed = true;
            return 1.0;
        }
        let now = Instant::now();
        let screen = ctx.content_rect();
        let pointer = ctx.pointer_hover_pos();
        let at_edge = pointer.is_some_and(|p| match self.settings.edge {
            DockEdge::Bottom => p.y >= screen.bottom() - EDGE_STRIP - 1.0,
            DockEdge::Top => p.y <= screen.top() + EDGE_STRIP + 1.0,
        });
        let on_dock = pointer.is_some_and(|p| {
            self.last_bar_rect.expand(12.0).contains(p) || self.lens_zone.contains(p)
        });
        let busy = ctx.any_popup_open() || Popup::is_any_open(ctx) || ctx.dragged_id().is_some();
        if at_edge || (self.revealed && on_dock) || busy {
            self.revealed = true;
            self.last_inside = now;
        } else if self.revealed && now.duration_since(self.last_inside) >= HIDE_DELAY {
            self.revealed = false;
        }
        if self.revealed {
            // Come back to check whether it is time to hide.
            ctx.request_repaint_after(HIDE_DELAY);
        }
        ctx.animate_bool_with_time_and_easing(
            Id::unique("barcraft-reveal"),
            self.revealed,
            0.26,
            egui::emath::easing::cubic_in_out,
        )
    }

    /// The dock's entrance when it starts: it rises from the screen edge and settles with a
    /// small overshoot. 1 once it is in place.
    fn entrance(&self, ctx: &egui::Context) -> f32 {
        const DURATION: f32 = 0.65;
        let t = self.started.elapsed().as_secs_f32() / DURATION;
        if t >= 1.0 {
            return 1.0;
        }
        ctx.request_repaint();
        egui::emath::easing::back_out(t.max(0.0))
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
        let bar = bar.shrink(GLASS_INSET);
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
        v.window_corner_radius = CornerRadius::same(14);
        v.menu_corner_radius = CornerRadius::same(12);
        v.selection.bg_fill = draw::color(theme.accent).gamma_multiply(0.75);
        v.hyperlink_color = draw::color(theme.accent);
        let soft = Shadow {
            offset: [0, 8],
            blur: 28,
            spread: 0,
            color: Color32::from_black_alpha(if theme.dark { 110 } else { 50 }),
        };
        v.popup_shadow = soft;
        v.window_shadow = soft;
        v.window_stroke = Stroke::new(1.0, draw::color(theme.bar_edge));
        // Flat controls that light up under the pointer, with rounded corners.
        for w in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = CornerRadius::same(7);
            w.expansion = 0.0;
        }
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.hovered.bg_stroke = Stroke::NONE;
        v.widgets.hovered.weak_bg_fill = draw::color(theme.hover);
        v.widgets.hovered.bg_fill = draw::color(theme.hover);
        v.widgets.active.bg_stroke = Stroke::NONE;
        v.widgets.active.weak_bg_fill = draw::color(theme.accent).gamma_multiply(0.5);
        ctx.set_visuals(v);
        ctx.all_styles_mut(|s| {
            s.spacing.button_padding = vec2(10.0, 5.0);
            s.spacing.menu_margin = Margin::same(6);
            s.spacing.item_spacing = vec2(6.0, 3.0);
            s.spacing.interact_size.y = 24.0;
        });
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
                    app_id: item.app_id.clone(),
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

    /// How high an app's icon is up in its launch bounce, in points: it hops while the app
    /// starts, at most three times, and settles once a window shows (after a full hop).
    fn bounce(&mut self, ctx: &egui::Context, item: &DockItem, icon_side: f32) -> f32 {
        const HOP: f32 = 0.55;
        let Some(start) = self.launching.get(&item.key) else {
            return 0.0;
        };
        let t = start.elapsed().as_secs_f32() / HOP;
        let hop = t.floor();
        if hop >= 3.0 || (item.is_running() && hop >= 1.0) {
            self.launching.remove(&item.key);
            return 0.0;
        }
        ctx.request_repaint();
        let height = icon_side * 0.5 * (1.0 - 0.25 * hop);
        // Up quickly, down a little faster, like something light thrown up.
        let f = t - hop;
        height * (std::f32::consts::PI * f).sin().powf(0.8)
    }

    /// Lays out one app's cell at its normal size and handles clicks on it. The icon itself is
    /// painted later, by [`paint_icons`], once the lens has said where and how big.
    #[allow(clippy::too_many_arguments)]
    fn app_button(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        item: &DockItem,
        foreground: Option<WindowId>,
        pids: &HashMap<isize, u32>,
        actions: &mut Vec<AppAction>,
    ) -> IconDraw {
        let size = self.settings.size;
        let bar_h = size.bar_height();
        let icon_side = size.icon_size();
        let (rect, resp) = ui.allocate_exact_size(vec2(icon_side + 16.0, bar_h), Sense::click());
        let is_front = item
            .windows
            .iter()
            .any(|w| Some(WindowId(w.id)) == foreground);
        let draw = IconDraw {
            cell: rect,
            texture: self.icon(ctx, item),
            name: item.name.clone(),
            hovered: resp.hovered(),
            pressed: resp.is_pointer_button_down_on(),
            is_front,
            running: item.is_running(),
            windows: item.windows.len(),
            bounce: self.bounce(ctx, item, icon_side),
        };

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
                        self.launching.insert(item.key.clone(), Instant::now());
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
        draw
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
/// One app of the row as laid out this frame, to be painted once the lens is known.
struct IconDraw {
    /// The app's cell at its normal size, where clicks land.
    cell: Rect,
    texture: Option<egui::TextureId>,
    name: String,
    hovered: bool,
    pressed: bool,
    is_front: bool,
    running: bool,
    windows: usize,
    /// How high the icon is in its launch bounce, in points.
    bounce: f32,
}

/// What [`paint_icons`] needs to know about the row this frame.
struct IconLook<'a> {
    theme: &'a Theme,
    edge: DockEdge,
    indicator: IndicatorStyle,
    icon_side: f32,
    /// The whole bar, and the part of it the app row shows.
    bar: Rect,
    view: Rect,
    lens: Lens,
    lens_x: f32,
    /// How far the lens is faded in, 0 to 1.
    amount: f32,
    /// Whether the pointer is over the row right now.
    in_row: bool,
    /// Spare room either side of the row that the outer icons may move into.
    pad: f32,
    magnify: bool,
}

/// Paints the app icons where the lens puts them, the one under the pointer on top and with its
/// name above it.
fn paint_icons(ctx: &egui::Context, painter: &egui::Painter, icons: &[IconDraw], look: &IconLook) {
    // Room for icons grown by the lens or up in a launch bounce.
    let grow_room = look.icon_side * (look.lens.peak - 1.0).max(0.5) + 8.0;
    let (top, bottom) = match look.edge {
        DockEdge::Bottom => (look.bar.top() - grow_room, look.bar.bottom()),
        DockEdge::Top => (look.bar.top(), look.bar.bottom() + grow_room),
    };
    let clip = Rect::from_min_max(
        pos2(look.view.left() - look.pad, top),
        pos2(look.view.right() + look.pad, bottom),
    );
    let painter = painter.with_clip_rect(clip);
    // Past either end of the row the lens stays on the end icon, as if the pointer were on it.
    let lens_x = match (icons.first(), icons.last()) {
        (Some(first), Some(last)) => look.lens_x.clamp(
            first.cell.center().x,
            last.cell.center().x.max(first.cell.center().x),
        ),
        _ => look.lens_x,
    };
    // The app under the pointer: by the lens while it is on (its cell may be under a grown
    // neighbour's picture otherwise), else by the cell.
    let pointed = if look.in_row && look.magnify {
        icons.iter().position(|i| {
            let half_gap = 1.0;
            (i.cell.left() - half_gap..i.cell.right() + half_gap).contains(&lens_x)
        })
    } else {
        icons.iter().position(|i| i.hovered)
    };
    let order = (0..icons.len())
        .filter(|&i| Some(i) != pointed)
        .chain(pointed);
    let mut name = None;
    for i in order {
        let Some(icon) = icons.get(i) else {
            continue;
        };
        let x0 = look.lens.position(icon.cell.left(), lens_x, look.amount);
        let x1 = look.lens.position(icon.cell.right(), lens_x, look.amount);
        let cx = (x0 + x1) * 0.5;
        let scale = look.lens.scale(icon.cell.center().x, lens_x, look.amount);
        let side = look.icon_side * scale;
        let lift = match look.edge {
            DockEdge::Bottom => -icon.bounce,
            DockEdge::Top => icon.bounce,
        };
        // Icons grow away from the screen edge, all standing on the same line.
        let rect = match look.edge {
            DockEdge::Bottom => {
                let base = icon.cell.center().y + look.icon_side / 2.0;
                Rect::from_min_max(
                    pos2(cx - side / 2.0, base - side),
                    pos2(cx + side / 2.0, base),
                )
            }
            DockEdge::Top => {
                let base = icon.cell.center().y - look.icon_side / 2.0;
                Rect::from_min_max(
                    pos2(cx - side / 2.0, base),
                    pos2(cx + side / 2.0, base + side),
                )
            }
        }
        .translate(vec2(0.0, lift));
        // Pressed icons darken, as a key does under a finger.
        let tint = if icon.pressed {
            Color32::from_gray(165)
        } else {
            Color32::WHITE
        };
        if Some(i) == pointed && !look.magnify {
            // Without magnification a soft patch behind the icon is the only sign of the pointer.
            let glow = Rect::from_center_size(icon.cell.center(), vec2(side + 12.0, side + 12.0));
            painter.add(draw::gradient_fill(
                &draw::squircle_points(glow),
                draw::color(look.theme.hover),
                draw::color(look.theme.tile),
            ));
        }
        match icon.texture {
            Some(tex) => {
                painter.image(
                    tex,
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    tint,
                );
            }
            None => draw::letter_tile(&painter, rect, &icon.name),
        }
        if icon.running {
            // Quiet marks in the text colour, a little brighter for the app in front; one per
            // window, up to three.
            let ink =
                draw::color(look.theme.text).gamma_multiply(if icon.is_front { 0.95 } else { 0.6 });
            // Halfway between the icon and the bar's edge.
            let gap = (icon.cell.height() - look.icon_side) * 0.25;
            let y = match look.edge {
                DockEdge::Bottom => icon.cell.bottom() - gap,
                DockEdge::Top => icon.cell.top() + gap,
            };
            let count = icon.windows.clamp(1, 3);
            match look.indicator {
                IndicatorStyle::Dot | IndicatorStyle::LargeDot => {
                    let r = if look.indicator == IndicatorStyle::Dot {
                        1.9
                    } else {
                        2.6
                    };
                    let step = r * 2.0 + 2.5;
                    let first = cx - step * (count as f32 - 1.0) / 2.0;
                    for k in 0..count {
                        painter.circle_filled(pos2(first + step * k as f32, y), r, ink);
                    }
                }
                IndicatorStyle::Line => {
                    let w = if icon.is_front { 16.0 } else { 8.0 } + 4.0 * (count as f32 - 1.0);
                    painter.rect_filled(
                        Rect::from_center_size(pos2(cx, y), vec2(w, 3.0)),
                        1.5,
                        ink,
                    );
                }
            }
        }
        if Some(i) == pointed {
            name = Some((rect, icon.name.as_str()));
        }
    }
    // The label fades in when the pointer arrives and simply follows it from icon to icon.
    let showing = name.is_some() && !Popup::is_any_open(ctx);
    let fade = ctx.animate_bool_with_time(Id::unique("barcraft-app-name-fade"), showing, 0.12);
    if let Some((rect, text)) = name
        && showing
    {
        paint_name(ctx, look, rect, text, fade);
    }
}

/// The app's name in a small label just past its icon, away from the screen edge, with a little
/// pointer at the icon. `fade` (0 to 1) is how far it has faded in.
fn paint_name(ctx: &egui::Context, look: &IconLook, icon: Rect, text: &str, fade: f32) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        Id::unique("barcraft-app-name"),
    ));
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        crate::fonts::font(crate::fonts::Weight::SemiBold, 12.5),
        draw::color(look.theme.text),
    );
    let size = galley.size() + vec2(22.0, 11.0);
    // It rises a few points into place as it fades in.
    let rise = (1.0 - fade) * 4.0;
    let y = match look.edge {
        DockEdge::Bottom => icon.top() - 11.0 - size.y / 2.0 + rise,
        DockEdge::Top => icon.bottom() + 11.0 + size.y / 2.0 - rise,
    };
    let mut rect = Rect::from_center_size(pos2(icon.center().x, y), size);
    // Kept on screen at the ends of a full-width bar.
    let screen = ctx.content_rect();
    let dx = (screen.left() + 4.0 - rect.left()).max(0.0)
        + (screen.right() - 4.0 - rect.right()).min(0.0);
    rect = rect.translate(vec2(dx, 0.0));
    let radius = CornerRadius::same(9);
    let fill = draw::color(look.theme.popup).gamma_multiply(fade);
    painter.add(
        Shadow {
            offset: [0, 3],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha((70.0 * fade) as u8),
        }
        .as_shape(rect, radius),
    );
    painter.rect_filled(rect, radius, fill);
    // The pointer: a small triangle from the label towards the icon's centre.
    let tip_x = icon
        .center()
        .x
        .clamp(rect.left() + 12.0, rect.right() - 12.0);
    let (base_y, tip_y) = match look.edge {
        DockEdge::Bottom => (rect.bottom() - 0.5, rect.bottom() + 5.0),
        DockEdge::Top => (rect.top() + 0.5, rect.top() - 5.0),
    };
    painter.add(Shape::convex_polygon(
        vec![
            pos2(tip_x - 6.0, base_y),
            pos2(tip_x + 6.0, base_y),
            pos2(tip_x, tip_y),
        ],
        fill,
        Stroke::NONE,
    ));
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, draw::color(look.theme.bar_edge).gamma_multiply(fade)),
        egui::StrokeKind::Inside,
    );
    let at = rect.center() - galley.size() / 2.0;
    painter.galley(
        at,
        galley,
        draw::color(look.theme.text).gamma_multiply(fade),
    );
}

/// The bar: a tinted pane, lighter towards the top as if lit from above, with a hairline rim and
/// a glint of light along its top edge. Glass gets its shadow from Windows, under the blur.
fn bar_background(rect: Rect, theme: &Theme) -> Shape {
    let r = bar_radius(theme);
    let radius = CornerRadius::same(r);
    let mut shapes = Vec::with_capacity(6);
    if !theme.glass {
        let shadow = Shadow {
            offset: [0, 4],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(if theme.dark { 90 } else { 45 }),
        }
        .as_shape(rect, radius);
        shapes.push(Shape::Rect(shadow));
    }
    shapes.push(Shape::rect_filled(rect, radius, draw::color(theme.bar)));
    // Light from above: strongest at the top, gone by the middle. Set in by a pixel so its
    // (unsmoothed) edge hides under the rim.
    let (light, glint, rim) = match (theme.glass, theme.dark) {
        (true, true) => (16, 70, Color32::from_white_alpha(34)),
        (true, false) => (60, 200, Color32::from_white_alpha(150)),
        (false, true) => (10, 40, Color32::from_white_alpha(22)),
        (false, false) => (50, 160, Color32::from_black_alpha(28)),
    };
    let inner = rect.shrink(1.0);
    let lit = Rect::from_min_max(inner.min, Pos2::new(inner.right(), inner.center().y + 4.0));
    shapes.push(draw::gradient_fill(
        &draw::rounded_points(lit, f32::from(r) - 1.0),
        Color32::from_white_alpha(light),
        Color32::TRANSPARENT,
    ));
    shapes.push(Shape::rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, rim),
        egui::StrokeKind::Inside,
    ));
    let edge = f32::from(r) * 0.8;
    shapes.push(draw::glint(
        rect.top() + 1.5,
        rect.left() + edge,
        rect.right() - edge,
        Color32::from_white_alpha(glint),
    ));
    Shape::Vec(shapes)
}

/// A soft, colourful backdrop for the preview window, standing in for a desktop wallpaper.
fn paint_wallpaper(ctx: &egui::Context) {
    let r = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let mut mesh = egui::Mesh::default();
    let corners = [
        (r.left_top(), Color32::from_rgb(36, 58, 120)),
        (r.right_top(), Color32::from_rgb(120, 62, 130)),
        (r.right_bottom(), Color32::from_rgb(214, 120, 84)),
        (r.left_bottom(), Color32::from_rgb(40, 120, 150)),
    ];
    for (p, c) in corners {
        mesh.colored_vertex(p, c);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
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
        if self.preview {
            paint_wallpaper(&ctx);
        }
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
                    app_name: w.app_name.clone(),
                    app_id: w.app_id.clone(),
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
        // Auto-hide: out while the pointer is on the dock (or at the screen edge) or a menu is
        // open; back out of sight a moment after it leaves.
        let auto_hide = self.settings.auto_hide && !self.preview;
        let shown = self.reveal_amount(&ctx, auto_hide) * self.entrance(&ctx);
        let slide = (1.0 - shown) * (bar_h + MARGIN + 6.0);
        let (anchor, offset) = match (self.preview, self.settings.edge) {
            (true, _) => (Align2::CENTER_CENTER, vec2(0.0, 0.0)),
            (false, DockEdge::Bottom) => (Align2::CENTER_BOTTOM, vec2(0.0, -MARGIN + slide)),
            (false, DockEdge::Top) => (Align2::CENTER_TOP, vec2(0.0, MARGIN - slide)),
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
            13.0
        };
        let app_cell = size.icon_size() + 16.0 + gap;
        let min_apps = app_cell * 3.0;
        let widgets_max = (max_inner - start_w - separator_w - min_apps - 4.0 * gap).max(120.0);
        // Widgets get the room they asked for last frame (up to their maximum); apps get the rest.
        let widgets_w = self.widgets_width.min(widgets_max);
        let apps_max = (max_inner - start_w - separator_w - widgets_w - 4.0 * gap).max(60.0);
        let apps_w = (items.len() as f32 * app_cell).min(apps_max);
        let base_inner = start_w + apps_w + separator_w + widgets_w + 4.0 * gap;

        // Magnification: a lens over the app row, faded in while the pointer is on it. The row
        // is laid out at its normal size; the lens only moves and grows the icons as they are
        // painted, and asks for spare room at both ends that depends on the fade alone, so the
        // bar never changes width as the pointer moves along it.
        let lens = Lens {
            peak: 1.0 + self.settings.magnify_percent as f32 / 100.0,
            reach: 2.5 * app_cell,
        };
        let lens_on = self.settings.magnify_icons;
        // With too many apps to fit, or no room to widen, the end icons are clipped instead.
        let lens_room = items.len() as f32 * app_cell <= apps_max
            && base_inner + 2.0 * lens.spread(1.0) <= max_inner;
        let lens_pointer = ctx.pointer_hover_pos();
        let pinned_hover = self
            .preview_hover
            .and_then(|i| self.last_cells.get(i))
            .map(|r| r.center().x);
        let in_row = lens_on
            && (pinned_hover.is_some() || lens_pointer.is_some_and(|p| self.lens_zone.contains(p)));
        if let Some(x) = pinned_hover {
            self.lens_x = x;
        } else if in_row && let Some(p) = lens_pointer {
            self.lens_x = p.x;
        }
        // Eased, so the icons swell quickly and settle gently.
        let lens_amount = ctx.animate_bool_with_time_and_easing(
            Id::unique("barcraft-lens"),
            in_row,
            0.2,
            egui::emath::easing::cubic_out,
        );
        let lens_pad = if lens_on && lens_room {
            lens.spread(lens_amount)
        } else {
            0.0
        };
        let mut icons: Vec<IconDraw> = Vec::with_capacity(items.len());

        // An anchored area only offers last frame's size to its contents, so a scroll area inside
        // could never grow; asking for the whole width up front lets the bar widen in one frame.
        let wanted_inner = base_inner.min(max_inner) + 2.0 * lens_pad;
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
            // Unconstrained, so auto-hide can slide the bar past the screen edge.
            .constrain(false)
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
                            // Drawn as an app tile of its own, brightening under the pointer.
                            let hot = ctx.animate_bool_with_time(
                                resp.id.with("hot"),
                                resp.hovered(),
                                0.12,
                            );
                            let icon = size.icon_size();
                            let tile = Rect::from_center_size(rect.center(), vec2(icon, icon));
                            draw::start_tile(
                                ui.painter(),
                                tile,
                                draw::color(theme.accent),
                                hot,
                                resp.is_pointer_button_down_on(),
                            );
                            let resp =
                                resp.on_hover_text("Start (right-click for the dock's menu)");
                            if resp.clicked() {
                                barcraft_win32::open_start_menu();
                            }
                            if resp.secondary_clicked() {
                                open_menu_from_start = true;
                            }

                            // Apps, scrolling sideways when there are too many to fit, with room
                            // either side for the lens to push the outer icons into.
                            ui.add_space(lens_pad);
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
                                            let icon = self.app_button(
                                                ui,
                                                &ctx,
                                                item,
                                                foreground,
                                                &pids,
                                                &mut app_actions,
                                            );
                                            icons.push(icon);
                                        }
                                    });
                                });
                            self.apps_scroll = scroll.state.clamped_offset().x;
                            self.apps_rect = scroll.inner_rect;
                            ui.add_space(lens_pad);

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
                            // A quiet divider between apps and widgets, with rounded ends.
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(separator_w, bar_h), Sense::hover());
                            let divider =
                                Rect::from_center_size(rect.center(), vec2(1.5, bar_h * 0.46));
                            ui.painter()
                                .rect_filled(divider, 1, draw::color(theme.bar_edge));
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
                                        ui.spacing_mut().item_spacing = vec2(WIDGET_GAP, 0.0);
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
                let bar_rect = frame.response.rect;
                ui.painter()
                    .set(background, bar_background(bar_rect, &theme));

                // The icons, painted over the finished row so the grown ones can reach past it.
                let look = IconLook {
                    theme: &theme,
                    edge: self.settings.edge,
                    indicator: self.settings.indicator,
                    icon_side: size.icon_size(),
                    bar: bar_rect,
                    view: self.apps_rect,
                    lens,
                    lens_x: self.lens_x,
                    amount: lens_amount,
                    in_row,
                    pad: lens_pad,
                    magnify: lens_on,
                };
                paint_icons(&ctx, ui.painter(), &icons, &look);
                self.last_cells = icons.iter().map(|i| i.cell).collect();
                // Where the pointer keeps the icons magnified next frame: the row and its spare
                // room, from the screen edge to the top of the grown icons.
                let reach_out =
                    (look.icon_side * (lens.peak - 1.0) - (bar_h - look.icon_side) / 2.0).max(0.0)
                        * lens_amount
                        + 4.0;
                let row = Rect::from_x_y_ranges(
                    (self.apps_rect.left() - lens_pad)..=(self.apps_rect.right() + lens_pad),
                    bar_rect.y_range(),
                );
                self.lens_zone = match self.settings.edge {
                    DockEdge::Bottom => Rect::from_min_max(
                        pos2(row.left(), row.top() - reach_out),
                        pos2(row.right(), row.bottom() + MARGIN + 2.0),
                    ),
                    DockEdge::Top => Rect::from_min_max(
                        pos2(row.left(), row.top() - MARGIN - 2.0),
                        pos2(row.right(), row.bottom() + reach_out),
                    ),
                };
                if !lens_on || self.apps_rect == Rect::NOTHING {
                    self.lens_zone = Rect::NOTHING;
                }
            });
        self.last_bar_rect = bar.response.rect;
        self.update_backdrop(
            bar.response.rect,
            ctx.pixels_per_point(),
            hidden_for_fullscreen || shown <= 0.01,
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
                .is_some_and(|p| bar_rect.expand(4.0).contains(p) || self.lens_zone.contains(p));
            let full = hovering
                || lens_amount > 0.001
                || ctx.any_popup_open()
                || Popup::is_any_open(&ctx)
                || bar.response.dragged();
            let region = if full {
                None
            } else if auto_hide && shown <= 0.01 {
                // Hidden: keep only a strip along the screen edge, to notice the pointer there.
                let screen = ctx.content_rect();
                let strip = match self.settings.edge {
                    DockEdge::Bottom => Rect::from_min_max(
                        Pos2::new(screen.left(), screen.bottom() - EDGE_STRIP),
                        screen.max,
                    ),
                    DockEdge::Top => Rect::from_min_max(
                        screen.min,
                        Pos2::new(screen.right(), screen.top() + EDGE_STRIP),
                    ),
                };
                Some(to_px(strip, scale))
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
