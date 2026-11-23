"""Build the four pigment-modulation swatches used by native infantry and kit meshes."""
from io import BytesIO
import json
from pathlib import Path
import struct

import numpy as np
from PIL import Image

SOURCE = Path(__file__).resolve().parent
ASSETS = SOURCE.parents[1] / "assets"
CELL = 512

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


def grain(rng, cells):
    # Periodic bicubic noise gives each scratch, thread or patch a softer edge.
    lattice = rng.normal(size=(cells, cells)).astype(np.float32)
    padded = Image.fromarray(np.pad(lattice, 2, mode="wrap"))
    return np.asarray(padded.resize((CELL, CELL), Image.Resampling.BICUBIC,
                                    box=(2, 2, cells + 2, cells + 2)))


def surface(base, relief):
    rgb = np.asarray(base, dtype=np.float32) + relief[..., None]
    return Image.fromarray(np.rint(rgb).clip(0, 255).astype(np.uint8))


def bake_actor_materials():
    rng = np.random.default_rng(4371)
    y, x = np.mgrid[:CELL, :CELL]
    macro = grain(rng, 8)
    fine = grain(rng, 170)

    # Coated receivers and optic housings: satin passing across the cast finish,
    # a brushed undertone and localized scuffing along one exposed edge.
    brushing = np.sin((y + 0.8 * macro) * 0.96)
    patina = 4.5 * macro + 3.2 * grain(rng, 35) + 2.8 * fine + 2.6 * brushing
    wear = np.exp(-((x - 5 - 1.6 * macro) / 3.0) ** 2) * np.clip(8 + macro * 18, 0, 30)
    for _ in range(35):
        x0, y0 = rng.uniform(4, CELL - 4, 2)
        length = rng.uniform(6, 54)
        dx = x - x0
        ridge = np.abs(y - y0 - dx * rng.uniform(-0.28, 0.28))
        wear += np.maximum(0, 1 - ridge / 1.1) * np.maximum(0, 1 - abs(dx) / length) * 24
    metal = surface([228, 231, 232], patina + wear)

    # Molded rubber and reinforced kit panels share this swatch in the rig.
    # Offset rows give the grip relief a distinct shape at middle distance.
    row = y // 12
    dx = (x + (row % 2) * 6) % 12 - 6
    dy = y % 12 - 6
    pebble = np.exp(-(dx ** 2 / 8 + dy ** 2 / 11))
    shoulder = np.exp(-((dx + 1.8) ** 2 / 7 + (dy + 2) ** 2 / 8))
    grip = surface([232, 231, 226], 4.5 * macro + 3.3 * fine - 20 * pebble + 13 * shoulder)

    # Ripstop field cloth. Two patch sizes read across the torso; reinforcement
    # threads and twin-needle stitching come into view on the first-person cuff.
    patch = grain(rng, 6) + 0.38 * grain(rng, 18) + 0.10 * grain(rng, 47)
    tones = np.array([[181, 189, 169], [207, 216, 189], [232, 231, 204], [247, 242, 217]])
    camo = tones[np.digitize(patch, [-0.58, 0.10, 0.96])]
    twill = np.cos((x + y * 0.6) * np.pi / 2.6) * np.sin(y * np.pi / 2.9)
    ripstop = 6 * ((x % 32 < 2) | (y % 32 < 2)).astype(float)
    seam = np.zeros_like(macro)
    seam_path = 49 + 5 * np.sin(2 * np.pi * y / CELL)
    seam -= 15 * np.exp(-((x - seam_path - 3) / 5) ** 2)
    for offset in (0, 8):
        strand = np.exp(-((x - seam_path - offset) / 1.15) ** 2)
        seam += 22 * strand * (np.cos(y * np.pi / 6) > -0.3)
    cloth = surface(camo, 5.2 * twill + 2.3 * fine + ripstop + seam)

    # Slot zero carries skin highlights, insignia, glass and flash sprites.
    # White is the authored identity multiplier for those vertex pigments.
    strip = Image.new("RGB", (4 * CELL, CELL), "white")
    for i, swatch in enumerate((metal, grip, cloth), 1):
        strip.paste(swatch, (i * CELL, 0))
    output = BytesIO()
    strip.save(output, "PNG", compress_level=9)
    png = output.getvalue()
    (ASSETS / "field-fabric.png").write_bytes(png)
    actor_material_preview(png)


if __name__ == "__main__":
    bake_actor_materials()
