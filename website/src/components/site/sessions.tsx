import { ClaudeMessage } from "@/components/brainless/claude/claude-message"
import { ClaudeTodoList } from "@/components/brainless/claude/claude-todo-list"
import { ClaudeToolCall } from "@/components/brainless/claude/claude-tool-call"
import { ClaudeDiff } from "@/components/brainless/claude/claude-diff"
import { ClaudeThinking } from "@/components/brainless/claude/claude-thinking"
import { ClaudePrompt } from "@/components/brainless/claude/claude-prompt"

export function ClaudePane() {
  return (
    <div className="space-y-2.5 font-mono text-[13px] leading-[1.6] text-[#c0caf5]">
      <ClaudeMessage role="user">profile the git poll and cut the processes it starts</ClaudeMessage>

      <ClaudeTodoList
        todos={[
          { label: "Count git processes per poll", status: "done" },
          { label: "Read status and branch in one call", status: "active" },
          { label: "Skip numstat when a side is clean", status: "todo" },
        ]}
      />

      <ClaudeToolCall tool="Read" arg="crates/engine/src/git/status.rs" result="Read 412 lines" />

      <ClaudeDiff
        file="crates/engine/src/git/status.rs"
        summary="Updated crates/engine/src/git/status.rs with 3 additions"
        lines={[
          { type: "ctx", n: 61, text: "pub fn summary_in_repo(cwd: &Path) {" },
          { type: "add", n: 62, text: '    let out = git(cwd, &["status", "--branch"])?;' },
          { type: "add", n: 63, text: "    let (head, rest) = split(&out);" },
          { type: "add", n: 64, text: "    Ok((counted(cwd, rest), head_line(head)))" },
        ]}
      />

      <ClaudeThinking />

      <div className="pt-1">
        <ClaudePrompt placeholder="ask anything" />
      </div>
    </div>
  )
}
