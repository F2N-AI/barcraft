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

/// A battery outline filled to `fraction`, with a bolt while charging.
pub fn battery(p: &Painter, rect: Rect, fraction: f32, charging: bool, c: Color32, fill: Color32) {
    let h = rect.height().min(rect.width()) * 0.42;
    let w = h * 1.9;
    let body = Rect::from_center_size(rect.center() - vec2(h * 0.08, 0.0), vec2(w, h));
    p.rect_stroke(body, 2.5, Stroke::new(1.4, c), egui::StrokeKind::Inside);
    let nub = Rect::from_min_size(
        pos2(body.right() + 1.0, body.center().y - h * 0.18),
        vec2(h * 0.14, h * 0.36),
    );
    p.rect_filled(nub, 1.0, c);
    let inner = body.shrink(2.6);
    let f = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let level = Rect::from_min_size(inner.min, vec2(inner.width() * f, inner.height()));
    p.rect_filled(level, 1.2, fill);
    if charging {
        let m = body.center();
        let bolt = vec![
            pos2(m.x + h * 0.08, m.y - h * 0.42),
            pos2(m.x - h * 0.22, m.y + h * 0.06),
            pos2(m.x + h * 0.02, m.y + h * 0.06),
            pos2(m.x - h * 0.08, m.y + h * 0.42),
            pos2(m.x + h * 0.22, m.y - h * 0.06),
            pos2(m.x - h * 0.02, m.y - h * 0.06),
        ];
        p.add(Shape::closed_line(bolt, Stroke::new(1.2, c)));
    }
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
