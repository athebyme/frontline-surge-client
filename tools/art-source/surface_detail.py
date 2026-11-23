"""Seamless material-specific surface marks over the authored colour swatches."""
import math

import numpy as np
from PIL import Image, ImageDraw


def surface_detail(swatch, tile):
    """The smooth paint, glass, light and damage masks keep their authored finish."""
    if tile not in (1, 2, 3, 4, 5, 6, 7, 8, 11, 12, 13, 14, 15, 26, 27, 28):
        return swatch
    pixels = np.asarray(swatch).copy()
    size = swatch.width
    rng = np.random.default_rng(94211 + tile)
    y, x = np.mgrid[0:size, 0:size] * (math.tau / size)
    # All layers meet exactly at UV wrap, including facets drawn across an edge.
    grain = rng.normal(0, 1, (size, size))
    patches = (np.sin(x * 3 + np.sin(y * 2)) * np.cos(y * 4 - x)
               + 0.35 * np.cos(x * 7 + y * 5))
    relief = Image.new("L", (size, size), 128)
    draw = ImageDraw.Draw(relief)

    def mark(points, value, stroke=0):
        px = np.array(points, dtype=float)
        # Only repeat a mark when its bounds touch an edge of the tile.
        offsets = []
        for col in range(2):
            values = px[:, col]
            offsets.append([0] + ([size] if values.min() <= stroke else [])
                           + ([-size] if values.max() >= size - stroke else []))
        for dx in offsets[0]:
            for dy in offsets[1]:
                line = [(int(round(a + dx)), int(round(b + dy))) for a, b in px]
                if stroke:
                    draw.line(line, fill=value, width=stroke)
                else:
                    draw.polygon(line, fill=value)

    if tile in (1, 13, 15):  # grass tufts, small leaves, cut stalks
        count = {1: 220, 13: 110, 15: 195}[tile]
        for _ in range(count):
            a, b = rng.uniform(0, size, 2)
            length = rng.uniform(4, 12) * size / 400
            slant = rng.uniform(-0.8, 0.8) if tile != 15 else 1.45
            if tile == 13:
                mark([(a,b),(a+length*.45,b-length*.7),(a+length,b-length*.6),
                      (a+length*.7,b-length*.1)], int(rng.integers(96,121)))
            else:
                for stem in (-0.4, 0.45):
                    end = a + length * (slant + stem)
                    mark([(a,b),(a+2,b),(end,b-length)], int(rng.integers(90,121)))
        micro = 2.8 * grain + 4.5 * patches
    elif tile in (3, 4, 12):  # fibrous wood, planks, upright bark fissures
        along, across = (y, x) if tile == 12 else (x, y)
        fibers = across * 38 + 1.8 * np.sin(along * 2 + across) + .6 * np.sin(along * 5)
        micro = -9 * np.maximum(0, np.cos(fibers)) ** 7 + 3 * patches + 1.5 * grain
        for _ in range(100):
            a, b = rng.uniform(0, size, 2)
            length = rng.uniform(3, 30)
            pts = [(a,b), (a+length*.66,b-1.5), (a+length,b-1.1), (a+length*.4,b+.5)]
            mark([(v,u) for u,v in pts] if tile == 12 else pts, int(rng.integers(91,124)))
        if tile != 12:
            # Small knot and adjacent interrupted growth rings, fixed within a board.
            for a, b in [(size * .26, size * .38), (size * .77, size * .85)]:
                for rw, rh in [(11,3.4),(6,2.3),(2,1)]:
                    draw.ellipse((a-rw,b-rh,a+rw,b+rh), outline=105)
    elif tile in (2, 5, 6, 7, 8, 14, 26, 28):
        # Grains in aggregate have a lit crown and an irregular darker edge.
        counts = {2: 470, 5: 370, 6: 260, 7: 420, 8: 170, 14: 1150, 26: 1250, 28: 590}
        radii = {2: 3.5, 5: 1.9, 6: 1.35, 7: 3.4, 8: 1.4, 14: 5.3, 26: 2.15, 28: 2.65}
        for _ in range(counts[tile]):
            a, b = rng.uniform(0, size, 2)
            r = rng.uniform(.55, radii[tile])
            pts = [(a-r,b),(a-r*.55,b-r*.78),(a+r*.55,b-r),(a+r,b+r*.24),(a+r*.22,b+r*.65)]
            mark(pts, int(rng.integers(85 if tile in (14,26,28) else 97, 126)))
            if r > 1.8:
                mark(pts[:3], int(rng.integers(131,149)), 1)
        micro = grain * (4.0 if tile in (14,26) else 2.5) + 3.4 * patches
        if tile in (5, 6, 7, 28):
            # Fine trowel striations and mineral bands under the coarse swatch.
            micro += 2.4 * np.cos(7*y + 1.8*np.sin(2*x+y))
    elif tile == 11:  # coarse woven sacks
        warp = np.sin(54*x + .36*np.sin(6*y))
        weft = np.sin(50*y + .28*np.cos(5*x))
        micro = 4.8 * (warp + weft) + 4 * warp * weft + 1.8 * grain
    else:  # rolled sheet, with small fixed heads on the stamped ribs
        micro = 2.6 * np.sin(40*x) + 1.4 * grain + 1.5 * patches
        for col in range(4):
            for row in range(2):
                a, b = int(size*(col+.145)/4), int(size*(row+.38)/2)
                draw.ellipse((a-2,b-2,a+2,b+2), fill=83)
                draw.line((a-1,b-1,a+1,b-1), fill=146)
    shift = np.asarray(relief, dtype=float) - 128 + micro
    pixels[:,:,:3] = np.clip(np.rint(pixels[:,:,:3] + shift[:,:,None]), 0, 255)
    return Image.fromarray(pixels)
