# Heft

**Find out what is taking up your disk, fast.**

Heft scans a disk or a folder and shows where the space went: the biggest
folders first, a treemap of everything, the largest files, the space per file
type and per age. Then it lets you get rid of what you do not need, to the
Recycle Bin or Trash by default. One native application for Windows and Linux.

## Install

**Heft 1.0.0.** Take the file for your system, or browse the
[release itself](https://github.com/Spillebulle/heft/releases/latest) for the
notes.

| Your system | x86-64 | ARM64 |
|---|---|---|
| Windows | [Installer](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft-setup-1.0.0-x64.exe) | [Installer](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft-setup-1.0.0-arm64.exe) |
| Debian, Ubuntu, Mint | [`.deb`](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft_1.0.0_amd64.deb) | [`.deb`](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft_1.0.0_arm64.deb) |
| Fedora, RHEL, openSUSE | [`.rpm`](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft-1.0.0-1.x86_64.rpm) | [`.rpm`](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft-1.0.0-1.aarch64.rpm) |
| Arch | [`.pkg.tar.zst`](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft-bin-1.0.0-1-x86_64.pkg.tar.zst) | not built |
| Any other Linux | [AppImage](https://github.com/Spillebulle/heft/releases/download/v1.0.0/Heft-1.0.0-x86_64.AppImage), one file with nothing to install | [AppImage](https://github.com/Spillebulle/heft/releases/download/v1.0.0/Heft-1.0.0-aarch64.AppImage) |
| Windows, `.msi` to deploy | [`.msi`](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft-1.0.0-x64.msi) | [`.msi`](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft-1.0.0-arm64.msi) |
| Windows, no installer | [`.zip`](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft-1.0.0-x86_64-pc-windows-msvc.zip) | [`.zip`](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft-1.0.0-aarch64-pc-windows-msvc.zip) |
| Linux, no package | [`.tar.gz`](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft-1.0.0-x86_64-unknown-linux-gnu.tar.gz) | [`.tar.gz`](https://github.com/Spillebulle/heft/releases/download/v1.0.0/heft-1.0.0-aarch64-unknown-linux-gnu.tar.gz) |

The `.deb`, the `.rpm` and the Arch package add the
[Spillebulle archive](https://spillebulle.github.io/packages/) as they install,
so `apt upgrade`, `pacman -Syu` or your usual system update carries Heft along
with everything else. On Arch the package adds a `[spillebulle]` repository to
`/etc/pacman.conf`, after your distribution's own. Nothing to configure.

To add the archive first and install from it, on Debian, Ubuntu, Mint and
Pop!_OS:

```sh
sudo mkdir -p /etc/apt/keyrings
curl -fsSL https://spillebulle.github.io/packages/spillebulle-archive.asc   | sudo gpg --dearmor -o /etc/apt/keyrings/spillebulle-archive.gpg
sudo tee /etc/apt/sources.list.d/spillebulle.sources >/dev/null <<'EOF'
Types: deb
URIs: https://spillebulle.github.io/packages/deb/
Suites: ./
Signed-By: /etc/apt/keyrings/spillebulle-archive.gpg
EOF
sudo apt update && sudo apt install heft
```

On Arch, CachyOS, EndeavourOS and Manjaro:

```sh
curl -fsSL https://spillebulle.github.io/packages/spillebulle-archive.asc -o /tmp/spillebulle-archive.asc
sudo pacman-key --add /tmp/spillebulle-archive.asc
sudo pacman-key --lsign-key 7A9EA69E033F3C3EF4F53EB4A72BC0FD5E061ADA
printf '%s
' 'SigLevel = Required DatabaseRequired'   'Server = https://spillebulle.github.io/packages/arch/$arch'   | sudo tee /etc/pacman.d/spillebulle >/dev/null
printf '
[spillebulle]
Include = /etc/pacman.d/spillebulle
' | sudo tee -a /etc/pacman.conf >/dev/null
sudo pacman -Syu heft-bin
```

On Fedora, RHEL and openSUSE:

```sh
sudo rpm --import https://spillebulle.github.io/packages/spillebulle-archive.asc
sudo tee /etc/yum.repos.d/spillebulle.repo >/dev/null <<'EOF'
[spillebulle]
name=Spillebulle
baseurl=https://spillebulle.github.io/packages/rpm/
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=https://spillebulle.github.io/packages/spillebulle-archive.asc
EOF
```

Heft runs as whatever user you started it as and reads what that user can
read; it never asks for administrator rights. The window needs a GPU with
Vulkan or Direct3D 12, which is any machine from the last decade. The text mode
needs none of the window's libraries and runs over SSH.

Heft can check for new versions when it starts. It asks you before the first
check, and you can change the answer in **Settings**. That request is the only
one Heft makes on its own behalf, and it goes out at most once every six hours.
The installer, the AppImage and the portable downloads update themselves; the
packages are updated by your package manager.

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
`libx11-dev`). The folder picker uses the desktop portal at run time.

## Licence

GPL-3.0-or-later. Icons are [Lucide](https://lucide.dev) (ISC). The typeface
is Archivo (SIL Open Font License).
