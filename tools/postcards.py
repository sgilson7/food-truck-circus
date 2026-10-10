#!/usr/bin/env python3
"""Postcards of each stop on the tour, drawn as TikZ and compiled, not
generated as pixels (Sam's diagrams-as-code skill: "ask for a text file, not
an image"; the prompt in advanced-prompt-engineering-prompts.md §8).

Each postcard is composed from sub-pictures defined once below (the truck,
the bed, the performer, buildings, trees, clouds, mountains ...) and placed
from the venue's own `props` in data/venues.json, so a postcard shows the
same landmarks the 3D stage does. The look follows Sam_Comm_final.png in
the vagrancy repository: a warm yellow sky with outlined clouds, salmon,
coral and pink buildings, sage and teal greens, everything outlined in one
plum ink, flat fills, and rooftop clutter (air-conditioning boxes, antennas,
tanks).

    python3 tools/postcards.py            # writes web/art/venue/<id>.png and tools/postcards/<id>.tex
    python3 tools/postcards.py rtp        # one venue
    python3 tools/postcards.py --fr       # the French cards, into .../venue/fr/

Facts each postcard must get right (the skill's list of what an image
generator gets wrong), and where this file enforces each:

- text: only the venue's name, city and date from venues.json, and the copy
  string `postcard_tag`; nothing typed here (`banner`). Special characters
  are escaped (`tex`).
- what is in it: the truck, the bed and the performer in every card
  (`postcard`, always drawn); the landmarks exactly the venue's props
  (`PROP_DRAWERS`), and nothing a venue does not list.
- placement: one projection (`project`) for every prop, from the same metres
  the 3D stage uses; far things drawn first (sorted by depth).
- sizes: one camera, so a 30 m building is always larger than a 10 m one at
  the same distance; the truck is the 7.2 m truck of balance.rs.
- colour is never the only cue: every shape has the plum outline.
"""
import json, math, random, re, shutil, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
# `--fr` draws the French cards: the tag from data/fr/copy.json, and each
# venue's French name and date from data/fr/venues.json laid over the English
FRENCH = "--fr" in sys.argv
DATA = json.loads((ROOT / "data" / "venues.json").read_text())
COPY = json.loads((ROOT / "data" / ("fr/copy.json" if FRENCH else "copy.json")).read_text())
if FRENCH:
    words = json.loads((ROOT / "data" / "fr" / "venues.json").read_text())["venues"]
    for v in DATA["venues"]:
        v.update(words.get(v["id"], {}))
OUT = ROOT / "web" / "art" / "venue" / ("fr" if FRENCH else "")
# the TikZ each postcard compiles from, kept beside this script
SRC = ROOT / "tools" / "postcards" / ("fr" if FRENCH else "")

# The canvas, in cm, and the camera: looking toward the truck from the lawn.
W, H = 16.0, 9.0
# from a little above the crowd, so the bed shows in front of the truck
CAM_Z, CAM_Y, FOCAL, HORIZON = 16.0, 6.0, 9.0, 5.2

# Sam_Comm_final's palette, by eye from the picture: the ink, the sky, the
# clouds, the warm walls and the greens.
INK = "3B2340"
SKY = "EEC86A"
SKY_HI = "F3D98C"
CLOUD = "F2D58A"
WALLS = ["E88A73", "EE9F7E", "E47A7E", "F0B07E", "D9765F"]
GREENS = ["8DBB7E", "6FA77A", "A9C97F", "5E9C6A"]
WINDOW = "F6E7A0"


def mix(hex_a, hex_b, t):
    a = [int(hex_a[i:i + 2], 16) for i in (0, 2, 4)]
    b = [int(hex_b[i:i + 2], 16) for i in (0, 2, 4)]
    return "".join(f"{round(x + (y - x) * t):02X}" for x, y in zip(a, b))


def warm(hex_colour, t=0.35):
    """A venue's own colour pulled toward the postcard's warm light."""
    return mix(hex_colour.lstrip("#").upper(), "F2C27A", t)


def tex(s):
    s = s.replace("\\", "").replace("&", "\\&").replace("%", "\\%").replace("#", "\\#")
    s = s.replace("·", "\\textperiodcentered{}").replace("–", "--").replace("’", "'")
    return s


def project(x, y, z):
    """World metres to canvas cm; None behind the camera."""
    d = CAM_Z - z
    if d <= 0.5:
        return None
    return (W / 2 + FOCAL * x / d, HORIZON + FOCAL * (y - CAM_Y) / d, d)


class Card:
    def __init__(self, seed):
        self.items = []  # (depth, tikz)
        self.rng = random.Random(seed)
        self.colours = {}

    def colour(self, hex_colour):
        name = "c" + hex_colour.lower()
        self.colours[name] = hex_colour
        return name

    def add(self, depth, code):
        self.items.append((depth, code))

    def poly(self, pts, fill, lw=0.9):
        f = self.colour(fill)
        path = " -- ".join(f"({x:.3f},{y:.3f})" for x, y in pts)
        return f"\\filldraw[fill={f}, draw=ink, line width={lw}pt, line join=round] {path} -- cycle;"

    def rect(self, x0, y0, x1, y1, fill, lw=0.9):
        return self.poly([(x0, y0), (x1, y0), (x1, y1), (x0, y1)], fill, lw)

    def circle(self, x, y, r, fill, lw=0.9):
        return f"\\filldraw[fill={self.colour(fill)}, draw=ink, line width={lw}pt] ({x:.3f},{y:.3f}) circle ({r:.3f});"

    def line(self, pts, lw=0.8):
        path = " -- ".join(f"({x:.3f},{y:.3f})" for x, y in pts)
        return f"\\draw[draw=ink, line width={lw}pt, line cap=round] {path};"


# ---- sub-pictures ------------------------------------------------------------------

def building(card, x, z, w, h, d, wall, windows=True, roof="flat", depth_bias=0.0):
    """A block of w by h metres with its face toward the camera at z + d/2."""
    face = z + d / 2
    p0, p1 = project(x - w / 2, 0, face), project(x + w / 2, h, face)
    if not p0 or not p1:
        return
    x0, y0, dd = p0
    x1, y1, _ = p1
    s = FOCAL / dd
    out = [card.rect(x0, y0, x1, y1, wall)]
    if windows:
        cols = max(1, int(w / 3.2))
        rows = max(1, int(h / 3.4))
        cw, rh = (x1 - x0) / cols, (y1 - y0) / rows
        for i in range(cols):
            for j in range(rows):
                if card.rng.random() < 0.12:
                    continue
                wx, wy = x0 + i * cw + cw * 0.22, y0 + j * rh + rh * 0.3
                lit = WINDOW if card.rng.random() < 0.7 else mix(wall, "3B2340", 0.25)
                out.append(card.rect(wx, wy, wx + cw * 0.56, wy + rh * 0.45, lit, 0.5))
    if roof == "gable":
        out.append(card.poly([(x0 - 0.1 * s, y1), (x1 + 0.1 * s, y1), ((x0 + x1) / 2, y1 + 3 * s)], mix(wall, INK, 0.3)))
    elif roof == "saw":
        teeth = max(2, int(w / 6))
        tw = (x1 - x0) / teeth
        for k in range(teeth):
            a = x0 + k * tw
            out.append(card.poly([(a, y1), (a + tw, y1), (a + tw, y1 + 1.6 * s)], mix(wall, INK, 0.25)))
    else:
        # rooftop clutter: air-conditioning boxes, an antenna, a tank
        n = max(1, int(w / 9))
        for _ in range(n):
            bx = card.rng.uniform(x0 + 0.1, max(x0 + 0.11, x1 - 0.6 * s))
            out.append(card.rect(bx, y1, bx + 1.4 * s, y1 + 0.9 * s, card.rng.choice(GREENS), 0.6))
        ax = card.rng.uniform(x0, x1)
        out.append(card.line([(ax, y1), (ax, y1 + 2.4 * s)], 0.6))
        out.append(card.line([(ax - 0.6 * s, y1 + 1.8 * s), (ax + 0.6 * s, y1 + 1.8 * s)], 0.5))
        out.append(card.line([(ax - 0.4 * s, y1 + 2.2 * s), (ax + 0.4 * s, y1 + 2.2 * s)], 0.5))
    card.add(dd + depth_bias, "\n".join(out))


def tree(card, kind, x, z, h, crown=None):
    base = project(x, 0, z)
    if not base:
        return
    bx, by, dd = base
    s = FOCAL / dd
    th = h * s
    green = warm(crown, 0.25) if crown else card.rng.choice(GREENS)
    bark = "7A5A4A"
    out = []
    if kind == "pine":
        out.append(card.rect(bx - 0.06 * th, by, bx + 0.06 * th, by + 0.3 * th, bark, 0.5))
        for k in range(3):
            y = by + 0.2 * th + k * 0.22 * th
            w = (0.42 - k * 0.1) * th
            out.append(card.poly([(bx - w, y), (bx + w, y), (bx, y + 0.45 * th)], mix(green, "2F5A3A", 0.35 + k * 0.05), 0.6))
    elif kind == "palm":
        top = (bx + 0.08 * th, by + th)
        out.append(card.poly([(bx - 0.03 * th, by), (bx + 0.03 * th, by), (top[0] + 0.02 * th, top[1]), (top[0] - 0.02 * th, top[1])], "B08A5A", 0.5))
        for a in range(0, 360, 50):
            r = math.radians(a)
            tip = (top[0] + math.cos(r) * 0.42 * th, top[1] + math.sin(r) * 0.18 * th - 0.08 * th)
            mid = ((top[0] + tip[0]) / 2, (top[1] + tip[1]) / 2 + 0.06 * th)
            out.append(card.poly([top, mid, tip, (mid[0], mid[1] - 0.05 * th)], green, 0.5))
    elif kind == "saguaro":
        g = "6FA77A"
        w = 0.08 * th
        out.append(card.rect(bx - w, by, bx + w, by + th, g, 0.6))
        out.append(card.circle(bx, by + th, w, g, 0.6))
        for side in (-1, 1):
            if card.rng.random() < 0.8:
                ay = by + th * card.rng.uniform(0.35, 0.55)
                ax = bx + side * 0.28 * th
                out.append(card.rect(min(bx, ax), ay - w * 0.8, max(bx, ax), ay + w * 0.8, g, 0.6))
                out.append(card.rect(ax - w * 0.8, ay, ax + w * 0.8, ay + 0.3 * th, g, 0.6))
                out.append(card.circle(ax, ay + 0.3 * th, w * 0.8, g, 0.6))
    else:
        # oak, plane, banyan, paloverde: a trunk and a cloud of crown
        trunk_h = 0.45 * th
        out.append(card.rect(bx - 0.05 * th, by, bx + 0.05 * th, by + trunk_h, bark, 0.5))
        wide = 1.6 if kind == "banyan" else 1.0
        if kind == "paloverde":
            green = "B5CC6A"
        for k in range(5):
            cx = bx + card.rng.uniform(-0.25, 0.25) * th * wide
            cy = by + trunk_h + card.rng.uniform(0.05, 0.35) * th
            out.append(card.circle(cx, cy, card.rng.uniform(0.17, 0.26) * th * wide ** 0.5, green, 0.6))
    card.add(dd, "\n".join(out))


def cloud(card, x, y, size):
    out = []
    for k in range(5):
        cx = x + (k - 2) * size * 0.55 + card.rng.uniform(-0.1, 0.1) * size
        cy = y + card.rng.uniform(0, 0.35) * size + (0.3 * size if k in (1, 2, 3) else 0)
        out.append(card.circle(cx, cy, size * card.rng.uniform(0.45, 0.62), CLOUD, 0.7))
    card.add(100000 + card.rng.random(), "\n".join(out))


def mountain(card, x, z, h, w, colour, snow):
    base_l, base_r, peak = project(x - w / 2, 0, z), project(x + w / 2, 0, z), project(x, h, z)
    if not (base_l and base_r and peak):
        return
    out = [card.poly([(base_l[0], base_l[1]), (base_r[0], base_r[1]), (peak[0], peak[1])], warm(colour, 0.45), 0.8)]
    if snow:
        f = 0.3
        sl = (peak[0] + (base_l[0] - peak[0]) * f, peak[1] + (base_l[1] - peak[1]) * f)
        sr = (peak[0] + (base_r[0] - peak[0]) * f, peak[1] + (base_r[1] - peak[1]) * f)
        out.append(card.poly([sl, (peak[0], peak[1]), sr, ((sl[0] + sr[0]) / 2, sl[1] + 0.08)], "FFF6E0", 0.7))
    card.add(peak[2], "\n".join(out))


def ground_band(card, z0, z1, colour, depth):
    a, b = project(0, 0, z0), project(0, 0, z1)
    if not (a and b):
        return
    card.add(depth, card.rect(-0.2, b[1], W + 0.2, a[1], colour, 0.7))


def truck(card):
    # the 7.2 m food truck of balance.rs, its side toward the bed
    x0, x1, z = -3.6, 3.6, -2.5
    p0, p1 = project(x0, 0.6, z), project(x1, 3.2, z)
    s = FOCAL / p0[2]
    out = [card.rect(p0[0], p0[1], p1[0], p1[1], "F4C430", 1.1)]
    out.append(card.rect(p0[0], p0[1] + 0.35 * s, p1[0], p0[1] + 0.65 * s, "2A9D8F", 0.7))
    win0, win1 = project(-2.3, 1.4, z), project(1.1, 2.4, z)
    out.append(card.rect(win0[0], win0[1], win1[0], win1[1], "FFB35C", 0.9))
    # the popcorn machine in the window
    pm0, pm1 = project(0.5, 1.45, z), project(1.0, 2.15, z)
    out.append(card.rect(pm0[0], pm0[1], pm1[0], pm1[1], "D62828", 0.6))
    out.append(card.rect(pm0[0] + 0.05 * s, pm0[1] + 0.25 * s, pm1[0] - 0.05 * s, pm1[1] - 0.05 * s, "FFE08A", 0.5))
    for k in range(8):
        a = win0[0] + (win1[0] - win0[0]) * k / 8
        b = win0[0] + (win1[0] - win0[0]) * (k + 1) / 8
        col = "E63946" if k % 2 == 0 else "FFF4E0"
        out.append(card.poly([(a, win1[1] + 0.05), (b, win1[1] + 0.05), (b + 0.05, win1[1] + 0.5 * s), (a + 0.05, win1[1] + 0.5 * s)], col, 0.6))
    # menu board, windscreen, wheels
    mb0, mb1 = project(1.4, 1.5, z), project(2.6, 2.5, z)
    out.append(card.rect(mb0[0], mb0[1], mb1[0], mb1[1], "2B2D42", 0.7))
    for k, c in enumerate(["FFD166", "06D6A0", "EF476F"]):
        y = mb1[1] - (k + 1) * (mb1[1] - mb0[1]) / 4
        out.append(card.line([(mb0[0] + 0.1, y), (mb1[0] - 0.1, y)], 1.2).replace("draw=ink", f"draw={card.colour(c)}"))
    out.append(card.rect(p1[0] - 0.25 * s, p0[1] + 1.3 * s, p1[0], p1[1] - 0.3 * s, "9FC5D8", 0.7))
    for wx in (-2.3, 2.3):
        w = project(wx, 0.45, z)
        out.append(card.circle(w[0], w[1], 0.45 * s, "2B2B33", 0.9))
        out.append(card.circle(w[0], w[1], 0.18 * s, "9CA3AF", 0.6))
    # bulbs along the roof
    for k in range(13):
        b = project(x0 + 7.2 * k / 12, 3.25, z)
        out.append(card.circle(b[0], b[1], 0.06, ["FFD166", "EF476F", "06D6A0"][k % 3], 0.4))
    card.add(p0[2], "\n".join(out))


def bed_and_performer(card):
    z = 0.4
    a, b = project(-2.3, 1.0, z + 1.1), project(2.3, 1.0, z + 1.1)
    c, d = project(-2.3, 1.0, z - 1.1), project(2.3, 1.0, z - 1.1)
    s = FOCAL / a[2]
    out = []
    for lx in (a, b):
        out.append(card.line([(lx[0], lx[1]), (lx[0], lx[1] - 1.0 * s)], 1.2))
    out.append(card.poly([(a[0] - 0.15, a[1]), (b[0] + 0.15, b[1]), (d[0] + 0.1, d[1]), (c[0] - 0.1, c[1])], "1D4ED8", 1.0))
    out.append(card.poly([(a[0] + 0.12, a[1] + 0.04), (b[0] - 0.12, b[1] + 0.04), (d[0] - 0.08, d[1] - 0.03), (c[0] + 0.08, c[1] - 0.03)], "2A2D36", 0.6))
    card.add(a[2] - 0.01, "\n".join(out))
    # the performer, tucked at the top of a back flip, a dashed arc behind
    top = project(0.2, 5.2, z)
    # drawn at twice its size: the postcard's subject
    s = 2 * FOCAL / top[2]
    px, py = top[0], top[1]
    arc = [project(-1.6 + 3.6 * t, 1.6 + 3.6 * math.sin(math.pi * t), z) for t in [i / 20 for i in range(21)]]
    path = " -- ".join(f"({p[0]:.3f},{p[1]:.3f})" for p in arc)
    perf = [f"\\draw[draw=ink, line width=0.7pt, dash pattern=on 2pt off 2pt] {path};"]
    # a performer in a tuck at the top of a back flip: a sub-picture in its
    # own frame (metres), turned and scaled into place
    for c in ("F5F5F0", "2B2B2B", "C98F6B", "D33F2F", "FFFFFF"):
        card.colour(c)
    perf.append(
        f"\\begin{{scope}}[shift={{({px:.3f},{py:.3f})}}, rotate=-35, scale={s:.3f}, line join=round]\n"
        "\\filldraw[fill=c2b2b2b, draw=ink, line width=0.9pt] (-0.12,-0.28) -- (0.42,0.02) -- (0.5,-0.08) -- (0.02,-0.4) -- cycle;\n"
        "\\filldraw[fill=c2b2b2b, draw=ink, line width=0.9pt] (0.42,0.02) -- (0.5,-0.08) -- (0.36,-0.46) -- (0.26,-0.4) -- cycle;\n"
        "\\filldraw[fill=cf5f5f0, draw=ink, line width=0.9pt, rounded corners=1.5pt] (-0.24,-0.32) rectangle (0.16,0.34);\n"
        "\\filldraw[fill=cd33f2f, draw=ink, line width=0.6pt] (-0.24,-0.3) rectangle (0.16,-0.22);\n"
        "\\filldraw[fill=cf5f5f0, draw=ink, line width=0.8pt] (0.02,0.26) -- (0.36,-0.04) -- (0.3,-0.12) -- (-0.04,0.16) -- cycle;\n"
        "\\filldraw[fill=cc98f6b, draw=ink, line width=0.8pt] (0.34,-0.08) circle (0.07);\n"
        "\\filldraw[fill=cc98f6b, draw=ink, line width=0.9pt] (0.02,0.54) circle (0.17);\n"
        "\\filldraw[fill=cffffff, draw=ink, line width=0.8pt] (-0.12,0.66) rectangle (0.16,0.78);\n"
        "\\filldraw[fill=cffffff, draw=ink, line width=0.8pt] (0.02,0.86) circle (0.13);\n"
        "\\end{scope}"
    )
    # motion lines behind
    for k in range(3):
        sx = px - (0.9 + 0.25 * k) * s
        perf.append(card.line([(sx, py + (k - 1) * 0.25 * s), (sx - 0.4 * s, py + (k - 1) * 0.3 * s)], 0.7))
    card.add(0.1, "\n".join(perf))


def crowd(card, colours):
    out = []
    rng = card.rng
    n = 26
    for k in range(n):
        x = (k + rng.uniform(-0.3, 0.3)) * W / (n - 1)
        y = rng.uniform(-0.15, 0.25)
        shirt = warm(rng.choice(colours), 0.25)
        skin = rng.choice(["F1C27D", "E0AC69", "C68642", "8D5524", "FFDBAC", "A1665E"])
        out.append(card.poly([(x - 0.42, y - 0.4), (x + 0.42, y - 0.4), (x + 0.32, y + 0.45), (x - 0.32, y + 0.45)], shirt, 0.8))
        out.append(card.circle(x, y + 0.75, 0.3, skin, 0.8))
        if rng.random() < 0.45:
            out.append(card.line([(x - 0.3, y + 0.3), (x - 0.55, y + 1.25)], 1.6))
            out.append(card.line([(x + 0.3, y + 0.3), (x + 0.55, y + 1.25)], 1.6))
    card.add(0.0, "\n".join(out))


def banner(card, venue):
    tag = tex(COPY["postcard_tag"])
    name, place = tex(venue["name"]), tex(f'{venue["city"]} · {venue["when"]}')
    return (
        f"\\node[anchor=north west, fill=cfff8ec, draw=ink, line width=1.2pt, rounded corners=4pt, "
        f"inner sep=5pt, text width=8.4cm, font=\\sffamily] at (0.35,8.7) "
        f"{{{{\\scriptsize\\bfseries\\color{{cd62828}}{tag}}}\\\\[1pt]"
        f"{{\\large\\bfseries {name}}}\\\\[1pt]{{\\footnotesize {place}}}}};"
    )


# ---- props, as the 3D stage lists them ----------------------------------------------

def draw_prop(card, p):
    rng = card.rng
    kind = p["kind"]
    at = p.get("at", [0, 0])
    col = p.get("colour", "#e88a73")
    if kind == "building":
        w, h, d = p["size"]
        building(card, at[0], at[1], w, h, d, warm(col), True, p.get("roof", "flat"))
    elif kind in ("gothic", "temple"):
        w, h, d = p.get("size", [24, 10, 12])
        building(card, at[0], at[1], w, h, d, warm(col), True, "flat")
        for k in (-1, 0, 1):
            tx = at[0] + k * (w / 2 - 2)
            th = h + (9 if k == 0 else 5)
            building(card, tx, at[1] + d / 2, 4, th, 0.5, warm(col, 0.25), False, "flat", -0.01)
            top = project(tx, th, at[1] + d / 2 + 0.25)
            if top:
                s = FOCAL / top[2]
                card.add(top[2] - 0.02, card.poly([(top[0] - 2 * s, top[1]), (top[0] + 2 * s, top[1]), (top[0], top[1] + 4 * s)], "6B7280", 0.8))
    elif kind == "mural":
        w, h, d = p["size"]
        building(card, at[0], at[1], w, h, d, "D8D0C2", False)
        face = at[1] + d / 2 + 0.05
        cols = p.get("colours", []) or ["#ef4444"]
        n = len(cols) * 2
        for k in range(n):
            x = at[0] - w / 2 + w * (k + 0.5) / n
            top = h * rng.uniform(0.5, 0.95)
            a, b = project(x - w / n / 2, 0, face), project(x + w / n / 2, top, face)
            card.add(a[2] - 0.01, card.rect(a[0], a[1], b[0], b[1], warm(cols[k % len(cols)], 0.2), 0.6))
    elif kind == "containers":
        for k in range(p.get("count", 6)):
            row, level = k % 5, k // 5
            x = at[0] + row * 3 - 6
            building(card, x, at[1] + level, 2.5, 2.6, 6, warm(rng.choice(p["colours"])), False, "none", -level * 0.01)
    elif kind == "street":
        n = p.get("count", 5)
        for k in range(n):
            x = at[0] + (k - n // 2) * 9
            building(card, x, at[1], 9, rng.uniform(7, 13), 10, warm(rng.choice(p["colours"])), True)
    elif kind == "victorians":
        n = p.get("count", 8)
        for k in range(n):
            x = at[0] + (k - n // 2) * 7
            building(card, x, at[1] + rng.choice([0, 4, 8]), 6, rng.uniform(9, 12), 10, warm(p["colours"][k % len(p["colours"])], 0.2), True, "gable")
    elif kind == "deco":
        n = p.get("count", 5)
        for k in range(n):
            x = at[0] + (k - n // 2) * 13
            building(card, x, at[1], 11, rng.uniform(10, 17), 10, warm(p["colours"][k % len(p["colours"])], 0.15), True, "flat")
    elif kind == "adobe":
        n = p.get("count", 4)
        for k in range(n):
            building(card, at[0] + (k - n // 2) * 11, at[1], 10, rng.uniform(4, 7), 9, warm(rng.choice(p["colours"])), True, "none")
    elif kind == "arches":
        n = p.get("count", 5)
        w = p.get("width", 5)
        h = p["height"] if isinstance(p["height"], (int, float)) else p["height"][0]
        total = w * n + 2 * (n + 1)
        building(card, at[0], at[1], total, h + 4, 14, warm(col, 0.2), False)
        for k in range(n):
            x = at[0] - total / 2 + 2 + w / 2 + k * (w + 2)
            a, b = project(x - w / 2, 0, at[1] + 7.05), project(x + w / 2, h, at[1] + 7.05)
            card.add(a[2] - 0.01, card.rect(a[0], a[1], b[0], b[1], "F3C46B", 0.7))
    elif kind == "mansion":
        building(card, at[0], at[1], 40, 12, 16, warm(col, 0.15), True)
        building(card, at[0], at[1] + 2, 14, 20, 12, warm(col, 0.1), True, "flat", -0.01)
        building(card, at[0], at[1] + 3, 7, 28, 7, warm(col, 0.05), False, "gable", -0.02)
    elif kind == "barn":
        building(card, at[0], at[1], 14, 8, 12, warm(col, 0.15), False, "gable")
        a, b = project(at[0] - 3, 0, at[1] + 6.05), project(at[0] + 3, 6, at[1] + 6.05)
        card.add(a[2] - 0.01, "\n".join([card.rect(a[0], a[1], b[0], b[1], warm(col, 0.3), 0.7),
                                           card.line([(a[0], a[1]), (b[0], b[1])], 1.2), card.line([(a[0], b[1]), (b[0], a[1])], 1.2)]))
    elif kind in ("silo", "stack", "water_tower"):
        h = p["height"] if isinstance(p["height"], (int, float)) else p["height"][0]
        w = {"silo": 5, "stack": 3, "water_tower": 7}[kind]
        if kind == "water_tower":
            building(card, at[0], at[1], w, 6, 1, warm(col), False, "gable")
            legs0 = project(at[0] - 3, 0, at[1])
            legs1 = project(at[0] + 3, h - 6, at[1])
            if legs0 and legs1:
                card.add(legs0[2] + 0.01, "\n".join([card.line([(legs0[0], legs0[1]), (legs0[0] + 0.1, legs1[1])], 0.9),
                                                     card.line([(legs1[0], legs0[1]), (legs1[0] - 0.1, legs1[1])], 0.9)]))
            top = project(at[0] - w / 2, h - 6, at[1])
            if top:
                s = FOCAL / top[2]
                card.add(top[2] - 0.01, card.rect(top[0], top[1], top[0] + w * s, top[1] + 5 * s, warm(col), 0.9))
        else:
            building(card, at[0], at[1], w, h, 1, warm(col, 0.2), False, "flat")
    elif kind == "mountains":
        for k in range(p.get("count", 6)):
            a = math.radians(-60 + 120 * k / max(1, p["count"] - 1) + rng.uniform(-5, 5))
            dist = p.get("distance", 300) + rng.uniform(0, 120)
            h = (p["height"] if isinstance(p["height"], (int, float)) else p["height"][0]) * rng.uniform(0.6, 1.1)
            mountain(card, math.sin(a) * dist, -math.cos(a) * dist, h, h * rng.uniform(2.0, 3.2), col.lstrip("#"), p.get("snow", False))
    elif kind == "peak":
        h = p["height"] if isinstance(p["height"], (int, float)) else p["height"][0]
        mountain(card, at[0], at[1], h, h * 3, col.lstrip("#"), True)
    elif kind == "hills":
        for k in range(p.get("count", 5)):
            a = math.radians(-70 + 140 * k / max(1, p["count"] - 1))
            dist = p.get("distance", 200) + rng.uniform(0, 80)
            x, z = math.sin(a) * dist, -math.cos(a) * dist
            l, r, top = project(x - 90, 0, z), project(x + 90, 0, z), project(x, rng.uniform(25, 45), z)
            if l and r and top:
                path = f"({l[0]:.3f},{l[1]:.3f}) .. controls ({l[0]:.3f},{top[1]:.3f}) and ({r[0]:.3f},{top[1]:.3f}) .. ({r[0]:.3f},{r[1]:.3f}) -- cycle"
                card.add(top[2], f"\\filldraw[fill={card.colour(warm(col, 0.3))}, draw=ink, line width=0.8pt] {path};")
    elif kind == "skyline":
        for k in range(p.get("count", 12)):
            a = math.radians(-55 + 110 * k / max(1, p["count"]) + rng.uniform(-3, 3))
            dist = p.get("distance", 100) + rng.uniform(0, 90)
            building(card, math.sin(a) * dist, -math.cos(a) * dist, rng.uniform(14, 30), rng.uniform(40, 140), 20,
                     warm(rng.choice(p["colours"]), 0.3), True)
    elif kind == "water":
        a = p["area"]
        ground_band(card, a[3], a[1], warm(col, 0.25), project(0, 0, a[1])[2] if project(0, 0, a[1]) else 1e5)
    elif kind == "trees":
        hmin, hmax = (p["height"] if isinstance(p["height"], list) else [p["height"], p["height"]])
        a = p["area"]
        for _ in range(p.get("count", 10)):
            x, z = rng.uniform(a[0], a[2]), rng.uniform(a[1], a[3])
            if abs(x) < 18 and -9 < z < 20:
                continue
            crown = rng.choice(p["colours"]) if p.get("colours") else None
            tree(card, p.get("tree", "oak"), x, z, rng.uniform(hmin, hmax), crown)
    elif kind == "tents":
        a = p["area"]
        for _ in range(p.get("count", 4)):
            x, z = rng.uniform(a[0], a[2]), rng.uniform(a[1], a[3])
            base_l, base_r, top = project(x - 2, 2.2, z), project(x + 2, 2.2, z), project(x, 3.6, z)
            g0 = project(x - 2, 0, z)
            if base_l and top and g0:
                card.add(top[2], "\n".join([card.line([(base_l[0], g0[1]), (base_l[0], base_l[1])], 0.6), card.line([(base_r[0], g0[1]), (base_r[0], base_r[1])], 0.6),
                                            card.poly([(base_l[0], base_l[1]), (base_r[0], base_r[1]), (top[0], top[1])], warm(rng.choice(p["colours"]), 0.1), 0.8)]))
    elif kind == "lights":
        f, t = p["from"], p["to"]
        pts = []
        for k in range(29):
            u = k / 28
            sag = 4 * (u * 2 % 1) * (1 - (u * 2 % 1)) * 1.5
            q = project(f[0] + (t[0] - f[0]) * u, 4.8 - sag, f[1])
            if q:
                pts.append(q)
        if pts:
            code = [card.line([(q[0], q[1]) for q in pts], 0.5)]
            for k, q in enumerate(pts):
                code.append(card.circle(q[0], q[1] - 0.05, 0.05, warm(p["colours"][k % len(p["colours"])], 0.0), 0.3))
            card.add(pts[0][2] - 0.5, "\n".join(code))
    elif kind == "playground":
        building(card, at[0], at[1], 1.6, 2, 1.6, "EF4444", False, "none")
    elif kind == "flag":
        b, t = project(at[0], 0, at[1]), project(at[0], 3, at[1])
        if b and t:
            s = FOCAL / b[2]
            card.add(b[2], "\n".join([card.line([(b[0], b[1]), (t[0], t[1])], 0.7),
                                      card.poly([(t[0], t[1]), (t[0] + 0.8 * s, t[1] - 0.25 * s), (t[0], t[1] - 0.5 * s)], "E11D48", 0.6)]))
    # lot, lamps, fence, piles, bales: ground clutter, left to the 3D stage


def postcard(venue):
    card = Card(venue["seed"])
    # sky: two warm bands; clouds; the ground the venue stands on
    sky = f"\\fill[c{SKY.lower()}] (0,0) rectangle ({W},{H});\n\\fill[c{SKY_HI.lower()}] (0,{HORIZON}) rectangle ({W},{HORIZON + 1.4});"
    card.colour(SKY)
    card.colour(SKY_HI)
    for k in range(5):
        cloud(card, card.rng.uniform(1, W - 1), card.rng.uniform(7.0, 8.6), card.rng.uniform(0.45, 0.8))
    card.colour("FFF8EC")
    card.colour("D62828")
    ground = warm(venue["ground"], 0.3)
    for p in venue["props"]:
        draw_prop(card, p)
    truck(card)
    bed_and_performer(card)
    crowd(card, venue["crowd"]["colours"])
    body = "\n".join(code for _, code in sorted(card.items, key=lambda it: -it[0]))
    colours = "\n".join(f"\\definecolor{{{n}}}{{HTML}}{{{h}}}" for n, h in sorted(card.colours.items()))
    ground_rect = f"\\filldraw[fill=cground, draw=ink, line width=1pt] (-0.2,-0.2) rectangle ({W + 0.2},{HORIZON});"
    return f"""\\documentclass[tikz,border=0pt]{{standalone}}
\\usepackage[utf8]{{inputenc}}
\\usepackage{{xcolor}}
\\definecolor{{ink}}{{HTML}}{{{INK}}}
\\definecolor{{cground}}{{HTML}}{{{ground}}}
{colours}
\\begin{{document}}
\\begin{{tikzpicture}}
\\clip (0,0) rectangle ({W},{H});
{sky}
{ground_rect}
{body}
{banner(card, venue)}
\\draw[draw=ink, line width=3pt] (0,0) rectangle ({W},{H});
\\end{{tikzpicture}}
\\end{{document}}
"""


def main():
    only = set(a for a in sys.argv[1:] if not a.startswith("--"))
    OUT.mkdir(parents=True, exist_ok=True)
    for v in DATA["venues"]:
        if only and v["id"] not in only:
            continue
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            (t / "card.tex").write_text(postcard(v))
            r = subprocess.run(["pdflatex", "-interaction=nonstopmode", "-halt-on-error", "card.tex"], cwd=t, capture_output=True, text=True)
            if r.returncode != 0:
                print(r.stdout[-2500:])
                raise SystemExit(f"{v['id']}: pdflatex failed")
            subprocess.run(["pdftoppm", "-png", "-r", "200", "-singlefile", str(t / "card.pdf"), str(t / "card")], check=True)
            shutil.copy(t / "card.png", OUT / f"{v['id']}.png")
            SRC.mkdir(parents=True, exist_ok=True)
            shutil.copy(t / "card.tex", SRC / f"{v['id']}.tex")
        print("wrote", OUT / f"{v['id']}.png")


if __name__ == "__main__":
    main()
