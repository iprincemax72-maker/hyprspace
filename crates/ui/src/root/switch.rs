use std::time::{Duration, Instant};

use gpui::{Context, Task};
use hyprspace_proto::{Agent, Command, Thread, ThreadKind};

use super::{Root, View};

const TO_STRUCTURED: Duration = Duration::from_secs(4);
const TO_TERMINAL: Duration = Duration::from_secs(20);

pub(crate) struct Switch {
    back: ThreadKind,
    had_view: bool,
    until: Instant,
    _check: Task<()>,
}

impl Root {
    pub(crate) fn switch_label(&self, t: &Thread) -> Option<&'static str> {
        if t.parent.is_some() || self.busy(t.id) || self.machines.remote(t.id).is_some() {
            return None;
        }
        let known = |a: Agent| matches!(a, Agent::Claude | Agent::Codex);
        match &t.kind {
            ThreadKind::Structured { launch } if known(launch.agent) => Some("Switch to terminal"),
            ThreadKind::Terminal { run: Some(run), .. }
                if known(run.agent) && run.resume.is_some() =>
            {
                Some("Switch to structured")
            }
            _ => None,
        }
    }

    pub(crate) fn switch_kind(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some((_, t)) = self.state.thread(id) else {
            return;
        };
        if self.switch_label(t).is_none() || self.switching.contains_key(&id) {
            return;
        }
        let back = t.kind.clone();
        let next = match &back {
            ThreadKind::Structured { launch } => {
                let mut run = launch.clone();
                run.rewind = None;
                ThreadKind::Terminal {
                    cwd: launch.cwd.clone(),
                    run: Some(run),
                }
            }
            ThreadKind::Terminal {
                cwd,
                run: Some(run),
            } => {
                let mut launch = run.clone();
                launch.cwd = cwd.clone();
                ThreadKind::Structured { launch }
            }
            ThreadKind::Terminal { run: None, .. } => return,
        };
        let had_view = self.views.contains_key(&id);
        self.drop_view(id);
        let Some(t) = self.state.thread_mut(id) else {
            return;
        };
        t.kind = next;
        let thread = t.clone();
        if let ThreadKind::Structured { launch } = &thread.kind
            && launch.agent == Agent::Claude
            && let Some(conversation) = launch.resume.clone()
        {
            self.client.send(Command::ImportHistory {
                journal: thread.journal(),
                conversation,
            });
        }
        self.make_view(&thread, None, true, cx);
        if let Some(View::Structured(v)) = self.views.get(&id) {
            v.update(cx, |v, cx| v.open_switched(cx));
        }
        let wait = match thread.kind {
            ThreadKind::Structured { .. } => TO_STRUCTURED,
            ThreadKind::Terminal { .. } => TO_TERMINAL,
        };
        let start = Instant::now();
        let check = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let more = this.update(cx, |r, cx| {
                    if !r.switching.contains_key(&id) {
                        return false;
                    }
                    if start.elapsed() >= wait {
                        r.switch_settled(id, cx);
                        return false;
                    }
                    if r.menu.is_some() {
                        cx.notify();
                    }
                    true
                });
                if !more.unwrap_or(false) {
                    break;
                }
            }
        });
        self.switching.insert(
            id,
            Switch {
                back,
                had_view,
                until: Instant::now() + wait,
                _check: check,
            },
        );
        self.save();
        cx.notify();
    }

    pub(crate) fn switch_wait(&self, id: u64) -> Option<u64> {
        let left = self
            .switching
            .get(&id)?
            .until
            .saturating_duration_since(Instant::now());
        Some(left.as_secs() + 1)
    }

    fn switch_settled(&mut self, id: u64, cx: &mut Context<Self>) {
        let terminal = matches!(
            self.state.thread(id).map(|(_, t)| &t.kind),
            Some(ThreadKind::Terminal { .. })
        );
        if terminal {
            self.switch_back(id, cx);
        } else {
            self.switching.remove(&id);
            cx.notify();
        }
    }

    pub(crate) fn switch_seen(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.switching.remove(&id).is_some() {
            cx.notify();
        }
    }

    pub(crate) fn switch_quit(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.switching.contains_key(&id) {
            self.switch_back(id, cx);
        }
    }

    pub(crate) fn switch_back(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(s) = self.switching.remove(&id) else {
            return;
        };
        self.drop_view(id);
        let Some(t) = self.state.thread_mut(id) else {
            return;
        };
        t.kind = s.back;
        let thread = t.clone();
        if s.had_view {
            self.make_view(&thread, None, true, cx);
        }
        self.save();
        self.notify(
            format!(
                "\"{}\" couldn't switch, so it's back the way it was.",
                thread.title
            ),
            cx,
        );
    }
}
