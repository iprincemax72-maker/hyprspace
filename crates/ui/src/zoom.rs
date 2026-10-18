use std::cell::Cell;

use gpui::Pixels;

pub const STEPS: [f32; 11] = [0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

thread_local! {
    static ZOOM: Cell<f32> = const { Cell::new(1.0) };
}

pub fn zoom() -> f32 {
    ZOOM.with(Cell::get)
}

pub fn set(z: f32) {
    ZOOM.with(|c| c.set(fit(z)));
}

pub fn px(v: f32) -> Pixels {
    gpui::px(v * zoom())
}

pub fn fit(z: f32) -> f32 {
    if z.is_finite() {
        z.clamp(STEPS[0], STEPS[STEPS.len() - 1])
    } else {
        1.0
    }
}

pub fn step(z: f32, up: bool) -> f32 {
    let next = if up {
        STEPS.iter().find(|s| **s > z + 0.001)
    } else {
        STEPS.iter().rev().find(|s| **s < z - 0.001)
    };
    next.copied().unwrap_or(fit(z))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_like_a_browser() {
        assert_eq!(step(1.0, true), 1.1);
        assert_eq!(step(1.0, false), 0.9);
        assert_eq!(step(2.0, true), 2.0);
        assert_eq!(step(0.5, false), 0.5);
        assert_eq!(step(1.05, true), 1.1);
        assert_eq!(fit(f32::NAN), 1.0);
    }
}
