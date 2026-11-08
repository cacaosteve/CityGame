#!/usr/bin/env python3
"""Generate CityGame's clean-room, 8x toolbar atlases from vector-like primitives."""

from pathlib import Path
from PIL import Image, ImageDraw

S = 8
OUT = Path(__file__).resolve().parent

BG_TOP = (19, 37, 53)
BG_BOTTOM = (27, 54, 72)
PANEL = (24, 45, 62)
BUTTON = (40, 68, 86)
BUTTON_HI = (79, 111, 128)
BUTTON_LO = (13, 28, 41)
EDGE = (75, 111, 130)
SEPARATOR = (47, 78, 96)
INK = (12, 25, 37)
PALE = (220, 235, 237)
MUTED = (153, 181, 192)
TEAL = (72, 218, 202)
BLUE = (98, 166, 255)
GREEN = (119, 220, 144)
GOLD = (255, 205, 111)
RED = (255, 120, 115)
PURPLE = (188, 157, 255)


def make_canvas(width, height):
    im = Image.new("RGBA", (width * S, height * S), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    for y in range(height):
        f = y / max(height - 1, 1)
        color = tuple(round(BG_TOP[i] * (1 - f) + BG_BOTTOM[i] * f) for i in range(3))
        d.line((0, y * S, width * S, y * S), fill=(*color, 255), width=S)
    return im, d


def rounded(d, box, fill, outline=None, radius=1.8, width=0.8):
    b = tuple(round(v * S) for v in box)
    d.rounded_rectangle(b, radius=round(radius * S), fill=fill,
                         outline=outline, width=max(1, round(width * S)) if outline else 1)


def line(d, points, color, width=1.35):
    coords = [(round(x * S), round(y * S)) for x, y in points]
    d.line(coords, fill=color, width=max(1, round(width * S)), joint="curve")


def polygon(d, points, fill, outline=None, width=0.8):
    coords = [(round(x * S), round(y * S)) for x, y in points]
    d.polygon(coords, fill=fill)
    if outline:
        line(d, [*points, points[0]], outline, width)


def ellipse(d, box, fill, outline=None, width=0.8):
    b = tuple(round(v * S) for v in box)
    d.ellipse(b, fill=fill, outline=outline, width=max(1, round(width * S)))


def icon(d, name, rect):
    x, y, w, h = rect
    unit = min(w, h) / 20.0
    cx, cy = x + w / 2, y + h / 2

    def p(a, b):
        return (cx + (a - 10) * unit, cy + (b - 10) * unit)

    def L(points, color=PALE, width=1.5):
        line(d, [p(a, b) for a, b in points], color, width * unit)

    def P(points, fill, outline=INK, width=0.85):
        polygon(d, [p(a, b) for a, b in points], fill, outline, width * unit)

    def E(box, fill, outline=INK, width=0.8):
        a, b, c, e = box
        ellipse(d, (*p(a, b), *p(c, e)), fill, outline, width * unit)

    def R(box, fill, outline=INK, radius=0.9):
        a, b, c, e = box
        rounded(d, (p(a, b)[0], p(a, b)[1], p(c, e)[0], p(c, e)[1]),
                fill, outline, radius * unit, 0.8 * unit)

    if name in ("terrain", "coast"):
        P([(2, 15), (7, 8), (10, 12), (14, 6), (19, 15)], GREEN, INK)
        L([(2, 16), (8, 16), (12, 14), (18, 14)], GOLD, 1.15)
        if name == "coast":
            L([(3, 18), (7, 17), (11, 18), (15, 17), (18, 18)], BLUE, 1.2)
    elif name in ("terrain_edit", "raise", "lower", "level", "stretch", "sea_up", "sea_down"):
        P([(2, 16), (7, 10), (11, 14), (16, 7), (19, 16)], (95, 132, 127), INK)
        if name in ("raise", "sea_up"):
            L([(10, 15), (10, 6)], GOLD, 1.7); L([(7, 9), (10, 6), (13, 9)], GOLD, 1.7)
        elif name in ("lower", "sea_down"):
            L([(10, 6), (10, 15)], BLUE, 1.7); L([(7, 12), (10, 15), (13, 12)], BLUE, 1.7)
        elif name == "level":
            L([(4, 10), (16, 10)], TEAL, 1.6); L([(4, 13), (16, 13)], TEAL, 1.1)
        elif name == "stretch":
            L([(4, 11), (16, 11)], GOLD, 1.4); L([(4, 11), (7, 8)], GOLD); L([(4, 11), (7, 14)], GOLD)
            L([(16, 11), (13, 8)], GOLD); L([(16, 11), (13, 14)], GOLD)
    elif name in ("tree", "trees", "forest", "parks"):
        P([(10, 2), (4, 11), (7, 11), (3, 16), (17, 16), (13, 11), (16, 11)], GREEN, INK)
        L([(10, 15), (10, 19)], GOLD, 1.5)
        if name in ("forest", "parks"):
            P([(3, 7), (1, 12), (5, 12)], TEAL, INK, 0.65)
            P([(17, 8), (15, 13), (19, 13)], GREEN, INK, 0.65)
    elif name in ("water", "stream", "river"):
        P([(10, 2), (5, 9), (4, 12), (6, 16), (10, 18), (14, 16), (16, 12), (15, 9)], BLUE, INK)
        L([(7, 12), (10, 11), (13, 12)], PALE, 1.0)
        if name in ("stream", "river"):
            L([(3, 18), (6, 17), (9, 18), (12, 17), (16, 18)], TEAL, 1.1)
    elif name in ("emergency", "fire", "police"):
        P([(10, 2), (17, 5), (16, 12), (13, 17), (10, 19), (7, 17), (4, 12), (3, 5)], RED, INK)
        if name == "fire":
            P([(10, 6), (12, 11), (10, 10), (8, 15), (7, 12)], GOLD, PALE, 0.5)
        else:
            R((8, 6, 12, 15), PALE)
            R((5, 9, 15, 12), PALE)
    elif name == "power":
        P([(12, 1), (5, 11), (9, 11), (7, 19), (16, 8), (11, 8)], GOLD, INK)
        L([(12, 1), (5, 11), (9, 11), (7, 19)], PALE, 0.55)
    elif name in ("reward", "star"):
        P([(10, 1), (12.4, 7.1), (19, 7.5), (13.7, 11.8), (15.5, 19), (10, 15), (4.5, 19), (6.3, 11.8), (1, 7.5), (7.6, 7.1)], GOLD, INK)
    elif name in ("road", "infrastructure"):
        P([(3, 18), (8, 2), (12, 2), (17, 18)], (65, 82, 91), INK)
        L([(10, 4), (10, 7), (10, 10), (10, 13), (10, 16)], GOLD, 1.0)
        if name == "infrastructure":
            L([(5, 15), (15, 7)], TEAL, 0.9)
    elif name == "rail":
        L([(4, 3), (4, 17)], PALE, 1.2); L([(16, 3), (16, 17)], PALE, 1.2)
        for yy in (5, 8, 11, 14, 17): L([(4, yy), (16, yy)], GOLD, 0.9)
        L([(5, 16), (15, 4)], BLUE, 1.1)
    elif name in ("transit", "bus", "airport", "seaport"):
        R((3, 5, 17, 15), BLUE if name != "seaport" else TEAL, INK, 1.5)
        R((5, 7, 9, 11), PALE); R((11, 7, 15, 11), PALE)
        E((5, 13, 8, 16), GOLD); E((12, 13, 15, 16), GOLD)
        if name == "airport": L([(10, 2), (10, 5)], PALE, 1.2)
        if name == "seaport": L([(2, 18), (6, 17), (10, 18), (14, 17), (18, 18)], BLUE)
    elif name in ("home", "residential", "mayor"):
        P([(2, 10), (10, 3), (18, 10), (16, 11), (16, 18), (4, 18), (4, 11)], GREEN, INK)
        R((8, 12, 12, 18), (120, 76, 54)); R((5, 11, 7.5, 13.5), PALE)
        R((13, 11, 15.5, 13.5), PALE)
    elif name in ("commercial", "city_map", "neighbors"):
        R((3, 7, 8, 18), BLUE); R((9, 3, 14, 18), TEAL); R((15, 9, 19, 18), PURPLE)
        for xx, top, end in ((4, 9, 16), (10, 5, 16), (16, 11, 16)):
            for yy in range(top, end, 3): R((xx, yy, xx + 1, yy + 1), PALE, None, 0)
        if name == "city_map": L([(3, 16), (8, 12), (12, 14), (18, 6)], GOLD, 1.2)
        if name == "neighbors": L([(2, 5), (5, 3)], GOLD, 1.1)
    elif name in ("industrial", "factory", "power_plant"):
        P([(2, 17), (2, 10), (7, 13), (7, 9), (12, 12), (12, 8), (18, 11), (18, 17)], (116, 137, 143), INK)
        R((14, 3, 17, 10), RED); L([(15, 3), (15, 1)], MUTED, 1)
        for xx in (5, 9, 13, 16): R((xx, 14, xx + 1.5, 15.5), GOLD, None, 0)
    elif name in ("education", "book", "library"):
        P([(3, 4), (9, 5), (10, 7), (11, 5), (17, 4), (17, 17), (11, 18), (10, 19), (9, 18), (3, 17)], PALE, INK)
        L([(10, 7), (10, 18)], BLUE, 1.1); L([(5, 8), (8, 9)], TEAL); L([(12, 9), (15, 8)], TEAL)
    elif name in ("civic", "city_hall"):
        P([(2, 7), (10, 2), (18, 7)], GOLD, INK)
        L([(3, 8), (17, 8)], PALE, 1.0)
        for xx in (5, 8.5, 12, 15):
            L([(xx, 8), (xx, 16)], PALE, 1.35)
        L([(3, 17), (17, 17)], TEAL, 1.6); L([(2, 19), (18, 19)], GOLD, 1.1)
    elif name in ("sign",):
        L([(9, 6), (9, 19)], GOLD, 1.4); R((5, 3, 17, 10), TEAL); L([(7, 6.5), (14, 6.5)], PALE)
    elif name in ("query", "search"):
        E((3, 2, 14, 13), (93, 147, 161), PALE, 1.2); L([(12, 12), (19, 19)], GOLD, 2.0)
        L([(6, 7), (8, 5)], PALE, 0.8)
    elif name in ("graphs", "graph"):
        L([(3, 17), (3, 4)], MUTED, 1); L([(3, 17), (18, 17)], MUTED, 1)
        P([(5, 14), (8, 10), (11, 12), (16, 5), (17, 7), (11, 15), (8, 13)], TEAL, INK, 0.6)
        E((5, 13, 7, 15), GOLD, None, 0); E((15, 4, 17, 6), RED, None, 0)
    elif name in ("people", "population"):
        E((3, 3, 9, 9), GOLD); E((11, 3, 17, 9), BLUE)
        P([(2, 17), (3, 11), (9, 10), (11, 17)], TEAL, INK)
        P([(9, 17), (11, 10), (17, 11), (18, 17)], PURPLE, INK)
    elif name in ("budget", "coins"):
        for yy, fill in ((5, GOLD), (9, TEAL), (13, BLUE)):
            ellipse(d, (*p(4, yy), *p(16, yy + 4)), fill, INK, 0.8 * unit)
            L([(5, yy + 2), (15, yy + 2)], PALE, 0.7)
    elif name in ("zones", "zone"):
        for xx, yy, fill in ((3, 4, GREEN), (11, 4, BLUE), (3, 12, GOLD), (11, 12, RED)):
            R((xx, yy, xx + 6, yy + 6), fill)
    elif name in ("underground", "layers"):
        P([(2, 6), (10, 2), (18, 6), (10, 10)], BLUE, INK)
        P([(3, 10), (10, 14), (17, 10), (18, 12), (10, 17), (2, 12)], TEAL, INK)
        L([(10, 17), (10, 20)], GOLD, 1.1)
    elif name in ("help",):
        E((2, 2, 18, 18), (50, 93, 118), TEAL, 1.1)
        L([(7, 7), (8, 5), (12, 5), (14, 7), (13, 9), (10, 11)], PALE, 1.5)
        E((9.1, 14, 10.9, 15.8), GOLD, None, 0)
    elif name in ("rotate_left", "rotate_right"):
        if name == "rotate_left":
            L([(15, 8), (12, 5), (8, 5), (5, 8), (5, 12), (8, 15), (13, 15)], TEAL, 1.6)
            P([(4, 5), (4, 10), (8, 7)], GOLD, INK, 0.65)
        else:
            L([(5, 8), (8, 5), (12, 5), (15, 8), (15, 12), (12, 15), (7, 15)], TEAL, 1.6)
            P([(16, 5), (16, 10), (12, 7)], GOLD, INK, 0.65)
    elif name in ("zoom_in", "zoom_out"):
        E((3, 2, 14, 13), (47, 82, 96), PALE, 1.2); L([(12, 12), (19, 19)], GOLD, 1.9)
        L([(6, 7.5), (11, 7.5)], TEAL, 1.2)
        if name == "zoom_in": L([(8.5, 5), (8.5, 10)], TEAL, 1.2)
    elif name == "center":
        E((3, 3, 17, 17), None, TEAL, 1.0); E((7, 7, 13, 13), None, PALE, 0.8)
        L([(10, 1), (10, 6)], GOLD); L([(10, 14), (10, 19)], GOLD)
        L([(1, 10), (6, 10)], GOLD); L([(14, 10), (19, 10)], GOLD)
    elif name == "generator":
        E((3, 3, 17, 17), None, TEAL, 1.0); L([(7, 5), (12, 5), (15, 8)], PALE, 1.3)
        P([(14, 5), (18, 7), (14, 9)], GOLD, INK, 0.5)
        L([(13, 15), (8, 15), (5, 12)], PALE, 1.3); P([(6, 16), (2, 14), (6, 12)], GOLD, INK, 0.5)
    elif name == "done":
        E((2, 2, 18, 18), TEAL, PALE, 0.8); L([(5, 10), (9, 14), (15, 6)], INK, 2.0)
    else:
        # Semantic fallback: a polished, high-contrast tool marker rather than a blank tile.
        E((4, 4, 16, 16), (47, 82, 96), TEAL, 1.1)
        L([(7, 10), (10, 13), (14, 7)], PALE, 1.3)


CITY_BUTTONS = [
    ("terrain_edit", (1, 1, 23, 24)), ("tree", (24, 1, 23, 24)), ("emergency", (47, 1, 23, 24)),
    ("power", (1, 25, 23, 24)), ("water", (24, 25, 23, 24)), ("reward", (47, 25, 23, 24)),
    ("road", (1, 49, 23, 24)), ("rail", (24, 49, 23, 24)), ("transit", (47, 49, 23, 24)),
    ("residential", (1, 73, 23, 24)), ("commercial", (24, 73, 23, 24)), ("industrial", (47, 73, 23, 24)),
    ("education", (1, 97, 23, 24)), ("civic", (24, 97, 23, 24)), ("parks", (47, 97, 23, 24)),
    ("sign", (1, 121, 34, 26)), ("query", (36, 121, 33, 26)),
    ("rotate_left", (9, 153, 26, 19)), ("rotate_right", (36, 153, 26, 19)),
    ("zoom_out", (1, 177, 23, 24)), ("zoom_in", (24, 177, 23, 24)), ("center", (47, 177, 23, 24)),
    ("city_map", (1, 202, 23, 19)), ("graphs", (24, 202, 23, 19)),
    ("population", (1, 222, 23, 19)), ("factory", (24, 222, 23, 19)),
    ("neighbors", (1, 242, 23, 19)), ("budget", (24, 242, 23, 19)),
    ("home", (1, 266, 34, 14)), ("sign", (36, 266, 33, 14)),
    ("infrastructure", (1, 281, 34, 14)), ("zones", (36, 281, 33, 14)),
    ("underground", (1, 296, 44, 14)), ("help", (46, 296, 23, 14)),
]

EDIT_BUTTONS = [
    ("coast", (1, 1, 48, 16)), ("river", (1, 17, 48, 16)), ("generator", (1, 88, 48, 16)),
    ("raise", (1, 107, 24, 21)), ("lower", (25, 107, 24, 21)),
    ("stretch", (1, 128, 24, 21)), ("level", (25, 128, 24, 21)),
    ("sea_up", (1, 149, 24, 21)), ("sea_down", (25, 149, 24, 21)),
    ("water", (1, 170, 24, 21)), ("stream", (25, 170, 24, 21)),
    ("tree", (1, 191, 24, 21)), ("forest", (25, 191, 24, 21)),
    ("zoom_out", (1, 218, 24, 21)), ("zoom_in", (25, 218, 24, 21)),
    ("rotate_left", (1, 239, 24, 21)), ("rotate_right", (25, 239, 24, 21)),
    ("center", (1, 260, 24, 21)), ("help", (25, 260, 24, 21)),
]


def button(d, name, rect):
    x, y, w, h = rect
    rounded(d, (x, y, x + w, y + h), BUTTON, EDGE, 1.4, 0.65)
    line(d, [(x + 2, y + 1), (x + w - 2, y + 1)], BUTTON_HI, 0.55)
    line(d, [(x + 2, y + h - 1), (x + w - 2, y + h - 1)], BUTTON_LO, 0.55)
    margin = 2.8 if h < 18 else 3.0
    box = (x + margin, y + margin, w - margin * 2, h - margin * 2)
    icon(d, name, box)


def city_palette():
    im, d = make_canvas(71, 312)
    rounded(d, (0.5, 0.5, 70.5, 311.5), None, EDGE, 3.2, 0.7)
    for y in (120, 150, 174, 200, 265):
        line(d, [(4, y), (67, y)], SEPARATOR, 0.55)
        line(d, [(4, y + 0.8), (67, y + 0.8)], (29, 56, 74), 0.45)
    for name, rect in CITY_BUTTONS:
        button(d, name, rect)
    # Distinctive section markers add visual grouping without imitating a legacy atlas.
    for y, color in ((119, TEAL), (199, BLUE), (264, GOLD)):
        rounded(d, (3, y - 0.9, 5.2, y + 1.5), color, None, 0.8, 0)
    return im


def edit_palette():
    im, d = make_canvas(50, 306)
    rounded(d, (0.5, 0.5, 49.5, 305.5), None, EDGE, 2.8, 0.7)
    for y in (34, 85, 106, 213, 240, 261, 286):
        line(d, [(3, y), (47, y)], SEPARATOR, 0.55)
    # Three high-density terrain sliders; the app paints the live handles above them.
    for x in (1, 17, 33):
        rounded(d, (x + 2, 35, x + 13, 82), (18, 37, 53), (66, 100, 115), 1.8, 0.55)
        for yy in range(39, 80, 4):
            line(d, [(x + 4, yy), (x + 6, yy)], MUTED if yy % 8 else TEAL, 0.6)
    for name, rect in EDIT_BUTTONS:
        button(d, name, rect)
    button(d, "done", (1, 287, 48, 16))
    return im


def classic_palette(image):
    """Recolor the toolbar surfaces to a neutral classic gray while keeping its icons."""
    logical_height = image.height // S
    colors = {
        BUTTON: (160, 160, 160),
        BUTTON_HI: (226, 226, 226),
        BUTTON_LO: (76, 76, 76),
        EDGE: (56, 56, 56),
        SEPARATOR: (104, 104, 104),
        (29, 56, 74): (118, 118, 118),
        (18, 37, 53): (160, 160, 160),
        (66, 100, 115): (192, 192, 192),
    }
    backgrounds = []
    for y in range(logical_height):
        factor = y / max(logical_height - 1, 1)
        background = tuple(
            round(BG_TOP[channel] * (1 - factor) + BG_BOTTOM[channel] * factor)
            for channel in range(3)
        )
        backgrounds.append(background)
        gray = round(sum(background) / 3)
        colors[background] = (gray, gray, gray)
    image = image.copy().convert("RGBA")
    pixels = image.get_flattened_data()
    width = image.width
    recolored = []
    for index, pixel in enumerate(pixels):
        rgb = pixel[:3]
        background = backgrounds[(index // width) // S]
        target = colors[background] if rgb == background else colors.get(rgb, rgb)
        recolored.append((*target, pixel[3]))
    image.putdata(recolored)
    return image


if __name__ == "__main__":
    city = city_palette()
    edit = edit_palette()
    city.save(OUT / "city.png", optimize=True)
    edit.save(OUT / "edit-map.png", optimize=True)
    classic_palette(city).save(OUT / "city-classic.png", optimize=True)
    classic_palette(edit).save(OUT / "edit-map-classic.png", optimize=True)
