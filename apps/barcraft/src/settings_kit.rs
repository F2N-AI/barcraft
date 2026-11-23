//! The pieces the settings window is built from: rows grouped on rounded panels, switches,
//! segmented controls, sliders, steppers, pill buttons and the sidebar's section icons. They are
//! drawn here to match the dock rather than egui's own look.
//!
//! A row's control is laid out right to left, so a row with several controls adds the rightmost
//! one first.

use std::ops::RangeInclusive;

use barcraft_core::Theme;
use egui::{
    Align, Color32, Layout, Painter, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind,
    Ui, pos2, vec2,
};

use crate::draw;
use crate::fonts::{self, Weight};

/// The settings window's colours, from the dock's theme.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub window: Color32,
    pub sidebar: Color32,
    pub panel: Color32,
    pub line: Color32,
    pub control: Color32,
    pub raised: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub danger: Color32,
}

impl Palette {
    pub fn new(theme: &Theme) -> Palette {
        let accent = draw::color(theme.accent);
        if theme.dark {
            Palette {
                dark: true,
                window: Color32::from_rgb(29, 29, 33),
                sidebar: Color32::from_rgb(22, 22, 26),
                panel: Color32::from_rgb(40, 40, 45),
                line: Color32::from_white_alpha(16),
                control: Color32::from_rgb(60, 60, 66),
                raised: Color32::from_rgb(104, 104, 112),
                text: Color32::from_rgb(242, 242, 245),
                muted: Color32::from_rgb(158, 160, 168),
                accent,
                danger: Color32::from_rgb(255, 92, 82),
            }
        } else {
            Palette {
                dark: false,
                window: Color32::from_rgb(242, 242, 246),
                sidebar: Color32::from_rgb(229, 229, 235),
                panel: Color32::WHITE,
                line: Color32::from_black_alpha(18),
                control: Color32::from_rgb(226, 226, 232),
                raised: Color32::WHITE,
                text: Color32::from_rgb(28, 28, 32),
                muted: Color32::from_rgb(108, 110, 118),
                accent,
                danger: Color32::from_rgb(215, 30, 30),
            }
        }
    }
}

/// Text in the interface face.
pub fn rich(text: &str, size: f32, weight: Weight, colour: Color32) -> RichText {
    RichText::new(text)
        .font(fonts::font(weight, size))
        .color(colour)
}

/// A page title.
pub fn title(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(rich(text, 24.0, Weight::SemiBold, pal.text));
}

/// A quiet line of explanation, wrapped to the width available.
pub fn caption(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.add_space(6.0);
    ui.add(egui::Label::new(rich(text, 12.0, Weight::Regular, pal.muted)).wrap());
}

/// Rows on one rounded panel, with an optional heading above it.
pub fn group<R>(
    ui: &mut Ui,
    pal: &Palette,
    heading: Option<&str>,
    add: impl FnOnce(&mut Rows<'_>) -> R,
) -> R {
    ui.add_space(18.0);
    if let Some(h) = heading {
        ui.label(rich(h, 12.5, Weight::SemiBold, pal.muted));
        ui.add_space(6.0);
    }
    let background = ui.painter().add(Shape::Noop);
    let inner = ui.scope(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let mut rows = Rows {
            ui,
            pal: *pal,
            first: true,
        };
        add(&mut rows)
    });
    let rect = inner.response.rect;
    ui.painter().set(
        background,
        Shape::Vec(vec![
            Shape::rect_filled(rect, 11, pal.panel),
            Shape::rect_stroke(rect, 11, Stroke::new(1.0, pal.line), StrokeKind::Inside),
        ]),
    );
    inner.inner
}

/// The rows of a [`group`].
pub struct Rows<'u> {
    pub ui: &'u mut Ui,
    pal: Palette,
    first: bool,
}

impl Rows<'_> {
    /// A label, with an optional second line under it, on the left; a control on the right.
    pub fn row<R>(
        &mut self,
        label: &str,
        sub: Option<&str>,
        control: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        self.separator();
        let pal = self.pal;
        let width = self.ui.available_width();
        // The text is measured first, so the row can be as tall as it needs and the text can sit
        // in its middle, level with the control.
        let text_w = ((width - 28.0) * 0.62).max(160.0);
        let painter = self.ui.painter().clone();
        let label_text = painter.layout(
            label.to_owned(),
            fonts::font(Weight::Regular, 14.0),
            pal.text,
            text_w,
        );
        let sub_text = sub.map(|s| {
            painter.layout(
                s.to_owned(),
                fonts::font(Weight::Regular, 12.0),
                pal.muted,
                text_w,
            )
        });
        let text_h = label_text.size().y + sub_text.as_ref().map_or(0.0, |g| g.size().y + 2.0);
        let height = (text_h + 22.0).max(44.0);
        self.ui
            .allocate_ui_with_layout(
                vec2(width, height),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 2.0);
                    ui.add_space(14.0);
                    ui.allocate_ui_with_layout(
                        vec2(text_w, text_h),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.label(label_text);
                            if let Some(g) = sub_text {
                                ui.label(g);
                            }
                        },
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(14.0);
                        control(ui)
                    })
                    .inner
                },
            )
            .inner
    }

    /// A whole row that can be clicked, with a tick on the right when `chosen`: one choice of
    /// several.
    pub fn choice(&mut self, label: &str, sub: Option<&str>, chosen: bool) -> bool {
        let pal = self.pal;
        let before = self.ui.cursor().min;
        let width = self.ui.available_width();
        // Painted under the row once its size is known.
        let highlight = self.ui.painter().add(Shape::Noop);
        self.row(label, sub, |ui| {
            let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
            if chosen {
                tick(ui.painter(), r, pal.accent);
            }
        });
        let area = Rect::from_min_max(before, pos2(before.x + width, self.ui.cursor().min.y));
        let id = self.ui.scope_id().with(("choice", label));
        let resp = self.ui.interact(area, id, Sense::click());
        if resp.hovered() && !chosen {
            self.ui.painter().set(
                highlight,
                Shape::rect_filled(area.shrink(3.0), 8, pal.line.gamma_multiply(0.7)),
            );
        }
        resp.clicked()
    }

    /// A row of its own making, padded like the others.
    pub fn custom<R>(&mut self, add: impl FnOnce(&mut Ui) -> R) -> R {
        self.separator();
        let width = self.ui.available_width();
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(self.ui, |ui| {
                ui.set_width(width - 28.0);
                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                add(ui)
            })
            .inner
    }

    fn separator(&mut self) {
        if !self.first {
            let top = self.ui.cursor().min;
            let right = top.x + self.ui.available_width();
            self.ui.painter().hline(
                (top.x + 14.0)..=right,
                top.y,
                Stroke::new(1.0, self.pal.line),
            );
        }
        self.first = false;
    }
}

/// A tick mark.
fn tick(p: &Painter, r: Rect, c: Color32) {
    let s = r.width();
    p.add(Shape::line(
        vec![
            r.left_top() + vec2(s * 0.18, s * 0.52),
            r.left_top() + vec2(s * 0.42, s * 0.76),
            r.left_top() + vec2(s * 0.84, s * 0.26),
        ],
        Stroke::new(2.2, c),
    ));
}

/// An on/off switch whose knob slides across.
pub fn toggle(ui: &mut Ui, pal: &Palette, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(42.0, 25.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("switch"), *on, 0.16);
    let enabled = ui.is_enabled();
    let mut track = pal.control.lerp_to_gamma(pal.accent, t);
    if !enabled {
        track = track.gamma_multiply(0.45);
    }
    let p = ui.painter();
    p.rect_filled(rect, 13, track);
    let r = rect.height() / 2.0 - 2.0;
    let x = egui::lerp((rect.left() + r + 2.0)..=(rect.right() - r - 2.0), t);
    let c = pos2(x, rect.center().y);
    p.circle_filled(c + vec2(0.0, 1.0), r, Color32::from_black_alpha(45));
    let knob = if enabled {
        Color32::WHITE
    } else {
        Color32::from_gray(200)
    };
    p.circle_filled(c, r, knob);
    resp
}

/// One choice of several, as a row of joined buttons. Returns whether the choice changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    pal: &Palette,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let font = fonts::font(Weight::Regular, 13.0);
    let galleys: Vec<_> = options
        .iter()
        .map(|(_, l)| {
            ui.painter()
                .layout_no_wrap((*l).to_owned(), font.clone(), pal.text)
        })
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 26.0).collect();
    let total = widths.iter().sum::<f32>() + 4.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, 30.0), Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(rect, 9, pal.control);
    let mut x = rect.left() + 2.0;
    let mut changed = false;
    for (i, ((v, label), g)) in options.iter().zip(galleys).enumerate() {
        let w = widths.get(i).copied().unwrap_or(0.0);
        let r = Rect::from_min_size(pos2(x, rect.top() + 2.0), vec2(w, rect.height() - 4.0));
        let id = ui.scope_id().with(("segment", *label, i));
        let resp = ui.interact(r, id, Sense::click());
        if resp.clicked() && *v != *value {
            *value = *v;
            changed = true;
        }
        if *v == *value {
            p.rect_filled(
                r.translate(vec2(0.0, 1.0)),
                7,
                Color32::from_black_alpha(30),
            );
            p.rect_filled(r, 7, pal.raised);
        } else if resp.hovered() {
            p.rect_filled(r, 7, pal.line);
        }
        let ink = if *v == *value && !pal.dark {
            pal.text
        } else if *v == *value {
            Color32::WHITE
        } else {
            pal.text.gamma_multiply(0.85)
        };
        p.galley(r.center() - g.size() / 2.0, g, ink);
        x += w;
    }
    changed
}

/// A slider for a whole number, with its value after it. Returns whether the value changed.
pub fn slider(
    ui: &mut Ui,
    pal: &Palette,
    value: &mut u32,
    range: RangeInclusive<u32>,
    suffix: &str,
) -> bool {
    // Right to left: the value first, then the slider before it.
    let shown = format!("{value}{suffix}");
    ui.add_sized(
        vec2(46.0, 24.0),
        egui::Label::new(rich(&shown, 13.0, Weight::Regular, pal.muted)),
    );
    let (rect, resp) = ui.allocate_exact_size(vec2(180.0, 26.0), Sense::click_and_drag());
    let (lo, hi) = (*range.start() as f32, *range.end() as f32);
    let track = Rect::from_center_size(rect.center(), vec2(rect.width() - 20.0, 4.0));
    let mut changed = false;
    if (resp.dragged() || resp.clicked())
        && let Some(at) = resp.interact_pointer_pos()
    {
        let t = ((at.x - track.left()) / track.width()).clamp(0.0, 1.0);
        let v = (lo + t * (hi - lo)).round() as u32;
        if v != *value {
            *value = v;
            changed = true;
        }
    }
    let t = ((*value as f32 - lo) / (hi - lo).max(1.0)).clamp(0.0, 1.0);
    let x = track.left() + t * track.width();
    let p = ui.painter();
    p.rect_filled(track, 2, pal.control);
    p.rect_filled(
        Rect::from_min_max(track.min, pos2(x, track.bottom())),
        2,
        pal.accent,
    );
    let knob = pos2(x, track.center().y);
    p.circle_filled(knob + vec2(0.0, 1.0), 9.0, Color32::from_black_alpha(45));
    p.circle_filled(knob, 9.0, Color32::WHITE);
    p.circle_stroke(knob, 9.0, Stroke::new(1.0, pal.line));
    changed
}

/// A number with − and + on either side. Returns whether the value changed.
pub fn stepper(
    ui: &mut Ui,
    pal: &Palette,
    value: &mut u32,
    range: RangeInclusive<u32>,
    step: u32,
    suffix: &str,
) -> bool {
    let (rect, _) = ui.allocate_exact_size(vec2(132.0, 30.0), Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(rect, 9, pal.control);
    let minus = Rect::from_min_size(rect.min, vec2(34.0, rect.height()));
    let plus = Rect::from_min_max(pos2(rect.right() - 34.0, rect.top()), rect.max);
    let mut changed = false;
    for (r, sign) in [(minus, -1i64), (plus, 1i64)] {
        let id = ui.scope_id().with(("stepper", sign));
        let resp = ui.interact(r, id, Sense::click());
        let next = (i64::from(*value) + sign * i64::from(step))
            .clamp(i64::from(*range.start()), i64::from(*range.end()));
        let can = next != i64::from(*value);
        if resp.hovered() && can {
            p.rect_filled(r.shrink(2.0), 7, pal.line);
        }
        let ink = if can {
            pal.text
        } else {
            pal.muted.gamma_multiply(0.5)
        };
        let c = r.center();
        p.line_segment(
            [c - vec2(5.0, 0.0), c + vec2(5.0, 0.0)],
            Stroke::new(1.8, ink),
        );
        if sign > 0 {
            p.line_segment(
                [c - vec2(0.0, 5.0), c + vec2(0.0, 5.0)],
                Stroke::new(1.8, ink),
            );
        }
        if resp.clicked() && can {
            *value = u32::try_from(next).unwrap_or(*value);
            changed = true;
        }
    }
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{value}{suffix}"),
        fonts::font(Weight::Regular, 13.5),
        pal.text,
    );
    changed
}

/// How a pill button looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The main action: accent colour, white text.
    Primary,
    /// Everything else.
    Plain,
    /// Something that removes or ends: red text.
    Danger,
}

/// A rounded button.
pub fn button(ui: &mut Ui, pal: &Palette, text: &str, kind: Kind) -> Response {
    let font = fonts::font(Weight::SemiBold, 13.0);
    let ink = match kind {
        Kind::Primary => Color32::WHITE,
        Kind::Plain => pal.text,
        Kind::Danger => pal.danger,
    };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, ink);
    let size = vec2(galley.size().x + 28.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("hot"), resp.hovered(), 0.12);
    let base = match kind {
        Kind::Primary => pal.accent,
        Kind::Plain | Kind::Danger => pal.control,
    };
    let mut fill = base.lerp_to_gamma(
        if pal.dark {
            Color32::WHITE
        } else {
            Color32::BLACK
        },
        0.08 * t,
    );
    if resp.is_pointer_button_down_on() {
        fill = fill.lerp_to_gamma(Color32::BLACK, 0.12);
    }
    if !ui.is_enabled() {
        fill = fill.gamma_multiply(0.5);
    }
    let p = ui.painter();
    p.rect_filled(rect, 15, fill);
    p.galley(rect.center() - galley.size() / 2.0, galley, ink);
    resp
}

/// What a small round button shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Left,
    Right,
    Remove,
}

/// A small round button with a glyph: move left or right, or remove.
pub fn round_button(ui: &mut Ui, pal: &Palette, glyph: Glyph, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
    let p = ui.painter();
    let c = rect.center();
    if glyph == Glyph::Remove {
        let fill = if resp.hovered() {
            pal.danger
        } else {
            pal.danger.gamma_multiply(0.85)
        };
        p.circle_filled(c, 9.0, fill);
        p.line_segment(
            [c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)],
            Stroke::new(2.0, Color32::WHITE),
        );
        return resp;
    }
    if enabled && resp.hovered() {
        p.circle_filled(c, 12.0, pal.line);
    }
    let ink = if enabled {
        pal.text
    } else {
        pal.muted.gamma_multiply(0.4)
    };
    let d = if glyph == Glyph::Left { -1.0 } else { 1.0 };
    p.add(Shape::line(
        vec![
            c + vec2(-2.5 * d, -5.0),
            c + vec2(2.5 * d, 0.0),
            c + vec2(-2.5 * d, 5.0),
        ],
        Stroke::new(1.8, ink),
    ));
    resp
}

/// A one-line text field on a rounded fill.
pub fn text_field(
    ui: &mut Ui,
    pal: &Palette,
    text: &mut String,
    hint: &str,
    width: f32,
) -> Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .desired_width(width)
            .font(fonts::font(Weight::Regular, 13.5))
            .text_color(pal.text)
            .frame(
                egui::Frame::new()
                    .fill(pal.control)
                    .corner_radius(9)
                    .inner_margin(egui::Margin::symmetric(11, 7)),
            ),
    )
}

/// A small rounded tag, like "Online".
pub fn tag(ui: &mut Ui, pal: &Palette, text: &str) -> Response {
    let font = fonts::font(Weight::SemiBold, 10.5);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, pal.muted);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 12.0, 18.0), Sense::hover());
    ui.painter().rect_filled(rect, 9, pal.control);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, pal.muted);
    resp
}

/// A rounded chip that can be picked, for filters.
pub fn chip(ui: &mut Ui, pal: &Palette, text: &str, chosen: bool) -> Response {
    let ink = if chosen { Color32::WHITE } else { pal.text };
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), fonts::font(Weight::Regular, 13.0), ink);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 24.0, 28.0), Sense::click());
    let fill = if chosen {
        pal.accent
    } else if resp.hovered() {
        pal.control.lerp_to_gamma(pal.text, 0.08)
    } else {
        pal.control
    };
    ui.painter().rect_filled(rect, 14, fill);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, ink);
    resp
}

/// The picture on a section's tile in the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionIcon {
    Gear,
    Appearance,
    Taskbar,
    Pin,
    Widgets,
    Cloud,
    Lock,
    Info,
}

/// A section's icon: a small rounded tile in `colour`, lit from above, with a white picture.
pub fn section_icon(p: &Painter, rect: Rect, colour: Color32, icon: SectionIcon) {
    p.add(draw::gradient_fill(
        &draw::rounded_points(rect, rect.width() * 0.26),
        colour.lerp_to_gamma(Color32::WHITE, 0.18),
        colour.lerp_to_gamma(Color32::BLACK, 0.1),
    ));
    let w = Color32::WHITE;
    let c = rect.center();
    let s = rect.width();
    let line = Stroke::new((s * 0.085).max(1.4), w);
    match icon {
        SectionIcon::Gear => {
            for k in 0..8 {
                let a = std::f32::consts::TAU * k as f32 / 8.0;
                let dir = vec2(a.cos(), a.sin());
                p.line_segment(
                    [c + dir * s * 0.2, c + dir * s * 0.37],
                    Stroke::new(s * 0.12, w),
                );
            }
            p.circle_filled(c, s * 0.25, w);
            p.circle_filled(c, s * 0.1, colour);
        }
        SectionIcon::Appearance => {
            let r = s * 0.3;
            p.circle_stroke(c, r, line);
            let half: Vec<_> = (0..=16)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 16.0;
                    c + vec2(a.cos(), a.sin()) * r
                })
                .collect();
            p.add(Shape::convex_polygon(half, w, Stroke::NONE));
        }
        SectionIcon::Taskbar => {
            let screen = Rect::from_center_size(c, vec2(s * 0.62, s * 0.46));
            p.rect_stroke(screen, 2, line, StrokeKind::Middle);
            let bar =
                Rect::from_min_max(pos2(screen.left(), screen.bottom() - s * 0.13), screen.max);
            p.rect_filled(bar, 1, w);
        }
        SectionIcon::Pin => {
            let head = c + vec2(0.0, -s * 0.12);
            p.line_segment([head, c + vec2(0.0, s * 0.33)], line);
            p.circle_filled(head, s * 0.17, w);
        }
        SectionIcon::Widgets => {
            let u = s * 0.14;
            let tall = Rect::from_min_size(c + vec2(-u * 2.2, -u * 2.0), vec2(u * 2.0, u * 4.0));
            let top = Rect::from_min_size(c + vec2(u * 0.2, -u * 2.0), vec2(u * 2.0, u * 1.8));
            let bottom = Rect::from_min_size(c + vec2(u * 0.2, u * 0.2), vec2(u * 2.0, u * 1.8));
            for r in [tall, top, bottom] {
                p.rect_filled(r, u * 0.45, w);
            }
        }
        SectionIcon::Cloud => {
            p.circle_filled(c + vec2(-s * 0.14, s * 0.05), s * 0.15, w);
            p.circle_filled(c + vec2(s * 0.04, -s * 0.04), s * 0.2, w);
            p.circle_filled(c + vec2(s * 0.2, s * 0.08), s * 0.12, w);
            p.rect_filled(
                Rect::from_min_max(c + vec2(-s * 0.14, s * 0.05), c + vec2(s * 0.2, s * 0.2)),
                0,
                w,
            );
        }
        SectionIcon::Lock => {
            let body = Rect::from_center_size(c + vec2(0.0, s * 0.1), vec2(s * 0.5, s * 0.36));
            p.rect_filled(body, 2, w);
            let r = s * 0.15;
            let arc: Vec<_> = (0..=12)
                .map(|i| {
                    let a = std::f32::consts::PI + std::f32::consts::PI * i as f32 / 12.0;
                    pos2(c.x, body.top()) + vec2(a.cos(), a.sin()) * r
                })
                .collect();
            p.add(Shape::line(arc, line));
        }
        SectionIcon::Info => {
            p.circle_filled(c + vec2(0.0, -s * 0.2), s * 0.07, w);
            p.rect_filled(
                Rect::from_center_size(c + vec2(0.0, s * 0.08), vec2(s * 0.12, s * 0.36)),
                s * 0.06,
                w,
            );
        }
    }
}

/// A sidebar entry: the section's tile and its name; filled with the accent when chosen.
pub fn nav_item(
    ui: &mut Ui,
    pal: &Palette,
    label: &str,
    colour: Color32,
    icon: SectionIcon,
    chosen: bool,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("hot"), resp.hovered(), 0.12);
    let p = ui.painter();
    if chosen {
        p.rect_filled(rect, 8, pal.accent);
    } else if t > 0.0 {
        p.rect_filled(rect, 8, pal.line.gamma_multiply(t));
    }
    let tile = Rect::from_min_size(
        pos2(rect.left() + 8.0, rect.center().y - 11.0),
        vec2(22.0, 22.0),
    );
    section_icon(p, tile, colour, icon);
    let ink = if chosen { Color32::WHITE } else { pal.text };
    p.text(
        pos2(tile.right() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        fonts::font(Weight::Regular, 14.0),
        ink,
    );
    resp
}
