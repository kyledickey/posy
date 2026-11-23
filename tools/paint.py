"""Paints posy's saved frames (`posy snapshot out.ans`) as pictures, the way
a terminal with built-in block drawing shows them. Needs Pillow, and Python
3.14+ for the names of the block characters."""
import re
import unicodedata

from PIL import Image, ImageDraw, ImageFont

CELL_W, CELL_H = 10, 20
BACKGROUND = (22, 22, 30)
TEXT = (221, 221, 221)
FONTS = [
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/System/Library/Fonts/Menlo.ttc",
]

# Block characters older than the "octants", by which squares they fill.
OLDER = {
    "12345678": 0x2588, "1": 0x1CEA8, "2": 0x1CEAB, "7": 0x1CEA3, "8": 0x1CEA0,
    "12": 0x1FB82, "123456": 0x1FB85, "78": 0x2582, "345678": 0x2586,
    "35": 0x1FBE6, "46": 0x1FBE7, "13": 0x2598, "24": 0x259D, "57": 0x2596,
    "68": 0x2597, "1357": 0x258C, "2468": 0x2590, "1234": 0x2580, "5678": 0x2584,
    "2457": 0x259E, "1368": 0x259A, "123457": 0x259B, "123468": 0x259C,
    "135678": 0x2599, "245678": 0x259F,
}


def _bits(cells):
    return sum(1 << (int(d) - 1) for d in cells)


BLOCKS = {chr(cp): _bits(cells) for cells, cp in OLDER.items()}
for cp in range(0x1CD00, 0x1CDE6):
    BLOCKS[chr(cp)] = _bits(unicodedata.name(chr(cp)).split("-")[1])


def _font():
    for path in FONTS:
        try:
            return ImageFont.truetype(path, 16)
        except OSError:
            pass
    return ImageFont.load_default()


FONT = _font()


def cells(path):
    """The frame as rows of (character, colour, background or None)."""
    rows = []
    for line in open(path, encoding="utf-8").read().split("\n"):
        fg, bg, row = TEXT, None, []
        for m in re.finditer(r"\x1b\[([0-9;]*)m|(.)", line):
            if m.group(2) is not None:
                row.append((m.group(2), fg, bg))
                continue
            p = m.group(1).split(";")
            if p[0] == "38":
                fg = tuple(map(int, p[2:5]))
            elif p[0] == "48":
                bg = tuple(map(int, p[2:5]))
            elif p[0] == "49":
                bg = None
            elif p[0] in ("0", ""):
                fg, bg = TEXT, None
        rows.append(row)
    while rows and not rows[-1]:
        rows.pop()
    return rows


def paint(path, size=None):
    """A picture of the frame saved at `path`. `size` is (columns, rows)."""
    rows = cells(path)
    cols = size[0] if size else max(len(r) for r in rows)
    count = size[1] if size else len(rows)
    img = Image.new("RGB", (cols * CELL_W, count * CELL_H), BACKGROUND)
    d = ImageDraw.Draw(img)
    for y, row in enumerate(rows):
        for x, (ch, fg, bg) in enumerate(row):
            left, top = x * CELL_W, y * CELL_H
            if bg:
                d.rectangle([left, top, left + CELL_W - 1, top + CELL_H - 1], fill=bg)
            if ch == " ":
                continue
            if ch in BLOCKS:
                for k in range(8):
                    if BLOCKS[ch] >> k & 1:
                        c, r = k % 2, k // 2
                        d.rectangle(
                            [left + c * 5, top + r * 5, left + c * 5 + 4, top + r * 5 + 4],
                            fill=fg,
                        )
            else:
                d.text((left + CELL_W / 2, top + CELL_H / 2), ch, font=FONT, fill=fg, anchor="mm")
    return img
