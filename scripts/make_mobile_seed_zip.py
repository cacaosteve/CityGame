#!/usr/bin/env python3
"""Build a mobile SeedAssets.zip with rewritten asset-settings.json (no absolute Mac paths).

Replacement art is authored at @8x for desktop. Mobile seeds bake @4x and @2x
tiers only (no full @8x pack) so 3GB-class devices can prefer the sharper tier
while 2GB-class devices fall back to the leaner one at runtime.
"""
from __future__ import annotations

import json
import os
import re
import shutil
import sys
import zipfile
from pathlib import Path

from PIL import Image

HOME = Path.home()
DEFAULT_SUPPORT = HOME / "Library/Application Support/CityGame"
LEGACY_SUPPORT = HOME / "Library/Application Support/SC2K Rust Viewer"
REPO = Path(__file__).resolve().parents[1]
MOBILE_SEED_REVISION = "2"
DENSITY_RE = re.compile(r"@(\d+)x", re.IGNORECASE)


def source_density(relative: str) -> int:
    match = DENSITY_RE.search(relative)
    if match:
        return int(match.group(1))
    return 8


def retarget_density_name(relative: str, target_density: int) -> str:
    if DENSITY_RE.search(relative):
        return DENSITY_RE.sub(f"@{target_density}x", relative, count=1)
    stem = Path(relative)
    return str(stem.with_name(f"{stem.stem}@{target_density}x{stem.suffix}"))


def bake_replacement_tier(src_root: Path, dest_root: Path, target_density: int) -> int:
    """Downscale reviewed sprite PNGs into a tier pack. Returns sprite count."""
    manifest_path = src_root / "asset-pack.json"
    pack = json.loads(manifest_path.read_text())
    baked_assets = []
    count = 0
    for entry in pack.get("assets", []):
        out_entry = dict(entry)
        relative = entry.get("file", "")
        status = entry.get("status", "")
        key = entry.get("key", "")
        if (
            status == "reviewed"
            and key.startswith("citygame.sprite.")
            and relative.lower().endswith(".png")
        ):
            src_path = src_root / relative
            if not src_path.is_file():
                raise FileNotFoundError(src_path)
            density = source_density(relative)
            scale = target_density / density
            out_rel = retarget_density_name(relative, target_density)
            out_path = dest_root / out_rel
            out_path.parent.mkdir(parents=True, exist_ok=True)
            with Image.open(src_path) as image:
                rgba = image.convert("RGBA")
                if abs(scale - 1.0) < 1e-6:
                    rgba.save(out_path, format="PNG", optimize=True)
                else:
                    new_w = max(1, int(round(rgba.width * scale)))
                    new_h = max(1, int(round(rgba.height * scale)))
                    resized = rgba.resize((new_w, new_h), Image.Resampling.LANCZOS)
                    resized.save(out_path, format="PNG", optimize=True)
            out_entry["file"] = out_rel
            count += 1
        baked_assets.append(out_entry)

    pack["assets"] = baked_assets
    pack["id"] = f"{pack.get('id', 'our-graphics')}@{target_density}x"
    pack["name"] = f"{pack.get('name', 'Our Graphics')} (@{target_density}x mobile)"
    dest_root.mkdir(parents=True, exist_ok=True)
    (dest_root / "asset-pack.json").write_text(json.dumps(pack, indent=2) + "\n")
    # Tiny marker so runtime / docs can identify the bake density.
    (dest_root / ".citygame-graphics-density").write_text(f"{target_density}\n")
    return count


def main() -> int:
    support = Path(os.environ.get("CITYGAME_MOBILE_ASSET_PACK", DEFAULT_SUPPORT))
    if not support.exists() and LEGACY_SUPPORT.exists():
        support = LEGACY_SUPPORT
    graphics_override = os.environ.get("CITYGAME_MOBILE_GRAPHICS")
    graphics = Path(graphics_override) if graphics_override else None
    out_zip = Path(sys.argv[1]) if len(sys.argv) > 1 else REPO / "packaging/ios/CityGame/SeedAssets.zip"

    if not support.is_dir():
        print(f"missing support pack: {support}", file=sys.stderr)
        return 1

    staging = REPO / "target/mobile-seed-staging"
    if staging.exists():
        shutil.rmtree(staging)
    staging.mkdir(parents=True)

    # Prefer a self-contained original pack under SeedAssets.
    if graphics is not None and graphics.is_dir():
        shutil.copytree(graphics, staging / "original-graphics")
        original = "original-graphics"
    else:
        # Fall back to imported Win95 pack as original.
        win95 = support / "Imported Assets" / "windows-95"
        if not win95.is_dir():
            print("no graphics pack available", file=sys.stderr)
            return 1
        shutil.copytree(win95, staging / "Imported Assets" / "windows-95")
        original = "Imported Assets/windows-95"

    # Always include imported win95 for effects/music when present.
    win95 = support / "Imported Assets" / "windows-95"
    if win95.is_dir() and original != "Imported Assets/windows-95":
        shutil.copytree(win95, staging / "Imported Assets" / "windows-95")

    cities = support / "Cities"
    if cities.is_dir():
        shutil.copytree(cities, staging / "Cities")

    replacements = Path(os.environ.get("CITYGAME_REPLACEMENT_PACK", REPO / "assets/replacements/our-graphics"))
    tier4 = staging / "replacements" / "our-graphics@4x"
    tier2 = staging / "replacements" / "our-graphics@2x"
    if replacements.is_dir() and (replacements / "asset-pack.json").is_file():
        n4 = bake_replacement_tier(replacements, tier4, 4)
        n2 = bake_replacement_tier(replacements, tier2, 2)
        print(f"baked mobile replacement tiers: @4x={n4} sprites, @2x={n2} sprites")
    else:
        print("warning: no authored replacement pack; seeding originals only", file=sys.stderr)

    has_mid = (tier4 / "asset-pack.json").is_file()
    settings = {
        "schema_version": 1,
        "original_graphics": original,
        "original_effects": "Imported Assets/windows-95"
        if (staging / "Imported Assets/windows-95").is_dir()
        else None,
        "original_music": "Imported Assets/windows-95"
        if (staging / "Imported Assets/windows-95").is_dir()
        else None,
        # Default mid tier; runtime may switch to @2x / original from device memory.
        "replacement_graphics": "replacements/our-graphics@4x" if has_mid else None,
        "graphics_preference": "prefer-replacement" if has_mid else "prefer-original",
        "citygame_effects": False,
        "imported_packs": [],
        "recent_city": None,
        "tile_set_graphics": None,
        "title_music_preference": "replacement",
    }
    if (staging / "Imported Assets/windows-95").is_dir():
        settings["imported_packs"] = [
            {"format": "windows95", "root": "Imported Assets/windows-95"}
        ]
    (staging / "asset-settings.json").write_text(json.dumps(settings, indent=2) + "\n")
    # Marker so Rust knows paths are relative to the pref root.
    (staging / ".citygame-relative-settings").write_text("1\n")
    (staging / ".citygame-mobile-seed-revision").write_text(f"{MOBILE_SEED_REVISION}\n")

    out_zip.parent.mkdir(parents=True, exist_ok=True)
    if out_zip.exists():
        out_zip.unlink()
    with zipfile.ZipFile(out_zip, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for path in staging.rglob("*"):
            if path.is_file():
                zf.write(path, path.relative_to(staging).as_posix())
    print(f"wrote {out_zip} ({out_zip.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
