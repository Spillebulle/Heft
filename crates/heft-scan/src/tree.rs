//! The tree of one scan: an arena, struct-of-arrays, `u32` indices.
//!
//! No node owns a path or a `String`. Names are bytes in one shared buffer
//! (`OsStr`'s encoded form, so non-UTF-8 names on Linux survive and are
//! deleted exactly), and a full path is rebuilt on demand by walking parents.
//! The budget is under 48 bytes per node before names; `heap_bytes` reports
//! what was actually used so `heft bench` can check it.
//!
//! A directory's children are always contiguous (`first_child ..
//! first_child + child_count`) because a directory is appended in one batch
//! when the walker has read it. That is what lets the walker take the lock once
//! per directory rather than once per file.

use std::ffi::OsStr;
use std::path::PathBuf;

pub type NodeId = u32;
pub const NONE: NodeId = u32::MAX;

/// Per-node flags.
pub mod flag {
    /// A directory (whether or not it was descended into).
    pub const DIR: u8 = 1 << 0;
    /// This node and everything under it has been read. Sizes are final.
    pub const COMPLETE: u8 = 1 << 1;
    /// A directory that could not be read. Its size is unknown, not zero.
    pub const UNREADABLE: u8 = 1 << 2;
    /// A mount point of another filesystem, not descended into.
    pub const OTHER_FS: u8 = 1 << 3;
    /// A symbolic link, junction or other reparse point. Never followed.
    pub const LINK: u8 = 1 << 4;
    /// A hard link whose file was already counted at another path.
    pub const HARDLINK_DUP: u8 = 1 << 5;
    /// A virtual filesystem (`/proc`, `/sys`, ...), never scanned.
    pub const VIRTUAL: u8 = 1 << 6;
    /// Removed by the user during this session; kept so indices stay stable.
    pub const REMOVED: u8 = 1 << 7;
}

/// What the walker found for one directory entry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EntryMeta {
    pub flags: u8,
    /// Apparent size: what a program reading the file sees.
    pub logical: u64,
    /// Allocated size: what deleting the file gives back.
    pub disk: u64,
    /// Last modification, seconds since 1970 (saturating; 0 when unknown).
    pub mtime: u32,
}

#[derive(Default)]
pub struct Tree {
    parent: Vec<NodeId>,
    first_child: Vec<NodeId>,
    child_count: Vec<u32>,
    disk: Vec<u64>,
    logical: Vec<u64>,
    /// Files in the subtree (1 for a file, 0 for a link or a duplicate).
    files: Vec<u32>,
    /// Newest modification in the subtree.
    mtime: Vec<u32>,
    flags: Vec<u8>,
    name_off: Vec<u32>,
    name_len: Vec<u16>,
    names: Vec<u8>,
    roots: Vec<NodeId>,
    /// Directories that could not be read, anywhere in the tree.
    unreadable: u64,
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.parent.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// Adds a scan root. `name` is the path as the user should read it.
    pub fn add_root(&mut self, name: &OsStr, meta: EntryMeta) -> NodeId {
        let id = self.push(NONE, name.as_encoded_bytes(), meta);
        self.roots.push(id);
        id
    }

    /// Appends one directory's entries as its children and adds their sizes
    /// to every ancestor. Returns the index of the first child.
    ///
    /// `names` holds the entries' names back to back; each entry carries its
    /// offset and length into it.
    pub fn add_children(
        &mut self,
        parent: NodeId,
        names: &[u8],
        entries: &[(u32, u16, EntryMeta)],
    ) -> NodeId {
        let base = self.parent.len() as NodeId;
        let p = parent as usize;
        debug_assert_eq!(self.child_count[p], 0, "a directory is appended once");
        self.first_child[p] = base;
        self.child_count[p] = entries.len() as u32;

        let (mut disk, mut logical, mut files, mut newest) = (0u64, 0u64, 0u32, 0u32);
        for &(off, len, meta) in entries {
            let name = &names[off as usize..off as usize + len as usize];
            self.push(parent, name, meta);
            if meta.flags & (flag::DIR | flag::HARDLINK_DUP | flag::LINK) == 0 {
                files += 1;
            }
            if meta.flags & flag::HARDLINK_DUP == 0 {
                disk += meta.disk;
                logical += meta.logical;
            }
            newest = newest.max(meta.mtime);
        }
        self.bubble(parent, disk, logical, files, newest);
        base
    }

    fn push(&mut self, parent: NodeId, name: &[u8], meta: EntryMeta) -> NodeId {
        let id = self.parent.len() as NodeId;
        let is_file = meta.flags & (flag::DIR | flag::HARDLINK_DUP | flag::LINK) == 0;
        let counted = meta.flags & flag::HARDLINK_DUP == 0;
        self.parent.push(parent);
        self.first_child.push(NONE);
        self.child_count.push(0);
        // A directory starts empty; its size is what its children add.
        let dir = meta.flags & flag::DIR != 0;
        self.disk.push(if dir || !counted { 0 } else { meta.disk });
        self.logical
            .push(if dir || !counted { 0 } else { meta.logical });
        self.files.push(is_file as u32);
        self.mtime.push(meta.mtime);
        let mut f = meta.flags;
        if !dir {
            f |= flag::COMPLETE;
        }
        self.flags.push(f);
        let len = name.len().min(u16::MAX as usize);
        self.name_off.push(self.names.len() as u32);
        self.name_len.push(len as u16);
        self.names.extend_from_slice(&name[..len]);
        id
    }

    /// Adds sizes to `node` and every ancestor.
    fn bubble(&mut self, mut node: NodeId, disk: u64, logical: u64, files: u32, newest: u32) {
        while node != NONE {
            let n = node as usize;
            self.disk[n] += disk;
            self.logical[n] += logical;
            self.files[n] += files;
            if newest > self.mtime[n] {
                self.mtime[n] = newest;
            }
            node = self.parent[n];
        }
    }

    pub fn set_flag(&mut self, node: NodeId, f: u8) {
        let n = node as usize;
        if f & flag::UNREADABLE != 0 && self.flags[n] & flag::UNREADABLE == 0 {
            self.unreadable += 1;
        }
        self.flags[n] |= f;
    }

    /// Forgets a directory's contents so it can be read again: its children
    /// are marked removed (indices stay stable), its sizes leave every
    /// ancestor, and it is no longer complete.
    pub fn clear_children(&mut self, node: NodeId) {
        let n = node as usize;
        for c in self.children(node) {
            self.flags[c as usize] |= flag::REMOVED;
        }
        let (d, l, f) = (self.disk[n], self.logical[n], self.files[n]);
        let mut a = node;
        while a != NONE {
            let i = a as usize;
            self.disk[i] -= d;
            self.logical[i] -= l;
            self.files[i] -= f;
            a = self.parent[i];
        }
        if self.flags[n] & flag::UNREADABLE != 0 {
            self.unreadable -= 1;
        }
        self.first_child[n] = NONE;
        self.child_count[n] = 0;
        self.flags[n] &= !(flag::COMPLETE | flag::UNREADABLE);
    }

    /// Marks a node removed and takes its sizes out of every ancestor.
    pub fn mark_removed(&mut self, node: NodeId) {
        let n = node as usize;
        if self.flags[n] & flag::REMOVED != 0 {
            return;
        }
        let (d, l, f) = (self.disk[n], self.logical[n], self.files[n]);
        let mut a = self.parent[n];
        while a != NONE {
            let i = a as usize;
            self.disk[i] -= d;
            self.logical[i] -= l;
            self.files[i] -= f;
            a = self.parent[i];
        }
        self.flags[n] |= flag::REMOVED;
    }

    pub fn parent(&self, n: NodeId) -> NodeId {
        self.parent[n as usize]
    }
    pub fn children(&self, n: NodeId) -> std::ops::Range<NodeId> {
        let f = self.first_child[n as usize];
        if f == NONE {
            return 0..0;
        }
        f..f + self.child_count[n as usize]
    }
    pub fn disk(&self, n: NodeId) -> u64 {
        self.disk[n as usize]
    }
    pub fn logical(&self, n: NodeId) -> u64 {
        self.logical[n as usize]
    }
    /// Size in the unit the user chose: on disk, or logical.
    pub fn size(&self, n: NodeId, logical: bool) -> u64 {
        if logical {
            self.logical(n)
        } else {
            self.disk(n)
        }
    }
    pub fn files(&self, n: NodeId) -> u32 {
        self.files[n as usize]
    }
    pub fn mtime(&self, n: NodeId) -> u32 {
        self.mtime[n as usize]
    }
    pub fn flags(&self, n: NodeId) -> u8 {
        self.flags[n as usize]
    }
    pub fn has(&self, n: NodeId, f: u8) -> bool {
        self.flags[n as usize] & f != 0
    }
    pub fn is_dir(&self, n: NodeId) -> bool {
        self.has(n, flag::DIR)
    }
    pub fn unreadable_count(&self) -> u64 {
        self.unreadable
    }

    pub fn name_bytes(&self, n: NodeId) -> &[u8] {
        let off = self.name_off[n as usize] as usize;
        &self.names[off..off + self.name_len[n as usize] as usize]
    }

    pub fn name(&self, n: NodeId) -> &OsStr {
        // SAFETY: every name was put in from `OsStr::as_encoded_bytes` on
        // this platform (by the walker or `add_root`), and never split.
        unsafe { OsStr::from_encoded_bytes_unchecked(self.name_bytes(n)) }
    }

    /// The node's name for display (lossy where the name is not UTF-8).
    pub fn display_name(&self, n: NodeId) -> String {
        self.name(n).to_string_lossy().into_owned()
    }

    /// The node's full path, rebuilt from its ancestors. A root's name is its
    /// path, so the walk stops there.
    pub fn path(&self, n: NodeId) -> PathBuf {
        let mut chain = Vec::new();
        let mut cur = n;
        while cur != NONE {
            chain.push(cur);
            cur = self.parent[cur as usize];
        }
        let mut p = PathBuf::new();
        for &c in chain.iter().rev() {
            p.push(self.name(c));
        }
        p
    }

    /// Depth below its root (a root is 0).
    pub fn depth(&self, mut n: NodeId) -> usize {
        let mut d = 0;
        while self.parent[n as usize] != NONE {
            n = self.parent[n as usize];
            d += 1;
        }
        d
    }

    /// True if `n` is `ancestor` or lies under it.
    pub fn is_within(&self, mut n: NodeId, ancestor: NodeId) -> bool {
        while n != NONE {
            if n == ancestor {
                return true;
            }
            n = self.parent[n as usize];
        }
        false
    }

    /// Children of `n` sorted largest first, removed nodes left out.
    pub fn sorted_children(&self, n: NodeId, logical: bool) -> Vec<NodeId> {
        let mut v: Vec<NodeId> = self
            .children(n)
            .filter(|&c| !self.has(c, flag::REMOVED))
            .collect();
        v.sort_unstable_by_key(|&c| std::cmp::Reverse(self.size(c, logical)));
        v
    }

    /// Bytes held by the arena itself (capacity, not length).
    pub fn heap_bytes(&self) -> usize {
        self.parent.capacity() * 4
            + self.first_child.capacity() * 4
            + self.child_count.capacity() * 4
            + self.disk.capacity() * 8
            + self.logical.capacity() * 8
            + self.files.capacity() * 4
            + self.mtime.capacity() * 4
            + self.flags.capacity()
            + self.name_off.capacity() * 4
            + self.name_len.capacity() * 2
            + self.names.capacity()
    }

    /// Sum over all roots.
    pub fn total(&self, logical: bool) -> u64 {
        self.roots.iter().map(|&r| self.size(r, logical)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(size: u64) -> EntryMeta {
        EntryMeta {
            flags: 0,
            logical: size,
            disk: size,
            mtime: 10,
        }
    }
    fn dir() -> EntryMeta {
        EntryMeta {
            flags: flag::DIR,
            ..Default::default()
        }
    }
    fn names(list: &[&str]) -> (Vec<u8>, Vec<(u32, u16)>) {
        let mut buf = Vec::new();
        let mut spans = Vec::new();
        for n in list {
            spans.push((buf.len() as u32, n.len() as u16));
            buf.extend_from_slice(n.as_bytes());
        }
        (buf, spans)
    }

    #[test]
    fn sizes_bubble_to_every_ancestor() {
        let mut t = Tree::new();
        let r = t.add_root(OsStr::new("/root"), dir());
        let (b, s) = names(&["a", "f"]);
        let base = t.add_children(r, &b, &[(s[0].0, s[0].1, dir()), (s[1].0, s[1].1, file(5))]);
        let a = base;
        let (b, s) = names(&["x", "y"]);
        t.add_children(
            a,
            &b,
            &[(s[0].0, s[0].1, file(100)), (s[1].0, s[1].1, file(1))],
        );
        assert_eq!(t.disk(a), 101);
        assert_eq!(t.disk(r), 106);
        assert_eq!(t.files(r), 3);
        assert_eq!(t.display_name(a), "a");
        assert_eq!(t.path(a + 2), PathBuf::from("/root").join("a").join("x"));
    }

    #[test]
    fn a_hard_link_duplicate_adds_nothing() {
        let mut t = Tree::new();
        let r = t.add_root(OsStr::new("/r"), dir());
        let (b, s) = names(&["one", "two"]);
        let dup = EntryMeta {
            flags: flag::HARDLINK_DUP,
            ..file(50)
        };
        t.add_children(r, &b, &[(s[0].0, s[0].1, file(50)), (s[1].0, s[1].1, dup)]);
        assert_eq!(t.disk(r), 50);
        assert_eq!(t.files(r), 1);
    }

    #[test]
    fn removal_takes_sizes_out_of_ancestors() {
        let mut t = Tree::new();
        let r = t.add_root(OsStr::new("/r"), dir());
        let (b, s) = names(&["d"]);
        let d = t.add_children(r, &b, &[(s[0].0, s[0].1, dir())]);
        let (b, s) = names(&["f"]);
        t.add_children(d, &b, &[(s[0].0, s[0].1, file(70))]);
        t.mark_removed(d);
        assert_eq!(t.disk(r), 0);
        assert_eq!(t.files(r), 0);
        assert!(t.sorted_children(r, false).is_empty());
    }
}
