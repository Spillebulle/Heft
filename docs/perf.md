# Performance log

Every measurement in `--release`, with the machine, the filesystem and the
entry count, so a regression has something to be compared with. Newest first.

| Date | Build | Machine / OS | Filesystem | Tree | Entries | Cold | Warm | Arena |
|---|---|---|---|---|---|---|---|---|
| 2026-11-23 | 3e65fa6 | Windows 11, NVMe | NTFS | `C:\Users\spill\Code` | 483,761 in 45,312 folders | 9.2 s | 2.5 s (≈190k entries/s) | 40.5 MiB, 88 B/entry with names |

Notes:

- Windows numbers are the folder walk (`FindFirstFileExW`, large fetch). The
  MFT walker is the planned fast path and should beat these by an order of
  magnitude on a whole volume.
- "Cold" was the first run after a long idle, not a reboot; treat it as an
  upper bound until it is measured properly.
- The headless window scan of the same tree finished in about 0.7 s once the
  filesystem cache was warm (status bar of `target/shots/`).
