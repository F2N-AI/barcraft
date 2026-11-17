//! Icons drawn in code, so BarCraft ships no artwork of anyone else's.

use std::f32::consts::{PI, TAU};

use barcraft_core::Rgba;
use barcraft_core::weather::Sky;
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

pub fn color(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0, c.1, c.2, c.3)
}

/// A 3×3 grid of dots: the button that opens the Start menu.
pub fn launcher(p: &Painter, rect: Rect, c: Color32) {
    let side = rect.width().min(rect.height()) * 0.5;
    let step = side / 2.0;
    let r = side * 0.12;
    let origin = rect.center() - vec2(step, step);
    for row in 0..3 {
        for col in 0..3 {
            p.circle_filled(origin + vec2(col as f32 * step, row as f32 * step), r, c);
        }
    }
}

/// A screen with a dashed line under it: "show desktop".
pub fn desktop(p: &Painter, rect: Rect, c: Color32) {
    let s = rect.width().min(rect.height()) * 0.5;
    let screen = Rect::from_center_size(rect.center() - vec2(0.0, s * 0.12), vec2(s, s * 0.68));
    p.rect_stroke(screen, 2.0, Stroke::new(1.5, c), egui::StrokeKind::Inside);
    let y = screen.bottom() + s * 0.22;
    p.line_segment(
        [
            pos2(screen.left() + s * 0.2, y),
            pos2(screen.right() - s * 0.2, y),
        ],
        Stroke::new(1.5, c),
    );
}

/// A speaker with one to three waves, or a cross when muted.
pub fn speaker(p: &Painter, rect: Rect, level: f32, muted: bool, c: Color32) {
    let s = rect.height().min(rect.width()) * 0.62;
    let left = rect.center().x - s * 0.45;
    let cy = rect.center().y;
    let body = vec![
        pos2(left, cy - s * 0.16),
        pos2(left + s * 0.18, cy - s * 0.16),
        pos2(left + s * 0.42, cy - s * 0.38),
        pos2(left + s * 0.42, cy + s * 0.38),
        pos2(left + s * 0.18, cy + s * 0.16),
        pos2(left, cy + s * 0.16),
    ];
    p.add(Shape::convex_polygon(body, c, Stroke::NONE));
    let cx = left + s * 0.42;
    if muted {
        let x0 = cx + s * 0.18;
        let d = s * 0.2;
        let stroke = Stroke::new(1.6, c);
        p.line_segment([pos2(x0, cy - d), pos2(x0 + 2.0 * d, cy + d)], stroke);
        p.line_segment([pos2(x0, cy + d), pos2(x0 + 2.0 * d, cy - d)], stroke);
        return;
    }
    let waves = if level <= 0.01 {
        0
    } else if level < 0.34 {
        1
    } else if level < 0.67 {
        2
    } else {
        3
    };
    for i in 0..waves {
        let r = s * (0.2 + 0.17 * i as f32);
        arc(
            p,
            pos2(cx, cy),
            r,
            -0.28 * PI,
            0.28 * PI,
            Stroke::new(1.5, c),
        );
    }
}

fn arc(p: &Painter, center: Pos2, r: f32, from: f32, to: f32, stroke: Stroke) {
    let steps = 12;
    let points: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = from + (to - from) * i as f32 / steps as f32;
            center + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    p.add(Shape::line(points, stroke));
}

/// A small line chart of values between 0 and 1, filled underneath.
pub fn sparkline(p: &Painter, rect: Rect, values: &[f32], capacity: usize, line: Color32) {
    if values.len() < 2 || capacity < 2 {
        return;
    }
    let step = rect.width() / (capacity - 1) as f32;
    let start = rect.right() - step * (values.len() - 1) as f32;
    let points: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            pos2(
                start + step * i as f32,
                rect.bottom() - rect.height() * v.clamp(0.0, 1.0),
            )
        })
        .collect();
    let fill = line.gamma_multiply(0.18);
    for pair in points.windows(2) {
        if let [a, b] = pair {
            let quad = vec![*a, *b, pos2(b.x, rect.bottom()), pos2(a.x, rect.bottom())];
            p.add(Shape::convex_polygon(quad, fill, Stroke::NONE));
        }
    }
    p.add(Shape::line(points, Stroke::new(1.3, line)));
}

/// A simple picture of the sky.
pub fn sky(p: &Painter, rect: Rect, sky: Sky, is_day: bool, c: Color32, accent: Color32) {
    let s = rect.height().min(rect.width()) * 0.62;
    let m = rect.center();
    let sun_or_moon = |p: &Painter, at: Pos2, r: f32| {
        if is_day {
            p.circle_filled(at, r, accent);
            for i in 0..8 {
                let a = TAU * i as f32 / 8.0;
                let d = vec2(a.cos(), a.sin());
                p.line_segment(
                    [at + d * r * 1.35, at + d * r * 1.75],
                    Stroke::new(1.3, accent),
                );
            }
        } else {
            p.circle_filled(at, r, accent);
            p.circle_filled(
                at + vec2(r * 0.45, -r * 0.3),
                r * 0.85,
                Color32::TRANSPARENT,
            );
        }
    };
    let cloud = |p: &Painter, at: Pos2, w: f32, fill: Color32| {
        p.circle_filled(at + vec2(-w * 0.22, w * 0.05), w * 0.22, fill);
        p.circle_filled(at + vec2(w * 0.05, -w * 0.08), w * 0.3, fill);
        p.circle_filled(at + vec2(w * 0.3, w * 0.07), w * 0.2, fill);
        p.rect_filled(
            Rect::from_min_max(at + vec2(-w * 0.22, w * 0.02), at + vec2(w * 0.3, w * 0.27)),
            0.0,
            fill,
        );
    };
    match sky {
        Sky::Clear => sun_or_moon(p, m, s * 0.28),
        Sky::PartlyCloudy => {
            sun_or_moon(p, m + vec2(-s * 0.15, -s * 0.15), s * 0.2);
            cloud(p, m + vec2(s * 0.08, s * 0.05), s * 0.75, c);
        }
        Sky::Cloudy | Sky::Unknown => cloud(p, m, s * 0.9, c),
        Sky::Fog => {
            for i in 0..3 {
                let y = m.y - s * 0.25 + s * 0.25 * i as f32;
                p.line_segment(
                    [pos2(m.x - s * 0.4, y), pos2(m.x + s * 0.4, y)],
                    Stroke::new(1.6, c),
                );
            }
        }
        Sky::Drizzle | Sky::Rain | Sky::Storm | Sky::Snow => {
            cloud(p, m + vec2(0.0, -s * 0.18), s * 0.85, c);
            let base = m.y + s * 0.18;
            for i in 0..3 {
                let x = m.x - s * 0.25 + s * 0.25 * i as f32;
                match sky {
                    Sky::Snow => {
                        p.circle_filled(pos2(x, base + s * 0.1), s * 0.05, c);
                    }
                    Sky::Storm if i == 1 => {
                        let bolt = vec![
                            pos2(x + s * 0.05, base - s * 0.02),
                            pos2(x - s * 0.08, base + s * 0.17),
                            pos2(x + s * 0.04, base + s * 0.17),
                            pos2(x - s * 0.06, base + s * 0.36),
                        ];
                        p.add(Shape::line(bolt, Stroke::new(1.5, accent)));
                    }
                    _ => {
                        let len = if matches!(sky, Sky::Drizzle) {
                            0.1
                        } else {
                            0.2
                        };
                        p.line_segment(
                            [pos2(x + s * 0.04, base), pos2(x - s * 0.04, base + s * len)],
                            Stroke::new(1.4, c),
                        );
                    }
                }
            }
        }
    }
}

/// A rounded tile with the first letter of the name, for apps whose icon can't be read.
pub fn letter_tile(p: &Painter, rect: Rect, name: &str, text: Color32, tile: Color32) {
    p.rect_filled(rect.shrink(rect.width() * 0.06), rect.width() * 0.22, tile);
    let letter: String = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".into());
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::proportional(rect.height() * 0.5),
        text,
    );
}

/// A ring filled clockwise from the top to `fraction`.
pub fn ring(
    p: &Painter,
    center: Pos2,
    radius: f32,
    fraction: f32,
    width: f32,
    track: Color32,
    fill: Color32,
) {
    p.circle_stroke(center, radius, Stroke::new(width, track));
    let f = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if f <= 0.0 {
        return;
    }
    let start = -PI / 2.0;
    let steps = ((48.0 * f).ceil() as usize).max(2);
    let points: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = start + TAU * f * i as f32 / steps as f32;
            center + vec2(a.cos(), a.sin()) * radius
        })
        .collect();
    p.add(Shape::line(points, Stroke::new(width, fill)));
}

/// A thin progress bar with rounded ends.
pub fn bar(p: &Painter, rect: Rect, fraction: f32, track: Color32, fill: Color32) {
    let r = rect.height() / 2.0;
    p.rect_filled(rect, r, track);
    let f = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if f > 0.0 {
        let w = (rect.width() * f).max(rect.height());
        p.rect_filled(
            Rect::from_min_size(rect.min, vec2(w, rect.height())),
            r,
            fill,
        );
    }
}

/// A chevron pointing up (or down): "more icons this way".
pub fn chevron(p: &Painter, rect: Rect, up: bool, c: Color32) {
    let s = rect.width().min(rect.height()) * 0.18;
    let m = rect.center();
    let dy = if up { s * 0.5 } else { -s * 0.5 };
    let points = vec![
        pos2(m.x - s, m.y + dy),
        pos2(m.x, m.y - dy),
        pos2(m.x + s, m.y + dy),
    ];
    p.add(Shape::line(points, Stroke::new(1.8, c)));
}

/// A bell: notifications.
pub fn bell(p: &Painter, rect: Rect, c: Color32) {
    let s = rect.width().min(rect.height()) * 0.42;
    let m = rect.center() - vec2(0.0, s * 0.08);
    let stroke = Stroke::new(1.5, c);
    let left = m.x - s * 0.38;
    let right = m.x + s * 0.38;
    let top = m.y - s * 0.32;
    let base = m.y + s * 0.3;
    arc(
        p,
        pos2(m.x, top + s * 0.38 * 0.0 + s * 0.06),
        s * 0.38,
        PI,
        TAU,
        stroke,
    );
    p.line_segment([pos2(left, top + s * 0.06), pos2(left, base)], stroke);
    p.line_segment([pos2(right, top + s * 0.06), pos2(right, base)], stroke);
    p.line_segment(
        [pos2(left - s * 0.14, base), pos2(right + s * 0.14, base)],
        stroke,
    );
    p.circle_filled(pos2(m.x, base + s * 0.2), s * 0.08, c);
}

/// A bin, a little fuller when it holds something.
pub fn bin(p: &Painter, rect: Rect, full: bool, c: Color32) {
    let s = rect.width().min(rect.height()) * 0.42;
    let m = rect.center();
    let stroke = Stroke::new(1.5, c);
    let top = m.y - s * 0.3;
    let bottom = m.y + s * 0.45;
    let body = vec![
        pos2(m.x - s * 0.32, top),
        pos2(m.x + s * 0.32, top),
        pos2(m.x + s * 0.24, bottom),
        pos2(m.x - s * 0.24, bottom),
    ];
    if full {
        p.add(Shape::convex_polygon(
            body.clone(),
            c.gamma_multiply(0.35),
            Stroke::NONE,
        ));
    }
    p.add(Shape::closed_line(body, stroke));
    p.line_segment(
        [
            pos2(m.x - s * 0.42, top - s * 0.1),
            pos2(m.x + s * 0.42, top - s * 0.1),
        ],
        stroke,
    );
    p.line_segment(
        [
            pos2(m.x - s * 0.1, top - s * 0.22),
            pos2(m.x + s * 0.1, top - s * 0.22),
        ],
        stroke,
    );
}

/// A microphone, crossed out when muted.
pub fn microphone(p: &Painter, rect: Rect, muted: bool, c: Color32) {
    let s = rect.width().min(rect.height()) * 0.42;
    let m = rect.center() - vec2(0.0, s * 0.1);
    let stroke = Stroke::new(1.5, c);
    let capsule = Rect::from_center_size(m - vec2(0.0, s * 0.12), vec2(s * 0.36, s * 0.62));
    p.rect_stroke(capsule, s * 0.18, stroke, egui::StrokeKind::Middle);
    arc(p, m, s * 0.34, 0.0, PI, stroke);
    p.line_segment(
        [pos2(m.x, m.y + s * 0.34), pos2(m.x, m.y + s * 0.55)],
        stroke,
    );
    if muted {
        p.line_segment(
            [
                pos2(m.x - s * 0.45, m.y - s * 0.5),
                pos2(m.x + s * 0.45, m.y + s * 0.5),
            ],
            Stroke::new(1.8, c),
        );
    }
}

/// A small filled triangle pointing up (or down), for upload and download.
pub fn arrow(p: &Painter, center: Pos2, size: f32, up: bool, c: Color32) {
    let h = size * 0.5;
    let points = if up {
        vec![
            pos2(center.x, center.y - h),
            pos2(center.x + h, center.y + h * 0.6),
            pos2(center.x - h, center.y + h * 0.6),
        ]
    } else {
        vec![
            pos2(center.x, center.y + h),
            pos2(center.x - h, center.y - h * 0.6),
            pos2(center.x + h, center.y - h * 0.6),
        ]
    };
    p.add(Shape::convex_polygon(points, c, Stroke::NONE));
}

#[derive(Clone, Copy)]
pub enum MediaGlyph {
    Play,
    Pause,
    Next,
    Previous,
}

/// Play, pause, next and previous signs.
pub fn media_glyph(p: &Painter, rect: Rect, glyph: MediaGlyph, c: Color32) {
    let s = rect.width().min(rect.height()) * 0.28;
    let m = rect.center();
    let tri = |dir: f32, at: Pos2| {
        vec![
            pos2(at.x - s * 0.5 * dir, at.y - s * 0.6),
            pos2(at.x + s * 0.6 * dir, at.y),
            pos2(at.x - s * 0.5 * dir, at.y + s * 0.6),
        ]
    };
    match glyph {
        MediaGlyph::Play => {
            p.add(Shape::convex_polygon(
                tri(1.0, m + vec2(s * 0.1, 0.0)),
                c,
                Stroke::NONE,
            ));
        }
        MediaGlyph::Pause => {
            for dx in [-0.3, 0.3] {
                p.rect_filled(
                    Rect::from_center_size(m + vec2(s * dx, 0.0), vec2(s * 0.32, s * 1.2)),
                    1.0,
                    c,
                );
            }
        }
        MediaGlyph::Next | MediaGlyph::Previous => {
            let dir = if matches!(glyph, MediaGlyph::Next) {
                1.0
            } else {
                -1.0
            };
            p.add(Shape::convex_polygon(
                tri(dir, m - vec2(s * 0.25 * dir, 0.0)),
                c,
                Stroke::NONE,
            ));
            let bar_x = m.x + s * 0.55 * dir;
            p.rect_filled(
                Rect::from_center_size(pos2(bar_x, m.y), vec2(s * 0.22, s * 1.2)),
                0.5,
                c,
            );
        }
    }
}

/// An analog clock face.
/// `time` is (hour, minute, second if shown).
pub fn analog_clock(
    p: &Painter,
    rect: Rect,
    time: (u32, u32, Option<u32>),
    face: Color32,
    ink: Color32,
    accent: Color32,
) {
    let (hour, minute, second) = time;
    let r = rect.width().min(rect.height()) * 0.4;
    let m = rect.center();
    p.circle_filled(m, r, face);
    for i in 0..12 {
        let a = TAU * i as f32 / 12.0 - PI / 2.0;
        let d = vec2(a.cos(), a.sin());
        let inner = if i % 3 == 0 { 0.74 } else { 0.84 };
        p.line_segment(
            [m + d * r * inner, m + d * r * 0.92],
            Stroke::new(1.0, ink.gamma_multiply(0.7)),
        );
    }
    let hand = |turns: f32, len: f32, width: f32, c: Color32| {
        let a = TAU * turns - PI / 2.0;
        p.line_segment(
            [m, m + vec2(a.cos(), a.sin()) * r * len],
            Stroke::new(width, c),
        );
    };
    let minutes = minute as f32 + second.unwrap_or(0) as f32 / 60.0;
    hand(((hour % 12) as f32 + minutes / 60.0) / 12.0, 0.5, 2.2, ink);
    hand(minutes / 60.0, 0.78, 1.6, ink);
    if let Some(s) = second {
        hand(s as f32 / 60.0, 0.84, 1.0, accent);
    }
    p.circle_filled(m, 2.0, ink);
}

/// The light over a cell's upper half and the colour of its thin edge.
fn cell_light(theme: &barcraft_core::Theme) -> (Color32, Color32) {
    // On glass, cells catch the light: a brighter edge reads better over a blurred desktop.
    match (theme.glass, theme.dark) {
        (true, true) => (Color32::from_white_alpha(10), Color32::from_white_alpha(34)),
        (true, false) => (
            Color32::from_white_alpha(60),
            Color32::from_white_alpha(140),
        ),
        (false, true) => (Color32::from_white_alpha(7), Color32::from_white_alpha(22)),
        (false, false) => (Color32::from_white_alpha(70), Color32::from_black_alpha(26)),
    }
}

/// Just the thin edge of a cell, to draw again over something painted inside it.
pub fn cell_edge(p: &Painter, rect: Rect, radius: f32, theme: &barcraft_core::Theme) {
    let r = radius.round().clamp(0.0, 255.0) as u8;
    let (_, edge) = cell_light(theme);
    p.rect_stroke(rect, r, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
}

/// Black or white, whichever reads better on `c`.
pub fn ink_on(c: Color32) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    let lum = 0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b);
    if lum > 150.0 {
        Color32::from_rgb(20, 22, 27)
    } else {
        Color32::WHITE
    }
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        ch(a.r(), b.r()),
        ch(a.g(), b.g()),
        ch(a.b(), b.b()),
        ch(a.a(), b.a()),
    )
}

/// How a liquid looks: its colour (opaque), how see-through it is, and where its waves are.
#[derive(Clone, Copy, Debug)]
pub struct Liquid {
    pub colour: Color32,
    /// 1 for a solid liquid, less to let what's behind (and text over it) show.
    pub opacity: f32,
    /// Moves the waves along, in radians; keep it still for a liquid at rest.
    pub phase: f32,
}

/// Liquid filling a rounded rectangle to `level` (0 to 1), its surface a gentle double wave.
pub fn liquid(p: &Painter, rect: Rect, radius: f32, level: f32, look: Liquid) {
    p.add(Shape::Vec(liquid_shapes(rect, radius, level, look)));
}

/// The shapes of [`liquid`], to paint underneath something drawn earlier.
pub fn liquid_shapes(rect: Rect, radius: f32, level: f32, look: Liquid) -> Vec<Shape> {
    let mut shapes = Vec::with_capacity(3);
    let level = if level.is_finite() {
        level.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (w, h) = (rect.width(), rect.height());
    if level <= 0.0 || w <= 1.0 || h <= 1.0 {
        return shapes;
    }
    let (c, phase) = (look.colour, look.phase);
    let opacity = look.opacity.clamp(0.0, 1.0);
    let r = radius.clamp(0.0, w.min(h) / 2.0);
    // The rounded rectangle's top and bottom at `x`.
    let bounds = |x: f32| -> (f32, f32) {
        let inset = if x < rect.left() + r {
            rect.left() + r - x
        } else if x > rect.right() - r {
            x - (rect.right() - r)
        } else {
            0.0
        };
        let lift = r - (r * r - inset.min(r).powi(2)).max(0.0).sqrt();
        (rect.top() + lift, rect.bottom() - lift)
    };
    let surface = rect.bottom() - h * level;
    // Smaller waves near empty and full, so they never spill out of the shape.
    let room = (surface - rect.top()).min(rect.bottom() - surface);
    let amp = (h * 0.07).min(2.4).min(room.max(0.0));
    let top_colour = mix(c, Color32::WHITE, 0.28);
    // Waves about as long as the shape is tall, so a wide card gets several, not one swell.
    let wave_len = w.min(h * 1.6);
    let steps = ((w / 3.0).ceil() as usize).clamp(24, 96);
    // The paler wave behind, a little higher and out of step, then the main one.
    let waves = [
        (
            amp * 0.85,
            phase + 2.1,
            0.85,
            top_colour.gamma_multiply(0.45),
            0.45,
        ),
        (amp, phase, 1.15, top_colour, 1.0),
    ];
    for (k, &(a, ph, length, top_c, alpha)) in waves.iter().enumerate() {
        let mean = if k == 0 { surface - a * 0.6 } else { surface };
        let bottom_c = c.gamma_multiply(alpha * opacity);
        let top_c = top_c.gamma_multiply(opacity);
        let mut mesh = egui::Mesh::default();
        let mut crest = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let x = rect.left() + w * i as f32 / steps as f32;
            let (lo, hi) = bounds(x);
            let wave = mean + a * (TAU * (x - rect.left()) / (wave_len * length) + ph).sin();
            let y_top = wave.max(lo).min(hi);
            let shade = ((y_top - rect.top()) / h).clamp(0.0, 1.0);
            mesh.colored_vertex(pos2(x, y_top), mix(top_c, bottom_c, shade * 0.5));
            mesh.colored_vertex(pos2(x, hi), bottom_c);
            crest.push(pos2(x, y_top));
            if i > 0 {
                let j = (2 * i) as u32;
                mesh.add_triangle(j - 2, j - 1, j);
                mesh.add_triangle(j - 1, j + 1, j);
            }
        }
        shapes.push(Shape::mesh(mesh));
        // A bright line along the main surface; it also smooths the wave's edge.
        if k == 1 && level < 0.97 {
            let line = mix(top_colour, Color32::WHITE, 0.35).gamma_multiply(opacity.max(0.5));
            shapes.push(Shape::line(crest, Stroke::new(1.2, line)));
        }
    }
    shapes
}

/// A cell's background: its fill, a faint light over the upper half and a thin edge, so cells
/// read as separate tiles on the bar.
pub fn cell(p: &Painter, rect: Rect, radius: f32, fill: Color32, theme: &barcraft_core::Theme) {
    let r = radius.round().clamp(0.0, 255.0) as u8;
    p.rect_filled(rect, r, fill);
    let (sheen, edge) = cell_light(theme);
    let upper = Rect::from_min_max(rect.min, pos2(rect.right(), rect.center().y));
    p.rect_filled(
        upper,
        egui::CornerRadius {
            nw: r,
            ne: r,
            sw: 0,
            se: 0,
        },
        sheen,
    );
    p.rect_stroke(rect, r, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
}

/// A lightning bolt, for charging.
pub fn bolt(p: &Painter, center: Pos2, height: f32, c: Color32) {
    let h = height / 2.0;
    let upper = vec![
        pos2(center.x + h * 0.25, center.y - h),
        pos2(center.x - h * 0.45, center.y + h * 0.12),
        pos2(center.x + h * 0.05, center.y + h * 0.12),
    ];
    let lower = vec![
        pos2(center.x - h * 0.05, center.y - h * 0.12),
        pos2(center.x + h * 0.45, center.y - h * 0.12),
        pos2(center.x - h * 0.25, center.y + h),
    ];
    p.add(Shape::convex_polygon(upper, c, Stroke::NONE));
    p.add(Shape::convex_polygon(lower, c, Stroke::NONE));
}
