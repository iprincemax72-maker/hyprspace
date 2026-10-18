use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::thread;
use std::time::{Duration, Instant};

use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::{Event, FolderEvent};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

const QUIET: Duration = Duration::from_millis(400);
const EVERY: Duration = Duration::from_secs(2);

#[derive(Clone, Default)]
pub struct Watch(Arc<Mutex<Inner>>);

#[derive(Default)]
struct Inner {
    watcher: Option<RecommendedWatcher>,
    roots: HashMap<String, (PathBuf, Vec<PathBuf>)>,
    tx: Option<UnboundedSender<Event>>,
}

fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    inner.lock().unwrap_or_else(|e| e.into_inner())
}

fn key(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let s = s.trim_end_matches('/');
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

fn noise(path: &str) -> bool {
    ["/.git/objects/", "/.git/logs/"]
        .iter()
        .any(|n| path.contains(n))
}

impl Watch {
    pub fn set(&self, cwds: Vec<PathBuf>, tx: UnboundedSender<Event>) {
        let mut want: HashMap<String, (PathBuf, Vec<PathBuf>)> = HashMap::new();
        for cwd in cwds {
            if let Some(root) = crate::folder::root(&cwd) {
                want.entry(key(&root))
                    .or_insert_with(|| (root, Vec::new()))
                    .1
                    .push(cwd);
            }
        }
        let mut inner = lock(&self.0);
        inner.tx = Some(tx);
        if inner.watcher.is_none() {
            inner.watcher = self.start();
        }
        let old: Vec<(String, PathBuf)> = inner
            .roots
            .iter()
            .map(|(k, (root, _))| (k.clone(), root.clone()))
            .collect();
        if let Some(w) = inner.watcher.as_mut() {
            for (k, root) in &old {
                if !want.contains_key(k) {
                    let _ = w.unwatch(root);
                }
            }
            for (k, (root, _)) in &want {
                if !old.iter().any(|(o, _)| o == k) {
                    let _ = w.watch(root, RecursiveMode::Recursive);
                }
            }
        }
        inner.roots = want;
    }

    fn start(&self) -> Option<RecommendedWatcher> {
        let (tx, rx) = mpsc::channel();
        let watcher = notify::recommended_watcher(tx).ok()?;
        let me = Arc::downgrade(&self.0);
        thread::spawn(move || settle(me, rx));
        Some(watcher)
    }
}

fn settle(me: Weak<Mutex<Inner>>, rx: Receiver<notify::Result<notify::Event>>) {
    let mut dirty: HashMap<String, (Instant, Instant)> = HashMap::new();
    let mut ran: HashMap<String, Instant> = HashMap::new();
    loop {
        let event = match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(event) => Some(event),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if let Some(event) = event {
            let Some(inner) = me.upgrade() else {
                return;
            };
            let roots: Vec<String> = lock(&inner).roots.keys().cloned().collect();
            let now = Instant::now();
            let mut mark = |r: &String| {
                dirty
                    .entry(r.clone())
                    .and_modify(|(_, last)| *last = now)
                    .or_insert((now, now));
            };
            match event {
                // the change buffer overflowed, so any folder may have changed
                Ok(e) if e.need_rescan() => roots.iter().for_each(&mut mark),
                Err(_) => roots.iter().for_each(&mut mark),
                Ok(e) => {
                    for path in &e.paths {
                        let p = key(path);
                        if noise(&p) {
                            continue;
                        }
                        let owner = roots
                            .iter()
                            .filter(|r| p == **r || p.starts_with(&format!("{r}/")))
                            .max_by_key(|r| r.len());
                        if let Some(r) = owner {
                            mark(r);
                        }
                    }
                }
            }
        }
        let now = Instant::now();
        let due: Vec<String> = dirty
            .iter()
            .filter(|(r, (first, last))| {
                let settled = now - *last >= QUIET || now - *first >= EVERY;
                settled && ran.get(*r).is_none_or(|t| now - *t >= EVERY)
            })
            .map(|(r, _)| r.clone())
            .collect();
        for r in due {
            dirty.remove(&r);
            ran.insert(r.clone(), now);
            let Some(inner) = me.upgrade() else {
                return;
            };
            let (cwds, tx) = {
                let inner = lock(&inner);
                (
                    inner
                        .roots
                        .get(&r)
                        .map(|(_, c)| c.clone())
                        .unwrap_or_default(),
                    inner.tx.clone(),
                )
            };
            let Some(tx) = tx else {
                continue;
            };
            for cwd in cwds {
                let status = crate::folder::status(&cwd);
                let _ = tx.unbounded_send(Event::Folder(FolderEvent::Git { cwd, status }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_reads_the_folders_status_again() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().join("repo");
        std::fs::create_dir_all(&cwd).unwrap();
        crate::git::testing::repo(&cwd);
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let watch = Watch::default();
        watch.set(vec![cwd.clone()], tx);
        std::thread::sleep(Duration::from_millis(300));
        std::fs::write(cwd.join("new.txt"), "x").unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = None;
        while Instant::now() < deadline && seen.is_none() {
            if let Ok(Event::Folder(FolderEvent::Git { status, .. })) = rx.try_recv() {
                seen = Some(status);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let status = seen.expect("a status after the file appeared");
        assert!(status.changes.iter().any(|c| c.path == "new.txt"));
    }

    #[test]
    fn git_internals_are_noise_and_paths_match_either_slash() {
        assert!(noise("c:/w/.git/objects/ab/cd"));
        assert!(!noise("c:/w/.git/index"));
        assert!(!noise("c:/w/src/a.rs"));
        if cfg!(windows) {
            assert_eq!(key(Path::new(r"C:\Work\App\")), "c:/work/app");
        }
    }
}
