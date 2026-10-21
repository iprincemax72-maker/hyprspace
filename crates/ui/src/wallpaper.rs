// The new-thread background as the composer screen draws it. Set from the saved appearance with
// the theme, and read while drawing, like the colors.

use std::cell::RefCell;

use gpui::{
    AnyElement, ObjectFit, StyledImage, div, img, linear_color_stop, linear_gradient, prelude::*,
    relative,
};
use hyprspace_proto::state::Background;

use crate::colors;
use crate::zoom::px;

thread_local! {
    static CURRENT: RefCell<Option<Background>> = const { RefCell::new(None) };
}

pub fn set(background: Option<Background>) {
    CURRENT.with(|c| *c.borrow_mut() = background);
}

pub fn showing() -> bool {
    CURRENT.with(|c| c.borrow().is_some())
}

pub fn hero() -> Option<AnyElement> {
    let b = CURRENT.with(|c| c.borrow().clone())?;
    let zoom = b.zoom.max(1.);
    let bg = colors::bg();
    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(relative(0.72))
            .max_h(px(760.))
            .overflow_hidden()
            .child(
                img(b.shown)
                    .absolute()
                    .w(relative(zoom))
                    .h(relative(zoom))
                    .left(relative(-(zoom - 1.) * b.x))
                    .top(relative(-(zoom - 1.) * b.y))
                    .object_fit(ObjectFit::Cover),
            )
            .child(div().absolute().inset_0().bg(linear_gradient(
                180.,
                linear_color_stop(bg.opacity(0.2), 0.35),
                linear_color_stop(bg, 1.),
            )))
            .into_any_element(),
    )
}

pub struct BuiltIn {
    pub slug: &'static str,
    pub title: &'static str,
    pub picture: &'static [u8],
    thumb: &'static [u8],
}

macro_rules! built_in {
    ($(($slug:literal, $title:literal)),* $(,)?) => {
        &[$(BuiltIn {
            slug: $slug,
            title: $title,
            picture: include_bytes!(concat!("../assets/wallpapers/", $slug, ".webp")),
            thumb: include_bytes!(concat!("../assets/wallpapers/thumbs/", $slug, ".webp")),
        }),*]
    };
}

/// From Omarchy's themes (THIRD_PARTY_NOTICES.md), resized to 2560 wide.
pub const BUILT_IN: &[BuiltIn] = built_in!(
    ("glowing-city", "Glowing city"),
    ("shaded-entrance", "Shaded entrance"),
    ("city-view", "City view"),
    ("ether", "Ether"),
    ("gateway", "Gateway"),
    ("totoro", "Totoro"),
    ("mountain-moon", "Mountain moon"),
    ("cosmic", "Cosmic"),
    ("meadow", "Meadow"),
    ("tree-tops", "Tree tops"),
    ("great-wave", "Great wave"),
    ("new-horizons", "New horizons"),
    ("cherry-blossom", "Cherry blossom"),
    ("pastel-clouds", "Pastel clouds"),
    ("ship-at-sea", "Ship at sea"),
    ("black-moon", "Black moon"),
    ("dusk-guardian", "Dusk guardian"),
    ("zen-boat", "Zen boat"),
    ("the-journey", "The journey"),
    ("industrial-moon", "Industrial moon"),
    ("liftoff", "Liftoff"),
    ("winding-road", "Winding road"),
    ("swirl-buck", "Swirl"),
    ("sunset-lake", "Sunset lake"),
    ("synth-scape", "Synth scape"),
    ("waves", "Waves"),
    ("color-curves", "Color curves"),
    ("twisted-stairs", "Twisted stairs"),
);

pub fn thumb_path(slug: &str) -> String {
    format!("wallpapers/thumbs/{slug}.webp")
}

pub fn thumb(path: &str) -> Option<&'static [u8]> {
    let slug = path
        .strip_prefix("wallpapers/thumbs/")?
        .strip_suffix(".webp")?;
    BUILT_IN.iter().find(|b| b.slug == slug).map(|b| b.thumb)
}
