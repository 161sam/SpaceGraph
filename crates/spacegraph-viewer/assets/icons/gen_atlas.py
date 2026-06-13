#!/usr/bin/env python3
"""Generate the node-face icon atlas (v0.4.1, dep-free — Python stdlib only).

Emits a 4x4 grid of 64px monochrome glyphs (white, alpha = coverage) as:
  - atlas.rgba : raw RGBA8 256x256, loaded at runtime via include_bytes! +
                 Image::new (no Bevy image-format feature needed).
  - atlas.png  : the same image, for human inspection / PR review only.

No runtime rasterization: the committed blob is the only thing the viewer reads.
Cell order must match `render::node_icon::IconId::cell`.
"""
import struct
import zlib

CELL = 64
COLS = 4
ROWS = 4
W = CELL * COLS
H = CELL * ROWS
PAD = 9  # glyph inset inside the cell

# RGBA buffer, transparent.
buf = bytearray(W * H * 4)


def px(x, y, a=255):
    if 0 <= x < W and 0 <= y < H:
        i = (y * W + x) * 4
        buf[i] = 255
        buf[i + 1] = 255
        buf[i + 2] = 255
        buf[i + 3] = max(buf[i + 3], a)


def disc(cx, cy, r, fill=False):
    for y in range(cy - r, cy + r + 1):
        for x in range(cx - r, cx + r + 1):
            d2 = (x - cx) ** 2 + (y - cy) ** 2
            if fill:
                if d2 <= r * r:
                    px(x, y)
            else:
                if (r - 1.4) ** 2 <= d2 <= (r + 0.4) ** 2:
                    px(x, y)


def line(x0, y0, x1, y1):
    dx = abs(x1 - x0)
    dy = -abs(y1 - y0)
    sx = 1 if x0 < x1 else -1
    sy = 1 if y0 < y1 else -1
    err = dx + dy
    while True:
        px(x0, y0)
        if x0 == x1 and y0 == y1:
            break
        e2 = 2 * err
        if e2 >= dy:
            err += dy
            x0 += sx
        if e2 <= dx:
            err += dx
            y0 += sy


def rect(x0, y0, x1, y1, fill=False):
    if fill:
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                px(x, y)
    else:
        line(x0, y0, x1, y0)
        line(x0, y1, x1, y1)
        line(x0, y0, x0, y1)
        line(x1, y0, x1, y1)


def tri(p0, p1, p2):
    line(*p0, *p1)
    line(*p1, *p2)
    line(*p2, *p0)


def tri_fill(p0, p1, p2):
    ys = [p0[1], p1[1], p2[1]]
    for y in range(min(ys), max(ys) + 1):
        xs = []
        for (ax, ay), (bx, by) in ((p0, p1), (p1, p2), (p2, p0)):
            if (ay <= y < by) or (by <= y < ay):
                xs.append(ax + (bx - ax) * (y - ay) / (by - ay))
        if len(xs) >= 2:
            xs.sort()
            for x in range(int(xs[0]), int(xs[-1]) + 1):
                px(x, y)


def draw(cell, kind):
    """Draw glyph `kind` into `cell`. All helpers below are cell-relative
    (0..CELL); they add the cell origin internally."""
    ox = (cell % COLS) * CELL
    oy = (cell // COLS) * CELL
    cx, cy = CELL // 2, CELL // 2

    def L(x0, y0, x1, y1):
        line(ox + x0, oy + y0, ox + x1, oy + y1)

    def R(x0, y0, x1, y1, fill=False):
        rect(ox + x0, oy + y0, ox + x1, oy + y1, fill)

    def D(x, y, r, fill=False):
        disc(ox + x, oy + y, r, fill)

    def T(p0, p1, p2, fill=False):
        f = tri_fill if fill else tri
        f((ox + p0[0], oy + p0[1]), (ox + p1[0], oy + p1[1]), (ox + p2[0], oy + p2[1]))

    def DOC():
        # document body with a folded top-right corner; returns inner bounds.
        x0, y0, x1, y1 = PAD + 3, PAD, CELL - PAD - 3, CELL - PAD
        fold = 11
        L(x0, y0, x1 - fold, y0)
        L(x1 - fold, y0, x1, y0 + fold)
        L(x1, y0 + fold, x1, y1)
        L(x1, y1, x0, y1)
        L(x0, y1, x0, y0)
        L(x1 - fold, y0, x1 - fold, y0 + fold)
        L(x1 - fold, y0 + fold, x1, y0 + fold)
        return x0, y0, x1, y1

    if kind == "process":  # diamond (octahedron face) + inner ring
        L(32, PAD, CELL - PAD, 32)
        L(CELL - PAD, 32, 32, CELL - PAD)
        L(32, CELL - PAD, PAD, 32)
        L(PAD, 32, 32, PAD)
        D(cx, cy, 7)
    elif kind == "user":  # head + shoulders
        D(cx, 22, 9)
        T((16, CELL - PAD), (48, CELL - PAD), (cx, 33))
    elif kind == "socket":  # ring (torus aperture)
        D(cx, cy, 17)
        D(cx, cy, 7)
    elif kind == "host":  # server rack
        R(PAD, PAD + 4, CELL - PAD, CELL - PAD - 4)
        L(PAD + 4, 26, CELL - PAD - 4, 26)
        L(PAD + 4, 40, CELL - PAD - 4, 40)
        D(PAD + 9, 19, 2, fill=True)
        D(PAD + 9, 33, 2, fill=True)
    elif kind == "alert":  # warning triangle + bang
        T((cx, PAD), (CELL - PAD, CELL - PAD), (PAD, CELL - PAD))
        L(32, 26, 32, 42)
        D(cx, 49, 2, fill=True)
    elif kind == "file":  # plain document
        DOC()
    elif kind == "image":  # doc + mountains + sun
        _, _, _, y1 = DOC()
        D(24, 26, 4)
        T((20, y1 - 2), (32, 34), (44, y1 - 2), fill=True)
    elif kind == "video":  # doc + play triangle
        DOC()
        T((26, 24), (26, 44), (44, 34), fill=True)
    elif kind == "text":  # doc + text lines
        x0, _, x1, _ = DOC()
        for i, yy in enumerate(range(24, 46, 6)):
            L(x0 + 5, yy, x1 - 5 - (4 if i % 2 else 0), yy)
    elif kind == "code":  # angle brackets < >
        L(28, 22, 18, 32)
        L(18, 32, 28, 42)
        L(36, 22, 46, 32)
        L(46, 32, 36, 42)
    elif kind == "json":  # braces { }
        L(26, 20, 22, 24)
        L(22, 24, 22, 30)
        L(22, 30, 18, 32)
        L(18, 32, 22, 34)
        L(22, 34, 22, 40)
        L(22, 40, 26, 44)
        L(38, 20, 42, 24)
        L(42, 24, 42, 30)
        L(42, 30, 46, 32)
        L(46, 32, 42, 34)
        L(42, 34, 42, 40)
        L(42, 40, 38, 44)
    elif kind == "log":  # doc + bulleted lines
        x0, _, x1, _ = DOC()
        for yy in range(24, 46, 7):
            D(x0 + 4, yy, 1, fill=True)
            L(x0 + 9, yy, x1 - 5, yy)
    elif kind == "audio":  # note
        D(24, 42, 6, fill=True)
        L(30, 42, 30, 18)
        L(30, 18, 44, 22)
        L(44, 22, 44, 38)
        D(40, 38, 5, fill=True)
    elif kind == "archive":  # box + band/zipper
        R(PAD, PAD + 3, CELL - PAD, CELL - PAD - 3)
        L(32, PAD + 3, 32, CELL - PAD - 3)
        for yy in range(PAD + 6, CELL - PAD - 4, 6):
            D(31, yy, 0, fill=True)
            D(33, yy, 0, fill=True)
    elif kind == "binary":  # chip with 1/0
        R(16, 16, 48, 48)
        for t in range(20, 45, 6):  # pins
            L(t, 10, t, 16)
            L(t, 48, t, 54)
            L(10, t, 16, t)
            L(48, t, 54, t)
        L(26, 26, 26, 38)  # 1
        R(32, 26, 40, 38)  # 0
    else:
        R(PAD, PAD, CELL - PAD, CELL - PAD)


ORDER = [
    "process",   # 0
    "user",      # 1
    "socket",    # 2
    "host",      # 3
    "alert",     # 4
    "file",      # 5
    "image",     # 6
    "video",     # 7
    "text",      # 8
    "code",      # 9
    "json",      # 10
    "log",       # 11
    "audio",     # 12
    "archive",   # 13
    "binary",    # 14
]

for i, k in enumerate(ORDER):
    draw(i, k)

with open("atlas.rgba", "wb") as f:
    f.write(buf)


def write_png(path, w, h, rgba):
    def chunk(tag, data):
        c = tag + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)

    raw = bytearray()
    for y in range(h):
        raw.append(0)  # filter: none
        raw.extend(rgba[(y * w) * 4:(y * w + w) * 4])
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


write_png("atlas.png", W, H, buf)
print(f"wrote atlas.rgba ({len(buf)} bytes) + atlas.png  ({W}x{H}, {len(ORDER)} glyphs)")
