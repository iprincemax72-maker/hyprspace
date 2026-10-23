// A project's own icon: a favicon, app icon or logo kept where projects usually keep one. Each
// folder is looked at once a run, off the main thread; a project with none keeps its initials.

use std::path::{Path, PathBuf};

use gpui::Context;
use hyprspace_proto::{Space, SpaceIcon};

use crate::root::Root;

const DIRS: &[&str] = &[
    "",
    "public",
    "static",
    "assets",
    "app",
    "src/app",
    "src",
    "brand",
    "web/public",
    "website/public",
    "docs",
    "scripts",
    ".github",
];

const NAMES: &[&str] = &[
    "favicon.svg",
    "icon.svg",
    "logo.svg",
    "favicon.png",
    "icon.png",
    "logo.png",
    "app-icon.png",
    "apple-touch-icon.png",
    "favicon.ico",
    "icon.ico",
];

fn find(folder: &Path) -> Option<PathBuf> {
    DIRS.iter()
        .flat_map(|d| NAMES.iter().map(move |n| folder.join(d).join(n)))
        .find(|p| p.is_file())
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Look {
    pub icon: Option<SpaceIcon>,
    pub hue: Option<u16>,
}

impl Root {
    /// Looks for the icons of this computer's spaces not looked at yet.
    pub(crate) fn look_up_icons(&mut self, cx: &mut Context<Self>) {
        let new: Vec<PathBuf> = self
            .state
            .spaces
            .iter()
            .filter(|s| s.machine.is_none())
            .filter_map(|s| s.cwd.clone())
            .filter(|c| !self.icons.contains_key(c))
            .collect();
        if new.is_empty() {
            return;
        }
        for c in &new {
            self.icons.insert(c.clone(), None);
        }
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move {
                    new.into_iter()
                        .filter_map(|c| find(&c).map(|i| (c, i)))
                        .collect::<Vec<_>>()
                })
                .await;
            if !found.is_empty() {
                let _ = this.update(cx, |r, cx| {
                    for (c, i) in found {
                        r.icons.insert(c, Some(i));
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// How the space's tag looks: the icon picked, else the one found in its folder, and the
    /// color picked.
    pub(crate) fn look_of(&self, space: &Space) -> Look {
        if space.machine.is_some() {
            return Look::default();
        }
        let icon = space.icon.clone().or_else(|| {
            let path = self.icons.get(space.cwd.as_ref()?)?.clone()?;
            Some(SpaceIcon::Image { path })
        });
        Look {
            icon,
            hue: space.hue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_first_icon_in_order() {
        let dir = std::env::temp_dir().join(format!("hs-icon-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("public")).unwrap();
        assert_eq!(find(&dir), None);
        std::fs::write(dir.join("public/favicon.ico"), b"").unwrap();
        std::fs::write(dir.join("public/logo.svg"), b"").unwrap();
        assert_eq!(find(&dir), Some(dir.join("public").join("logo.svg")));
        std::fs::write(dir.join("icon.png"), b"").unwrap();
        assert_eq!(find(&dir), Some(dir.join("").join("icon.png")));
        let _ = std::fs::remove_dir_all(dir);
    }
}
