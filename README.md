# Heft

**Find out what is taking up your disk, fast.**

Heft scans a disk or a folder and shows where the space went: the biggest
folders first, a treemap of everything, the largest files, the space per file
type and per age. Then it lets you get rid of what you do not need, to the
Recycle Bin or Trash by default. One native application for Windows and Linux.

## What it does

- **Fast scans.** A parallel walker that reads each folder once and sizes its
  entries without opening them. On Linux it uses `getdents64` and `statx`
  relative to the folder; on Windows, `FindFirstFileExW` with large fetches.
- **Honest numbers.** Space on disk (what deleting frees) or logical size, hard
  links counted once, links never followed, other filesystems left alone, and
  folders that could not be read are counted and named rather than silently
  left out of the total.
- **Views:** Folders (expand from the top), Treemap, File types, Largest files,
  Age.
- **Delete safely.** To the Recycle Bin / Trash by default; permanent deletion
  asks again; system folders are refused with a reason; every target is
  checked again just before it is removed.
- **A terminal mode**, the same engine without the window:

  ```sh
  heft scan ~/ --depth 2 --files 10
  heft scan C:\ --json
  heft bench /usr
  ```

## What is not there yet

- **Packages and the updater.** No `.msi`, `.deb`, `.rpm` or Arch package
  yet, and no self-update. Build from source for now.
- **The NTFS fast path.** Windows uses the folder walk for now; reading the
  master file table directly (WizTree's approach) needs an elevated helper
  that is not built. Until it is, a hard-linked file on Windows is counted at
  every path it has (the folder walk cannot see link counts), so
  `C:\Windows\WinSxS` reads larger than the space it really takes.
- **Uninstall with Brokey.** Recognising that a folder belongs to an installed
  application and handing it to Brokey is agreed (`docs/brokey-handoff.md`)
  but not built.
- **Sunburst, Duplicates and Suggestions** (known caches and build folders)
  are planned, not built.
- macOS is not a target.

## Building

```sh
cargo run --release              # the window
cargo run --release -- C:\Users  # the window, scanning a folder at once
cargo run --release -- scan ~/   # the terminal mode
cargo test --workspace
```

Linux needs the usual windowing headers (`libwayland-dev`, `libxkbcommon-dev`,
`libx11-dev`) and GTK 3 for the folder picker.

## Licence

GPL-3.0-or-later. Icons are [Lucide](https://lucide.dev) (ISC). The typeface
is Archivo (SIL Open Font License).
