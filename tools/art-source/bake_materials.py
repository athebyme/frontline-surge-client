"""Rasterize the hand-painted field-materials.svg with Inkscape, Pillow and NumPy.

Run from any directory. The sheet's original cell order is part of the mesh format.
RGB modulates mesh pigment. Runtime gloss is kept independent of RGB opacity.
"""
from io import BytesIO
import json
from pathlib import Path
import struct
import subprocess
import tempfile

import numpy as np
from PIL import Image

SOURCE = Path(__file__).resolve().parent
ASSETS = SOURCE.parents[1] / "assets"
CELL, BORDER = 256, 2
CORE = CELL - 2 * BORDER
SURFACES = [*range(18), 25, 26, 27, 28, 29]
MASKS = (25, 29)
# Roughness comes from physical material type. Small paint chips scatter the highlight.
GLOSS = {8: .09, 9: .44, 10: .015, 11: .008, 16: .88, 17: .0, 26: .07, 27: .39, 28: .035}


def actor_material_preview(png):
    """Keep Blender's embedded preview identical to the native four-swatch texture."""
    source = SOURCE / "field-actors.glb"
    raw = source.read_bytes()
    metadata_length, = struct.unpack_from("<I", raw, 12)
    metadata = json.loads(raw[20:20+metadata_length])
    binary = raw[28+metadata_length:]
    image = next(i for i in metadata["images"] if i["name"] == "Field materials")
    view = metadata["bufferViews"][image["bufferView"]]
    offset, length = view.get("byteOffset", 0), view["byteLength"]
    if binary[offset:offset+length] == png:
        return
    old_block_length = (length+3) & ~3
    new_block = png + bytes(-len(png) % 4)
    shift = len(new_block) - old_block_length
    new_binary = binary[:offset] + new_block + binary[offset+old_block_length:]
    for other in metadata["bufferViews"]:
        if other.get("byteOffset", 0) > offset:
            other["byteOffset"] += shift
    view["byteLength"] = len(png)
    metadata["buffers"][0]["byteLength"] += shift
    encoded = json.dumps(metadata, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    encoded += b" " * (-len(encoded) % 4)
    output = (struct.pack("<4sIII4s", b"glTF", 2, 28+len(encoded)+len(new_binary), len(encoded), b"JSON")
              + encoded + struct.pack("<I4s", len(new_binary), b"BIN\0") + new_binary)
    source.write_bytes(output)


def render_sheet():
    with tempfile.TemporaryDirectory() as temp:
        path = Path(temp) / "sheet.png"
        subprocess.run(["inkscape", str(SOURCE / "field-materials.svg"), "--export-area-page",
                        "--export-width=2048", "--export-height=1024",
                        f"--export-filename={path}"], check=True)
        with Image.open(path) as sheet:
            return sheet.convert("RGBA")


def swatch(sheet, tile):
    row, col = divmod(tile, 8)
    return sheet.crop((col*CELL, row*CELL, (col+1)*CELL, (row+1)*CELL))


def main():
    sheet = render_sheet()
    atlas = np.zeros((4*CELL, 8*CELL, 4), dtype=np.uint8)
    for tile in SURFACES:
        original = swatch(sheet, tile)
        rgba = np.array(original.resize((CORE,CORE), Image.Resampling.LANCZOS))
        if tile not in MASKS:
            if np.min(rgba[:,:,3]) != 255:
                raise ValueError(f"Unpainted pixels in material {tile}")
            gloss = GLOSS.get(tile, .025)
            if tile == 9:
                gloss = gloss * np.clip((rgba[:,:,0].astype(float)-178.)/64., .16, 1.)
            rgba[:,:,3] = np.rint(255*gloss).astype(np.uint8)
        row, col = divmod(tile, 8)
        for size, border, left, top in ((256,2,0,0),(64,2,6*CELL,3*CELL),(16,1,0,3*CELL)):
            width = size - 2 * border
            # Filter all channels independently. Alpha carries gloss, except on the damage masks.
            channels = [Image.fromarray(rgba[:,:,i]).resize((width,width), Image.Resampling.BOX)
                        for i in range(4)]
            core = np.stack(channels, axis=2)
            padded = np.pad(core, ((border,border),(border,border),(0,0)), mode="wrap")
            x, y = left + col*size, top + row*size
            atlas[y:y+size, x:x+size] = padded
    Image.fromarray(atlas).save(ASSETS / "terrain-atlas.png", compress_level=9)
    fabric = Image.new("RGB", (CELL*4,CELL))
    for i, tile in enumerate((0,10,9,31)):
        fabric.paste(swatch(sheet,tile).convert("RGB"), (i*CELL,0))
    output = BytesIO()
    fabric.save(output, "PNG", compress_level=9)
    png = output.getvalue()
    (ASSETS / "field-fabric.png").write_bytes(png)
    actor_material_preview(png)


if __name__ == "__main__":
    main()
