import { X } from "lucide-react"
import { ClaudePane } from "../sessions"
import { Mark } from "./bits"

export function TerminalThread() {
  return (
    <div className="app overflow-hidden rounded-2xl border border-white/[0.12] p-2 shadow-[0_24px_56px_-28px_rgba(0,0,0,0.7)]">
      <div className="flex h-[30px] items-center gap-1 pr-1.5 pl-2">
        <Mark agent="claude" />
        <span className="ml-[3px] min-w-0 shrink truncate text-[12px] font-semibold">Profile the git poll</span>
        <span className="flex min-w-2 flex-1 pl-1.5">
          <span className="truncate rounded-[5px] bg-white/[0.05] px-1.5 py-px text-[11px] text-(--t3)">Opus 5.5</span>
        </span>
        <span className="flex size-[22px] items-center justify-center text-(--t3)">
          <X className="size-[13px]" />
        </span>
      </div>
      <div className="overflow-hidden border border-(--b1) p-3.5">
        <ClaudePane />
      </div>
    </div>
  )
}
