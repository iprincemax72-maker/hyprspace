use gpui::{
    MouseButton, MouseMoveEvent, Pixels, Point, ScrollDelta, ScrollWheelEvent, Window, point, px,
};

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 10.;
/// Each wheel line zooms by this much; a mouse wheel's notch is three lines on Windows, about a
/// fifth. A touchpad's pixels count as lines this many to one.
const STEP: f32 = 1.06;
const PIXELS_PER_LINE: f32 = 20.;
/// The share of the way to the wheel's zoom each frame covers.
const EASE: f32 = 0.28;
const CLICK_SLOP: f32 = 4.;

pub(crate) struct Pan {
    pub zoom: f32,
    /// The zoom the wheel is heading to, and the point it zooms around.
    target: f32,
    anchor: Point<Pixels>,
    /// How far the picture's center sits from where it opened.
    pub offset: Point<Pixels>,
    /// While dragging: where the pointer went down, and the offset then.
    drag: Option<(Point<Pixels>, Point<Pixels>)>,
    /// The picture's center before any move, as of the last frame.
    pub center: Point<Pixels>,
}

/// The offset after zooming from `from` to `to` around `anchor`, so the point under it stays put.
/// `center` is the image's center before the zoom.
fn zoom_around(
    offset: Point<Pixels>,
    center: Point<Pixels>,
    anchor: Point<Pixels>,
    from: f32,
    to: f32,
) -> Point<Pixels> {
    let k = 1. - to / from;
    point(
        offset.x + (anchor.x - center.x) * k,
        offset.y + (anchor.y - center.y) * k,
    )
}

impl Pan {
    pub fn new() -> Self {
        Self {
            zoom: 1.,
            target: 1.,
            anchor: point(px(0.), px(0.)),
            offset: point(px(0.), px(0.)),
            drag: None,
            center: point(px(0.), px(0.)),
        }
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub fn wheel(&mut self, e: &ScrollWheelEvent) {
        let lines = match e.delta {
            ScrollDelta::Lines(d) => d.y,
            ScrollDelta::Pixels(d) => f32::from(d.y) / PIXELS_PER_LINE,
        };
        self.target = (self.target * STEP.powf(lines)).clamp(MIN_ZOOM, MAX_ZOOM);
        self.anchor = e.position;
    }

    /// One frame of easing toward the wheel's zoom.
    pub fn ease(&mut self, window: &mut Window) {
        if self.zoom == self.target {
            return;
        }
        let near = (self.target - self.zoom).abs() < 0.002;
        let next = if near {
            self.target
        } else {
            self.zoom + (self.target - self.zoom) * EASE
        };
        let center = point(self.center.x + self.offset.x, self.center.y + self.offset.y);
        self.offset = zoom_around(self.offset, center, self.anchor, self.zoom, next);
        self.zoom = next;
        if !near {
            window.request_animation_frame();
        }
    }

    pub fn fit(&mut self) {
        self.zoom = 1.;
        self.target = 1.;
        self.offset = point(px(0.), px(0.));
        self.drag = None;
    }

    pub fn grab(&mut self, at: Point<Pixels>) {
        self.drag = Some((at, self.offset));
    }

    pub fn drag(&mut self, e: &MouseMoveEvent) -> bool {
        let Some((from, start)) = self.drag else {
            return false;
        };
        // a button let go outside the window ends the drag at the next move
        if e.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
        } else {
            self.offset = point(
                start.x + e.position.x - from.x,
                start.y + e.position.y - from.y,
            );
        }
        true
    }

    /// Ends a drag. True when the pointer barely moved, so it was a click.
    pub fn release(&mut self, at: Point<Pixels>) -> bool {
        let Some((from, _)) = self.drag.take() else {
            return false;
        };
        f32::from(at.x - from.x).abs() < CLICK_SLOP && f32::from(at.y - from.y).abs() < CLICK_SLOP
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zooming_keeps_the_point_under_the_pointer_still() {
        let center = point(px(500.), px(400.));
        let anchor = point(px(600.), px(450.));
        let offset = zoom_around(point(px(0.), px(0.)), center, anchor, 1., 2.);
        // the image point under the anchor was 100,50 from its center; at 2x it is 200,100, and
        // the new center sits that far back from the anchor
        assert_eq!(offset, point(px(-100.), px(-50.)));
        let back = zoom_around(offset, center + offset, anchor, 2., 1.);
        assert_eq!(back, point(px(0.), px(0.)));
    }

    #[test]
    fn a_still_press_is_a_click_and_a_move_is_not() {
        let mut pan = Pan::new();
        pan.grab(point(px(100.), px(100.)));
        assert!(pan.release(point(px(102.), px(99.))));
        pan.grab(point(px(100.), px(100.)));
        assert!(!pan.release(point(px(130.), px(100.))));
        assert!(!pan.release(point(px(100.), px(100.))));
    }
}
