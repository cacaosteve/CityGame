#!/bin/sh
# Generate CityGame icons for macOS, Windows, Linux, Android, and iPadOS
# from assets/citygame-icon.png (same artwork as the macOS dock icon).
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
export CITYGAME_ICON_ROOT="$root"

python3 - "$@" <<'PY'
from __future__ import annotations

import argparse
import json
import os
import subprocess
from pathlib import Path

from PIL import Image

parser = argparse.ArgumentParser(description="Generate CityGame platform icons")
parser.add_argument("--macos-only", action="store_true", help="only regenerate the macOS .icns")
args = parser.parse_args()

root = Path(os.environ["CITYGAME_ICON_ROOT"])
src = Image.open(root / "assets/citygame-icon.png").convert("RGBA")
# Artwork is pre-rounded with transparent corners; iOS/Android need opaque full-bleed.
ICON_FILL = (32, 59, 75, 255)  # matches the drawn navy border


def opaque_fullbleed(img: Image.Image) -> Image.Image:
    """Composite onto solid fill so OS masks never show white through transparent corners."""
    base = Image.new("RGBA", img.size, ICON_FILL)
    base.alpha_composite(img)
    return base.convert("RGB")


def save_png(img: Image.Image, path: Path, size: int, *, fullbleed: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    out = opaque_fullbleed(img) if fullbleed else img
    resized = out.resize((size, size), Image.Resampling.LANCZOS)
    # iOS App Icons must not include an alpha channel (transparent → white on Home Screen).
    if fullbleed and resized.mode != "RGB":
        resized = resized.convert("RGB")
    resized.save(path, format="PNG")


print("==> macOS .icns")
iconset = root / "target/citygame-icon.iconset"
if iconset.exists():
    for p in iconset.iterdir():
        p.unlink()
else:
    iconset.mkdir(parents=True)
for size in (16, 32, 128, 256, 512):
    # macOS icons retain the PNG's transparent rounded corners. Flattening this
    # artwork changes the appearance of the app icon in macOS.
    save_png(src, iconset / f"icon_{size}x{size}.png", size)
    save_png(src, iconset / f"icon_{size}x{size}@2x.png", size * 2)
subprocess.check_call(
    ["iconutil", "-c", "icns", str(iconset), "-o", str(root / "assets/CityGame.icns")]
)

print("==> City map document icon")
from PIL import ImageDraw

scale = 3
canvas = Image.new("RGBA", (1024 * scale, 1024 * scale), (0, 0, 0, 0))
draw = ImageDraw.Draw(canvas)

def scaled_box(box):
    return tuple(round(value * scale) for value in box)

def scaled_points(points):
    return [(round(x * scale), round(y * scale)) for x, y in points]

def rounded(box, radius, fill, outline=None, width=1):
    draw.rounded_rectangle(
        scaled_box(box), radius=round(radius * scale), fill=fill,
        outline=outline, width=round(width * scale),
    )

def polygon(points, fill, outline=None, width=1):
    draw.polygon(scaled_points(points), fill=fill)
    if outline:
        draw.line(scaled_points(points + [points[0]]), fill=outline,
                  width=round(width * scale), joint="curve")

def line(points, fill, width=1, joint="curve"):
    draw.line(scaled_points(points), fill=fill, width=round(width * scale), joint=joint)

# A folded map sheet sits on the same deep navy and teal palette as the CityGame
# launcher icon. Oversampling keeps the page silhouette clean at Finder sizes.
rounded((150, 105, 890, 950), 78, (3, 18, 25, 105))
rounded((126, 70, 866, 915), 76, (232, 235, 211, 255), (128, 216, 205, 255), 16)
polygon([(650, 72), (650, 280), (864, 280)], (177, 217, 205, 255),
        (83, 151, 153, 255), 12)
line([(650, 280), (864, 280)], (83, 151, 153, 255), 12)

# Dimetric map tile with a fine street grid, a river, and tiny city blocks.
map_points = [(496, 310), (780, 452), (496, 594), (212, 452)]
map_mask = Image.new("L", canvas.size, 0)
ImageDraw.Draw(map_mask).polygon(scaled_points(map_points), fill=255)
polygon(map_points, (181, 151, 91, 255), (45, 87, 96, 255), 14)
grid = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
grid_draw = ImageDraw.Draw(grid)
for offset in range(-160, 820, 54):
    grid_draw.line(scaled_points([(170, offset), (822, offset + 326)]),
                   fill=(103, 108, 73, 110), width=3 * scale)
    grid_draw.line(scaled_points([(170, offset + 326), (822, offset)]),
                   fill=(103, 108, 73, 110), width=3 * scale)
canvas.alpha_composite(Image.composite(grid, Image.new("RGBA", canvas.size), map_mask))
draw = ImageDraw.Draw(canvas)

# Bright water and a pale road follow the map's two axes.
details = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
draw = ImageDraw.Draw(details)
line([(250, 452), (355, 482), (442, 455), (528, 423), (612, 447), (746, 414)],
     (27, 98, 221, 255), 30)
line([(250, 452), (355, 482), (442, 455), (528, 423), (612, 447), (746, 414)],
     (78, 177, 238, 255), 14)
line([(275, 515), (390, 458), (505, 400), (642, 346), (734, 393)],
     (50, 61, 63, 255), 48)
line([(275, 515), (390, 458), (505, 400), (642, 346), (734, 393)],
     (196, 202, 192, 255), 31)
line([(275, 515), (390, 458), (505, 400), (642, 346), (734, 393)],
     (237, 220, 166, 255), 5)
canvas.alpha_composite(Image.composite(details, Image.new("RGBA", canvas.size), map_mask))
draw = ImageDraw.Draw(canvas)

def block(cx, cy, width, height, color_left, color_right, roof):
    half = width // 2
    polygon([(cx - half, cy), (cx, cy - 17), (cx, cy + height - 17),
             (cx - half, cy + height)], color_left)
    polygon([(cx, cy - 17), (cx + half, cy), (cx + half, cy + height),
             (cx, cy + height - 17)], color_right)
    polygon([(cx - half, cy), (cx, cy - 17), (cx + half, cy), (cx, cy + 17)], roof)
    for wy in range(cy + 22, cy + height - 10, 28):
        rounded((cx - half + 12, wy, cx - half + 20, wy + 12), 2,
                (255, 224, 160, 255))
        rounded((cx + 8, wy - 5, cx + 16, wy + 7), 2,
                (255, 224, 160, 255))

block(438, 390, 86, 102, (55, 112, 122, 255), (105, 183, 176, 255),
      (139, 209, 197, 255))
block(535, 365, 102, 142, (49, 101, 114, 255), (90, 164, 165, 255),
      (165, 225, 207, 255))
block(629, 408, 70, 88, (62, 123, 129, 255), (122, 194, 184, 255),
      (183, 224, 194, 255))

doc_icon = canvas.resize((1024, 1024), Image.Resampling.LANCZOS)
doc_icon.save(root / "assets/citygame-cityfile-icon.png")
doc_iconset = root / "target/citygame-cityfile.iconset"
if doc_iconset.exists():
    for path in doc_iconset.iterdir():
        path.unlink()
else:
    doc_iconset.mkdir(parents=True)
for size in (16, 32, 128, 256, 512):
    save_png(doc_icon, doc_iconset / f"icon_{size}x{size}.png", size)
    save_png(doc_icon, doc_iconset / f"icon_{size}x{size}@2x.png", size * 2)
subprocess.check_call(
    ["iconutil", "-c", "icns", str(doc_iconset), "-o", str(root / "assets/CityGameCity.icns")]
)

if args.macos_only:
    raise SystemExit(0)

print("==> Windows .ico")
opaque_fullbleed(src).save(
    root / "assets/CityGame.ico",
    format="ICO",
    sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)

print("==> Linux hicolor PNGs")
linux = root / "packaging/linux/icons/hicolor"
for size in (16, 32, 48, 64, 128, 256, 512, 1024):
    save_png(src, linux / f"{size}x{size}/apps/com.citygame.CityGame.png", size, fullbleed=True)
scalable = linux / "scalable/apps"
scalable.mkdir(parents=True, exist_ok=True)
(scalable / "com.citygame.CityGame.svg").write_bytes(
    (root / "assets/citygame-icon.svg").read_bytes()
)

print("==> Android mipmaps")
android_res = root / "packaging/android/app/src/main/res"
densities = {
    "mdpi": (48, 108),
    "hdpi": (72, 162),
    "xhdpi": (96, 216),
    "xxhdpi": (144, 324),
    "xxxhdpi": (192, 432),
}
src_opaque = opaque_fullbleed(src)
for dens, (launcher, fg) in densities.items():
    d = android_res / f"mipmap-{dens}"
    save_png(src, d / "ic_launcher.png", launcher, fullbleed=True)
    save_png(src, d / "ic_launcher_round.png", launcher, fullbleed=True)
    # Adaptive foreground: art centered on 108dp-scaled canvas (opaque so mask is clean).
    canvas = Image.new("RGB", (fg, fg), ICON_FILL[:3])
    art = src_opaque.resize((int(fg * 0.72), int(fg * 0.72)), Image.Resampling.LANCZOS)
    canvas.paste(art, ((fg - art.width) // 2, (fg - art.height) // 2))
    (d).mkdir(parents=True, exist_ok=True)
    canvas.save(d / "ic_launcher_foreground.png", format="PNG")

anydpi = android_res / "mipmap-anydpi-v26"
anydpi.mkdir(parents=True, exist_ok=True)
adaptive = """<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/ic_launcher_background"/>
    <foreground android:drawable="@mipmap/ic_launcher_foreground"/>
</adaptive-icon>
"""
(anydpi / "ic_launcher.xml").write_text(adaptive)
(anydpi / "ic_launcher_round.xml").write_text(adaptive)
values = android_res / "values"
values.mkdir(parents=True, exist_ok=True)
(values / "ic_launcher_background.xml").write_text(
    """<?xml version="1.0" encoding="utf-8"?>
<resources>
    <color name="ic_launcher_background">#203B4B</color>
</resources>
"""
)

print("==> iOS / iPadOS AppIcon.appiconset")
ios = root / "packaging/ios/CityGame/Assets.xcassets/AppIcon.appiconset"
ios.mkdir(parents=True, exist_ok=True)
# filename -> pixel size (must be opaque full-bleed; iOS fills alpha with white)
ios_files = {
    "AppIcon-1024.png": 1024,
    "icon-20.png": 20,
    "icon-20@2x.png": 40,
    "icon-20@3x.png": 60,
    "icon-29.png": 29,
    "icon-29@2x.png": 58,
    "icon-29@3x.png": 87,
    "icon-40.png": 40,
    "icon-40@2x.png": 80,
    "icon-40@3x.png": 120,
    "icon-60@2x.png": 120,
    "icon-60@3x.png": 180,
    "icon-76.png": 76,
    "icon-76@2x.png": 152,
    "icon-83.5@2x.png": 167,
    "icon-1024.png": 1024,
}
for name, px in ios_files.items():
    save_png(src, ios / name, px, fullbleed=True)

contents = {
    "images": [
        {
            "idiom": "universal",
            "platform": "ios",
            "size": "1024x1024",
            "filename": "AppIcon-1024.png",
        },
        {"idiom": "iphone", "scale": "2x", "size": "20x20", "filename": "icon-20@2x.png"},
        {"idiom": "iphone", "scale": "3x", "size": "20x20", "filename": "icon-20@3x.png"},
        {"idiom": "iphone", "scale": "2x", "size": "29x29", "filename": "icon-29@2x.png"},
        {"idiom": "iphone", "scale": "3x", "size": "29x29", "filename": "icon-29@3x.png"},
        {"idiom": "iphone", "scale": "2x", "size": "40x40", "filename": "icon-40@2x.png"},
        {"idiom": "iphone", "scale": "3x", "size": "40x40", "filename": "icon-40@3x.png"},
        {"idiom": "iphone", "scale": "2x", "size": "60x60", "filename": "icon-60@2x.png"},
        {"idiom": "iphone", "scale": "3x", "size": "60x60", "filename": "icon-60@3x.png"},
        {"idiom": "ipad", "scale": "1x", "size": "20x20", "filename": "icon-20.png"},
        {"idiom": "ipad", "scale": "2x", "size": "20x20", "filename": "icon-20@2x.png"},
        {"idiom": "ipad", "scale": "1x", "size": "29x29", "filename": "icon-29.png"},
        {"idiom": "ipad", "scale": "2x", "size": "29x29", "filename": "icon-29@2x.png"},
        {"idiom": "ipad", "scale": "1x", "size": "40x40", "filename": "icon-40.png"},
        {"idiom": "ipad", "scale": "2x", "size": "40x40", "filename": "icon-40@2x.png"},
        {"idiom": "ipad", "scale": "1x", "size": "76x76", "filename": "icon-76.png"},
        {"idiom": "ipad", "scale": "2x", "size": "76x76", "filename": "icon-76@2x.png"},
        {"idiom": "ipad", "scale": "2x", "size": "83.5x83.5", "filename": "icon-83.5@2x.png"},
        {
            "idiom": "ios-marketing",
            "scale": "1x",
            "size": "1024x1024",
            "filename": "icon-1024.png",
        },
    ],
    "info": {"author": "citygame", "version": 1},
}
(ios / "Contents.json").write_text(json.dumps(contents, indent=2) + "\n")

print("==> iOS BrandLogo imageset")
brand = root / "packaging/ios/CityGame/Assets.xcassets/BrandLogo.imageset"
brand.mkdir(parents=True, exist_ok=True)
save_png(src, brand / "BrandLogo.png", 256, fullbleed=True)
(brand / "Contents.json").write_text(
    json.dumps(
        {
            "images": [
                {"filename": "BrandLogo.png", "idiom": "universal", "scale": "1x"},
                {"idiom": "universal", "scale": "2x"},
                {"idiom": "universal", "scale": "3x"},
            ],
            "info": {"author": "citygame", "version": 1},
        },
        indent=2,
    )
    + "\n"
)

print("Done.")
PY
