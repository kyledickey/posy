#!/usr/bin/env python3
"""Remakes the pictures in the README from the real program.

    cargo build --release && python3 tools/readme_media.py

Each one is posy's own output (`posy snapshot`), painted cell by cell."""
import os
import subprocess
import sys
import tempfile

from PIL import Image

sys.path.insert(0, os.path.dirname(__file__))
from paint import BACKGROUND, paint

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
POSY = os.path.join(ROOT, "target", "release", "posy")
DOCS = os.path.join(ROOT, "docs")
WORK = tempfile.mkdtemp(prefix="posy-media-")
# Start from a clean slate, not whatever config this machine has.
ENV = dict(os.environ, XDG_CONFIG_HOME=os.path.join(WORK, "config"))


def snap(name, args, size, at=9.0, keys=None, frames=1, every=0.1):
    out = os.path.join(WORK, name + ".ans")
    cmd = [POSY, "--color", "truecolor", *args, "snapshot", out]
    cmd += ["--size", f"{size[0]}x{size[1]}", "--at", str(at)]
    if keys:
        cmd += ["--keys", keys]
    if frames > 1:
        cmd += ["--frames", str(frames), "--every", str(every)]
    subprocess.run(cmd, check=True, env=ENV, stdout=subprocess.DEVNULL)
    if frames == 1:
        return paint(out, size)
    return [paint(os.path.join(WORK, f"{name}-{i:03}.ans"), size) for i in range(frames)]


def shrink(img, scale):
    return img.resize((round(img.width * scale), round(img.height * scale)), Image.LANCZOS)


def save_gif(frames, name, scale, seconds_each):
    frames = [shrink(f, scale) for f in frames]
    # One palette for the whole animation keeps colours from flickering.
    sample = Image.new("RGB", (frames[0].width, frames[0].height * 4))
    for i, k in enumerate([0, len(frames) // 3, 2 * len(frames) // 3, len(frames) - 1]):
        sample.paste(frames[k], (0, i * frames[0].height))
    palette = sample.quantize(colors=255, method=Image.Quantize.MEDIANCUT)
    quantized = [f.quantize(palette=palette, dither=Image.Dither.NONE) for f in frames]
    path = os.path.join(DOCS, name)
    quantized[0].save(
        path,
        save_all=True,
        append_images=quantized[1:],
        duration=round(seconds_each * 1000),
        loop=0,
        optimize=True,
    )
    print(f"{name}: {os.path.getsize(path) / 1e6:.1f} MB, {len(frames)} frames")


def save_png(img, name, scale=1.0):
    path = os.path.join(DOCS, name)
    shrink(img, scale).save(path, optimize=True)
    print(f"{name}: {os.path.getsize(path) / 1e3:.0f} KB")


def side_by_side(images, columns, gap=16):
    w, h = images[0].size
    rows = -(-len(images) // columns)
    sheet = Image.new(
        "RGB", (columns * w + (columns - 1) * gap, rows * h + (rows - 1) * gap), BACKGROUND
    )
    for i, img in enumerate(images):
        sheet.paste(img, ((i % columns) * (w + gap), (i // columns) * (h + gap)))
    return sheet


os.makedirs(DOCS, exist_ok=True)

# The bouquet growing in, then turning once.
hero = snap(
    "hero",
    ["-b", "roses", "--style", "fine", "--speed", "0.8", "--seed", "7"],
    (104, 40),
    at=0.4,
    frames=150,
    every=0.1,
)
save_gif(hero, "posy.gif", 0.6, 0.1)

presets = ["classic", "wildflower", "sunset", "spring", "blush", "midnight"]
save_png(
    side_by_side([snap(p, ["-b", p, "--style", "fine"], (84, 36)) for p in presets], 3),
    "bouquets.png",
    0.5,
)

save_png(
    snap("menu", ["-b", "classic", "--style", "fine"], (130, 44), keys="m,down,down,down,down,down,down,down,down,down,enter,down,down,down"),
    "menu.png",
    0.7,
)

save_png(
    snap("ask", ["-b", os.path.join(ROOT, "examples", "ask.toml"), "--style", "fine"], (120, 44), keys="h"),
    "ask.png",
    0.7,
)

styles = [snap(s, ["-b", "sunset", "--style", s], (70, 34)) for s in ["fine", "blocks", "ascii"]]
save_png(side_by_side(styles, 3), "styles.png", 0.6)
