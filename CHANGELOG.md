# Changelog

The notes for each version are what the GitHub release publishes, verbatim.
Newest first.

## 1.0.0

- Scans a disk or a folder fast, with a parallel walker that reads each folder once; the totals grow while it runs and you can stop at any time.
- Counts what deleting would free (or the logical size), counts a hard link once on Linux, never follows links or crosses into other filesystems, and names the folders it could not read instead of leaving them out.
- The folder tree puts the largest folders first, with each one's share of its parent; expand from there.
- The treemap shows everything at once, coloured by kind of file, with a details panel for what you select; double-click to zoom in, Backspace to go up.
- File types, Largest files and Age views show where the space went by extension, by file and by how long it has been left alone.
- Delete moves to the Recycle Bin or Trash; deleting permanently asks a second time, and system folders are refused.
- `heft scan` and `heft bench` run the same engine in a terminal, with `--json` for scripts.
- Installers for Windows and packages for Debian, Fedora and Arch that keep Heft up to date, plus an AppImage; Heft asks before it ever checks for a new version.
