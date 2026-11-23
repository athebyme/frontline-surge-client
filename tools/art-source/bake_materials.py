"""Rebuild the field material atlas. Requires Python 3, NumPy and Pillow.

RGB modulates authored mesh pigment; alpha controls surface gloss. Every occupied
cell has two wrapped texels on each side, including around the corners.
"""
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw

PATH = Path(__file__).resolve().parents[2] / "assets" / "terrain-atlas.png"
CELL, GUTTER = 256, 2
SIZE = CELL - GUTTER * 2
Y, X = (np.indices((SIZE, SIZE)) + 0.5) / SIZE
TAU = 2 * np.pi


def noise(rng, columns, rows=None):
    rows = rows or columns
    grid = rng.uniform(-1, 1, (rows, columns))
    u, v = X * columns, Y * rows
    i, j = u.astype(int), v.astype(int)
    fu, fv = u % 1, v % 1
    fu, fv = fu**2 * (3 - 2 * fu), fv**2 * (3 - 2 * fv)
    a, b = grid[j, i], grid[j, (i + 1) % columns]
    c, d = grid[(j + 1) % rows, i], grid[(j + 1) % rows, (i + 1) % columns]
    return (a * (1 - fu) + b * fu) * (1 - fv) + (c * (1 - fu) + d * fu) * fv


def fade(lo, hi, values):
    t = np.clip((values - lo) / (hi - lo), 0, 1)
    return t * t * (3 - 2 * t)


def color(value, warmth=0, gloss=0.03):
    return np.stack(np.broadcast_arrays(value + warmth, value + 0.12 * warmth,
                                        value - 0.85 * warmth, gloss + X * 0), axis=2)


def strokes(rng, count, length, width, shape):
    """An overlapping, periodic field of individual blades, leaves or scratches."""
    result = Image.new("F", (SIZE, SIZE))
    draw = ImageDraw.Draw(result)
    for _ in range(count):
        x, y = rng.uniform(0, SIZE, 2)
        length_px, width_px = rng.uniform(*length), rng.uniform(*width)
        angle = rng.uniform(-0.75, 0.75) + (np.pi / 2 if shape == "scratch" else 0)
        dx, dy = length_px * np.sin(angle), length_px * np.cos(angle)
        wx, wy = width_px * np.cos(angle), -width_px * np.sin(angle)
        value = rng.uniform(-0.8, 1)
        for ox in (-SIZE, 0, SIZE):
            for oy in (-SIZE, 0, SIZE):
                px, py = x + ox, y + oy
                if not (-length_px <= px <= SIZE + length_px and -length_px <= py <= SIZE + length_px):
                    continue
                tip = (px + dx, py + dy)
                if shape == "leaf":
                    points = [(px, py), (px + dx*.48 + wx, py + dy*.48 + wy), tip,
                              (px + dx*.58 - wx, py + dy*.58 - wy)]
                else:
                    points = [(px - wx, py - wy), (px + wx, py + wy), tip]
                draw.polygon(points, fill=value)
    return np.asarray(result)


def cells(rng, count, warp):
    """Toroidal first/second nearest stones, with a reproducible tint per stone."""
    px, py = X + warp, Y + np.roll(warp, SIZE//3, axis=1)
    first, second = np.full(X.shape, 4.), np.full(X.shape, 4.)
    tone = np.zeros_like(X)
    for cx, cy, shade in rng.random((count, 3)):
        dx, dy = (px-cx+.5) % 1 - .5, (py-cy+.5) % 1 - .5
        dist = dx*dx + .78*dy*dy
        closer = dist < first
        second = np.where(closer, first, np.minimum(dist, second))
        first = np.minimum(dist, first)
        tone = np.where(closer, shade, tone)
    return np.sqrt(second) - np.sqrt(first), tone


def fracture(tile, rng):
    scale = 3
    mask = Image.new("L", (SIZE*scale, SIZE*scale))
    pen = ImageDraw.Draw(mask)
    center = rng.uniform(.34,.65,2) * SIZE
    def stroke(points, width, opacity):
        pen.line([tuple(np.rint(p*scale).astype(int)) for p in points],
                 fill=opacity, width=max(1,round(width*scale)))
    def branch(start, angle, segments, width):
        points = [start.copy()]
        for _ in range(segments):
            yaw = angle + rng.uniform(-.5,.5)
            delta = np.array([np.cos(yaw),np.sin(yaw)]) * rng.uniform(5,13)
            points.append(points[-1]+delta)
        for i,(a,b) in enumerate(zip(points,points[1:])):
            taper = 1 - i/len(points)
            stroke([a,b],width*taper+.45,round(95+160*taper))
        return points
    for i in range(4 if tile == 25 else 3):
        angle = i*TAU/(4 if tile == 25 else 3) + rng.uniform(-.43,.43)
        path = branch(center, angle, rng.integers(8,13), rng.uniform(2.5,4.0))
        for j in (2,5):
            branch(path[j], angle+rng.choice([-1,1])*rng.uniform(.51,1.14), rng.integers(3,7),1.9)
        for p in path[1:5:2]:
            chip = [p + rng.uniform(-4,4,2) for _ in range(3)]
            pen.polygon([tuple(np.rint(v*scale).astype(int)) for v in chip],fill=220)
    alpha = np.asarray(mask.resize((SIZE,SIZE),Image.Resampling.LANCZOS),dtype=float)/255.
    return color(.91 + .045*noise(rng,15), gloss=alpha)


def bake(tile):
    rng = np.random.default_rng(5100 + tile)
    broad, medium, fine = noise(rng, 3), noise(rng, 12), noise(rng, 60)
    grains = rng.normal(0, 0.017, X.shape)
    rough = .035 * fine + grains
    if tile in (0, 17):
        return color(.991 + .005 * fine, gloss=0.)
    if tile in (25,29):
        return fracture(tile, rng)
    if tile == 1:  # Meadow: green clumps interleave with dry seed heads.
        blades = strokes(rng, 2200, (3, 14), (.45, 1.7), "grass")
        return color(.89 + .063*broad + .055*medium + .14*blades + rough,
                     .034*broad - .043*medium + .033*blades)
    if tile == 2:  # Packed soil, grit and granular compression around stones.
        edge, tone = cells(rng, 94, .011*medium)
        grit = fade(.019, .046, edge) * fade(.48, .66, tone)
        return color(.91 + .072*broad + .054*medium + rough + .071*grit, .030*medium)
    if tile in (3, 4):  # Broad timber grain with knots and sawn plank edges.
        board = np.floor(Y*4).astype(int)
        ring = 18*Y + .59*noise(rng, 3, 8) + .24*noise(rng, 6, 13)
        for x, y in ((.19,.27),(.76,.79)):
            dx, dy = ((X-x+.5)%1-.5)/.17, ((Y-y+.5)%1-.5)/.048
            ring += 1.67*np.exp(-1.9*(dx*dx+dy*dy))
        grain = (.5+.5*np.cos(TAU*ring))**4
        value = .96 - .110*grain + .046*noise(rng, 5, 73) + rough*.65
        if tile == 4:
            phase = (Y*4 + .008*medium) % 1
            joint = np.minimum(phase, 1-phase)
            value += np.array([.018,-.038,.026,-.012])[board] - .235*(1-fade(.007,.053,joint))
            value += .04*np.exp(-((phase-.060)/.023)**2)
        return color(value, .025*broad + .014*grain)
    if tile == 5:  # Staggered fired brick with chipped arrises and lime mortar.
        wav = .010 * medium + .006 * fine
        row = np.floor(Y*8 + wav).astype(int)
        v, u = (Y*8+wav)%1, (X*4 + .5*(row%2)+wav)%1
        edge = np.minimum(np.minimum(u,1-u)*2., np.minimum(v,1-v))
        binder = 1-fade(.035,.082,edge)
        bricks = rng.uniform(-.087,.078,(8,4))
        individual = bricks[row%8,np.floor(X*4+.5*(row%2)+wav).astype(int)%4]
        value = .954 + individual + rough + .044*medium
        return color(value*(1-binder) + (.74+.10*fine)*binder,
                     .040*(1-binder)*medium - .039*binder)
    if tile in (6, 28):  # Mineral plaster, or poured concrete with impressions and aggregate.
        pores = fade(.42,.85,noise(rng, 95))
        value = .951 + .047*broad + .032*medium + rough*.75 - .11*pores
        if tile == 28:
            form = np.minimum(Y%0.5,.5-Y%0.5)
            value -= .12 * (1-fade(.003,.013,form))
            holes = np.hypot((X*2+.25)%1-.5,(Y*2+.35)%1-.5)
            value -= .16 * (1-fade(.016,.046,holes))
        return color(value,.018*broad)
    if tile == 7:
        edge, stone = cells(rng, 19, .019*medium + .004*fine)
        seam = 1-fade(.0015,.012,edge)
        return color(.88 + .135*stone - .105*seam + .040*medium + rough,
                     .048*(stone-.45) + .029*broad)
    if tile == 8:  # Overlapping curved roof tiles, offset every alternate row.
        row = np.floor(Y*6).astype(int)
        u, v = (X*5+.50*(row%2)+.008*medium)%1, (Y*6)%1
        crown = np.sin(np.pi*u)**.65
        joints = .18*(1-fade(.018,.084,v)) + .050*fade(.83,.97,v)
        tiles = rng.uniform(-.047,.047,(6,5))[row%6,np.floor(X*5+.5*(row%2)).astype(int)%5]
        return color(.855 + .117*crown - joints + tiles + rough*.75, .022*broad, .13+.07*crown)
    if tile in (9, 27):  # Enamel, scratches and small blooms of oxidation.
        scratches = strokes(rng, 130, (3,19), (.09,.42), "scratch")
        patina = fade(.23,.72,medium*.55 + fine*.45)
        value = .953 + .026*broad + rough*.65 - .14*patina + .093*scratches
        if tile == 27:
            value *= .83 + .17 * (.5+.5*np.cos(8*TAU*X))
        return color(value, .085*patina, .46 - .35*patina)
    if tile in (10, 11):
        weave = np.cos(TAU*X*(56 if tile==10 else 38))*np.cos(TAU*Y*(52 if tile==10 else 34))
        twill = np.sin(TAU*(X*24+Y*34))
        return color(.929 + .055*weave + .021*twill + .032*medium + rough*.47, .015*broad)
    if tile == 12:
        furrow = (.5+.5*np.cos(TAU*(X*14 + .62*noise(rng,8,3))))**3
        return color(.963 - .207*furrow + .064*noise(rng,37,5) + rough, .030*medium)
    if tile == 13:
        leaves = strokes(rng, 2300, (4,13), (1.4,3.5), "leaf")
        light = .13*leaves + .085*broad + .064*medium
        return color(.90 + light + rough*.5, .029*broad + .053*leaves)
    if tile in (14, 26):
        edge, tone = cells(rng, 210, .004*fine)
        gravel = (.18*tone - .082)*(fade(.002,.013,edge))
        if tile == 26:
            tar = fade(.46,.78,broad+.38*medium)
            return color(.919 + .32*gravel + .032*medium + rough*1.5 - .058*tar,
                         .009*medium, .053+.12*tar)
        return color(.915 + gravel - .083*(1-fade(.001,.008,edge)) + rough, .058*(tone-.5))
    if tile == 15:
        stalks = strokes(rng, 2500, (4,15), (.4,1.2), "grass")
        furrow = .5 + .5*np.cos(TAU*(3*X + .12*medium))
        return color(.88 + .092*furrow + .041*broad + .135*stalks + rough, .028*furrow)
    if tile == 16:  # Dusty glass with smooth highlights left to the shader.
        dust = fade(.03,.77,broad + .24*medium)
        return color(.985 - .061*dust + rough*.14, -.015 + .012*dust, .88 - .58*dust)
    raise ValueError(tile)


def main():
    atlas = np.zeros((CELL*4, CELL*8, 4), dtype=np.uint8)
    for tile in [*range(18), 25, 26, 27, 28, 29]:
        core = np.rint(np.clip(bake(tile), 0, 1) * 255).astype(np.uint8)
        # Neighbours outside the cell interior are the opposite edge of this same material.
        padded = np.pad(core, ((GUTTER,GUTTER),(GUTTER,GUTTER),(0,0)), mode="wrap")
        row, col = divmod(tile, 8)
        atlas[row*CELL:(row+1)*CELL, col*CELL:(col+1)*CELL] = padded
        # Downsample each material in isolation. Hardware mipmaps of the full
        # atlas filter into unrelated materials and see discontinuities at fract(uv).
        for size, border, left, top in ((64,2,6*CELL,3*CELL), (16,1,0,3*CELL)):
            width = size-2*border
            # Alpha stores an independent material channel, not premultiplied colour.
            channels = [Image.fromarray(core[:,:,i]).resize((width,width), Image.Resampling.BOX)
                        for i in range(4)]
            reduced = np.stack(channels, axis=2)
            reduced = np.pad(reduced, ((border,border),(border,border),(0,0)), mode="wrap")
            x, y = left+col*size, top+row*size
            atlas[y:y+size, x:x+size] = reduced
    Image.fromarray(atlas).save(PATH, compress_level=9)


if __name__ == "__main__":
    main()
