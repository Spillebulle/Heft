# Changelog

## 0.0.1 (unreleased)

The first build. Not released yet: there are no packages and no updater.

- Scans any disk or folder with a parallel walker: `getdents64` and `statx` relative to the folder on Linux, `FindFirstFileExW` with large fetches on Windows.
- Counts the space a file takes on disk (the default) or its logical size; hard links once; does not follow links or cross into other filesystems; says how many folders could not be read instead of leaving them out silently.
- Folders view: the largest folders first, expand from there.
- Treemap: squarified, shaded, coloured by file kind; double-click to zoom in, Backspace to go up.
- File types, Largest files and Age views.
- Delete moves to the Recycle Bin or Trash; deleting permanently asks a second time. System folders are refused.
- `heft scan` and `heft bench` in a terminal.
