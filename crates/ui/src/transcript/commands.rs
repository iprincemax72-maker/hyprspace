use gpui::Context;
use hyprspace_proto::Permission;

use super::{TranscriptEvent, TranscriptView};
use crate::suggest::Item;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Own {
    Model,
    Effort,
    Plan,
    Clear,
    Context,
    Rewind,
}

const OWN: &[(&str, Own, &str)] = &[
    ("model", Own::Model, "Pick the model"),
    ("effort", Own::Effort, "Pick the effort"),
    ("plan", Own::Plan, "Plan before changing anything"),
    ("clear", Own::Clear, "Start a new thread here"),
    ("new", Own::Clear, "Start a new thread here"),
    ("context", Own::Context, "How full the context window is"),
    ("rewind", Own::Rewind, "Go back to an earlier message"),
];

const TERMINAL_ONLY: &[&str] = &[
    "add-dir",
    "agents",
    "approvals",
    "bug",
    "config",
    "diff",
    "doctor",
    "exit",
    "export",
    "hooks",
    "ide",
    "init-github-app",
    "install-github-app",
    "login",
    "logout",
    "mcp",
    "memory",
    "permissions",
    "privacy-settings",
    "quit",
    "resume",
    "status",
    "statusline",
    "terminal-setup",
    "theme",
    "upgrade",
    "vim",
];

pub(crate) enum Slash {
    Own(Own),
    TerminalOnly(String),
}

pub(crate) fn parse(text: &str, known: &[String]) -> Option<Slash> {
    let name = text.strip_prefix('/')?.split_whitespace().next()?;
    if let Some(&(_, own, _)) = OWN.iter().find(|(n, ..)| *n == name) {
        return Some(Slash::Own(own));
    }
    (TERMINAL_ONLY.contains(&name) && !known.iter().any(|k| k == name))
        .then(|| Slash::TerminalOnly(name.to_string()))
}

pub(crate) fn items() -> Vec<Item> {
    OWN.iter()
        .map(|(name, _, detail)| Item {
            label: name.to_string(),
            detail: detail.to_string(),
            insert: format!("/{name}"),
        })
        .collect()
}

impl TranscriptView {
    pub(super) fn run_slash(&mut self, slash: Slash, cx: &mut Context<Self>) {
        match slash {
            Slash::Own(Own::Model) => self.want_menu = Some(false),
            Slash::Own(Own::Effort) => self.want_menu = Some(true),
            Slash::Own(Own::Plan) if self.launch.permission == Permission::Plan => {
                self.model.note("This thread already plans first.");
            }
            Slash::Own(Own::Plan) => self.pick_permission(Permission::Plan, cx),
            Slash::Own(Own::Clear) => cx.emit(TranscriptEvent::NewThread),
            Slash::Own(Own::Context) => {
                let line = match self.model.context {
                    Some((used, window)) => format!(
                        "{}% of the context window is used, {} of {} tokens.",
                        (used as f64 / window.max(1) as f64 * 100.).round(),
                        tokens(used),
                        tokens(window)
                    ),
                    None => format!(
                        "{} hasn't said how full its context is yet. Send a message first.",
                        self.launch.agent.name()
                    ),
                };
                self.model.note(line);
            }
            Slash::Own(Own::Rewind) => self
                .model
                .note("Point at one of your messages and click the pencil to edit from there."),
            Slash::TerminalOnly(name) => self
                .model
                .note(format!("/{name} only works in a terminal thread.")),
        }
        cx.notify();
    }
}

fn tokens(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{}k", n / 1_000),
        _ => format!("{:.1}M", n as f64 / 1_000_000.),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_commands_win_and_screens_stay_in_the_terminal() {
        assert!(matches!(parse("/model", &[]), Some(Slash::Own(Own::Model))));
        assert!(matches!(
            parse("/clear now", &[]),
            Some(Slash::Own(Own::Clear))
        ));
        assert!(matches!(parse("/mcp", &[]), Some(Slash::TerminalOnly(n)) if n == "mcp"));
        assert!(parse("/mcp", &["mcp".into()]).is_none());
        assert!(parse("/compact", &[]).is_none());
        assert!(parse("/usr/bin/env", &[]).is_none());
        assert!(parse("model", &[]).is_none());
    }

    #[test]
    fn token_counts_read_short() {
        assert_eq!(tokens(950), "950");
        assert_eq!(tokens(68_400), "68k");
        assert_eq!(tokens(1_000_000), "1.0M");
    }
}
