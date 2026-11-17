//! Icon magnification, as a lens over the row of app icons.
//!
//! The row is laid out at its normal size; the lens only changes where each icon is drawn and
//! how big. Near the pointer the row is stretched, so the icons there grow and push their
//! neighbours aside; further away nothing changes size and every icon moves by the same amount.
//!
//! The point under the pointer never moves, which keeps clicks honest: the icon drawn under
//! the pointer is always the one whose (unmagnified) cell is under it. And because the far
//! shift depends only on how far the lens is faded in, not on where the pointer is, the row
//! needs the same extra room wherever the pointer goes.

use std::f32::consts::PI;

/// A magnifying lens over a row of icons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lens {
    /// How big the icon right under the pointer gets, e.g. 1.6 for 60 % larger.
    pub peak: f32,
    /// How far either side of the pointer the lens reaches, in the row's units (points).
    pub reach: f32,
}

impl Lens {
    /// How much bigger than normal things are at `x`, with the pointer at `pointer` and the lens
    /// faded in by `amount` (0 = off, 1 = fully on).
    pub fn scale(&self, x: f32, pointer: f32, amount: f32) -> f32 {
        1.0 + self.strength(amount) * falloff(self.normalised(x - pointer))
    }

    /// Where the point at `x` of the unmagnified row is drawn.
    pub fn position(&self, x: f32, pointer: f32, amount: f32) -> f32 {
        let d = x - pointer;
        let w = self.normalised(d);
        // The stretch integrated from the pointer out to `x`.
        let stretch = self.strength(amount) * self.reach * 0.5 * (w + (PI * w).sin() / PI);
        x + d.signum() * stretch
    }

    /// How far the lens pushes everything beyond its reach outward, on each side. The row needs
    /// this much spare room at both ends so the outer icons don't cover what is next to it.
    pub fn spread(&self, amount: f32) -> f32 {
        self.strength(amount) * self.reach * 0.5
    }

    fn strength(&self, amount: f32) -> f32 {
        let peak = if self.peak.is_finite() {
            self.peak
        } else {
            1.0
        };
        let amount = if amount.is_finite() { amount } else { 0.0 };
        (peak - 1.0).max(0.0) * amount.clamp(0.0, 1.0)
    }

    /// Distance from the pointer as a share of the reach, 0 at the pointer and 1 at the rim.
    fn normalised(&self, d: f32) -> f32 {
        if self.reach > 0.0 && d.is_finite() {
            (d.abs() / self.reach).min(1.0)
        } else {
            1.0
        }
    }
}

/// 1 at the pointer, easing to 0 at the rim of the lens with no corner at either end.
fn falloff(w: f32) -> f32 {
    ((PI * w).cos() + 1.0) * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENS: Lens = Lens {
        peak: 1.6,
        reach: 120.0,
    };

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn the_point_under_the_pointer_stays_put() {
        for pointer in [-50.0, 0.0, 33.3, 400.0] {
            assert!(close(LENS.position(pointer, pointer, 1.0), pointer));
        }
    }

    #[test]
    fn the_icon_under_the_pointer_is_the_largest() {
        assert!(close(LENS.scale(200.0, 200.0, 1.0), 1.6));
        assert!(LENS.scale(230.0, 200.0, 1.0) < 1.6);
        assert!(LENS.scale(230.0, 200.0, 1.0) > LENS.scale(260.0, 200.0, 1.0));
        assert!(close(LENS.scale(200.0 + 120.0, 200.0, 1.0), 1.0));
        assert!(close(LENS.scale(200.0 - 500.0, 200.0, 1.0), 1.0));
    }

    #[test]
    fn neighbours_move_aside_and_order_is_kept() {
        let pointer = 100.0;
        let mut last = f32::NEG_INFINITY;
        let mut x = -300.0;
        while x <= 500.0 {
            let p = LENS.position(x, pointer, 1.0);
            assert!(p > last, "drawn order changed at {x}");
            last = p;
            x += 2.5;
        }
        // Left of the pointer things move left, right of it they move right.
        assert!(LENS.position(70.0, pointer, 1.0) < 70.0);
        assert!(LENS.position(130.0, pointer, 1.0) > 130.0);
    }

    #[test]
    fn beyond_the_reach_everything_moves_by_the_spread() {
        let spread = LENS.spread(1.0);
        assert!(close(spread, 0.6 * 120.0 * 0.5));
        for pointer in [0.0, 57.0, 300.0] {
            assert!(close(
                LENS.position(pointer + 500.0, pointer, 1.0),
                pointer + 500.0 + spread
            ));
            assert!(close(
                LENS.position(pointer - 500.0, pointer, 1.0),
                pointer - 500.0 - spread
            ));
        }
    }

    #[test]
    fn faded_out_or_flat_lenses_change_nothing() {
        let flat = Lens {
            peak: 1.0,
            reach: 120.0,
        };
        for x in [-100.0, 0.0, 90.0] {
            assert!(close(LENS.position(x, 10.0, 0.0), x));
            assert!(close(LENS.scale(x, 10.0, 0.0), 1.0));
            assert!(close(flat.position(x, 10.0, 1.0), x));
        }
        assert!(close(LENS.spread(0.0), 0.0));
        assert!(close(LENS.spread(0.5), LENS.spread(1.0) / 2.0));
    }

    #[test]
    fn bad_numbers_fall_back_to_no_magnification() {
        let broken = Lens {
            peak: f32::NAN,
            reach: 0.0,
        };
        assert!(close(broken.scale(5.0, 5.0, 1.0), 1.0));
        assert!(close(broken.position(5.0, 0.0, 1.0), 5.0));
        assert!(close(LENS.scale(5.0, 5.0, f32::NAN), 1.0));
        assert!(LENS.position(f32::NAN, 0.0, 1.0).is_nan());
    }
}
