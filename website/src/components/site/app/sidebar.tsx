import { Check, ChevronRight, Folder, GitBranch, Plus, RefreshCw, Search, Settings } from "lucide-react"
import { cn } from "@/lib/utils"
import { AgentMark, Tag, type Agent } from "./bits"

export function stopwatch(secs: number) {
  if (secs < 60) return `${secs}s`
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${String(secs % 60).padStart(2, "0")}s`
  return `${Math.floor(secs / 3600)}h ${String(Math.floor(secs / 60) % 60).padStart(2, "0")}m`
}

type Row = {
  space: string
  title: string
  agent: Agent
  state: "working" | "waiting" | "done" | "idle"
  since?: number
  ago?: string
  doing?: string
  place?: [string, "branch" | "folder"]
  selected?: boolean
}

export const ROWS: Row[] = [
  {
    space: "hyprspace",
    title: "Keep a conversation through a restart",
    agent: "claude",
    state: "working",
    since: 187,
    doing: "Read crates/engine/src/persist.rs",
    selected: true,
  },
  {
    space: "website",
    title: "Rewrite the hero for the new app",
    agent: "codex",
    state: "waiting",
    since: 412,
    doing: "Wants to run bun run build",
  },
  {
    space: "Agent-Sdk",
    title: "Stream tool results to the phone",
    agent: "claude",
    state: "working",
    since: 64,
    doing: "2 subagents running",
  },
  {
    space: "hyprspace",
    title: "Cache the dock view",
    agent: "claude",
    state: "done",
    ago: "4m",
    place: ["perf-dock", "branch"],
  },
  {
    space: "website",
    title: "Fix the install script on macOS",
    agent: "codex",
    state: "idle",
    ago: "1h",
    place: ["main", "branch"],
  },
  {
    space: "Agent-Sdk",
    title: "Draft the 0.3 release notes",
    agent: "claude",
    state: "idle",
    ago: "3h",
    place: ["Agent-Sdk", "folder"],
  },
]

export function ThreadRow({ row, secs }: { row: Row; secs: number }) {
  const working = row.state === "working"
  const waiting = row.state === "waiting"
  const recede = working && !row.selected
  return (
    <div
      className={cn(
        "relative flex flex-col gap-[3px] overflow-hidden rounded-lg px-2 pt-2 pb-[9px]",
        row.selected && "bg-(--s3)",
        !row.selected && waiting && "bg-[rgb(59_130_246/0.07)]",
        recede && "opacity-70",
      )}
    >
      {working && (
        <span className="hs-sheen pointer-events-none absolute inset-y-0 flex w-[60%]">
          <i className="flex-1 bg-gradient-to-r from-transparent to-[rgb(245_158_11/0.035)]" />
          <i className="flex-1 bg-gradient-to-r from-[rgb(245_158_11/0.035)] to-transparent" />
        </span>
      )}
      <div className="flex min-w-0 items-center gap-[7px]">
        <Tag space={row.space} />
        <span className="min-w-0 flex-1 truncate text-[11.5px] text-(--t3)">{row.space}</span>
        {row.since !== undefined ? (
          <span className="shrink-0 font-app-mono text-[10.5px] font-medium text-(--busy)">
            {stopwatch(row.since + secs)}
          </span>
        ) : (
          <span className="shrink-0 text-[11px] text-(--t3)">{row.ago}</span>
        )}
      </div>
      <div className={cn("truncate text-[13.5px] text-(--t1)", recede ? "font-normal" : "font-medium")}>
        {row.title}
      </div>
      <div className="flex min-w-0 items-center gap-1.5 text-[11px] text-(--t3)">
        {row.doing ? (
          <span className={cn("min-w-0 flex-1 truncate", waiting ? "text-(--wait)" : "text-(--t2)")}>
            {row.doing}
          </span>
        ) : (
          <span className="flex min-w-0 flex-1 items-center gap-[5px]">
            {row.place?.[1] === "branch" ? (
              <GitBranch className="size-[11px] shrink-0" />
            ) : (
              <Folder className="size-[11px] shrink-0" />
            )}
            <span className="min-w-0 truncate font-app-mono text-[10.5px]">{row.place?.[0]}</span>
          </span>
        )}
        {waiting && <i className="hs-wait block size-[7px] shrink-0 rounded-full bg-(--wait)" />}
        {row.state === "done" && (
          <span className="flex size-3 shrink-0 items-center justify-center rounded-full bg-(--ok)">
            <Check className="size-[9px] text-white" strokeWidth={3} />
          </span>
        )}
        <AgentMark agent={row.agent} working={working} />
      </div>
    </div>
  )
}

function Shelf({ label, count }: { label: string; count: number }) {
  return (
    <div className="flex h-[26px] items-center gap-2 px-2 text-[11.5px] font-medium text-(--t3)">
      {label}
      <span className="font-app-mono text-[10.5px]">{count}</span>
      <i className="block h-px flex-1 bg-white/[0.08]" />
      <ChevronRight className="size-3 rotate-90" />
    </div>
  )
}

export function Sidebar({ secs }: { secs: number }) {
  return (
    <aside className="hidden w-[272px] shrink-0 flex-col border-r border-(--b0) lg:flex">
      <div className="mb-1 flex items-center gap-1.5 border-b border-(--b0) px-2 py-2.5">
        <div className="flex h-8 min-w-0 flex-1 items-center gap-2 rounded-lg border border-(--b0) pr-1.5 pl-2.5">
          <Search className="size-3.5 shrink-0 text-(--t3)" />
          <span className="flex-1 text-[13px] text-(--t3)">Search</span>
          <span className="flex h-5 items-center rounded-[5px] border border-(--b1) px-[5px] text-[10.5px] font-medium text-(--t3)">
            Ctrl K
          </span>
        </div>
        <span className="flex size-8 shrink-0 items-center justify-center rounded-lg border border-(--b0) text-(--t2)">
          <Plus className="size-4" />
        </span>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-0.5 px-2 pt-0.5">
        {ROWS.map((row) => (
          <ThreadRow key={row.title} row={row} secs={secs} />
        ))}
        <div className="flex flex-col gap-0.5 pt-2.5">
          <Shelf label="Snoozed" count={2} />
          <Shelf label="Settled" count={14} />
        </div>
      </div>

      <div className="flex items-center border-t border-(--b0) px-2 pt-[7px] pb-2">
        <span className="flex h-7 flex-1 items-center gap-[9px] rounded-md px-2 text-[12.5px] text-(--t2)">
          <Settings className="size-4 text-(--t3)" />
          Settings
        </span>
        <span className="flex size-7 items-center justify-center text-(--t3)">
          <RefreshCw className="size-[15px]" />
        </span>
      </div>
    </aside>
  )
}
