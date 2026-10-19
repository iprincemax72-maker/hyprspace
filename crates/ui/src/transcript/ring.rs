use gpui::{AnyElement, PathBuilder, Pixels, Point, canvas, div, point, prelude::*};

use crate::colors;
use crate::pace::Looping;
use crate::zoom::px;

const STEP: f32 = 16.;

pub(super) fn says_ultrathink(text: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let text = text.to_lowercase();
    text.match_indices("ultrathink")
        .any(|(at, m)| !text[..at].ends_with(word) && !text[at + m.len()..].starts_with(word))
}

pub(super) fn ring(radius: f32, turning: bool) -> AnyElement {
    let frame = div().absolute().inset_0();
    if turning {
        frame
            .looping(3000, move |d, t| d.child(band(radius, t)))
            .into_any_element()
    } else {
        frame.child(band(radius, 0.)).into_any_element()
    }
}

fn band(radius: f32, turn: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let half = f32::from(px(2.)) / 2.;
            let r = (f32::from(px(radius)) - half).max(0.);
            let pts = outline(
                [
                    f32::from(bounds.left()) + half,
                    f32::from(bounds.top()) + half,
                ],
                [
                    f32::from(bounds.right()) - half,
                    f32::from(bounds.bottom()) - half,
                ],
                r,
            );
            let n = pts.len();
            let lengths: Vec<f32> = (0..n).map(|i| dist(pts[i].0, pts[(i + 1) % n].0)).collect();
            let total = lengths.iter().sum::<f32>().max(1.);
            let mut along = 0.;
            for i in 0..n {
                let ((p, pn), (q, qn)) = (pts[i], pts[(i + 1) % n]);
                let mid = (along + lengths[i] / 2.) / total;
                along += lengths[i];
                let len = lengths[i].max(0.001);
                // overlap the next piece a little, or seams show between them
                let reach = [
                    q[0] + (q[0] - p[0]) / len * 0.6,
                    q[1] + (q[1] - p[1]) / len * 0.6,
                ];
                let mut b = PathBuilder::fill();
                b.move_to(at(p, pn, half));
                b.line_to(at(reach, qn, half));
                b.line_to(at(reach, qn, -half));
                b.line_to(at(p, pn, -half));
                b.close();
                if let Ok(path) = b.build() {
                    window.paint_path(path, colors::rainbow((mid - turn).rem_euclid(1.) * 360.));
                }
            }
        },
    )
    .size_full()
}

fn outline(min: [f32; 2], max: [f32; 2], r: f32) -> Vec<([f32; 2], [f32; 2])> {
    let corners = [
        (max[0] - r, min[1] + r, -90f32),
        (max[0] - r, max[1] - r, 0.),
        (min[0] + r, max[1] - r, 90.),
        (min[0] + r, min[1] + r, 180.),
    ];
    let arcs: Vec<([f32; 2], [f32; 2])> = corners
        .iter()
        .flat_map(|&(cx, cy, from)| {
            (0..=8).map(move |i| {
                let a = (from + 90. * i as f32 / 8.).to_radians();
                let n = [a.cos(), a.sin()];
                ([cx + r * n[0], cy + r * n[1]], n)
            })
        })
        .collect();
    let mut out = Vec::new();
    for (i, &(p, n)) in arcs.iter().enumerate() {
        out.push((p, n));
        let q = arcs[(i + 1) % arcs.len()].0;
        let pieces = (dist(p, q) / STEP).floor() as usize;
        for k in 1..pieces {
            let f = k as f32 / pieces as f32;
            out.push(([p[0] + (q[0] - p[0]) * f, p[1] + (q[1] - p[1]) * f], n));
        }
    }
    out
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

fn at(p: [f32; 2], n: [f32; 2], k: f32) -> Point<Pixels> {
    point(gpui::px(p[0] + n[0] * k), gpui::px(p[1] + n[1] * k))
}

#[cfg(test)]
mod tests {
    use super::says_ultrathink;

    #[test]
    fn ultrathink_counts_only_as_a_whole_word() {
        assert!(says_ultrathink("ultrathink"));
        assert!(says_ultrathink("Please ULTRATHINK about this."));
        assert!(says_ultrathink("fix it, ultrathink"));
        assert!(!says_ultrathink("ultrathinking"));
        assert!(!says_ultrathink("no_ultrathink"));
        assert!(!says_ultrathink("think hard"));
    }
}
