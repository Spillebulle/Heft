//! A scan: roots in, a live tree out.
//!
//! A work-stealing pool (rayon) reads one directory per task. A task reads its
//! directory with no lock held, classifies the entries, then takes the tree's
//! write lock **once** to append the whole batch and spawns a task per
//! subdirectory. Readers (the window, the text mode) take the read lock, or
//! `try_read` it so a frame never waits on the walker.
//!
//! Sizes bubble up as each batch lands, so every folder's total is an honest
//! partial total while the scan runs, and final once its `COMPLETE` flag is
//! set. A folder is complete when it and every folder under it has been read.

use crate::tree::{EntryMeta, NONE, NodeId, Tree, flag};
use crate::walk::{self, Kind, RootInfo};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Options {
    /// Descend into other filesystems mounted below a root. Virtual
    /// filesystems are skipped regardless.
    pub cross_filesystems: bool,
    /// Worker threads; 0 picks one per core.
    pub threads: usize,
    /// Read modification times (the Age view). Costs a little on Linux.
    pub mtime: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            cross_filesystems: false,
            threads: 0,
            mtime: true,
        }
    }
}

/// Live counters, readable without any lock.
#[derive(Debug, Clone, Copy, Default)]
pub struct Progress {
    pub entries: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub unreadable: u64,
    /// Entries listed but not sized.
    pub unsizable: u64,
    pub finished: bool,
    pub cancelled: bool,
    pub elapsed: Duration,
}

struct Root {
    fs_path: PathBuf,
    info: RootInfo,
}

pub struct Shared {
    tree: RwLock<Tree>,
    pending: Mutex<HashMap<NodeId, u32>>,
    hardlinks: Mutex<HashSet<(u64, u64)>>,
    roots: Mutex<Vec<Root>>,
    opts: Options,
    cancel: AtomicBool,
    finished: AtomicBool,
    entries: AtomicU64,
    dirs: AtomicU64,
    bytes: AtomicU64,
    unsizable: AtomicU64,
    /// Bumped on every change, so a reader can tell whether to rebuild.
    generation: AtomicU64,
    started: Instant,
    elapsed_ms: AtomicU64,
}

/// A running or finished scan. Cheap to clone; every clone sees the same tree.
#[derive(Clone)]
pub struct Scan {
    shared: Arc<Shared>,
}

impl Scan {
    /// Starts scanning `roots` on a background thread and returns at once.
    pub fn start(roots: Vec<PathBuf>, opts: Options) -> Scan {
        let shared = Arc::new(Shared {
            tree: RwLock::new(Tree::new()),
            pending: Mutex::new(HashMap::new()),
            hardlinks: Mutex::new(HashSet::new()),
            roots: Mutex::new(Vec::new()),
            opts,
            cancel: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            entries: AtomicU64::new(0),
            dirs: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            unsizable: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            started: Instant::now(),
            elapsed_ms: AtomicU64::new(0),
        });
        let s = shared.clone();
        std::thread::Builder::new()
            .name("heft-scan".into())
            .spawn(move || {
                let mut jobs = Vec::new();
                for r in roots {
                    if let Some(job) = add_root(&s, &r) {
                        jobs.push(job);
                    }
                }
                run(&s, jobs);
            })
            .expect("spawn scan thread");
        Scan { shared }
    }

    /// Scans synchronously; for the text mode and tests.
    pub fn run_blocking(roots: Vec<PathBuf>, opts: Options) -> Scan {
        let scan = Scan::start(roots, opts);
        while !scan.shared.finished.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(2));
        }
        scan
    }

    pub fn tree(&self) -> RwLockReadGuard<'_, Tree> {
        self.shared.tree.read().unwrap_or_else(|e| e.into_inner())
    }

    /// The tree if no writer holds it right now.
    pub fn try_tree(&self) -> Option<RwLockReadGuard<'_, Tree>> {
        self.shared.tree.try_read().ok()
    }

    pub(crate) fn tree_mut(&self) -> RwLockWriteGuard<'_, Tree> {
        self.shared.tree.write().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn bump(&self) {
        self.shared.generation.fetch_add(1, Ordering::Release);
    }

    pub fn generation(&self) -> u64 {
        self.shared.generation.load(Ordering::Acquire)
    }

    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Release);
    }

    pub fn is_finished(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire)
    }

    pub fn options(&self) -> &Options {
        &self.shared.opts
    }

    pub fn progress(&self) -> Progress {
        let s = &self.shared;
        let finished = s.finished.load(Ordering::Acquire);
        let elapsed = if finished {
            Duration::from_millis(s.elapsed_ms.load(Ordering::Acquire))
        } else {
            s.started.elapsed()
        };
        Progress {
            entries: s.entries.load(Ordering::Relaxed),
            dirs: s.dirs.load(Ordering::Relaxed),
            bytes: s.bytes.load(Ordering::Relaxed),
            unreadable: self.try_tree().map(|t| t.unreadable_count()).unwrap_or(0),
            unsizable: s.unsizable.load(Ordering::Relaxed),
            finished,
            cancelled: s.cancel.load(Ordering::Relaxed),
            elapsed,
        }
    }

    /// The path to hand the operating system for `node` (on Windows, the
    /// `\\?\` form, so long paths work).
    pub fn fs_path(&self, node: NodeId) -> PathBuf {
        let t = self.tree();
        fs_path_in(&self.shared, &t, node)
    }

    /// `fs_path` against a tree guard the caller already holds. Taking a
    /// second read lock while holding one can deadlock behind a waiting
    /// writer.
    pub fn fs_path_with(&self, t: &Tree, node: NodeId) -> PathBuf {
        fs_path_in(&self.shared, t, node)
    }

    /// Reads `node` again from disk, replacing what was there. Returns at
    /// once; the tree updates as the rescan proceeds.
    pub fn rescan(&self, node: NodeId) {
        if !self.is_finished() {
            return;
        }
        let path;
        {
            let mut t = self.tree_mut();
            if !t.is_dir(node) || t.has(node, flag::REMOVED) {
                return;
            }
            t.clear_children(node);
            path = fs_path_in(&self.shared, &t, node);
        }
        self.shared.finished.store(false, Ordering::Release);
        self.shared.cancel.store(false, Ordering::Release);
        self.bump();
        let s = self.shared.clone();
        let root = root_of(&s, node);
        std::thread::spawn(move || run(&s, vec![Job { node, path, root }]));
    }
}

fn fs_path_in(s: &Shared, t: &Tree, node: NodeId) -> PathBuf {
    let roots = s.roots.lock().unwrap();
    let mut chain = Vec::new();
    let mut cur = node;
    while t.parent(cur) != NONE {
        chain.push(cur);
        cur = t.parent(cur);
    }
    let ri = t.roots().iter().position(|&r| r == cur).unwrap_or(0);
    let mut p = roots
        .get(ri)
        .map(|r| r.fs_path.clone())
        .unwrap_or_else(|| t.path(cur));
    for &c in chain.iter().rev() {
        p.push(t.name(c));
    }
    p
}

fn root_of(s: &Shared, node: NodeId) -> usize {
    let t = s.tree.read().unwrap();
    let mut cur = node;
    while t.parent(cur) != NONE {
        cur = t.parent(cur);
    }
    t.roots().iter().position(|&r| r == cur).unwrap_or(0)
}

struct Job {
    node: NodeId,
    path: PathBuf,
    root: usize,
}

/// The path as the user should read it, and the path to give the OS.
fn root_paths(p: &Path) -> (PathBuf, PathBuf) {
    let abs = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    #[cfg(windows)]
    {
        let s = abs.to_string_lossy();
        let display = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            PathBuf::from(format!(r"\\{rest}"))
        } else if let Some(rest) = s.strip_prefix(r"\\?\") {
            PathBuf::from(rest)
        } else {
            abs.clone()
        };
        (display, abs)
    }
    #[cfg(not(windows))]
    {
        (abs.clone(), abs)
    }
}

fn add_root(s: &Arc<Shared>, p: &Path) -> Option<Job> {
    let (display, fs_path) = root_paths(p);
    let meta = std::fs::symlink_metadata(&fs_path);
    let info = walk::root_info(&fs_path).unwrap_or_default();
    let mut t = s.tree.write().unwrap();
    let ri = {
        let mut roots = s.roots.lock().unwrap();
        roots.push(Root {
            fs_path: fs_path.clone(),
            info,
        });
        roots.len() - 1
    };
    match meta {
        Ok(m) if m.is_dir() => {
            let id = t.add_root(
                display.as_os_str(),
                EntryMeta {
                    flags: flag::DIR,
                    ..Default::default()
                },
            );
            if walk::is_virtual_fs(&fs_path) {
                t.set_flag(id, flag::VIRTUAL | flag::COMPLETE);
                return None;
            }
            Some(Job {
                node: id,
                path: fs_path,
                root: ri,
            })
        }
        Ok(m) => {
            t.add_root(
                display.as_os_str(),
                EntryMeta {
                    flags: 0,
                    logical: m.len(),
                    disk: m.len(),
                    mtime: 0,
                },
            );
            None
        }
        Err(_) => {
            let id = t.add_root(
                display.as_os_str(),
                EntryMeta {
                    flags: flag::DIR,
                    ..Default::default()
                },
            );
            t.set_flag(id, flag::UNREADABLE | flag::COMPLETE);
            None
        }
    }
}

fn run(s: &Arc<Shared>, jobs: Vec<Job>) {
    let threads = if s.opts.threads == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    } else {
        s.opts.threads
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("heft-walk-{i}"))
        .build()
        .expect("build walker pool");
    let infos: Vec<RootInfo> = s.roots.lock().unwrap().iter().map(|r| r.info).collect();
    let infos = &infos;
    pool.scope(|scope| {
        for job in jobs {
            scope.spawn(move |sc| visit(sc, s, infos, job));
        }
    });
    s.elapsed_ms
        .store(s.started.elapsed().as_millis() as u64, Ordering::Release);
    s.finished.store(true, Ordering::Release);
    s.generation.fetch_add(1, Ordering::Release);
}

fn visit<'a>(scope: &rayon::Scope<'a>, s: &'a Arc<Shared>, infos: &'a [RootInfo], job: Job) {
    let Job { node, path, root } = job;
    if s.cancel.load(Ordering::Relaxed) {
        return;
    }
    let info = infos.get(root).copied().unwrap_or_default();
    let listing = match walk::read_dir(&path, &info, s.opts.mtime) {
        Ok(l) => l,
        Err(_) => {
            let mut t = s.tree.write().unwrap();
            t.set_flag(node, flag::UNREADABLE);
            complete(s, &mut t, node);
            s.generation.fetch_add(1, Ordering::Release);
            return;
        }
    };

    let mut metas = Vec::with_capacity(listing.entries.len());
    let mut descend: Vec<usize> = Vec::new();
    let mut bytes = 0u64;
    for (i, &(off, len, raw)) in listing.entries.iter().enumerate() {
        let mut f = 0u8;
        match raw.kind {
            Kind::Dir => {
                f |= flag::DIR;
                let crosses = raw.dev != 0 && info.dev != 0 && raw.dev != info.dev;
                if crosses && !s.opts.cross_filesystems {
                    f |= flag::OTHER_FS | flag::COMPLETE;
                } else if crosses
                    && walk::is_virtual_fs(&path.join(name_of(&listing.names, off, len)))
                {
                    f |= flag::VIRTUAL | flag::COMPLETE;
                } else {
                    descend.push(i);
                }
            }
            Kind::Link => f |= flag::LINK,
            Kind::File if raw.nlink > 1 && raw.ino != 0 => {
                let fresh = s.hardlinks.lock().unwrap().insert((raw.dev, raw.ino));
                if !fresh {
                    f |= flag::HARDLINK_DUP;
                }
            }
            _ => {}
        }
        if f & flag::HARDLINK_DUP == 0 {
            bytes += raw.disk;
        }
        metas.push((
            off,
            len,
            EntryMeta {
                flags: f,
                logical: raw.logical,
                disk: raw.disk,
                mtime: raw.mtime,
            },
        ));
    }

    let base = {
        let mut t = s.tree.write().unwrap();
        let base = t.add_children(node, &listing.names, &metas);
        if descend.is_empty() {
            complete(s, &mut t, node);
        } else {
            s.pending.lock().unwrap().insert(node, descend.len() as u32);
        }
        base
    };
    s.entries.fetch_add(metas.len() as u64, Ordering::Relaxed);
    s.dirs.fetch_add(1, Ordering::Relaxed);
    s.bytes.fetch_add(bytes, Ordering::Relaxed);
    s.unsizable
        .fetch_add(listing.unsizable as u64, Ordering::Relaxed);
    s.generation.fetch_add(1, Ordering::Release);

    for i in descend {
        let (off, len, _) = listing.entries[i];
        let child = Job {
            node: base + i as NodeId,
            path: path.join(name_of(&listing.names, off, len)),
            root,
        };
        scope.spawn(move |sc| visit(sc, s, infos, child));
    }
}

fn name_of(names: &[u8], off: u32, len: u16) -> &OsStr {
    // SAFETY: the walker filled `names` from `OsStr::as_encoded_bytes`.
    unsafe {
        OsStr::from_encoded_bytes_unchecked(&names[off as usize..off as usize + len as usize])
    }
}

/// Marks `node` complete, then any ancestor whose last outstanding folder
/// that was.
fn complete(s: &Shared, t: &mut Tree, mut node: NodeId) {
    let mut pending = s.pending.lock().unwrap();
    loop {
        t.set_flag(node, flag::COMPLETE);
        pending.remove(&node);
        let p = t.parent(node);
        if p == NONE {
            return;
        }
        match pending.get_mut(&p) {
            Some(n) if *n > 1 => {
                *n -= 1;
                return;
            }
            Some(_) => node = p,
            None => return,
        }
    }
}
