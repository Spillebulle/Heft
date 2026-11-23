# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Heft is a disk space analyser: point it at a disk or a folder and it shows
what is taking the room, fast, and lets you get rid of it. One native binary
with a window, Windows and Linux, installable and self-updating like the rest
of the family. The name is the weight of a thing in the hand, which is the one
question the app answers about every folder: how heavy is this?

**Not started.** Nothing below is code yet. Everything is the decided shape;
treat "Decisions" and "Invariants" as settled (they were reasoned about before
the first line, and re-litigating them costs more than following them) and
"Open" as genuinely open. When code and this file disagree, either the code is
wrong or this file is stale, and the fix is to make them agree in the same
commit.

**Performance is the first requirement, on every platform.** A scan that is
correct and slow is the defect this app exists to avoid; WizTree is the bar on
Windows and `gdu`/`dust` are the bar on Linux. Every decision below that looks
like extra work is there because the simpler version is measurably slower.

The house references are `../Muster` and `../Umber` (Rust + egui workspaces
with the same interface language, installer, packaging and updater) and
`../Windup`. When a question here has an answer there, take it; their
CLAUDE.md files are long because every rule in them was learned from a shipped
defect, and inheriting those is the point of the family.

## Decisions

| | |
|---|---|
| Language | Rust, 2024 edition, stable toolchain, one workspace |
| Interface | egui + wgpu through winit, exactly Umber's and Muster's stack. Not Tauri: the treemap is hundreds of thousands of rectangles redrawn on every hover and zoom, and the tree is millions of nodes; both stay in one process's memory and go straight to the GPU, with no serialising across a webview bridge |
| Shape | **One binary.** The window is the product; `heft <subcommand>` text mode comes out of the same executable (scanning, benchmarking, JSON output) |
| Targets | Windows x86-64 + ARM64, Linux x86-64 + ARM64 (Arch/CachyOS, Debian/Ubuntu, Fedora). **macOS is not a target for now** (decided with the user, 2026-11-23). The walker sits behind one module per platform, so if that changes, `getattrlistbulk` is the fast path and it plugs in there |
| Packages | `.msi` and a setup `.exe`, `.deb`, `.rpm`, Arch `heft-bin` package, AppImage, tarball, all from `packaging/` ported from Muster |
| Updates | Muster's `update` module, ported: releases API, conditional request, swaps the binary, never writes to a package manager's copy |
| Distribution | The house archive, `../Packages` (§18 of the style guide). `.deb`/`.rpm`/Arch packages enrol the machine exactly as `../Packages/ENROLMENT.md` says |
| Repo / id | `github.com/Spillebulle/heft`, application id `io.github.spillebulle.heft` |
| Accent | `--accent-h` **100**, moss. Taken so far: Brokey 18, Windup 28, Umber 60/68, Muster 200, Bulle-Block 245, Tally 255, Pictureframe 254, Trove 310, Sheeets 330, HomeLab 349. 100 sits clear of Umber, of `good` (145) and of `caution`/`critical` (20 to 40) |

Rejected, so they do not come back:

- **Tauri / a web interface.** See the Interface row. Brokey is Tauri because it
  is a store of pages; Heft is a canvas of data, which is Umber's problem.
- **Shelling out to `du`, `dir`, `find` or PowerShell.** Parsing text is slower
  than the syscalls it wraps and loses non-UTF-8 names.
- **Paths stored as strings per node.** Ten million `PathBuf`s is gigabytes.
  See the tree model.
- **Deleting by default with no undo.** The default is the Recycle Bin / Trash.

## Commands

Planned; keep this list true as they come into existence.

```sh
cargo run --release                          # the window
cargo run --release -- scan ~/ --top 20      # text mode, same engine
cargo run --release -- scan C:\ --json       # machine-readable, for tests and comparison
cargo run --release -- bench <path>          # timed scan: entries, bytes, seconds, entries/s, peak memory
cargo test --workspace                       # everything
cargo test -p heft-scan                      # engine only, synthetic trees in a tempdir, fast
cargo bench -p heft-scan                     # criterion: walker, tree build, treemap layout
cargo clippy --workspace --all-targets
cargo fmt --all
sh packaging/check.sh                        # application id and archive paths agree everywhere
```

CI runs `fmt --check`, `clippy` and `test` with `RUSTFLAGS: -D warnings` on
**every runner `release.yml` builds on**. Adding a target to the release matrix
means adding its runner to CI in the same commit (Umber tagged a green release
that failed on `windows-11-arm`).

**Always measure in `--release`.** A debug build of the walker is ten times
slower and tells you nothing. Measure cold and warm cache separately (Linux:
`echo 3 | sudo tee /proc/sys/vm/drop_caches`; Windows: reboot or a second
volume), and write the numbers into `docs/perf.md` with the machine, the
filesystem and the entry count, so a regression has something to be compared
with.

## Architecture

### Three crates, layered so the engine is testable without a window

| Crate | Contains | Must not depend on |
|---|---|---|
| `heft-scan` | walkers per platform, the tree arena, aggregation, file types, largest files, duplicates, owner lookup (which application a path belongs to), deletion | wgpu, winit, egui |
| `heft-app` | window, views, treemap layout and rendering, `update/`, the installer | — |
| `heft-desktop` | binary entry point, subcommand dispatch | — |

The filesystem goes behind a trait in `heft-scan` with one real implementation
per platform and an in-memory one for tests, so aggregation, hard-link
accounting, exclusions, deletion bookkeeping and the views' data can be tested
without touching a disk. The walkers themselves are tested against real
temporary trees, because that is the only honest test of a syscall path.

### The walker: the fast path per platform

**Linux.** A work-stealing pool of threads (one per core, capped; measure, do
not assume more is faster on a spinning disk), each reading directories with
`getdents64` on a directory fd and sizing entries with `statx` relative to that
fd (`AT_SYMLINK_NOFOLLOW`, mask only `STATX_TYPE | STATX_SIZE | STATX_BLOCKS |
STATX_INO | STATX_NLINK`, plus mtime only when the Age view is on). Never build
an absolute path to stat a file: `openat`/`statx` from the parent's fd is both
faster and correct when a path exceeds `PATH_MAX`. `d_type` saves a `statx` on
directories where the filesystem fills it. `io_uring` is a candidate for a
second walker and goes in only if `bench` says it wins on NVMe *and* does not
lose on ext4/btrfs/HDD.

**Windows.** Two walkers, the fast one first:

1. **The NTFS master file table.** Reading `$MFT` directly (the way WizTree and
   Everything do) sizes a whole volume in seconds regardless of folder count.
   It needs administrator rights, so it runs in `heft-helper`, elevated through
   `ShellExecuteEx` with `runas`, streaming records back on a named pipe whose
   DACL admits only this user and Administrators: Brokey's `transaction/elevate`
   pattern, copied. The helper has a closed list of one operation, "read the
   MFT of volume X", and refuses everything else, with a test that says so.
   **The window is never elevated.**
2. **The directory walk**, for non-NTFS volumes (exFAT, FAT32, ReFS, network
   shares) and when elevation is declined: the same work-stealing pool over
   `NtQueryDirectoryFile` (or `FindFirstFileExW` with `FindExInfoBasic` and
   `FIND_FIRST_EX_LARGE_FETCH`), which returns sizes with the names so no
   per-file stat is needed. Long paths use the `\\?\` prefix throughout.

Which walker ran is shown in the status bar, with the reason if the fast one
did not ("Folder walk: the MFT needs administrator rights").

### What is counted, and how

- **Two sizes per node: on disk and logical.** Allocated size (`st_blocks *
  512`, or the cluster-rounded / compressed size on NTFS) is the default
  because it is what frees space when deleted; logical size is a toggle.
  Sparse files, compressed files and cloud placeholders (OneDrive "online
  only") are where they differ, and the row says so.
- **A hard-linked file is counted once**, keyed by (device, inode) or NTFS file
  id, at the first path the walk reaches; its other paths show it as "also
  linked from". A test builds a hard link and checks the total.
- **Symlinks, junctions and reparse points are not followed.** They are listed
  with their own (tiny) size.
- **One filesystem at a time by default.** A scan of `/` does not descend into
  other mounts; each mount is a separate root the user can add. `/proc`,
  `/sys`, `/dev`, `/run` and other virtual filesystems (detected by `statfs`
  type, not by name) are never scanned.
- **What could not be read is counted, never dropped.** Permission-denied
  folders appear as rows ("Could not be read: permission denied") and the
  total says how many there were, because a total that silently excludes
  `/var/lib/docker` is a lie about where the space went.

### The tree model

One arena per scan, struct-of-arrays, `u32` indices: parent, first child,
next sibling, size on disk, logical size, file count, flags, and an offset into
**one** interned name buffer. Names are bytes (`OsStr`), so non-UTF-8 names on
Linux survive and are shown lossy but deleted exactly. Full paths are rebuilt
on demand by walking parents. Target: **under 40 bytes per entry** before
names, so ten million files fit in well under a gigabyte; `bench` reports peak
memory so the budget is checked, not hoped.

Sizes are aggregated bottom-up as each directory completes, so a folder's size
is final the moment its subtree is, and partial totals during a scan are
honest partial totals.

### Scanning while the window stays live

- **A scan in progress is a normal state, not a modal.** The root folders
  appear at once and their sizes grow as the walk proceeds; the user can
  expand, sort and look at the treemap of what has been found so far, and
  cancel at any time (takes effect within one directory read).
- The walker never takes a lock per file. Threads build private batches and
  publish completed directories to the arena; the UI reads a snapshot at a
  throttled rate (a few times a second while scanning) and never blocks a
  frame on the walker.
- **Progress is honest.** The number of entries is unknown until the end, so
  the bar is an empty rail plus a sentence with live figures ("1,204,331 files,
  412 GB, 6 s"), never an invented percentage. The one exception is the MFT
  walker, which knows the record count up front and may show a real fraction.
- A rescan of one folder replaces that subtree in place; the rest of the tree
  is kept.

### Views

All views read the same tree and share one selection: selecting a folder in
one selects it in all, and the breadcrumb at the top is the shared location.

1. **Tree** (the default, the app's main screen). The scan roots, then their
   top folders, sorted by size, each row with a size bar relative to its
   parent, the size, the share of the parent, the file count and the newest
   modification. Expand from there. Dense, `--font-figure` throughout, columns
   aligned.
2. **Treemap.** Squarified layout, cushion shading in the WinDirStat manner,
   coloured by file type, click to select, double-click to zoom into a folder,
   breadcrumb or Backspace to zoom out. Layout runs off the UI thread for the
   current zoom root and is cached as a vertex buffer; rectangles under a pixel
   are merged into their parent rather than drawn, so frame time depends on
   screen pixels, not on file count.
3. **Sunburst.** Rings by depth, for seeing how deep the weight sits; same
   colours, same zoom.
4. **File types.** Size and count per extension (and per kind: video, images,
   archives, disk images, build output, caches), each one selectable to light
   it up in the treemap.
5. **Largest files.** The top N files anywhere under the selection, kept in a
   bounded heap during the walk so it costs nothing extra.
6. **Age.** Space by last modification, bucketed (this week, this month, this
   year, older), for finding the forgotten.
7. **Duplicates** (opt-in, it reads file contents). Group by size, then by a
   hash of the first and last 64 KiB, then by a full hash, only for groups that
   survive; progress over a known total, so this one may show a percentage.
8. **Suggestions.** Known reclaimable places, found in the scan rather than
   guessed: `node_modules`, Cargo `target/`, `.gradle`, `__pycache__`, package
   manager caches (pacman, apt, dnf, npm, pip, cargo registry), `~/.cache`,
   Windows `Temp`, the Recycle Bin / Trash itself, old Windows Update files,
   browser caches, Docker images. Each says what it is, how big, and what
   removing it costs ("rebuilt on the next build"). Never deleted
   automatically.

### Removing things

- **Delete moves to the Recycle Bin / Trash by default** (`IFileOperation` on
  Windows, the freedesktop Trash spec on Linux, honouring per-mount
  `.Trash-$uid`). Permanent delete is a separate, explicit action, and its
  confirmation states the exact size and file count.
- **Re-check before acting.** Between the scan and the click the path may have
  changed; the deletion re-stats each target and refuses anything whose type or
  identity differs from what was scanned.
- **Protected places refuse with a reason**: the system root, `C:\Windows`,
  `Program Files`, `/usr`, `/boot`, `/etc`, a mount point itself, the user's
  home folder itself. The control is disabled with a tooltip, not hidden.
- Deletion runs off the UI thread over a known total, so its progress is real;
  completed removals are subtracted from the tree at once, with no rescan.
- **Heft does not elevate to delete.** A file that needs root or administrator
  rights says so and leaves it to the user.

### When the space is an application: hand off to Brokey

If the selected folder belongs to an installed application, the right way to
reclaim it is an uninstall, not a delete; deleting half an application is how
people end up with a broken package database. So `heft-scan::owner` answers
"which application is this?" from the machine's own records:

- Windows: Add/Remove Programs `InstallLocation`, MSIX package folders, winget,
  Scoop and Chocolatey directories.
- Linux: the pacman local database file lists, dpkg's `.list` files, the rpm
  database, Flatpak (`/var/lib/flatpak/app`, `~/.local/share/flatpak`, and
  `~/.var/app` for its data), snap, AppImages.

A folder with an owner shows **"Uninstall with Brokey"** in place of Delete
(Delete stays available behind a warning), and Heft starts Brokey on that
application's page. **Heft never uninstalls anything itself**; the robust
uninstaller is Brokey's job and is being built there.

**The contract is `docs/brokey-handoff.md`**, a copy of Brokey's own
(agreed with the Brokey session 2026-11-23; Brokey's copy wins if they differ,
re-copy it). In short: `brokey --show <source>:<id>`, one argument, Brokey's
`PackageRef` spelling (`pacman:steam`, `winget:Valve.Steam`,
`arp:HKLM\<key>`, `flatpak:flathub/app/org.gimp.GIMP/x86_64/stable`,
`msix:<family name>` ...). Exit 0 means handed to a window, 2 means the ref was
not understood. **Process exit is not a "done" signal**, so Heft rescans the
folder when the started process exits *and* whenever its window regains focus.
Support is detected **by version** (`--show` arrives in Brokey 0.1.8; read it
from the package database / ARP `DisplayVersion` or `brokey --version`), never
by probing a flag, because an older Brokey opens its window on an unknown flag.
Brokey is found at `/usr/bin/brokey` on Linux (an AppImage counts as not
installed) and through the `App Paths\brokey.exe` key on Windows. Below 0.1.8
the control is drawn disabled with a tooltip naming the version needed; with no
Brokey it says so and links to Brokey's release page. Ownership lookup reuses Brokey's
readers' logic where it can, but Heft reads the databases itself and does not
link `brokey-core`, so the two release independently.

## Interface

UI follows `../Design-Principles/STYLE-GUIDE.md` and its tokens; accent hue is
**100**. Desktop app, so the root is not stamped `web`. Never a raw hex in a
widget. Umber's `crates/umber-app/src/theme.rs` is the reference implementation
of the token table for egui: port it, as Muster did, and keep the size table
identical. §16 of the guide is the checklist this project is held to: 34 px top
bar with the accent mark, 240 px sidebar, 26 px status bar; selection is a
neutral fill plus strong text plus a small accent mark, never an accent
background; hairlines everywhere, shadows only under things that float; every
figure monospaced and tabular; icons are **Lucide's**, path data vendored
verbatim (Muster's `icons.rs` and `lucide.rs` are the template), with a
tooltip on every icon-only control; Archivo bundled; British spelling,
sentence case, no em dashes, no emoji in anything a user reads; painted
controls, never stock egui widgets where §7 has an answer.

Decided for this application specifically:

- **The sidebar is the list of scan roots** (disks with their used/free bar,
  and folders the user added), plus the views. The content area is the current
  view; the status bar carries the walker used, entries, totals, elapsed time
  and unreadable-folder count.
- **The treemap's colours are a stated departure**, the way Muster's device
  icons are. §2.5 reserves colour for state, but a treemap in one colour cannot
  be read. What keeps it in the family: **no file-type colour names a hex.**
  Each is the accent's own lightness and chroma with the hue moved (Muster's
  `theme::hued`), so both themes come out of the recipe, and hues stay clear of
  20 to 40 so nothing reads as a warning. The selected rectangle gets the
  accent outline; nothing else on the map is accent-coloured.
- **Sizes use one formatter** everywhere (binary units, labelled KiB/MiB/GiB
  or decimal KB/MB/GB by a single setting, defaulting to what the platform's
  own file manager shows), so the same folder never reads two ways on one
  screen.
- **A number on screen is a fact.** A folder still being scanned says so next
  to its size; a total that excludes unreadable folders says how many.

## Packaging, the installer, the updater

Port from Muster rather than designing again; its `packaging/`, `tools/`,
`ci.yml` and `release.yml` are the templates, and `packaging/check.sh` keeps
the application id and the archive paths consistent across the AppStream
file, the desktop entry, the icons, the scriptlets and the updater.

- **The Linux packages enrol the machine in the house archive**, with the
  paths and scripts of `../Packages/ENROLMENT.md` copied, not re-derived, and
  `packaging/linux/spillebulle-archive.asc` copied byte for byte from Muster.
  No path is shared with another package; sources are written by scriptlets,
  created never overwritten, never removed, `.disabled` honoured.
- **At the first release**, add `Spillebulle/Heft` to `../Packages/projects`,
  give this repository `ARCHIVE_DISPATCH_TOKEN`, and copy the "Tell the
  archive" step into `release.yml`. A release is not finished until the
  archive's publish run is green.
- **The updater never prints a command that cannot work.** The remedy is chosen
  by `update::install::detect`, a pure function of an injected probe; with the
  archive it prints the upgrade command, without it it names the exact release
  asset, and where no asset exists for this machine it names none. A test fails
  if a forbidden sentence reaches a machine without the archive.
- **The update check has a budget**: off until the user is asked, throttled and
  persisted, and conditional (`ETag` / `If-None-Match`, so a 304 costs nothing
  against GitHub's sixty-an-hour limit). Muster learned this one with 403s.
- **Cutting a release is pushing a tag**, after CI is green on that exact
  commit. The version lives only in `[workspace.package]`; `CHANGELOG.md` is the
  release notes, published verbatim, and a test fails if the current version's
  section is missing or not newest.
- **An asset name is stated in several places** (release workflow, README
  download table, the updater's `wanted_asset`, the test that checks them).
  Rename all or none.
- **Nothing says "verified" unless releases are signed.** They are not.
- Windows: the swap is rename-then-replace; the setup `.exe` carries the MSI
  appended after the PE and runs from a temporary copy. winget manifest at the
  first stable release (§18.4).

## Repository presentation

README, GitHub description and topics, banner and mark, screenshots, `docs/`
layout and CHANGELOG follow §17 of the style guide, with Umber's README as the
reference. The banner is made with `../Design-Principles/banner.py`. The README
keeps an honest "What is not there yet" list. Screenshots are dark theme, of a
real scan, with nothing personal in the paths.

## Conduct

- **Nothing leaves the machine.** No telemetry, no uploading of file lists,
  no remote lookups of paths or hashes. The update check is the only request
  Heft makes on its own behalf.
- **Heft only reads until told otherwise.** Scanning never modifies a file,
  not even an access time where the platform lets us avoid it (`O_NOATIME`
  where permitted). Every write is a deletion the user asked for.
- **Errors name what went wrong and what to do.**

## Invariants

- The window never runs as root or Administrator. The one elevated process is
  `heft-helper`, with a closed operation list and a test that it refuses
  everything else.
- `heft-scan` has no GUI dependency, and no test of aggregation, accounting or
  deletion bookkeeping needs a real disk.
- No per-node `PathBuf` or `String` in the tree.
- No lock taken per file in the walker; no frame blocked on the walker.
- No invented percentage; no total that silently excludes what could not be
  read; no hard link counted twice.
- Delete goes to the Trash unless the user chose otherwise for that action.
- Heft never uninstalls; it hands off to Brokey.
- Never a raw hex in a widget, never a Unicode glyph as an icon, never an
  accent background for a selected thing.

## Things that look like shortcuts and are not

- `walkdir` / `std::fs::read_dir` + `metadata()` per entry. Fine for a
  prototype, two to five times slower than the per-platform walkers, and
  `metadata()` follows the path from the root each time.
- `jwalk` or another crate as the final walker. A reasonable first step to have
  something on screen; the target is the fd-relative walker above, chosen by
  `bench`, not by taste.
- Recomputing the whole treemap every frame. Lay out once per zoom root and
  size, on a worker, and cache it.
- Holding the arena behind one `Mutex` shared by walker and UI.
- Using `std::fs::remove_dir_all` for Delete. It is permanent, and it does not
  re-check what it is about to remove.
- A spinner over an unknown, a per-file-type colour written as a hex, a stock
  egui `ComboBox` or `Checkbox`.

## Open

- **A Linux elevated scan** (pkexec helper) so `/` can be scanned completely
  without running the window as root. Same closed-list rule as the Windows
  helper.
- **Saving a scan** to reopen later or compare two scans ("what grew since
  last week"). Wants a compact on-disk form of the arena; not before the
  in-memory one has settled.
