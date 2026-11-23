"""Bake the hand-painted field-materials.svg and seamless close-range detail.

Run from any directory. The sheet's original cell order is part of the mesh format.
RGB modulates mesh pigment. Runtime gloss is kept independent of RGB opacity.
"""
from io import BytesIO
from pathlib import Path
import shutil
import subprocess

import numpy as np
from PIL import Image

from bake_actor_materials import bake_actor_materials
from surface_detail import surface_detail

SOURCE = Path(__file__).resolve().parent
ASSETS = SOURCE.parents[1] / "assets"
CELL, BORDER = 400, 2
CORE = CELL - 2 * BORDER
SURFACES = [*range(18), 25, 26, 27, 28, 29]
MASKS = (25, 29)
# Roughness comes from physical material type. Small paint chips scatter the highlight.
GLOSS = {8: .09, 9: .44, 10: .015, 11: .008, 16: .88, 17: .0, 26: .07, 27: .39, 28: .035}


def render_sheet():
    source = str(SOURCE / "field-materials.svg")
    width, height = 8*CELL, 4*CELL
    if shutil.which("inkscape"):
        cmd = ["inkscape", source, "--export-area-page", "--export-type=png",
               f"--export-width={width}", f"--export-height={height}", "--export-filename=-"]
    else:
        # sharp uses librsvg; preserve vector precision by rasterizing at native density.
        script = ("require('sharp')(process.argv[1],{density:Number(process.argv[2])})"
                  ".png().toBuffer().then(b=>process.stdout.write(b))"
                  ".catch(e=>{console.error(e);process.exit(1)})")
        cmd = ["node", "-e", script, source, str(width/512*72)]
    pixels = subprocess.run(cmd, check=True, stdout=subprocess.PIPE).stdout
    with Image.open(BytesIO(pixels)) as sheet:
        if sheet.size != (width, height):
            raise ValueError(f"Unexpected SVG dimensions {sheet.size}")
        return sheet.convert("RGBA")


def swatch(sheet, tile):
    row, col = divmod(tile, 8)
    return sheet.crop((col*CELL, row*CELL, (col+1)*CELL, (row+1)*CELL))


def bake_terrain():
    sheet = render_sheet()
    atlas = np.zeros((2048, 2048, 4), dtype=np.uint8)
    for slot, tile in enumerate(SURFACES):
        original = surface_detail(swatch(sheet, tile), tile)
        rgba = np.array(original.resize((CORE,CORE), Image.Resampling.LANCZOS))
        if tile not in MASKS:
            if np.min(rgba[:,:,3]) != 255:
                raise ValueError(f"Unpainted pixels in material {tile}")
            gloss = GLOSS.get(tile, .025)
            if tile == 9:
                gloss = gloss * np.clip((rgba[:,:,0].astype(float)-178.)/64., .16, 1.)
            rgba[:,:,3] = np.rint(255*gloss).astype(np.uint8)
        # 5 columns of full-size swatches. Filtered levels occupy the two unused
        # slots at the end of the final row. All three levels have wrapped gutters.
        for size, border, columns, left, top in ((400,2,5,0,0),(100,2,8,1200,1600),(25,1,8,1200,1900)):
            row, col = divmod(slot, columns)
            width = size - 2 * border
            # Filter all channels independently. Alpha carries gloss, except on the damage masks.
            channels = [Image.fromarray(rgba[:,:,i]).resize((width,width), Image.Resampling.BOX)
                        for i in range(4)]
            core = np.stack(channels, axis=2)
            padded = np.pad(core, ((border,border),(border,border),(0,0)), mode="wrap")
            x, y = left + col*size, top + row*size
            atlas[y:y+size, x:x+size] = padded
    Image.fromarray(atlas).save(ASSETS / "terrain-atlas.png", compress_level=9)


if __name__ == "__main__":
    bake_terrain()
    bake_actor_materials()
