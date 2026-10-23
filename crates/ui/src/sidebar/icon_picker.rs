// A project's icon and color, picked from a thread's menu: an emoji from a short set or typed in,
// an image file, or back to the icon found in its folder; and a hue for its tag and name.

use gpui::{
    AnyElement, AppContext, ClickEvent, Context, Entity, Focusable, FontWeight, PathPromptOptions,
    Pixels, Point, Subscription, Window, div, prelude::*,
};
use hyprspace_proto::SpaceIcon;
use unicode_segmentation::UnicodeSegmentation;

use crate::assets::icon;
use crate::input::{InputEvent, TextInput};
use crate::root::Root;
use crate::zoom::px;
use crate::{colors, widgets};

const EMOJIS: &[&str] = &[
    "🚀", "⚡", "🔥", "✨", "🌟", "💎", "🎯", "🧠", "🤖", "👾", "🎮", "🎨", "🎬", "🎵", "📱", "💻",
    "🛠️", "⚙️", "🧪", "📦", "📚", "📝", "💬", "🌐", "🌙", "🌊", "🌲", "🍀", "☕", "🍕", "🐙", "🦊",
    "🐸", "🦄", "🐝", "🐳", "❤️", "💜", "💚", "💙",
];

const HUES: &[u16] = &[25, 55, 85, 130, 165, 200, 235, 265, 300, 335];

pub(crate) struct IconPicker {
    at: Point<Pixels>,
    space: u64,
    input: Entity<TextInput>,
    _keys: Subscription,
}

impl Root {
    pub(crate) fn open_icon_picker(
        &mut self,
        space: u64,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| TextInput::new("Or type one", false, cx));
        let keys = cx.subscribe_in(
            &input,
            window,
            move |r, input, e: &InputEvent, _, cx| match e {
                InputEvent::Submit => {
                    let text = input.read(cx).text().trim().to_string();
                    if let Some(emoji) = text.graphemes(true).next() {
                        let emoji = emoji.to_string();
                        r.set_space_icon(space, Some(SpaceIcon::Emoji { emoji }), cx);
                    }
                }
                InputEvent::Cancel => r.close_icon_picker(cx),
                _ => {}
            },
        );
        window.focus(&input.focus_handle(cx), cx);
        self.icon_picker = Some(IconPicker {
            at,
            space,
            input,
            _keys: keys,
        });
        cx.notify();
    }

    fn close_icon_picker(&mut self, cx: &mut Context<Self>) {
        self.icon_picker = None;
        cx.notify();
    }

    fn set_space_icon(&mut self, space: u64, icon: Option<SpaceIcon>, cx: &mut Context<Self>) {
        if let Some(s) = self.state.space_mut(space) {
            s.icon = icon;
            self.save();
        }
        self.close_icon_picker(cx);
    }

    fn set_space_hue(&mut self, space: u64, hue: Option<u16>, cx: &mut Context<Self>) {
        if let Some(s) = self.state.space_mut(space) {
            s.hue = hue;
            self.save();
        }
        self.close_icon_picker(cx);
    }

    fn pick_icon_image(&mut self, space: u64, cx: &mut Context<Self>) {
        self.icon_picker = None;
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update(cx, |r, cx| {
                r.set_space_icon(space, Some(SpaceIcon::Image { path }), cx)
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn icon_popup(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let picker = self.icon_picker.as_ref()?;
        let space = picker.space;
        let (picked, hue, name) = self
            .state
            .space(space)
            .map(|s| (s.icon.is_some(), s.hue, s.name.clone()))?;
        let swatches =
            HUES.iter().map(|&h| {
                div()
                    .id(("hue", h as usize))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(22.))
                    .rounded_full()
                    .cursor_pointer()
                    .when(hue == Some(h), |d| {
                        d.border_2().border_color(colors::text1())
                    })
                    .child(
                        div()
                            .size(px(14.))
                            .rounded_full()
                            .bg(colors::tag(&name, Some(h)).1),
                    )
                    .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                        r.set_space_hue(space, Some(h), cx)
                    }))
            });
        let grid = EMOJIS.iter().enumerate().map(|(i, &e)| {
            div()
                .id(("emoji", i))
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.))
                .rounded(px(6.))
                .text_size(px(16.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.06)))
                .child(e)
                .on_click(cx.listener(move |r, _: &ClickEvent, _, cx| {
                    let emoji = e.to_string();
                    r.set_space_icon(space, Some(SpaceIcon::Emoji { emoji }), cx)
                }))
        });
        let row = |id: &'static str, glyph: &'static str, label: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(10.))
                .h(px(30.))
                .px(px(10.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|s| s.bg(colors::ink(0.06)))
                .child(icon(glyph, 13., colors::text3()))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(colors::text1())
                        .child(label),
                )
        };
        let heading = |text: &'static str| {
            div()
                .px(px(10.))
                .pt(px(4.))
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colors::text3())
                .child(text)
        };
        let close = cx.listener(|r, _: &(), _, cx| r.close_icon_picker(cx));
        Some(widgets::popup(
            picker.at,
            widgets::Open::Down,
            window,
            move |w, cx| close(&(), w, cx),
            div()
                .w(px(250.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(heading("Icon"))
                .child(div().px(px(4.)).flex().flex_wrap().children(grid))
                .child(
                    div()
                        .mx(px(6.))
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .px(px(8.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(colors::border1())
                        .text_size(px(12.5))
                        .text_color(colors::text1())
                        .cursor_text()
                        .child(picker.input.clone()),
                )
                .child(heading("Color"))
                .child(div().px(px(6.)).flex().justify_between().children(swatches))
                .child(widgets::menu_rule())
                .child(
                    row("icon-image", "image-plus", "Choose an image...").on_click(
                        cx.listener(move |r, _: &ClickEvent, _, cx| r.pick_icon_image(space, cx)),
                    ),
                )
                .when(picked, |d| {
                    d.child(
                        row("icon-reset", "rotate-ccw", "Use the folder's icon").on_click(
                            cx.listener(move |r, _: &ClickEvent, _, cx| {
                                r.set_space_icon(space, None, cx)
                            }),
                        ),
                    )
                })
                .when(hue.is_some(), |d| {
                    d.child(
                        row("hue-reset", "rotate-ccw", "Use the default color").on_click(
                            cx.listener(move |r, _: &ClickEvent, _, cx| {
                                r.set_space_hue(space, None, cx)
                            }),
                        ),
                    )
                }),
        ))
    }
}
