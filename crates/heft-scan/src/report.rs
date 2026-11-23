//! Derived views over a tree: largest files, file types, age.
//!
//! Each is one pass over the subtree under a node, computed on demand (a pass
//! over ten million nodes is tens of milliseconds) and meant to run off the UI
//! thread. None of them allocates per node.

use crate::kinds::{self, FileKind};
use crate::tree::{NodeId, Tree, flag};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// Calls `f` for every live node under `root` (inclusive), depth first.
pub fn walk_subtree(t: &Tree, root: NodeId, mut f: impl FnMut(NodeId)) {
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if t.has(n, flag::REMOVED) {
            continue;
        }
        f(n);
        stack.extend(t.children(n));
    }
}

fn counted_file(t: &Tree, n: NodeId) -> bool {
    t.flags(n) & (flag::DIR | flag::LINK | flag::HARDLINK_DUP | flag::REMOVED) == 0
}

/// The `limit` largest files under `root`, largest first.
pub fn largest_files(t: &Tree, root: NodeId, limit: usize, logical: bool) -> Vec<NodeId> {
    let mut heap: BinaryHeap<Reverse<(u64, NodeId)>> = BinaryHeap::with_capacity(limit + 1);
    walk_subtree(t, root, |n| {
        if !counted_file(t, n) {
            return;
        }
        let s = t.size(n, logical);
        if heap.len() < limit {
            heap.push(Reverse((s, n)));
        } else if let Some(Reverse((min, _))) = heap.peek()
            && s > *min
        {
            heap.pop();
            heap.push(Reverse((s, n)));
        }
    });
    let mut v: Vec<(u64, NodeId)> = heap.into_iter().map(|Reverse(x)| x).collect();
    v.sort_unstable_by_key(|x| Reverse(x.0));
    v.into_iter().map(|(_, n)| n).collect()
}

#[derive(Debug, Clone)]
pub struct TypeStat {
    /// Lowercase extension, or empty for files with none.
    pub ext: String,
    pub kind: FileKind,
    pub bytes: u64,
    pub count: u64,
}

/// Size and count per extension under `root`, largest first.
pub fn by_extension(t: &Tree, root: NodeId, logical: bool) -> Vec<TypeStat> {
    let mut map: HashMap<String, (u64, u64)> = HashMap::new();
    walk_subtree(t, root, |n| {
        if !counted_file(t, n) {
            return;
        }
        let ext = kinds::extension(t.name_bytes(n)).unwrap_or_default();
        let e = map.entry(ext).or_default();
        e.0 += t.size(n, logical);
        e.1 += 1;
    });
    let mut v: Vec<TypeStat> = map
        .into_iter()
        .map(|(ext, (bytes, count))| TypeStat {
            kind: if ext.is_empty() {
                FileKind::Other
            } else {
                kinds::kind_of_ext(&ext)
            },
            ext,
            bytes,
            count,
        })
        .collect();
    v.sort_unstable_by_key(|s| Reverse(s.bytes));
    v
}

/// Size and count per kind, in `FileKind::ALL` order.
pub fn by_kind(stats: &[TypeStat]) -> Vec<(FileKind, u64, u64)> {
    let mut out: Vec<(FileKind, u64, u64)> = FileKind::ALL.iter().map(|&k| (k, 0, 0)).collect();
    for s in stats {
        let e = &mut out[s.kind.index()];
        e.1 += s.bytes;
        e.2 += s.count;
    }
    out
}

pub const AGE_BUCKETS: [(&str, u64); 6] = [
    ("This week", 7),
    ("This month", 30),
    ("Last three months", 91),
    ("This year", 365),
    ("Last three years", 3 * 365),
    ("Older", u64::MAX),
];

/// Bytes and files per age bucket (by last modification) under `root`.
/// Files with no known time are counted in the last slot, "Unknown".
pub fn by_age(t: &Tree, root: NodeId, now_secs: u64, logical: bool) -> Vec<(u64, u64)> {
    let mut out = vec![(0u64, 0u64); AGE_BUCKETS.len() + 1];
    walk_subtree(t, root, |n| {
        if !counted_file(t, n) {
            return;
        }
        let m = t.mtime(n) as u64;
        let slot = if m == 0 {
            AGE_BUCKETS.len()
        } else {
            let days = now_secs.saturating_sub(m) / 86_400;
            AGE_BUCKETS
                .iter()
                .position(|&(_, d)| days < d)
                .unwrap_or(AGE_BUCKETS.len() - 1)
        };
        out[slot].0 += t.size(n, logical);
        out[slot].1 += 1;
    });
    out
}
