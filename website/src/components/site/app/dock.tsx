import { ArrowUp, Check, FileCode, FolderTree, GitBranch, X } from "lucide-react"
import { cn } from "@/lib/utils"
import { Counts } from "./bits"

const FILES: { name: string; dir: string; added: number; removed: number; status: string; ticked: boolean }[] = [
  { name: "terminal.rs", dir: "crates/engine/src", added: 18, removed: 4, status: "M", ticked: true },
  { name: "hooks.rs", dir: "crates/engine/src", added: 6, removed: 2, status: "M", ticked: true },
  { name: "resume.rs", dir: "crates/engine/tests", added: 40, removed: 6, status: "A", ticked: false },
]

export function Tick({ on, some }: { on?: boolean; some?: boolean }) {
  return (
    <span
      className={cn(
        "flex size-[15px] shrink-0 items-center justify-center rounded-[4px] border",
        on || some ? "border-(--accent) bg-(--accent)" : "border-[rgb(116_116_116/0.55)]",
      )}
    >
      {on && <Check className="size-[11px] text-(--on-accent)" strokeWidth={3} />}
      {some && <i className="block h-[1.5px] w-[7px] bg-(--on-accent)" />}
    </span>
  )
}

export function GitFiles() {
  return (
    <div className="px-1.5 py-1">
      {FILES.map((f) => (
        <div key={f.name} className="flex h-7 items-center gap-2 rounded-md px-1.5">
          <Tick on={f.ticked} />
          <span className="flex min-w-0 flex-1 items-center gap-[7px]">
            <FileCode className="size-3.5 shrink-0 text-[#f2ad73]" />
            <span className="truncate text-[12.5px]">{f.name}</span>
            <span className="min-w-0 flex-1 truncate text-[11px] text-(--t3)">{f.dir}</span>
          </span>
          <Counts added={f.added} removed={f.removed} className="gap-[5px] text-[10.5px]" />
          <span
            className={cn(
              "w-3 text-center font-app-mono text-[10.5px] font-semibold",
              f.status === "A" ? "text-(--ok)" : "text-(--busy)",
            )}
          >
            {f.status}
          </span>
        </div>
      ))}
    </div>
  )
}

export function CommitBox() {
  return (
    <div className="flex flex-col gap-2 border-t border-(--b1) p-3">
      <div className="flex flex-col rounded-lg border border-(--b2) bg-(--s2) text-[12.5px]">
        <div className="px-2.5 py-2 font-medium">Resume a conversation after a restart</div>
        <i className="block h-px bg-white/[0.06]" />
        <div className="min-h-14 px-2.5 py-2 text-(--t3)">Description</div>
      </div>
      <div className="flex gap-1.5">
        <span className="flex h-[26px] flex-1 items-center justify-center rounded-md bg-(--accent) text-[12px] font-medium text-(--on-accent)">
          Commit to resume-fix
        </span>
        <span className="flex h-[26px] flex-1 items-center justify-center rounded-lg bg-white/[0.06] text-[12px] font-medium">
          Commit and push
        </span>
      </div>
      <div className="text-[11px] text-(--t3)">2 of 3 files ticked</div>
    </div>
  )
}

export function Dock() {
  return (
    <aside className="hidden w-[320px] shrink-0 flex-col border-l border-(--b0) xl:flex">
      <div className="flex h-[38px] shrink-0 items-center gap-0.5 border-b border-(--b1) px-1.5">
        <span className="flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[12.5px] font-medium text-(--t3)">
          <FolderTree className="size-[13px]" />
          Files
        </span>
        <span className="flex items-center gap-1.5 rounded-md bg-(--s3) px-2.5 py-1.5 text-[12.5px] font-medium">
          <GitBranch className="size-[13px]" />
          Git
          <span className="rounded bg-white/[0.08] px-[5px] font-app-mono text-[10.5px] text-(--t2)">3</span>
        </span>
        <span className="flex-1" />
        <span className="flex size-[26px] items-center justify-center text-(--t3)">
          <X className="size-3.5" />
        </span>
      </div>

      <div className="flex h-9 shrink-0 items-center gap-[7px] border-b border-(--b1) px-3">
        <GitBranch className="size-[13px] text-(--t3)" />
        <span className="flex-1 truncate font-app-mono text-[12px]">resume-fix</span>
        <span className="flex h-6 items-center gap-1 rounded-md border border-(--b2) bg-(--s2) px-[9px] text-[11.5px] font-medium">
          <ArrowUp className="size-3" />
          Push 2
        </span>
      </div>

      <div className="flex h-8 shrink-0 items-center border-b border-(--b1) px-3">
        <span className="flex items-center gap-2 text-[12px] text-(--t2)">
          <Tick some />3 changed files
        </span>
        <span className="flex-1" />
        <Counts added={64} removed={12} className="text-[11px]" />
      </div>

      <div className="flex-1">
        <GitFiles />
      </div>
      <CommitBox />
    </aside>
  )
}
