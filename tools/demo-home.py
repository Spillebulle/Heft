r"""A made-up home folder for the README's screenshots: realistic names, sizes
and dates, nothing personal. Windows only. About 186 GB appears in seconds,
because files are extended without being written (NTFS defers the zeroing),
but it is real allocation: put it on a disk with the room.

    python tools/demo-home.py E:\Home
"""
import os, random, sys, time, pathlib, ctypes, msvcrt
k32 = ctypes.WinDLL("kernel32")
k32.SetFilePointerEx.argtypes = [ctypes.c_void_p, ctypes.c_longlong, ctypes.c_void_p, ctypes.c_uint]
k32.SetEndOfFile.argtypes = [ctypes.c_void_p]

root = pathlib.Path(sys.argv[1])
random.seed(7)
GB, MB, KB = 1 << 30, 1 << 20, 1 << 10
now = time.time()
DAY = 86400

def f(rel, size, age_days=None):
    p = root / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    with open(p, "wb") as h:
        # Python's truncate writes zeros on Windows; SetEndOfFile allocates
        # without writing (NTFS defers zeroing to the valid data length).
        handle = msvcrt.get_osfhandle(h.fileno())
        k32.SetFilePointerEx(handle, ctypes.c_longlong(int(size)), None, 0)
        assert k32.SetEndOfFile(handle)
    age = age_days if age_days is not None else random.uniform(1, 1400)
    t = now - age * DAY
    os.utime(p, (t, t))

def many(folder, n, ext, lo, hi, age=(1, 1400), names=None):
    for i in range(n):
        name = names(i) if names else f"{i:04d}"
        f(f"{folder}/{name}.{ext}", random.uniform(lo, hi), random.uniform(*age))

# Videos
for name, gb, age in [("Holiday 2023/Coast day 1", 3.8, 600), ("Holiday 2023/Coast day 2", 2.9, 598),
                      ("Holiday 2023/Ferry", 1.4, 597), ("Wedding/Ceremony", 6.2, 900),
                      ("Wedding/Speeches", 2.3, 900), ("Wedding/Dancing", 4.1, 899),
                      ("Screen recordings/Tutorial draft", 1.1, 40), ("Screen recordings/Bug report", 0.4, 12)]:
    f(f"Videos/{name}.mp4", gb * GB, age)
many("Videos/Phone", 60, "mov", 40 * MB, 600 * MB, (20, 1200), lambda i: f"IMG_{4100 + i}")

# Pictures
for year in (2022, 2023, 2024, 2025):
    many(f"Pictures/{year}", random.randint(120, 260), "jpg", 2 * MB, 9 * MB,
         ((2026 - year) * 365 - 300, (2026 - year) * 365), lambda i: f"DSC{random.randint(1000, 9999)}_{i}")
many("Pictures/Raw", 90, "arw", 24 * MB, 32 * MB, (300, 700))
many("Pictures/Wallpapers", 30, "png", 1 * MB, 12 * MB, (100, 1300))

# Music
for artist in ("The Lanterns", "Northbound", "Ola Strand", "Paper Harbour", "Sølvtone"):
    for album in range(random.randint(1, 3)):
        many(f"Music/{artist}/Album {album + 1}", random.randint(8, 14), "flac", 18 * MB, 45 * MB, (800, 1500))

# Downloads
f("Downloads/ubuntu-24.04.2-desktop-amd64.iso", 6.1 * GB, 200)
f("Downloads/Win11_24H2_English_x64.iso", 5.4 * GB, 420)
f("Downloads/archlinux-2025.10.01-x86_64.iso", 1.4 * GB, 55)
f("Downloads/project-assets.zip", 2.2 * GB, 90)
f("Downloads/old-backup.tar.zst", 3.3 * GB, 700)
many("Downloads", 40, "pdf", 100 * KB, 12 * MB, (1, 500), lambda i: f"document-{i}")
many("Downloads", 12, "exe", 5 * MB, 180 * MB, (10, 600), lambda i: f"setup-{i}")

# Games
for game, gb in [("Starfall", 38), ("Tiny Harbour", 2.6), ("Ridge Racer Club", 14)]:
    left = gb * GB
    k = 0
    while left > 0:
        s = min(left, random.uniform(0.2, 4) * GB)
        f(f"Games/{game}/data/pak{k:02d}.pak", s, 120)
        left -= s
        k += 1
    many(f"Games/{game}/shaders", 80, "bin", 50 * KB, 4 * MB, (10, 120))
    f(f"Games/{game}/{game.replace(' ', '')}.exe", random.uniform(40, 160) * MB, 120)

# Projects: build output and dependencies, the classic weight
for proj, kind in [("website", "node"), ("dashboard", "node"), ("raytracer", "rust"), ("notes-app", "rust"), ("scraper", "python")]:
    many(f"Projects/{proj}/src", random.randint(20, 60), {"node": "ts", "rust": "rs", "python": "py"}[kind], 1 * KB, 60 * KB, (1, 200))
    if kind == "node":
        for pkg in range(140):
            many(f"Projects/{proj}/node_modules/pkg-{pkg}", random.randint(3, 12), "js", 2 * KB, 900 * KB, (30, 200))
    elif kind == "rust":
        many(f"Projects/{proj}/target/release/deps", 300, "rlib", 200 * KB, 30 * MB, (1, 60))
        many(f"Projects/{proj}/target/debug/incremental", 200, "bin", 1 * MB, 40 * MB, (1, 30))
    else:
        many(f"Projects/{proj}/.venv/lib", 400, "py", 2 * KB, 400 * KB, (100, 300))

# Documents
many("Documents/Invoices", 80, "pdf", 80 * KB, 1 * MB, (30, 1400))
many("Documents/Reports", 30, "docx", 40 * KB, 6 * MB, (10, 900))
many("Documents/Spreadsheets", 25, "xlsx", 20 * KB, 3 * MB, (5, 700))
f("Documents/Taxes 2024.pdf", 2.1 * MB, 230)

# Virtual machines
f("Virtual machines/Fedora 43/Fedora 43.vdi", 18.5 * GB, 30)
f("Virtual machines/Windows test/Windows test.vhdx", 24 * GB, 75)
print("ok")

# Folders carry the newest time of what is inside, as a lived-in disk does.
for d, dirs, files in os.walk(root, topdown=False):
    times = [os.stat(os.path.join(d, x)).st_mtime for x in dirs + files]
    if times:
        os.utime(d, (max(times), max(times)))
print("folder times set")
