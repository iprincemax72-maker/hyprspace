// Settings, General, This Mac: screen recording and accessibility. macOS charges whatever runs in
// a terminal to the app that opened it, so an agent's `screencapture` or System Events script is
// HyprSpace asking. Without these the agent just fails ("could not create image from rect").

use std::ffi::c_void;

use gpui::{AnyElement, App, ClickEvent, Context, prelude::*};

use super::Root;
use super::controls::{group, row, text};
use crate::widgets;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    static kAXTrustedCheckOptionPrompt: *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: *const c_void;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
    fn CFRelease(cf: *const c_void);
}

fn screen_allowed() -> bool {
    // SAFETY: no arguments, a yes or no out
    unsafe { CGPreflightScreenCaptureAccess() }
}

fn accessibility_allowed() -> bool {
    // SAFETY: no arguments, a yes or no out
    unsafe { AXIsProcessTrusted() }
}

/// macOS's own prompt, which also puts HyprSpace in the list; the pane opens too, since macOS asks
/// only once and the switch is there after that.
fn ask_screen(cx: &mut App) {
    // SAFETY: no arguments; it shows the prompt at most once
    unsafe { CGRequestScreenCaptureAccess() };
    cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture");
}

fn ask_accessibility(cx: &mut App) {
    // SAFETY: a one-entry dictionary of CF constants, made with CF's own callbacks and released
    // after the call that reads it
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        );
        if !options.is_null() {
            AXIsProcessTrustedWithOptions(options);
            CFRelease(options);
        }
    }
    cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility");
}

impl Root {
    pub(super) fn mac_group(&self, cx: &mut Context<Self>) -> AnyElement {
        let control = |id: &'static str, allowed: bool, ask: fn(&mut App)| -> AnyElement {
            if allowed {
                text("Allowed").into_any_element()
            } else {
                widgets::button(id, "Allow")
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                        ask(cx);
                        cx.notify();
                    }))
                    .into_any_element()
            }
        };
        group(
            "This Mac",
            vec![
                row(
                    "Screen recording",
                    "Lets agents, and anything run in a HyprSpace terminal, see the whole screen. macOS may ask you to reopen HyprSpace.",
                    control("allow-screen", screen_allowed(), ask_screen),
                ),
                row(
                    "Accessibility",
                    "Lets them click, type and read any app on this Mac.",
                    control(
                        "allow-accessibility",
                        accessibility_allowed(),
                        ask_accessibility,
                    ),
                ),
            ],
        )
        .into_any_element()
    }
}
