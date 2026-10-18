// The spinner shown while something runs, after loading.dev's Eclipse: two dots trading places,
// one passing behind the other.

use std::f32::consts::TAU;

use gpui::{AnyElement, ElementId, Hsla, IntoElement, Transformation, div, prelude::*, radians};

use crate::assets::icon;
use crate::pace::Looping;
use crate::zoom::px;

/// Two dots circling each other on an ellipse seen edge on: the one going behind shrinks and
/// fades, so they read as passing. Sized to sit on a 12.5px line. `id` must be unique among its
/// siblings.
pub fn eclipse(id: impl Into<ElementId>, color: Hsla) -> AnyElement {
    const W: f32 = 14.;
    const H: f32 = 12.5;
    const DOT: f32 = 5.;
    const REACH: f32 = 3.5;
    div()
        .id(id)
        .flex_none()
        .relative()
        .w(px(W))
        .h(px(H))
        .children([0., 0.5].map(|offset: f32| {
            div()
                .absolute()
                .rounded_full()
                .bg(color)
                .looping(1200, move |d, t| {
                    let a = (t + offset) * TAU;
                    // depth: 1 nearest, -1 furthest
                    let near = a.sin();
                    let size = DOT * (1. + 0.2 * near);
                    d.size(px(size))
                        .left(px(W / 2. + REACH * a.cos() - size / 2.))
                        .top(px((H - size) / 2.))
                        .opacity(0.65 + 0.35 * near)
                })
        }))
        .into_any_element()
}

pub fn compass(color: Hsla) -> AnyElement {
    icon("compass", 13., color)
        .looping(2000, |s, t| {
            let quarters = t * 4.;
            let step = quarters.floor();
            let f = quarters - step;
            let eased = if f < 0.5 {
                4. * f * f * f
            } else {
                1. - (-2. * f + 2.).powi(3) / 2.
            };
            s.with_transformation(Transformation::rotate(radians((step + eased) * TAU / 4.)))
        })
        .into_any_element()
}
