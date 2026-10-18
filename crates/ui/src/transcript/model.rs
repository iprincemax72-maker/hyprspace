// What a transcript holds, built from prompts, answers and run events. Live events and a
// journal replay go through the same calls, so a thread reopened after a restart looks the way
// it did. No GPUI here, so all of it is unit tested.

use std::path::PathBuf;
use std::time::Instant;

use hyprspace_proto::run::RunFile;
use hyprspace_proto::{Answer, Prompt, RunEvent, RunStatus, Tool};

use crate::markdown::{self, Block};

pub enum Item {
    User {
        text: String,
        images: Vec<PathBuf>,
        /// Sent while a run was live, so it joined that run.
        steer: bool,
    },
    Text {
        source: String,
        /// Parsed lazily: a streaming reply changes many times between paints.
        blocks: Option<Vec<Block>>,
    },
    Thinking {
        text: String,
        open: bool,
    },
    Tool {
        id: String,
        tool: Tool,
        /// (ok, output) once the call ended.
        done: Option<(bool, String)>,
        open: bool,
    },
    Agent(Subagent),
    Approval {
        request: String,
        tool: Tool,
        reason: Option<String>,
        always: bool,
        answer: Option<Answer>,
        /// The session ended before anyone answered.
        expired: bool,
    },
    Error(String),
    Finished {
        status: RunStatus,
        ms: u64,
        error: Option<String>,
    },
    Note(String),
    Changes {
        before: String,
        after: String,
        files: Vec<RunFile>,
        all: bool,
        undo: Undo,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Undo {
    Ready,
    Busy,
    Done(String),
    Failed(String),
}

/// An Agent call: the subagent's own tool calls as they happen, then its report.
pub struct Subagent {
    pub id: String,
    pub description: String,
    pub agent_type: String,
    pub prompt: String,
    pub calls: Vec<Call>,
    /// What it wrote along the way, shown until the report comes.
    pub said: String,
    pub state: AgentState,
    pub report: String,
    /// The report (or what it said) parsed, lazily like reply text.
    pub blocks: Option<Vec<Block>>,
    pub since: Instant,
    pub open: bool,
    pub prompt_open: bool,
}

pub struct Call {
    pub id: String,
    pub tool: Tool,
    pub done: Option<(bool, String)>,
    pub open: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Working,
    Done,
    Failed,
    /// The session ended before it reported.
    Stopped,
}

impl Subagent {
    fn new(id: String, description: String, agent_type: String, prompt: String) -> Self {
        Self {
            id,
            description,
            agent_type,
            prompt,
            calls: Vec::new(),
            said: String::new(),
            state: AgentState::Working,
            report: String::new(),
            blocks: None,
            since: Instant::now(),
            open: false,
            prompt_open: false,
        }
    }

    /// The final answer once there is one, or else what it said so far.
    pub fn answer(&self) -> &str {
        if self.report.trim().is_empty() {
            &self.said
        } else {
            &self.report
        }
    }

    fn call(&mut self, id: &str) -> Option<&mut Call> {
        self.calls.iter_mut().rev().find(|c| c.id == id)
    }
}

/// Where a thread stands, for the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Idle,
    Working,
    Waiting,
    Done,
    Failed,
}

/// A terminal session's state from its agent's hooks.
impl From<hyprspace_proto::AgentState> for Status {
    fn from(s: hyprspace_proto::AgentState) -> Self {
        match s {
            hyprspace_proto::AgentState::Idle => Status::Idle,
            hyprspace_proto::AgentState::Working => Status::Working,
            hyprspace_proto::AgentState::Waiting => Status::Waiting,
            hyprspace_proto::AgentState::Done => Status::Done,
        }
    }
}

struct Run {
    since: Instant,
    said: bool,
}

#[derive(Default)]
pub struct Transcript {
    pub items: Vec<Item>,
    run: Option<Run>,
    /// The CLI's own name for its model, once it started.
    pub model: Option<String>,
    /// The id `Launch::resume` takes, once the CLI started.
    pub thread: Option<String>,
    pub cwd: Option<PathBuf>,
    /// Tokens in the context window and its size, as of the latest reply.
    pub context: Option<(u64, u64)>,
    pub compacting: bool,
    last: Option<RunStatus>,
    failed: bool,
}

impl Transcript {
    pub fn running(&self) -> bool {
        self.run.is_some()
    }

    /// A run or a subagent is live, so timers on screen count.
    pub fn ticking(&self) -> bool {
        self.running()
            || self
                .items
                .iter()
                .any(|i| matches!(i, Item::Agent(a) if a.state == AgentState::Working))
    }

    /// The last call of the live run that hasn't ended.
    pub fn running_tool(&self) -> Option<&Tool> {
        self.run.as_ref()?;
        self.items.iter().rev().find_map(|i| match i {
            Item::Tool {
                tool, done: None, ..
            } => Some(tool),
            _ => None,
        })
    }

    /// Each working subagent's id, label and seconds since it started.
    pub fn live_agents(&self) -> Vec<(String, String, u64)> {
        self.items
            .iter()
            .filter_map(|i| match i {
                Item::Agent(a) if a.state == AgentState::Working => {
                    let label = if a.description.trim().is_empty() {
                        a.agent_type.clone()
                    } else {
                        a.description.clone()
                    };
                    Some((a.id.clone(), label, a.since.elapsed().as_secs()))
                }
                _ => None,
            })
            .collect()
    }

    pub fn last_reply(&self) -> String {
        self.items
            .iter()
            .rev()
            .take_while(|i| !matches!(i, Item::User { .. }))
            .find_map(|i| match i {
                Item::Text { source, .. } if !source.trim().is_empty() => Some(source.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Seconds the live run has taken so far.
    pub fn elapsed(&self) -> Option<u64> {
        self.run.as_ref().map(|r| r.since.elapsed().as_secs())
    }

    pub fn status(&self) -> Status {
        let waiting = self.items.iter().any(|i| {
            matches!(
                i,
                Item::Approval {
                    answer: None,
                    expired: false,
                    ..
                }
            )
        });
        if waiting {
            Status::Waiting
        } else if self.run.is_some() {
            Status::Working
        } else if self.failed || self.last == Some(RunStatus::Failed) {
            Status::Failed
        } else if self.last.is_some() {
            Status::Done
        } else {
            Status::Idle
        }
    }

    /// A prompt the user sent. Mid-run it steers; otherwise it starts a run.
    pub fn prompt(&mut self, prompt: &Prompt) {
        self.items.push(Item::User {
            text: prompt.text.clone(),
            images: prompt.images.clone(),
            steer: self.run.is_some(),
        });
        if self.run.is_none() {
            self.failed = false;
            self.run = Some(Run {
                since: Instant::now(),
                said: false,
            });
        }
    }

    pub fn answered(&mut self, request: &str, answer: Answer) {
        for item in self.items.iter_mut().rev() {
            if let Item::Approval {
                request: r,
                answer: a,
                ..
            } = item
                && r == request
            {
                *a = Some(answer);
                return;
            }
        }
    }

    /// The engine could not start or reach the session.
    pub fn fail(&mut self, message: String) {
        self.items.push(Item::Error(message));
        self.end_session();
    }

    pub fn cut(&mut self, ix: usize) {
        self.items.truncate(ix);
        self.run = None;
        self.last = None;
        self.failed = false;
        self.compacting = false;
        self.context = None;
    }

    pub fn after_command(&self, ix: usize) -> bool {
        self.items[..ix.min(self.items.len())]
            .iter()
            .rev()
            .find_map(|i| match i {
                Item::User { text, .. } => Some(slash(text)),
                _ => None,
            })
            .unwrap_or(false)
    }

    pub fn note(&mut self, text: impl Into<String>) {
        self.items.push(Item::Note(text.into()));
    }

    pub fn apply(&mut self, event: RunEvent) {
        match event {
            RunEvent::Started {
                model, thread, cwd, ..
            } => {
                self.model = Some(model);
                self.thread = Some(thread);
                self.cwd = Some(cwd);
            }
            RunEvent::Commands { .. } => {}
            RunEvent::Text { text } => {
                if let Some(run) = self.run.as_mut() {
                    run.said = true;
                }
                match self.items.last_mut() {
                    Some(Item::Text { source, blocks }) => {
                        source.push_str(&text);
                        *blocks = None;
                    }
                    _ => self.items.push(Item::Text {
                        source: text,
                        blocks: None,
                    }),
                }
            }
            RunEvent::Thinking { text } => match self.items.last_mut() {
                Some(Item::Thinking { text: t, .. }) => t.push_str(&text),
                _ => self.items.push(Item::Thinking { text, open: false }),
            },
            RunEvent::Tool {
                id,
                tool:
                    Tool::Agent {
                        description,
                        agent_type,
                        prompt,
                    },
            } => match self.agent(&id) {
                Some(a) => {
                    a.description = description;
                    a.agent_type = agent_type;
                    a.prompt = prompt;
                }
                None => self.items.push(Item::Agent(Subagent::new(
                    id,
                    description,
                    agent_type,
                    prompt,
                ))),
            },
            RunEvent::Tool { id, tool } => match self.tool(&id) {
                Some(Item::Tool { tool: t, .. }) => *t = tool,
                _ => self.items.push(Item::Tool {
                    id,
                    tool,
                    done: None,
                    open: false,
                }),
            },
            RunEvent::ToolDone { id, ok, output } => {
                if let Some(a) = self.agent(&id) {
                    a.state = if ok {
                        AgentState::Done
                    } else {
                        AgentState::Failed
                    };
                    a.report = output;
                    a.blocks = None;
                } else if let Some(Item::Tool { done, .. }) = self.tool(&id) {
                    *done = Some((ok, output));
                }
            }
            RunEvent::SubagentTool { parent, id, tool } => {
                if let Some(a) = self.agent(&parent) {
                    match a.call(&id) {
                        Some(c) => c.tool = tool,
                        None => a.calls.push(Call {
                            id,
                            tool,
                            done: None,
                            open: false,
                        }),
                    }
                }
            }
            RunEvent::SubagentToolDone {
                parent,
                id,
                ok,
                output,
            } => {
                if let Some(c) = self.agent(&parent).and_then(|a| a.call(&id)) {
                    c.done = Some((ok, output));
                }
            }
            RunEvent::SubagentText { parent, text } => {
                if let Some(a) = self.agent(&parent) {
                    if !a.said.is_empty() {
                        a.said.push_str("\n\n");
                    }
                    a.said.push_str(text.trim());
                    a.blocks = None;
                }
            }
            RunEvent::Woke => {
                if self.run.is_none() {
                    self.run = Some(Run {
                        since: Instant::now(),
                        said: false,
                    });
                }
            }
            RunEvent::Approval {
                request,
                tool,
                reason,
                always,
            } => self.items.push(Item::Approval {
                request,
                tool,
                reason,
                always,
                answer: None,
                expired: false,
            }),
            RunEvent::Context { used, window } => self.context = Some((used, window)),
            RunEvent::Steered | RunEvent::Usage { .. } | RunEvent::Limited { .. } => {}
            RunEvent::Error { message } => self.items.push(Item::Error(message)),
            RunEvent::Compacting => self.compacting = true,
            RunEvent::Compacted { before, after } => {
                self.compacting = false;
                if let (Some(a), Some((_, window))) = (after, self.context) {
                    self.context = Some((a, window));
                }
                self.note(match (before, after) {
                    (Some(b), Some(a)) => format!(
                        "Compacted the conversation from {} to {} tokens.",
                        tokens(b),
                        tokens(a)
                    ),
                    (Some(b), None) => {
                        format!("Compacted the conversation from {} tokens.", tokens(b))
                    }
                    _ => "Compacted the conversation.".into(),
                });
            }
            RunEvent::Finished {
                status,
                ms,
                text,
                error,
            } => {
                self.compacting = false;
                let run = self.run.take();
                if !text.is_empty() && !run.as_ref().is_some_and(|r| r.said) {
                    self.items.push(Item::Text {
                        source: text,
                        blocks: None,
                    });
                }
                self.expire();
                self.last = Some(status);
                // the CLI often reports a failure as an error and again as the run's end
                let said = |e: &String| {
                    self.items
                        .iter()
                        .rev()
                        .take_while(|i| !matches!(i, Item::User { .. }))
                        .any(|i| matches!(i, Item::Error(m) if m == e))
                };
                let error = error.filter(|e| !said(e));
                self.items.push(Item::Finished { status, ms, error });
            }
            RunEvent::Failed { message } => {
                self.items.push(Item::Error(message));
                self.end_session();
            }
            RunEvent::Changes {
                before,
                after,
                files,
            } => self.items.push(Item::Changes {
                before,
                after,
                files,
                all: false,
                undo: Undo::Ready,
            }),
            RunEvent::Undone { before, result } => {
                let card = self.items.iter_mut().rev().find_map(|i| match i {
                    Item::Changes {
                        before: b, undo, ..
                    } if *b == before => Some(undo),
                    _ => None,
                });
                if let Some(undo) = card {
                    *undo = match result {
                        Ok(line) => Undo::Done(line),
                        Err(e) => Undo::Failed(e),
                    };
                }
            }
        }
    }

    /// After a journal replay: whatever was live when the app closed is over now.
    pub fn settle(&mut self) -> Option<usize> {
        self.compacting = false;
        let cut = self.run.take().map(|_| {
            self.note("The app closed before this run finished.");
            self.items.len() - 1
        });
        self.expire();
        self.stop_agents();
        cut
    }

    fn end_session(&mut self) {
        self.run = None;
        self.compacting = false;
        self.failed = true;
        self.expire();
        self.stop_agents();
    }

    // a subagent outlives its run, not its session
    fn stop_agents(&mut self) {
        for item in &mut self.items {
            if let Item::Agent(a) = item
                && a.state == AgentState::Working
            {
                a.state = AgentState::Stopped;
            }
        }
    }

    fn agent(&mut self, id: &str) -> Option<&mut Subagent> {
        self.items.iter_mut().rev().find_map(|i| match i {
            Item::Agent(a) if a.id == id => Some(a),
            _ => None,
        })
    }

    // an approval left open when its run or session ended can't be answered any more
    fn expire(&mut self) {
        for item in &mut self.items {
            if let Item::Approval {
                answer: None,
                expired,
                ..
            } = item
            {
                *expired = true;
            }
        }
    }

    fn tool(&mut self, id: &str) -> Option<&mut Item> {
        self.items
            .iter_mut()
            .rev()
            .find(|i| matches!(i, Item::Tool { id: t, .. } if t == id))
    }

    /// Parses any reply text that changed since the last paint.
    pub fn parse(&mut self, skip: Option<usize>) {
        let mut command = false;
        for (ix, item) in self.items.iter_mut().enumerate() {
            match item {
                Item::User { text, .. } => command = slash(text),
                Item::Text { .. } if Some(ix) == skip => {}
                Item::Text { source, blocks } if blocks.is_none() => {
                    *blocks = Some(reply(source, command));
                }
                Item::Agent(a) if a.blocks.is_none() => {
                    a.blocks = Some(markdown::parse(a.answer()));
                }
                _ => {}
            }
        }
    }
}

fn tokens(n: u64) -> String {
    if n >= 1000 {
        format!("{}K", (n + 500) / 1000)
    } else {
        n.to_string()
    }
}

fn slash(text: &str) -> bool {
    text.trim_start()
        .strip_prefix('/')
        .and_then(|rest| rest.split_whitespace().next())
        .is_some_and(|name| !name.contains('/'))
}

pub fn reply(source: &str, command: bool) -> Vec<Block> {
    if !command {
        return markdown::parse(source);
    }
    let kept: Vec<String> = source.lines().map(|l| format!("{l}  ")).collect();
    markdown::parse(&kept.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_to_slash_commands_keep_their_lines() {
        assert!(slash("/usage") && slash(" /compact now"));
        assert!(!slash("/usr/bin/env") && !slash("hi /usage"));
        let mut t = Transcript::default();
        t.prompt(&Prompt::text("/usage"));
        t.apply(text("a: 1\nb: 2"));
        assert!(t.after_command(t.items.len()));
        assert_ne!(reply("a: 1\nb: 2", true), reply("a: 1\nb: 2", false));
    }

    fn text(s: &str) -> RunEvent {
        RunEvent::Text { text: s.into() }
    }

    fn finished(status: RunStatus, text: &str) -> RunEvent {
        RunEvent::Finished {
            status,
            ms: 1200,
            text: text.into(),
            error: None,
        }
    }

    fn approval(request: &str) -> RunEvent {
        RunEvent::Approval {
            request: request.into(),
            tool: Tool::Command {
                command: "ls".into(),
            },
            reason: None,
            always: true,
        }
    }

    #[test]
    fn a_run_streams_then_finishes() {
        let mut t = Transcript::default();
        assert_eq!(t.status(), Status::Idle);
        t.prompt(&Prompt::text("hi"));
        assert_eq!(t.status(), Status::Working);
        t.apply(text("Hel"));
        t.apply(text("lo"));
        t.apply(RunEvent::Usage {
            input: 10,
            output: 2,
        });
        t.apply(finished(RunStatus::Done, "Hello"));
        assert_eq!(t.status(), Status::Done);
        // the final text is not added twice when it already streamed
        let texts: Vec<_> = t
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Text { source, .. } => Some(source.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["Hello"]);
        assert!(matches!(t.items.last(), Some(Item::Finished { .. })));
    }

    #[test]
    fn steers_join_the_live_run_and_approvals_wait() {
        let mut t = Transcript::default();
        t.prompt(&Prompt::text("go"));
        t.prompt(&Prompt::text("left"));
        assert!(matches!(&t.items[1], Item::User { steer: true, .. }));
        t.apply(approval("r1"));
        assert_eq!(t.status(), Status::Waiting);
        t.answered("r1", Answer::AllowAlways);
        assert_eq!(t.status(), Status::Working);
        t.apply(approval("r2"));
        t.apply(finished(RunStatus::Done, "ok"));
        assert!(matches!(
            &t.items[3],
            Item::Approval {
                expired: true,
                answer: None,
                ..
            }
        ));
        // a reply that never streamed shows from the final text
        assert!(matches!(&t.items[4], Item::Text { source, .. } if source == "ok"));
        // the next prompt starts a fresh run, not a steer
        t.prompt(&Prompt::text("again"));
        assert!(matches!(
            t.items.last(),
            Some(Item::User { steer: false, .. })
        ));
    }

    #[test]
    fn tools_update_in_place_and_failures_end_the_session() {
        let mut t = Transcript::default();
        t.prompt(&Prompt::text("x"));
        let tool = |c: &str| RunEvent::Tool {
            id: "t1".into(),
            tool: Tool::Command { command: c.into() },
        };
        t.apply(tool("l"));
        t.apply(tool("ls"));
        t.apply(RunEvent::ToolDone {
            id: "t1".into(),
            ok: true,
            output: "a.txt".into(),
        });
        let tools: Vec<_> = t
            .items
            .iter()
            .filter(|i| matches!(i, Item::Tool { .. }))
            .collect();
        assert_eq!(tools.len(), 1);
        assert!(
            matches!(tools[0], Item::Tool { done: Some((true, _)), tool: Tool::Command { command }, .. } if command == "ls")
        );
        t.apply(RunEvent::Failed {
            message: "gone".into(),
        });
        assert_eq!(t.status(), Status::Failed);
        assert!(!t.running());
    }

    #[test]
    fn a_failure_is_said_once() {
        let mut t = Transcript::default();
        t.prompt(&Prompt::text("x"));
        t.apply(RunEvent::Error {
            message: "model refused".into(),
        });
        t.apply(RunEvent::Usage {
            input: 0,
            output: 0,
        });
        t.apply(RunEvent::Finished {
            status: RunStatus::Failed,
            ms: 5,
            text: String::new(),
            error: Some("model refused".into()),
        });
        assert!(matches!(
            t.items.last(),
            Some(Item::Finished { error: None, .. })
        ));
    }

    fn spawn(id: &str) -> RunEvent {
        RunEvent::Tool {
            id: id.into(),
            tool: Tool::Agent {
                description: "Write a poem".into(),
                agent_type: "general-purpose".into(),
                prompt: "Four lines.".into(),
            },
        }
    }

    fn agent(t: &Transcript, id: &str) -> AgentState {
        t.items
            .iter()
            .find_map(|i| match i {
                Item::Agent(a) if a.id == id => Some(a.state),
                _ => None,
            })
            .expect("a subagent card")
    }

    #[test]
    fn a_subagent_gathers_its_calls_and_outlives_the_run() {
        let mut t = Transcript::default();
        t.prompt(&Prompt::text("go"));
        t.apply(spawn("a1"));
        t.apply(spawn("a2"));
        t.apply(RunEvent::SubagentTool {
            parent: "a1".into(),
            id: "s1".into(),
            tool: Tool::Command {
                command: "ls".into(),
            },
        });
        t.apply(RunEvent::SubagentToolDone {
            parent: "a1".into(),
            id: "s1".into(),
            ok: true,
            output: "a.txt".into(),
        });
        t.apply(RunEvent::SubagentText {
            parent: "a1".into(),
            text: "Found it.".into(),
        });
        t.apply(RunEvent::ToolDone {
            id: "a1".into(),
            ok: true,
            output: "Report.".into(),
        });
        t.apply(finished(RunStatus::Done, "ok"));
        // two cards, no tool rows, and the main reply holds none of the subagent's text
        assert_eq!(
            t.items
                .iter()
                .filter(|i| matches!(i, Item::Agent(_)))
                .count(),
            2
        );
        assert!(!t.items.iter().any(|i| matches!(i, Item::Tool { .. })));
        let Some(Item::Agent(a)) = t.items.iter().find(|i| matches!(i, Item::Agent(_))) else {
            panic!()
        };
        assert_eq!((a.calls.len(), a.answer()), (1, "Report."));
        assert!(matches!(a.calls[0].done, Some((true, _))));
        assert_eq!(agent(&t, "a1"), AgentState::Done);
        // the background one still works after the run ended, and the timer keeps counting
        assert_eq!(agent(&t, "a2"), AgentState::Working);
        assert!(!t.running() && t.ticking());
        // the CLI picks up its result in a run of its own
        t.apply(RunEvent::ToolDone {
            id: "a2".into(),
            ok: false,
            output: String::new(),
        });
        assert_eq!(agent(&t, "a2"), AgentState::Failed);
        t.apply(RunEvent::Woke);
        assert_eq!(t.status(), Status::Working);
        t.apply(finished(RunStatus::Done, "both done"));
        assert!(
            matches!(&t.items[t.items.len() - 2], Item::Text { source, .. } if source == "both done")
        );
        t.parse(None);
    }

    #[test]
    fn a_subagent_left_working_stops_with_the_session() {
        let mut t = Transcript::default();
        t.prompt(&Prompt::text("go"));
        t.apply(spawn("a1"));
        t.apply(finished(RunStatus::Done, "ok"));
        t.settle();
        assert_eq!(agent(&t, "a1"), AgentState::Stopped);
        assert!(!t.ticking());
    }

    #[test]
    fn a_replay_cut_off_mid_run_settles() {
        let mut t = Transcript::default();
        t.apply(RunEvent::Started {
            agent: hyprspace_proto::Agent::Claude,
            model: "m".into(),
            thread: "th".into(),
            cwd: PathBuf::from("/w"),
        });
        t.prompt(&Prompt::text("x"));
        t.apply(approval("r1"));
        t.settle();
        assert_eq!(t.thread.as_deref(), Some("th"));
        assert!(!t.running());
        assert!(matches!(t.items.last(), Some(Item::Note(_))));
        assert_ne!(t.status(), Status::Waiting);
        t.parse(None);
    }

    #[test]
    fn a_run_s_changes_card_takes_its_undo() {
        use hyprspace_proto::run::{ChangeKind, RunFile};
        let mut t = Transcript::default();
        let file = RunFile {
            path: "a.rs".into(),
            kind: ChangeKind::Update,
            added: 2,
            removed: 1,
        };
        for before in ["t1", "t2"] {
            t.apply(RunEvent::Changes {
                before: before.into(),
                after: "x".into(),
                files: vec![file.clone()],
            });
        }
        t.apply(RunEvent::Undone {
            before: "t1".into(),
            result: Ok("Put back 1 file.".into()),
        });
        let undos: Vec<&Undo> = t
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Changes { undo, .. } => Some(undo),
                _ => None,
            })
            .collect();
        assert_eq!(
            undos,
            [&Undo::Done("Put back 1 file.".into()), &Undo::Ready]
        );
    }

    #[test]
    fn a_compaction_leaves_a_note() {
        let mut t = Transcript {
            context: Some((38_400, 1_000_000)),
            ..Default::default()
        };
        t.apply(RunEvent::Compacting);
        assert!(t.compacting);
        t.apply(RunEvent::Compacted {
            before: Some(38_400),
            after: Some(5_600),
        });
        assert!(!t.compacting);
        assert_eq!(t.context, Some((5_600, 1_000_000)));
        t.apply(RunEvent::Compacted {
            before: None,
            after: None,
        });
        let notes: Vec<&str> = t
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Note(n) => Some(n.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            notes,
            [
                "Compacted the conversation from 38K to 6K tokens.",
                "Compacted the conversation."
            ]
        );
        t.apply(RunEvent::Compacting);
        t.apply(RunEvent::Failed {
            message: "gone".into(),
        });
        assert!(!t.compacting);
        t.apply(RunEvent::Compacting);
        t.settle();
        assert!(!t.compacting);
    }

    #[test]
    #[ignore = "a measurement: cargo test --release -p hyprspace-ui perf_ -- --ignored --nocapture"]
    fn perf_opening_a_long_thread() {
        let reply = "Here is what changed, with `code` and **bold**.\n\n- a\n- b\n".repeat(8);
        let t = std::time::Instant::now();
        let mut m = Transcript::default();
        for i in 0..1_000 {
            m.prompt(&Prompt::text(format!("prompt {i}")));
            m.apply(RunEvent::Tool {
                id: format!("t{i}"),
                tool: Tool::Read {
                    path: "src/a.rs".into(),
                },
            });
            m.apply(RunEvent::ToolDone {
                id: format!("t{i}"),
                ok: true,
                output: "ok".into(),
            });
            m.apply(text(&reply));
            m.apply(RunEvent::Finished {
                status: RunStatus::Done,
                ms: 10,
                text: String::new(),
                error: None,
            });
        }
        let replay = t.elapsed();
        let t = std::time::Instant::now();
        m.parse(None);
        let parsed = t.elapsed();
        eprintln!("thread: {replay:?} to replay 1,000 runs, {parsed:?} to parse their replies");
        assert!(replay + parsed < std::time::Duration::from_millis(250));
    }
}
