//! The walker against real temporary trees: the only honest test of a
//! syscall path.

use heft_scan::tree::flag;
use heft_scan::{report, NodeId, Options, Scan, Tree};
use std::fs;
use std::path::Path;

fn write(p: &Path, bytes: usize) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, vec![7u8; bytes]).unwrap();
}

fn find(t: &Tree, parent: NodeId, name: &str) -> NodeId {
    t.children(parent)
        .find(|&c| t.display_name(c) == name)
        .unwrap_or_else(|| panic!("{name} not found"))
}

fn scan(root: &Path) -> Scan {
    Scan::run_blocking(vec![root.to_path_buf()], Options { threads: 4, ..Default::default() })
}

#[test]
fn logical_sizes_and_counts_add_up() {
    let d = tempfile::tempdir().unwrap();
    write(&d.path().join("a/one.bin"), 1000);
    write(&d.path().join("a/b/two.bin"), 2000);
    write(&d.path().join("a/b/c/three.bin"), 3000);
    write(&d.path().join("top.txt"), 10);
    fs::create_dir_all(d.path().join("empty")).unwrap();

    let s = scan(d.path());
    let t = s.tree();
    let r = t.roots()[0];
    assert_eq!(t.logical(r), 6010);
    assert_eq!(t.files(r), 4);
    let a = find(&t, r, "a");
    assert_eq!(t.logical(a), 6000);
    assert!(t.has(r, flag::COMPLETE));
    assert!(t.has(a, flag::COMPLETE));
    let empty = find(&t, r, "empty");
    assert!(t.is_dir(empty) && t.has(empty, flag::COMPLETE));
    assert_eq!(t.logical(empty), 0);
    // On-disk size is at least the logical size for plain files.
    assert!(t.disk(r) >= 6010 || cfg!(not(any(windows, target_os = "linux"))));
    assert!(s.progress().finished);
}

#[test]
fn many_folders_complete() {
    let d = tempfile::tempdir().unwrap();
    for i in 0..40 {
        for j in 0..5 {
            write(&d.path().join(format!("d{i}/e{j}/f.bin")), 100);
        }
    }
    let s = scan(d.path());
    let t = s.tree();
    let r = t.roots()[0];
    assert_eq!(t.files(r), 200);
    assert_eq!(t.logical(r), 20_000);
    let mut incomplete = 0;
    report::walk_subtree(&t, r, |n| {
        if !t.has(n, flag::COMPLETE) {
            incomplete += 1;
        }
    });
    assert_eq!(incomplete, 0);
}

#[test]
fn largest_files_and_types() {
    let d = tempfile::tempdir().unwrap();
    write(&d.path().join("x/movie.mkv"), 5000);
    write(&d.path().join("y/song.mp3"), 300);
    write(&d.path().join("y/other.mp3"), 200);
    write(&d.path().join("notes"), 50);
    let s = scan(d.path());
    let t = s.tree();
    let r = t.roots()[0];
    let big = report::largest_files(&t, r, 2, true);
    assert_eq!(big.len(), 2);
    assert_eq!(t.display_name(big[0]), "movie.mkv");
    assert_eq!(t.display_name(big[1]), "song.mp3");
    let types = report::by_extension(&t, r, true);
    assert_eq!(types[0].ext, "mkv");
    let mp3 = types.iter().find(|s| s.ext == "mp3").unwrap();
    assert_eq!((mp3.bytes, mp3.count), (500, 2));
}

#[test]
fn rescan_replaces_a_subtree() {
    let d = tempfile::tempdir().unwrap();
    write(&d.path().join("a/one.bin"), 1000);
    write(&d.path().join("b/two.bin"), 50);
    let s = scan(d.path());
    let a = {
        let t = s.tree();
        find(&t, t.roots()[0], "a")
    };
    write(&d.path().join("a/more.bin"), 500);
    s.rescan(a);
    while !s.is_finished() {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let t = s.tree();
    let r = t.roots()[0];
    assert_eq!(t.logical(a), 1500);
    assert_eq!(t.logical(r), 1550);
    assert_eq!(t.files(r), 3);
}

#[cfg(unix)]
#[test]
fn symlinks_are_not_followed() {
    let d = tempfile::tempdir().unwrap();
    write(&d.path().join("real/big.bin"), 10_000);
    std::os::unix::fs::symlink(d.path().join("real"), d.path().join("link")).unwrap();
    let s = scan(d.path());
    let t = s.tree();
    let r = t.roots()[0];
    let link = find(&t, r, "link");
    assert!(t.has(link, flag::LINK));
    assert_eq!(t.files(r), 1);
    assert!(t.logical(r) < 20_000);
}

#[cfg(target_os = "linux")]
#[test]
fn a_hard_link_is_counted_once() {
    let d = tempfile::tempdir().unwrap();
    write(&d.path().join("a/file.bin"), 64 * 1024);
    fs::create_dir_all(d.path().join("b")).unwrap();
    fs::hard_link(d.path().join("a/file.bin"), d.path().join("b/same.bin")).unwrap();
    let s = scan(d.path());
    let t = s.tree();
    let r = t.roots()[0];
    assert_eq!(t.logical(r), 64 * 1024);
    assert_eq!(t.files(r), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn an_unreadable_folder_is_reported_not_dropped() {
    use std::os::unix::fs::PermissionsExt;
    if rustix_is_root() {
        return; // root reads everything; nothing to prove
    }
    let d = tempfile::tempdir().unwrap();
    write(&d.path().join("locked/secret.bin"), 100);
    fs::set_permissions(d.path().join("locked"), fs::Permissions::from_mode(0o000)).unwrap();
    let s = scan(d.path());
    let t = s.tree();
    let locked = find(&t, t.roots()[0], "locked");
    assert!(t.has(locked, flag::UNREADABLE));
    assert_eq!(t.unreadable_count(), 1);
    drop(t);
    fs::set_permissions(d.path().join("locked"), fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(target_os = "linux")]
fn rustix_is_root() -> bool {
    std::fs::metadata("/proc/self").map(|m| {
        use std::os::unix::fs::MetadataExt;
        m.uid() == 0
    }).unwrap_or(false)
}
