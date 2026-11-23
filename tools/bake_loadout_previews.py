"""Render workshop thumbnails from the same packed models as the native character rig."""
from pathlib import Path
import struct

import numpy as np
from PIL import Image, ImageDraw


ASSETS = Path(__file__).resolve().parents[1] / "assets"
CELL = (288, 104)
AA = 3
WEAPONS = ("mr7", "vx9", "ar4")
OPTICS = ("irons", "reflex", "scope")
GRIPS = ("standard", "light", "stable")
# The magazine is the one rig part with a separate rest offset in actors::Gun.
FEED = {"mr7": (0, -.067, -.098), "vx9": (0, -.075, -.117), "ar4": (0, -.067, -.088)}


def load_parts():
    data = (ASSETS / "field-actors.fra").read_bytes()
    assert data[:4] == b"FRA1"
    count, = struct.unpack_from("<H", data, 4)
    offset = 6
    vertex = np.dtype([("pos", "<f4", 3), ("color", "u1", 4), ("uv", "<u2", 2)])
    parts = {}
    for _ in range(count):
        n = data[offset]
        name = data[offset + 1:offset + n + 1].decode("ascii")
        offset += n + 1
        n_vertices, n_indices = struct.unpack_from("<II", data, offset)
        offset += 8
        vertices = np.frombuffer(data, vertex, n_vertices, offset)
        offset += n_vertices * vertex.itemsize
        indices = np.frombuffer(data, "<u2", n_indices, offset).reshape(-1, 3)
        offset += n_indices * 2
        parts[name] = vertices["pos"][indices], vertices["color"][indices, :3].mean(axis=1)
    assert offset == len(data)
    return parts


RIGHT = np.array([.182, 0., -.983])
UP = np.array([-.127, .992, -.024])


def frame(parts, weapon):
    model = np.concatenate([
        parts[weapon][0], parts[weapon + "_slide"][0], parts[weapon + "_mag"][0] + FEED[weapon],
        *[parts["optic_" + optic][0] for optic in OPTICS],
        *[parts["grip_" + grip][0] for grip in GRIPS if grip != "standard"],
    ]).reshape(-1, 3)
    profile = np.stack((model @ RIGHT, -(model @ UP)), axis=1)
    size = np.ptp(profile, axis=0)
    center = (profile.min(axis=0) + profile.max(axis=0)) / 2
    return center, min((CELL[0] - 14) / size[0], (CELL[1] - 12) / size[1])


def picture(parts, weapon, optic, grip, framing):
    names = [weapon, weapon + "_slide", weapon + "_mag", "optic_" + optic]
    if grip != "standard":
        names.append("grip_" + grip)
    world = np.concatenate([parts[n][0] + (FEED[weapon] if n.endswith("_mag") else 0) for n in names])
    paint = np.concatenate([parts[n][1] for n in names])
    # Mostly in profile, with enough elevation to reveal the selected optic and grip.
    toward = np.cross(RIGHT, UP)
    projected = np.stack((world @ RIGHT, -(world @ UP)), axis=2)
    normal = np.cross(world[:, 1] - world[:, 0], world[:, 2] - world[:, 0])
    normal /= np.maximum(np.linalg.norm(normal, axis=1, keepdims=True), 1e-9)
    normal *= np.where((normal @ toward)[:, None] < 0, -1, 1)
    light = np.clip(normal @ np.array([.51, .79, .29]), 0, 1)
    color = np.clip((paint / 255) ** .77 * (161 + 88 * light[:, None]) + 31, 0, 255)
    # Use a fixed framing per weapon so changing attachments never moves the body.
    center, factor = framing
    projected = (projected - center) * factor + np.array(CELL) / 2
    target = Image.new("RGBA", (CELL[0] * AA, CELL[1] * AA))
    draw = ImageDraw.Draw(target)
    for i in np.argsort((world @ toward).mean(axis=1), kind="stable"):
        draw.polygon((projected[i] * AA).reshape(-1).tolist(), fill=tuple(color[i].astype(int)) + (255,))
    return target.resize(CELL, Image.Resampling.LANCZOS)


if __name__ == "__main__":
    parts = load_parts()
    sheet = Image.new("RGBA", (CELL[0] * len(GRIPS), CELL[1] * len(WEAPONS) * len(OPTICS)))
    for row, weapon in enumerate(WEAPONS):
        framing = frame(parts, weapon)
        for i, optic in enumerate(OPTICS):
            for j, grip in enumerate(GRIPS):
                origin = CELL[0] * j, CELL[1] * (len(OPTICS) * row + i)
                sheet.paste(picture(parts, weapon, optic, grip, framing), origin)
    sheet.save(ASSETS / "loadout-previews.png", optimize=True)
