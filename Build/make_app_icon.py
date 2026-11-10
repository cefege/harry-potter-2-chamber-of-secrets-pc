#!/usr/bin/env python3
"""Draws the macOS app icon (original artwork, no game assets) and builds AppIcon.icns.

A midnight-blue rounded tile with a starry sky and a golden lightning bolt. Rendered at 4x and
downsampled for smooth edges. Requires Pillow and macOS `iconutil`.

    python3 Build/make_app_icon.py   # writes Build/macos/AppIcon.png and Build/macos/AppIcon.icns
"""
import math
import random
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter

OUT = Path(__file__).resolve().parent / "macos"
S = 4096  # working size (4x of 1024)


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(len(a)))


def squircle_mask(size, inset, radius):
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle((inset, inset, size - inset, size - inset), radius=radius, fill=255)
    return mask


def gradient(size, top, bottom, centre_glow):
    img = Image.new("RGB", (size, size))
    px = img.load()
    cx, cy = size * 0.5, size * 0.42
    maxd = size * 0.75
    for y in range(size):
        base = lerp(top, bottom, y / (size - 1))
        for x in range(0, size):
            d = math.hypot(x - cx, y - cy) / maxd
            glow = max(0.0, 1.0 - d) ** 2
            px[x, y] = lerp(base, centre_glow, glow * 0.55)
    return img


def bolt_points(size):
    # A classic three-segment lightning bolt, slightly tilted.
    u = size / 1024
    pts = [(560, 150), (330, 560), (500, 560), (410, 880), (720, 420), (545, 420), (650, 150)]
    k = 0.86  # keep clear of the rim
    return [((512 + (x - 512) * k) * u, (515 + (y - 515) * k) * u) for x, y in pts]


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    inset, radius = int(S * 0.098), int(S * 0.225)
    small = 1024
    bg = gradient(small, (14, 18, 58), (44, 16, 72), (70, 60, 140)).resize((S, S), Image.BICUBIC)
    draw = ImageDraw.Draw(bg)

    # Stars and a few four-point sparkles.
    rng = random.Random(1992)
    for _ in range(160):
        x, y = rng.uniform(inset, S - inset), rng.uniform(inset, S * 0.9)
        r = rng.choice([4, 5, 6, 8, 10])
        a = rng.randint(120, 255)
        draw.ellipse((x - r, y - r, x + r, y + r), fill=(255, 255, 240 if a > 200 else 220))
    for x, y, r in [(0.24, 0.26, 70), (0.78, 0.30, 52), (0.70, 0.74, 60), (0.30, 0.70, 40)]:
        x, y = x * S, y * S
        w = r * 0.22
        draw.polygon([(x, y - r), (x + w, y - w), (x + r, y), (x + w, y + w), (x, y + r), (x - w, y + w), (x - r, y), (x - w, y - w)], fill=(255, 246, 210))

    # Golden bolt: glow, dark outline, gradient fill, highlight.
    pts = bolt_points(S)
    glow = Image.new("L", (S, S), 0)
    ImageDraw.Draw(glow).polygon(pts, fill=255)
    glow = glow.filter(ImageFilter.GaussianBlur(S * 0.03))
    bg = Image.composite(Image.new("RGB", (S, S), (255, 200, 90)), bg, glow.point(lambda v: int(v * 0.55)))

    outline = Image.new("L", (S, S), 0)
    ImageDraw.Draw(outline).polygon(pts, fill=255)
    outline = outline.filter(ImageFilter.MaxFilter(int(S * 0.012) | 1))
    bg = Image.composite(Image.new("RGB", (S, S), (70, 34, 6)), bg, outline)

    fill_mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(fill_mask).polygon(pts, fill=255)
    gold = Image.new("RGB", (S, S))
    gpx = gold.load()
    for y in range(0, S, 4):
        c = lerp((255, 236, 150), (196, 128, 26), y / S)
        for x in range(0, S, 4):
            shade = lerp(c, (255, 250, 220), max(0.0, 0.35 - abs((x - y * 0.35) / S - 0.33)))
            for dy in range(4):
                for dx in range(4):
                    gpx[x + dx, y + dy] = shade
    bg = Image.composite(gold, bg, fill_mask)

    # Thin gold rim inside the tile edge.
    rim = Image.new("L", (S, S), 0)
    rd = ImageDraw.Draw(rim)
    rd.rounded_rectangle((inset + 22, inset + 22, S - inset - 22, S - inset - 22), radius=radius - 22, outline=255, width=26)
    bg = Image.composite(Image.new("RGB", (S, S), (212, 168, 76)), bg, rim)

    # Tile shape with a soft drop shadow on a transparent canvas.
    mask = squircle_mask(S, inset, radius)
    shadow = mask.filter(ImageFilter.GaussianBlur(S * 0.015)).point(lambda v: int(v * 0.45))
    shadow = ImageChops.offset(shadow, 0, int(S * 0.012))
    canvas = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    canvas.paste((0, 0, 0, 255), (0, 0), shadow)
    tile = bg.convert("RGBA")
    tile.putalpha(mask)
    canvas = Image.alpha_composite(canvas, tile)
    icon = canvas.resize((1024, 1024), Image.LANCZOS)
    icon.save(OUT / "AppIcon.png")

    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / "AppIcon.iconset"
        iconset.mkdir()
        for base in (16, 32, 128, 256, 512):
            icon.resize((base, base), Image.LANCZOS).save(iconset / f"icon_{base}x{base}.png")
            icon.resize((base * 2, base * 2), Image.LANCZOS).save(iconset / f"icon_{base}x{base}@2x.png")
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(OUT / "AppIcon.icns")], check=True)
    print(OUT / "AppIcon.icns")


if __name__ == "__main__":
    main()
