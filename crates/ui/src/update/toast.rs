// The corner card (updater.css `.updater`): "Update x available" with Restart and update, a
// spinner and progress bar while it installs, the error with Retry, and What's new after an
// update. It shows only when there is something to act on or read.

use gpui::{
    AnyElement, App, ClickEvent, Div, Entity, FontWeight, MouseButton, Transformation, div,
    percentage, prelude::*, relative,
};
use hyprspace_proto::update::Step;

use super::{Phase, Swipe, Updater, VERSION, running, step_text};
use crate::assets::icon;
use crate::pace::Looping;
use crate::zoom::px;
use crate::{colors, widgets};

/// The cards for the window's bottom right corner, or None when there is nothing to show.
pub fn overlay(updater: &Entity<Updater>, working: usize, cx: &App) -> Option<AnyElement> {
    let u = updater.read(cx);
    let news = u
        .whats_new
        .clone()
        .map(|notes| swipeable(updater, true, whats_new(updater, notes), cx));
    let card = match (&u.phase, u.confirm) {
        (Phase::Busy(step), _) => Some(busy(*step, u.progress())),
        _ if u.dismissed => None,
        (Phase::Available(_) | Phase::Failed(_), Some(n)) => Some(confirm(updater, n, working)),
        (Phase::Available(r), None) => Some(available(updater, &r.version, working)),
        (Phase::Failed(m), None) => Some(failed(updater, m, working)),
        _ => None,
    };
    let card = match (&u.phase, card) {
        (Phase::Busy(_), card) => card,
        (_, card) => card.map(|c| swipeable(updater, false, c, cx)),
    };
    if news.is_none() && card.is_none() {
        return None;
    }
    Some(
        div()
            .absolute()
            .right(px(14.))
            .bottom(px(14.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(8.))
            .children(news)
            .children(card)
            .into_any_element(),
    )
}

pub(super) const DISMISS: f32 = 90.;

fn swipeable(updater: &Entity<Updater>, news: bool, card: AnyElement, cx: &App) -> AnyElement {
    let dx = updater
        .read(cx)
        .swipe
        .filter(|s| s.news == news)
        .map_or(0., |s| s.dx);
    let (down, moved, up, out) = (
        updater.clone(),
        updater.clone(),
        updater.clone(),
        updater.clone(),
    );
    div()
        .relative()
        // clicks on the card stay on the card
        .occlude()
        .left(px(dx))
        .opacity(1. - (dx / (DISMISS * 2.)).min(0.7))
        .child(card)
        .on_mouse_down(MouseButton::Left, move |e, _, cx| {
            down.update(cx, |u, cx| {
                u.swipe = Some(Swipe {
                    news,
                    from: e.position.x.into(),
                    dx: 0.,
                });
                cx.notify();
            })
        })
        .on_mouse_move(move |e, _, cx| {
            if e.pressed_button != Some(MouseButton::Left) {
                return;
            }
            moved.update(cx, |u, cx| {
                if let Some(s) = u.swipe.as_mut().filter(|s| s.news == news) {
                    let x: f32 = e.position.x.into();
                    s.dx = ((x - s.from) / crate::zoom::zoom()).max(0.);
                    cx.notify();
                }
            })
        })
        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
            up.update(cx, |u, cx| u.end_swipe(news, cx))
        })
        .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
            out.update(cx, |u, cx| u.end_swipe(news, cx))
        })
        .into_any_element()
}

fn frame() -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(9.))
        .py(px(8.))
        .pl(px(12.))
        .pr(px(10.))
        .rounded(px(10.))
        .border_1()
        .border_color(colors::border2())
        .bg(colors::surface2())
        .shadow(colors::shadow())
        .text_size(px(12.))
        .text_color(colors::text1())
}

fn dot(error: bool) -> Div {
    div().flex_none().size(px(7.)).rounded_full().bg(if error {
        colors::error()
    } else {
        colors::accent()
    })
}

fn close(updater: &Entity<Updater>, id: &'static str, news: bool) -> AnyElement {
    let updater = updater.clone();
    div()
        .id(id)
        .flex_none()
        .px(px(2.))
        .cursor_pointer()
        .child(icon("x", 13., colors::text3()))
        .on_click(move |_: &ClickEvent, _, cx| {
            updater.update(cx, |u, cx| {
                if news {
                    u.close_whats_new(cx)
                } else {
                    u.dismiss(cx)
                }
            })
        })
        .into_any_element()
}

fn install_button(updater: &Entity<Updater>, label: &'static str, working: usize) -> AnyElement {
    let updater = updater.clone();
    widgets::primary("update-install", label)
        .h(px(24.))
        .px(px(9.))
        .font_weight(FontWeight::NORMAL)
        .on_click(move |_: &ClickEvent, _, cx| updater.update(cx, |u, cx| u.install(working, cx)))
        .into_any_element()
}

fn available(updater: &Entity<Updater>, version: &str, working: usize) -> AnyElement {
    frame()
        .child(dot(false))
        .child(
            div()
                .whitespace_nowrap()
                .child(format!("Update {version} available")),
        )
        .child(install_button(updater, "Restart and update", working))
        .child(close(updater, "update-later", false))
        .into_any_element()
}

fn confirm(updater: &Entity<Updater>, n: usize, working: usize) -> AnyElement {
    frame()
        .max_w(px(460.))
        .child(dot(false))
        .child(div().min_w_0().child(running(n)))
        .child(install_button(updater, "Update anyway", working))
        .child(close(updater, "update-later", false))
        .into_any_element()
}

fn failed(updater: &Entity<Updater>, message: &str, working: usize) -> AnyElement {
    frame()
        .max_w(px(460.))
        .child(dot(true))
        .child(div().min_w_0().child(message.to_string()))
        .child(install_button(updater, "Retry", working))
        .child(close(updater, "update-dismiss", false))
        .into_any_element()
}

fn busy(step: Step, progress: Option<f32>) -> AnyElement {
    let spinner = icon("ring", 13., colors::accent()).looping(700, |s, t| {
        s.with_transformation(Transformation::rotate(percentage(t)))
    });
    let bar = match progress {
        Some(p) => div()
            .h_full()
            .w(relative(p))
            .rounded_full()
            .bg(colors::accent())
            .into_any_element(),
        // size unknown: a slice that slides across
        None => div()
            .absolute()
            .h_full()
            .w(relative(0.35))
            .rounded_full()
            .bg(colors::accent())
            .looping(1100, |d, t| d.left(relative(-0.35 + 1.35 * t)))
            .into_any_element(),
    };
    frame()
        .occlude()
        .flex_col()
        .items_stretch()
        .gap(px(8.))
        .min_w(px(230.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(spinner)
                .child(div().whitespace_nowrap().child(step_text(step))),
        )
        .child(
            div()
                .relative()
                .h(px(4.))
                .w_full()
                .rounded_full()
                .overflow_hidden()
                .bg(colors::surface3())
                .child(bar),
        )
        .into_any_element()
}

fn whats_new(updater: &Entity<Updater>, notes: Vec<String>) -> AnyElement {
    let body: Vec<AnyElement> = if notes.is_empty() {
        vec![div().child("Thanks for updating.").into_any_element()]
    } else {
        notes
            .into_iter()
            .map(|n| {
                div()
                    .flex()
                    .gap(px(7.))
                    .child(div().flex_none().text_color(colors::text3()).child("•"))
                    .child(div().min_w_0().child(n))
                    .into_any_element()
            })
            .collect()
    };
    frame()
        .items_start()
        .w(px(360.))
        .py(px(10.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("What's new in v{VERSION}")),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .text_color(colors::text2())
                        .line_height(px(17.))
                        .children(body),
                ),
        )
        .child(close(updater, "whats-new-close", true))
        .into_any_element()
}
