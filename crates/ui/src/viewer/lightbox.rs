// An image over the whole window, after T3 Code's: Ctrl+click on an image in a terminal opens it
// here rather than in the viewer card. The wheel zooms around the pointer, easing into each step; dragging
// moves the image; a double-click fits it again. Esc, the close button or a click outside closes
// it. It is its own view, so a frame of zoom or drag redraws only this, and it holds the keyboard
// while it is open, so nothing typed reaches the terminal under it.

use std::path::PathBuf;

use gpui::{
    AnyElement, Context, CursorStyle, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, ScrollWheelEvent,
    StyledImage, Window, div, img, point, prelude::*,
};

use crate::assets::icon;
use crate::colors;
use crate::viewer::pan::Pan;
use crate::zoom::px;

/// The share of the window an image fills when it opens. A smaller one shows at its own size.
const FIT: (f32, f32) = (0.86, 0.78);
/// Before its size is read, the image draws in a box this big.
const UNSIZED: (f32, f32) = (480., 320.);

/// The lightbox asks to close: its button, or a click outside the image.
pub struct Close;

pub struct Lightbox {
    path: PathBuf,
    name: String,
    size: Option<(u32, u32)>,
    pan: Pan,
    focus: FocusHandle,
    /// What had the keyboard before, which gets it back on close. Taken on the first frame.
    back: Option<FocusHandle>,
    took_focus: bool,
}

impl Focusable for Lightbox {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<Close> for Lightbox {}

/// The image's size when it opens: fitted into the window's share, never scaled up.
pub(crate) fn fitted(size: Option<(u32, u32)>, window: (f32, f32)) -> (f32, f32) {
    let Some((w, h)) = size.filter(|(w, h)| *w > 0 && *h > 0) else {
        return UNSIZED;
    };
    let (w, h) = (w as f32, h as f32);
    let scale = (window.0 * FIT.0 / w).min(window.1 * FIT.1 / h).min(1.);
    (w * scale, h * scale)
}

impl Lightbox {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        // the size is read off the file's header, away from the UI thread
        let file = path.clone();
        cx.spawn(async move |this, cx| {
            let size = cx
                .background_executor()
                .spawn(async move { image::image_dimensions(&file).ok() })
                .await;
            let _ = this.update(cx, |l, cx| {
                l.size = size;
                cx.notify();
            });
        })
        .detach();
        Self {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            path,
            size: None,
            pan: Pan::new(),
            focus: cx.focus_handle(),
            back: None,
            took_focus: false,
        }
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // after the click that closed it, which would otherwise hand the keyboard back to this
        if let Some(back) = self.back.take() {
            window.defer(cx, move |window, cx| window.focus(&back, cx));
        }
        cx.emit(Close);
    }
}

impl Render for Lightbox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.took_focus {
            self.took_focus = true;
            self.back = window.focused(cx);
            let focus = self.focus.clone();
            window.defer(cx, move |window, cx| window.focus(&focus, cx));
        }
        let view = window.viewport_size();
        let z = crate::zoom::zoom();
        let (vw, vh) = (f32::from(view.width) / z, f32::from(view.height) / z);
        self.pan.center = point(view.width / 2., view.height / 2.);
        self.pan.ease(window);
        let (fw, fh) = fitted(self.size, (vw, vh));
        let (w, h) = (fw * self.pan.zoom, fh * self.pan.zoom);
        let left = vw / 2. + f32::from(self.pan.offset.x) / z - w / 2.;
        let top = vh / 2. + f32::from(self.pan.offset.y) / z - h / 2.;
        let dragging = self.pan.dragging();
        let scrim = colors::hsla(colors::theme().shadow);
        let close = div()
            .id("lightbox-close")
            .absolute()
            // past the image's top right corner, kept on screen
            .left(px((left + w + 8.).min(vw - 40.)))
            .top(px((top - 36.).max(12.)))
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.))
            .rounded_full()
            .bg(colors::surface3())
            .border_1()
            .border_color(colors::border2())
            .cursor_pointer()
            .hover(|s| s.bg(colors::surface2()))
            .child(icon("x", 14., colors::text1()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|l, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    l.close(window, cx);
                }),
            );
        let caption = div()
            .absolute()
            .left_0()
            .right_0()
            .top(px((top + h + 12.).min(vh - 34.)))
            .flex()
            .justify_center()
            .child(
                div()
                    .px(px(8.))
                    .py(px(3.))
                    .rounded(px(6.))
                    .bg(colors::surface2())
                    .text_size(px(12.))
                    .text_color(colors::text2())
                    .child(if self.pan.target() == 1. {
                        self.name.clone()
                    } else {
                        format!("{}  {}%", self.name, (self.pan.target() * 100.).round())
                    }),
            );
        let image = div()
            .id("lightbox-image")
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(w))
            .h(px(h))
            .cursor(if dragging {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OpenHand
            })
            .child(
                img(self.path.clone())
                    .size_full()
                    .rounded(px(8.))
                    .object_fit(ObjectFit::Contain),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|l, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    if e.click_count == 2 {
                        l.pan.fit();
                    } else {
                        l.pan.grab(e.position);
                    }
                    cx.notify();
                }),
            );
        let body = div()
            .id("lightbox")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|l, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" {
                    l.close(window, cx);
                }
            }))
            .relative()
            .w(view.width)
            .h(view.height)
            .occlude()
            .bg(scrim.opacity(0.86))
            .when(dragging, |d| d.cursor(CursorStyle::ClosedHand))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|l, _: &MouseDownEvent, window, cx| l.close(window, cx)),
            )
            .on_scroll_wheel(cx.listener(|l, e: &ScrollWheelEvent, _, cx| {
                l.pan.wheel(e);
                cx.notify();
            }))
            .on_mouse_move(cx.listener(|l, e: &MouseMoveEvent, _, cx| {
                if l.pan.drag(e) {
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|l, e: &MouseUpEvent, _, cx| {
                    l.pan.release(e.position);
                    cx.notify();
                }),
            )
            .child(image)
            .child(caption)
            .child(close);
        let body: AnyElement = crate::slide::ease_in(body, "lightbox-in", 140, |d, t| d.opacity(t));
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_fitted_and_never_scaled_up() {
        assert_eq!(fitted(Some((200, 100)), (1000., 800.)), (200., 100.));
        let (w, h) = fitted(Some((4000, 2000)), (1000., 800.));
        assert!((w - 860.).abs() < 0.01 && (h - 430.).abs() < 0.01);
        assert_eq!(fitted(None, (1000., 800.)), UNSIZED);
    }
}
