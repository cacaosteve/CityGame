#!/usr/bin/env python3
"""Bake the newspaper-only type atlas; not used by the game at runtime.

Usage: python3 scripts/build_newspaper_typeface.py /path/to/SourceSerif4-Regular.otf /path/to/LICENSE.md
Source: https://github.com/adobe-fonts/source-serif (SIL Open Font License 1.1).
"""
import hashlib
import json
from pathlib import Path
import sys
from PIL import Image, ImageDraw, ImageFont, PngImagePlugin

root = Path(__file__).resolve().parent.parent
out = root / 'assets/ui/newspaper'
out.mkdir(parents=True, exist_ok=True)
source, license_path = map(Path, sys.argv[1:3])
font = ImageFont.truetype(str(source), 96)
codes = sorted(set(range(32, 256)) | {0x2013, 0x2014, 0x2018, 0x2019, 0x201c, 0x201d, 0x2026})
atlas = Image.new('RGBA', (2048, 1920), (255, 255, 255, 0))
draw = ImageDraw.Draw(atlas)
glyphs = []
for i, code in enumerate(codes):
    ch = chr(code)
    left, top, right, bottom = font.getbbox(ch, anchor='ls')
    x, y = (i % 16) * 128 + 3, (i // 16) * 128 + 3
    w, h = right - left, bottom - top
    assert w <= 122 and h <= 122
    draw.text((x - left, y - top), ch, font=font, fill='white', anchor='ls')
    glyphs.append(dict(code=code, x=x, y=y, w=w, h=h, left=left, top=top,
                       advance=font.getlength(ch)))
license_text = license_path.read_text()
metadata = PngImagePlugin.PngInfo()
metadata.add_text('Title', 'CityGame newspaper serif bitmap atlas')
metadata.add_text('License', license_text)
atlas.save(out / 'serif.png', pnginfo=metadata, optimize=True)
(out / 'serif.json').write_text(json.dumps(glyphs, separators=(',', ':')) + '\n')
(out / 'OFL.txt').write_text(license_text)
(out / 'README.md').write_text(f'''# Newspaper typeface

`serif.png` is the CityGame newspaper serif bitmap atlas, baked from Adobe's
[Source Serif 4 Regular](https://github.com/adobe-fonts/source-serif) under the
[SIL Open Font License 1.1](OFL.txt). The copyright notice and license are also
embedded in the PNG metadata. The original font file is not bundled.

Source font SHA-256: `{hashlib.sha256(source.read_bytes()).hexdigest()}`.

Rebuild with `scripts/build_newspaper_typeface.py` and the upstream font and
license paths. The Rust runtime embeds the atlas and metrics; Python and system
fonts are not required to run CityGame.
''')
print(out)
