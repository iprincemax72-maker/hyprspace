// Reading a working tree: what changed, the diff of one file, and the branch.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use hyprspace_proto::git::{BranchInfo, FileChange};

use super::{Result, empty, git, git_cmd};

pub fn ls_files(cwd: &Path) -> Result<Vec<String>> {
    let out = git(cwd, &["ls-files", "-co", "--exclude-standard", "-z"])?;
    Ok(out
        .split('\0')
        .filter(|f| !f.is_empty())
        .map(String::from)
        .collect())
}

/// Whether `cwd` is inside a git work tree.
pub fn is_repo(cwd: &Path) -> bool {
    !empty(cwd) && git(cwd, &["rev-parse", "--is-inside-work-tree"]).is_ok()
}

/// The top folder of the repo holding `cwd`, which change paths are relative to.
pub fn root(cwd: &Path) -> Option<PathBuf> {
    if empty(cwd) {
        return None;
    }
    let top = git(cwd, &["rev-parse", "--show-toplevel"]).ok()?;
    Some(PathBuf::from(top.trim()))
}

/// Changed files in the repo containing `cwd`, with +/- line counts where git has them. Not a
/// repo means nothing to show, quietly.
pub fn changes(cwd: &Path) -> Result<Vec<FileChange>> {
    if !is_repo(cwd) {
        return Ok(vec![]);
    }
    changes_in_repo(cwd)
}

/// `changes` for a folder already known to be in a repo.
pub fn changes_in_repo(cwd: &Path) -> Result<Vec<FileChange>> {
    let status = git(
        cwd,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    Ok(counted(cwd, porcelain(&status)))
}

pub fn summary_in_repo(cwd: &Path) -> Result<(Vec<FileChange>, BranchInfo)> {
    let out = git(
        cwd,
        &[
            "status",
            "--porcelain=v1",
            "--branch",
            "-z",
            "--untracked-files=all",
        ],
    )?;
    let (head, rest) = match out.strip_prefix("## ") {
        Some(h) => h.split_once('\0').unwrap_or((h, "")),
        None => ("", out.as_str()),
    };
    Ok((counted(cwd, porcelain(rest)), head_line(head)))
}

fn counted(cwd: &Path, entries: Vec<(String, String)>) -> Vec<FileChange> {
    let touched = |column: usize| {
        entries
            .iter()
            .any(|(code, _)| !matches!(code.as_bytes()[column], b' ' | b'?' | b'!'))
    };
    // line counts from staged + unstaged numstat, summed per path
    let mut counts: HashMap<String, (u32, u32)> = HashMap::new();
    for (column, args) in [
        (1, ["diff", "--numstat"].as_slice()),
        (0, ["diff", "--numstat", "--cached"].as_slice()),
    ] {
        if !touched(column) {
            continue;
        }
        if let Ok(s) = git(cwd, args) {
            for (path, a, r) in numstat(&s) {
                let e = counts.entry(path).or_insert((0, 0));
                e.0 += a;
                e.1 += r;
            }
        }
    }
    entries
        .into_iter()
        .map(|(status, path)| {
            let (added, removed) = counts.get(&path).copied().unwrap_or((0, 0));
            FileChange {
                path,
                status,
                added,
                removed,
            }
        })
        .collect()
}

fn head_line(head: &str) -> BranchInfo {
    let mut bi = BranchInfo {
        is_repo: true,
        ..BranchInfo::default()
    };
    if head.starts_with("No commits yet on ") || head.starts_with("Initial commit on ") {
        return bi;
    }
    if head.starts_with("HEAD (no branch)") {
        bi.branch = "HEAD".into();
        return bi;
    }
    let (names, tracking) = match head.split_once(" [") {
        Some((n, t)) => (n, t.trim_end_matches(']')),
        None => (head, ""),
    };
    let (branch, upstream) = match names.split_once("...") {
        Some((b, _)) => (b, true),
        None => (names, false),
    };
    bi.branch = branch.to_string();
    if upstream && tracking != "gone" {
        bi.upstream = true;
        for part in tracking.split(", ") {
            if let Some(n) = part.strip_prefix("ahead ") {
                bi.ahead = n.parse().unwrap_or(0);
            } else if let Some(n) = part.strip_prefix("behind ") {
                bi.behind = n.parse().unwrap_or(0);
            }
        }
    }
    bi
}

// "<added>\t<removed>\t<path>" per line; binary files report "-" and count as 0
fn numstat(s: &str) -> Vec<(String, u32, u32)> {
    s.lines()
        .filter_map(|line| {
            let mut p = line.split('\t');
            let a = p.next()?.parse().unwrap_or(0);
            let r = p.next()?.parse().unwrap_or(0);
            Some((p.next()?.to_string(), a, r))
        })
        .collect()
}

// -z gives NUL-separated records with NO quoting or escaping, so odd paths survive intact. Each
// record is "XY <path>"; a rename or copy (R or C in either status column) is followed by an EXTRA
// NUL-terminated field holding the original path: take the new path and skip that field.
fn porcelain(s: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    let mut parts = s.split('\0');
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue; // trailing empty field, or a malformed short record
        }
        let code = &entry[..2];
        let b = code.as_bytes();
        if b[0] == b'R' || b[0] == b'C' || b[1] == b'R' || b[1] == b'C' {
            let _ = parts.next();
        }
        out.push((code.to_string(), entry[3..].to_string()));
    }
    out
}

/// Unified diff for one file. Falls back through HEAD, then staged, then untracked (all added).
pub fn diff(cwd: &Path, path: &str) -> Result<String> {
    if empty(cwd) {
        return Ok(String::new());
    }
    let d = git(cwd, &["diff", "HEAD", "--", path]).unwrap_or_default();
    if !d.trim().is_empty() {
        return Ok(d);
    }
    let staged = git(cwd, &["diff", "--cached", "--", path]).unwrap_or_default();
    if !staged.trim().is_empty() {
        return Ok(staged);
    }
    // untracked file: diff against nothing so it shows as all-added. --no-index exits 1 when the
    // files differ, which is expected here, so read stdout regardless of status.
    let out = git_cmd()
        .arg("-C")
        .arg(cwd)
        .args(["diff", "--no-index", "--", "/dev/null", path])
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// The current branch and how far ahead or behind its upstream it is.
pub fn branch_info(cwd: &Path) -> BranchInfo {
    if !is_repo(cwd) {
        return BranchInfo::default();
    }
    branch_in_repo(cwd)
}

/// `branch_info` for a folder already known to be in a repo.
pub fn branch_in_repo(cwd: &Path) -> BranchInfo {
    let mut bi = BranchInfo {
        is_repo: true,
        ..BranchInfo::default()
    };
    bi.branch = git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_default()
        .trim()
        .to_string();
    // "<behind>\t<ahead>" relative to the upstream, if one is set
    if let Ok(s) = git(
        cwd,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    ) {
        let mut it = s.split_whitespace();
        bi.behind = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        bi.ahead = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        bi.upstream = true;
    }
    bi
}

#[cfg(test)]
mod tests {
    use super::super::testing;
    use super::*;

    #[test]
    fn porcelain_skips_the_rename_source() {
        let s = "R  new.txt\0old.txt\0 M a b.txt\0?? x\0";
        assert_eq!(
            porcelain(s),
            [
                ("R ".to_string(), "new.txt".to_string()),
                (" M".to_string(), "a b.txt".to_string()),
                ("??".to_string(), "x".to_string()),
            ]
        );
    }

    #[test]
    fn numstat_counts_binary_files_as_zero() {
        assert_eq!(
            numstat("3\t1\ta.rs\n-\t-\timg.png\n"),
            [("a.rs".to_string(), 3, 1), ("img.png".to_string(), 0, 0)]
        );
    }

    #[test]
    fn reads_changes_diff_and_branch_from_a_real_repo() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        assert!(!is_repo(cwd));
        assert_eq!(changes(cwd).unwrap(), []);

        testing::repo(cwd);
        std::fs::write(cwd.join("a.txt"), "one\n").unwrap();
        testing::commit_all(cwd, "first");
        std::fs::write(cwd.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(cwd.join("new.txt"), "hi\n").unwrap();

        let mut got = changes(cwd).unwrap();
        got.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(got.len(), 2);
        assert_eq!(
            (got[0].path.as_str(), got[0].status.as_str()),
            ("a.txt", " M")
        );
        assert_eq!((got[0].added, got[0].removed), (1, 0));
        assert_eq!(
            (got[1].path.as_str(), got[1].status.as_str()),
            ("new.txt", "??")
        );

        assert!(diff(cwd, "a.txt").unwrap().contains("+two"));
        assert!(diff(cwd, "new.txt").unwrap().contains("+hi"));

        assert_eq!(
            root(cwd).unwrap().canonicalize().unwrap(),
            cwd.canonicalize().unwrap()
        );
        let bi = branch_info(cwd);
        assert!(bi.is_repo);
        assert_eq!(bi.branch, "main");
        assert!(!bi.upstream);
    }

    fn sorted(mut v: Vec<FileChange>) -> Vec<FileChange> {
        v.sort_by(|a, b| a.path.cmp(&b.path));
        v
    }

    fn same_as_six_calls(cwd: &Path, state: &str) {
        let (changes, branch) = summary_in_repo(cwd).unwrap();
        assert_eq!(
            sorted(changes),
            sorted(changes_in_repo(cwd).unwrap()),
            "{state}"
        );
        assert_eq!(branch, branch_in_repo(cwd), "{state}");
    }

    #[test]
    fn one_status_reads_what_six_calls_did() {
        let dir = tempfile::tempdir().unwrap();
        let (work, remote) = (dir.path().join("work"), dir.path().join("remote.git"));
        std::fs::create_dir_all(&work).unwrap();
        let cwd = work.as_path();
        testing::repo(cwd);
        std::fs::write(cwd.join("loose.txt"), "x\n").unwrap();
        same_as_six_calls(cwd, "no commits yet");

        testing::commit_all(cwd, "first");
        same_as_six_calls(cwd, "clean");

        testing::bare(&remote);
        testing::push_upstream(cwd, &remote);
        std::fs::write(cwd.join("a.txt"), "one\n").unwrap();
        testing::commit_all(cwd, "second");
        same_as_six_calls(cwd, "ahead of its upstream");

        std::fs::write(cwd.join("a.txt"), "one\ntwo\n").unwrap();
        super::git(cwd, &["add", "a.txt"]).unwrap();
        std::fs::write(cwd.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        std::fs::write(cwd.join("b c.txt"), "spaced\n").unwrap();
        super::git(cwd, &["mv", "loose.txt", "moved.txt"]).unwrap();
        same_as_six_calls(cwd, "staged, unstaged, renamed and untracked");
        let (changes, _) = summary_in_repo(cwd).unwrap();
        let a = changes.iter().find(|c| c.path == "a.txt").unwrap();
        assert_eq!((a.status.as_str(), a.added, a.removed), ("MM", 2, 0));

        super::git(cwd, &["config", "branch.main.merge", "refs/heads/gone"]).unwrap();
        let raw = super::git(cwd, &["status", "--porcelain=v1", "--branch", "-z"]).unwrap();
        assert!(raw.starts_with("## main...origin/gone [gone]"), "{raw}");
        same_as_six_calls(cwd, "upstream gone");

        super::git(cwd, &["checkout", "-q", "--detach"]).unwrap();
        same_as_six_calls(cwd, "detached");
    }

    #[test]
    fn head_lines_read_like_branch_in_repo() {
        let bi = head_line("main...origin/main [ahead 2, behind 3]");
        assert_eq!(
            (bi.branch.as_str(), bi.upstream, bi.ahead, bi.behind),
            ("main", true, 2, 3)
        );
        let bi = head_line("feature/x...origin/feature/x");
        assert_eq!((bi.branch.as_str(), bi.upstream), ("feature/x", true));
        assert!(!head_line("main...origin/main [gone]").upstream);
        assert_eq!(head_line("main").branch, "main");
        assert_eq!(head_line("HEAD (no branch)").branch, "HEAD");
        assert_eq!(head_line("No commits yet on main").branch, "");
    }
}
