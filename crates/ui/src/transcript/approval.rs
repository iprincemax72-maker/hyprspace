// An approval prompt in the transcript: what the agent wants to run, why, the diff or command,
// and the buttons. Once answered or expired it stays as a record of what happened.

use gpui::{AnyElement, ClickEvent, Context, FontWeight, IntoElement, div, prelude::*};
use hyprspace_proto::{Answer, Tool};

use super::{TranscriptView, tool};
use crate::assets::icon;
use crate::colors;
use crate::zoom::px;

#[allow(clippy::too_many_arguments)]
pub fn card(
    v: &TranscriptView,
    request: &str,
    t: &Tool,
    reason: Option<&str>,
    always: bool,
    answer: Option<Answer>,
    expired: bool,
    cx: &mut Context<TranscriptView>,
) -> AnyElement {
    match super::ask::named(t) {
        Some((super::ask::QUESTION, input)) => {
            return super::ask::question_card(v, request, input, answer, expired, cx);
        }
        Some((super::ask::PLAN, input)) => {
            return super::ask::plan_card(v, Some(request), input, answer, expired, 0, cx);
        }
        _ => {}
    }
    if answer.is_some() || expired {
        return answered(t, answer);
    }
    let detail = match t {
        Tool::Edit { changes } => Some(tool::diffs(changes)),
        _ => tool::input(t).map(|i| tool::mono(&i)),
    };
    let button = |id: &'static str, label: &'static str, a: Answer, main: bool| {
        let request = request.to_string();
        let b = div()
            .id((id, v.id.0))
            .flex()
            .items_center()
            .h(px(26.))
            .px(px(10.))
            .rounded(px(6.))
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .child(label);
        let b = if main {
            b.bg(colors::text1())
                .text_color(colors::bg())
                .hover(|s| s.bg(colors::text1().opacity(0.85)))
        } else {
            b.text_color(colors::text2())
                .hover(|s| s.bg(colors::ink(0.06)).text_color(colors::text1()))
        };
        b.on_click(cx.listener(move |v, _: &ClickEvent, _, cx| v.answer(request.clone(), a, cx)))
    };
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .px(px(14.))
        .py(px(12.))
        .rounded(px(12.))
        .bg(colors::ink(0.035))
        .border_1()
        .border_color(colors::ink(0.08))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(icon("hand", 14., colors::text2()))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors::text1())
                        .child(ask(t)),
                ),
        )
        .children(reason.map(|r| {
            div()
                .text_size(px(12.5))
                .text_color(colors::text2())
                .child(r.to_string())
        }))
        .children(detail)
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(button("deny", "Deny", Answer::Deny, false))
                .when(always, |d| {
                    d.child(button("always", "Always allow", Answer::AllowAlways, false))
                })
                .child(button("allow", "Allow", Answer::Allow, true)),
        )
        .into_any_element()
}

fn ask(t: &Tool) -> String {
    match t {
        Tool::Command { .. } => "Run this command?".into(),
        Tool::Edit { changes } if changes.len() == 1 => "Make this edit?".into(),
        Tool::Edit { .. } => "Make these edits?".into(),
        Tool::Mcp { server, tool, .. } if tool.is_empty() => format!("{server} is asking"),
        _ => format!("{}?", tool::label(t)),
    }
}

fn answered(t: &Tool, answer: Option<Answer>) -> AnyElement {
    let (mark, tint, state) = match answer {
        Some(Answer::Allow) => ("check", colors::ok(), "Allowed"),
        Some(Answer::AllowAlways) => ("check", colors::ok(), "Allowed for this session"),
        Some(Answer::Deny) => ("x", colors::error(), "Denied"),
        None => ("clock", colors::text3(), "Not answered"),
    };
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(12.))
        .py(px(8.))
        .rounded(px(10.))
        .bg(colors::ink(0.035))
        .child(icon(mark, 13., tint))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(12.5))
                .text_color(colors::text2())
                .child(tool::label(t)),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(12.))
                .text_color(colors::text3())
                .child(state),
        )
        .into_any_element()
}
