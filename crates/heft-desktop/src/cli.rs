//! The text mode: `heft scan`, `heft bench`, `heft --version`.
//!
//! Returns `None` when the arguments ask for the window.

use heft_scan::units::{Units, format_count, format_duration, format_size};
use heft_scan::{NodeId, Options, Scan, Tree, report, tree::flag};
use std::path::PathBuf;

const USAGE: &str = "\
Usage:
  heft                          open the window
  heft <path>... [--view <v>]   open the window scanning these (v: folders,
                                treemap, types, largest, age)
  heft scan <path>... [options] scan and print the largest folders
  heft bench <path>...          scan and print timings and memory
  heft --version

Scan options:
  --top <n>        rows per level (default 15)
  --depth <n>      levels to print (default 1)
  --files <n>      also list the n largest files
  --logical        sizes as programs read them, not as allocated on disk
  --json           machine-readable output
  --cross          descend into other filesystems
  --threads <n>    walker threads (default: one per core)
  --binary | --decimal   size units
";

struct Args {
    paths: Vec<PathBuf>,
    top: usize,
    depth: usize,
    files: usize,
    logical: bool,
    json: bool,
    cross: bool,
    threads: usize,
    units: Units,
}

fn parse(rest: &[String]) -> Result<Args, String> {
    let mut a = Args {
        paths: Vec::new(),
        top: 15,
        depth: 1,
        files: 0,
        logical: false,
        json: false,
        cross: false,
        threads: 0,
        units: Units::platform_default(),
    };
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        let mut num = |name: &str| -> Result<usize, String> {
            it.next()
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| format!("{name} needs a number."))
        };
        match arg.as_str() {
            "--top" => a.top = num("--top")?,
            "--depth" => a.depth = num("--depth")?,
            "--files" => a.files = num("--files")?,
            "--threads" => a.threads = num("--threads")?,
            "--logical" => a.logical = true,
            "--json" => a.json = true,
            "--cross" => a.cross = true,
            "--binary" => a.units = Units::Binary,
            "--decimal" => a.units = Units::Decimal,
            s if s.starts_with("--") => return Err(format!("Unknown option {s}.")),
            p => a.paths.push(PathBuf::from(p)),
        }
    }
    if a.paths.is_empty() {
        return Err("Give at least one folder or disk to scan.".into());
    }
    Ok(a)
}

pub fn run(args: &[String]) -> Option<i32> {
    let first = args.first()?;
    let code = match first.as_str() {
        "--version" | "-V" => {
            println!("heft {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "--help" | "-h" | "help" => {
            print!("{USAGE}");
            0
        }
        "scan" | "bench" => match parse(&args[1..]) {
            Ok(a) => {
                if first == "scan" {
                    scan(&a)
                } else {
                    bench(&a)
                }
            }
            Err(e) => {
                eprintln!("{e}\n\n{USAGE}");
                2
            }
        },
        other if other == "--view" || std::path::Path::new(other).exists() => return None,
        other if looks_like_path(other) => {
            eprintln!("No such file or folder: {other}");
            if cfg!(windows)
                && other.len() > 2
                && other.as_bytes()[1] == b':'
                && !other.contains(['\\', '/'])
            {
                eprintln!(
                    "The backslashes may have been eaten by the shell. \
                     Use forward slashes (C:/Users/me) or quote the path."
                );
            }
            2
        }
        other => {
            eprintln!("Unknown command {other}.\n\n{USAGE}");
            2
        }
    };
    Some(code)
}

fn options(a: &Args) -> Options {
    Options {
        cross_filesystems: a.cross,
        threads: a.threads,
        mtime: true,
    }
}

fn scan(a: &Args) -> i32 {
    let s = Scan::run_blocking(a.paths.clone(), options(a));
    let t = s.tree();
    let p = s.progress();
    if a.json {
        print!("{}", json(&t, a));
        return 0;
    }
    for &r in t.roots() {
        println!(
            "{}  {}  ({} files)",
            format_size(t.size(r, a.logical), a.units),
            t.display_name(r),
            format_count(t.files(r) as u64)
        );
        print_level(&t, r, a, 1);
        if a.files > 0 {
            println!("\n  Largest files:");
            for f in report::largest_files(&t, r, a.files, a.logical) {
                println!(
                    "  {:>10}  {}",
                    format_size(t.size(f, a.logical), a.units),
                    t.path(f).display()
                );
            }
        }
    }
    println!(
        "\n{} entries in {} folders, {}.",
        format_count(p.entries),
        format_count(p.dirs),
        format_duration(p.elapsed)
    );
    if t.unreadable_count() > 0 {
        println!(
            "{} folders could not be read, so the totals leave them out.",
            format_count(t.unreadable_count())
        );
    }
    0
}

fn print_level(t: &Tree, n: NodeId, a: &Args, depth: usize) {
    if depth > a.depth {
        return;
    }
    let total = t.size(n, a.logical).max(1);
    let kids = t.sorted_children(n, a.logical);
    for &c in kids.iter().take(a.top) {
        let size = t.size(c, a.logical);
        let mut note = String::new();
        if t.has(c, flag::UNREADABLE) {
            note.push_str("  (could not be read)");
        } else if t.has(c, flag::OTHER_FS) {
            note.push_str("  (another filesystem, not scanned)");
        } else if t.has(c, flag::LINK) {
            note.push_str("  (link, not followed)");
        }
        println!(
            "{}{:>10}  {:>5.1}%  {}{}{}",
            "  ".repeat(depth),
            format_size(size, a.units),
            size as f64 * 100.0 / total as f64,
            t.display_name(c),
            if t.is_dir(c) {
                std::path::MAIN_SEPARATOR_STR
            } else {
                ""
            },
            note
        );
        if t.is_dir(c) {
            print_level(t, c, a, depth + 1);
        }
    }
    if kids.len() > a.top {
        println!(
            "{}{:>10}  and {} more",
            "  ".repeat(depth),
            "",
            kids.len() - a.top
        );
    }
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn json(t: &Tree, a: &Args) -> String {
    fn node(t: &Tree, n: NodeId, a: &Args, depth: usize, out: &mut String) {
        out.push_str(&format!(
            "{{\"name\":\"{}\",\"disk\":{},\"logical\":{},\"files\":{},\"dir\":{},\"unreadable\":{}",
            esc(&t.display_name(n)),
            t.disk(n),
            t.logical(n),
            t.files(n),
            t.is_dir(n),
            t.has(n, flag::UNREADABLE)
        ));
        if t.is_dir(n) && depth < a.depth {
            out.push_str(",\"children\":[");
            for (i, &c) in t
                .sorted_children(n, a.logical)
                .iter()
                .take(a.top)
                .enumerate()
            {
                if i > 0 {
                    out.push(',');
                }
                node(t, c, a, depth + 1, out);
            }
            out.push(']');
        }
        out.push('}');
    }
    let mut out = String::from("[");
    for (i, &r) in t.roots().iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        node(t, r, a, 0, &mut out);
    }
    out.push_str("]\n");
    out
}

fn bench(a: &Args) -> i32 {
    let s = Scan::run_blocking(a.paths.clone(), options(a));
    let p = s.progress();
    let t = s.tree();
    let secs = p.elapsed.as_secs_f64().max(1e-9);
    println!("walker      {}", heft_scan::walk::walker_name());
    println!("entries     {}", format_count(p.entries));
    println!("folders     {}", format_count(p.dirs));
    println!("on disk     {}", format_size(t.total(false), a.units));
    println!("logical     {}", format_size(t.total(true), a.units));
    println!("time        {}", format_duration(p.elapsed));
    println!(
        "rate        {} entries/s",
        format_count((p.entries as f64 / secs) as u64)
    );
    println!(
        "arena       {} ({:.1} bytes per entry, names included)",
        format_size(t.heap_bytes() as u64, Units::Binary),
        t.heap_bytes() as f64 / t.len().max(1) as f64
    );
    if let Some(peak) = peak_memory() {
        println!("peak memory {}", format_size(peak, Units::Binary));
    }
    println!("unreadable  {}", format_count(t.unreadable_count()));
    println!("unsizable   {}", format_count(p.unsizable));
    0
}

#[cfg(target_os = "linux")]
fn peak_memory() -> Option<u64> {
    // VmHWM: the resident set's high-water mark.
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(not(target_os = "linux"))]
fn peak_memory() -> Option<u64> {
    None
}

/// A drive letter, a separator or a leading dot: meant as a path, not a command.
fn looks_like_path(s: &str) -> bool {
    let b = s.as_bytes();
    s.contains(['/', '\\'])
        || s.starts_with('.')
        || s.starts_with('~')
        || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
}
