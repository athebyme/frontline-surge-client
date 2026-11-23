"""Bake CC0 takes into the game's mono sample bank. See docs/audio.md."""
import argparse
from functools import lru_cache
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
from concurrent.futures import ThreadPoolExecutor
import zlib

import numpy as np
from scipy import signal

RATE = 32000
ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument("sources", type=Path, help="directory containing the source recordings")
parser.add_argument("--fetch", action="store_true", help="download missing sources from the pinned manifest")
parser.add_argument("--output", type=Path, default=ROOT / "assets/field-audio.fsbk")
args = parser.parse_args()
args.sources.mkdir(parents=True, exist_ok=True)
manifest = json.loads(Path(__file__).with_name("sources.json").read_text())


def verify(item):
    filename, source = item
    path = args.sources / filename
    def correct(candidate):
        return candidate.is_file() and hashlib.sha256(candidate.read_bytes()).hexdigest() == source["sha256"]

    if correct(path):
        return
    if not args.fetch:
        raise ValueError(f"Missing or changed source: {filename}. Use --fetch to restore it.")
    with tempfile.NamedTemporaryFile(dir=args.sources, delete=False) as download:
        temp = Path(download.name)
    try:
        subprocess.run(["curl", "--fail", "--location", "--silent", "--show-error",
                        "--retry", "2", source["url"], "-o", str(temp)], check=True)
        if not correct(temp):
            raise ValueError(f"Source download differs from the pinned take: {filename}")
        temp.replace(path)
    finally:
        temp.unlink(missing_ok=True)


def low(x, hz):
    return signal.sosfilt(signal.butter(2, hz, fs=RATE, output="sos"), x)


def high(x, hz):
    return signal.sosfilt(signal.butter(2, hz, btype="highpass", fs=RATE, output="sos"), x)


def normalize(x):
    return x / max(np.max(np.abs(x)), 1e-7)


@lru_cache(None)
def read(key):
    filename = str(key) + (".mp3" if isinstance(key, int) else ".ogg")
    raw = subprocess.check_output(["ffmpeg", "-loglevel", "error", "-i", str(args.sources / filename),
                                   "-f", "f32le", "-ac", "1", "-ar", str(RATE), "-"])
    return np.frombuffer(raw, dtype="<f4").astype(np.float64)


def take(key, onset=0., duration=1., speed=1., hz=14500):
    source = read(key)
    # Locate the physical transient in a short window around each hand-marked take.
    start = max(0, round((onset - 0.045) * RATE))
    window = source[start:start + round(0.14 * RATE)]
    hits = np.flatnonzero(np.abs(window) > max(0.0004, 0.065 * np.max(np.abs(window))))
    if len(hits):
        start = max(0, start + hits[0] - round(0.0012 * RATE))
    x = source[start:start + round(duration * RATE * speed)]
    if speed != 1.:
        x = np.interp(np.arange(0, len(x), speed), np.arange(len(x)), x)
    x = normalize(low(high(x, 34), hz))
    n = min(len(x) // 3, round(RATE * min(0.09, duration * 0.26)))
    if n:
        x[-n:] *= np.linspace(1., 0., n) ** 2
    x[:8] *= np.linspace(0., 1., min(8, len(x)))
    return x


def layer(*parts):
    length = max(round(at * RATE) + len(x) for at, _, x in parts)
    result = np.zeros(length)
    for at, gain, x in parts:
        start = round(at * RATE)
        result[start:start + len(x)] += gain * x
    return result


def make(cue, v):
    # Three distinct takes for repeated actions; the mounted gun keeps its own exterior tail.
    metal = lambda base, **kw: take(f"{base}_{v:03}", **kw)
    cloth = lambda: take(f"cloth{v + 1}", duration=.40)
    latch = lambda: take("metalLatch", duration=.25, speed=[.92, 1., 1.08][v])
    rifle = lambda **kw: take(136399, [.061, 4.63, 10.15][v], **kw)
    pistol = lambda **kw: take(377786, [.28, 3.54, 6.93][v], **kw)
    echo = lambda **kw: take(613669, 2.96, **kw)
    heavy = lambda **kw: take(668071, .06, **kw)
    varied = [.965, 1., 1.035][v]
    if cue == "Shot":
        return layer((0, .97, take(46655, .40, .67, varied*.94)), (.017,.21, rifle(duration=.24, speed=.78,hz=2700)), (.085,.13,echo(duration=.55,hz=1150)))
    if cue == "Patrol":
        return layer((0,1,rifle(duration=.47,speed=1.08)), (.029,.17,latch()))
    if cue == "Smg":
        return layer((0,1,pistol(duration=.25,speed=1.06,hz=11500)), (.028,.18,metal("impactMetal_light",duration=.10)))
    if cue == "Mounted":
        return layer((0,1,rifle(duration=.46,speed=.85,hz=10000)), (.012,.43,heavy(duration=.19,speed=.90,hz=1500)), (.042,.20,latch()))
    if cue == "Distant":
        return layer((0,.82,rifle(duration=.60,speed=.87,hz=1400)), (.05,.38,echo(duration=.77,hz=850)))
    if cue == "Cannon":
        return layer((0,.95,heavy(duration=.66,speed=.68*varied,hz=8800)), (.018,.80,echo(duration=1.35,speed=.72,hz=1550)), (.123,.17,rifle(duration=.6,hz=1800)))
    if cue == "Apc":
        return layer((0,.95,heavy(duration=.44,speed=1.05*varied,hz=7300)), (.016,.42,rifle(duration=.50,speed=.82,hz=2600)), (.06,.16,echo(duration=.66,hz=1400)))
    if cue in ("Blast","ShellBlast"):
        deeper = cue == "ShellBlast"
        return layer((0,.92,echo(duration=1.4 if deeper else 1.1,speed=varied*(.81 if deeper else 1.02))),
                     (0,.67,heavy(duration=.32,speed=.79*varied,hz=2900)),
                     (.074,.18,metal("impactMining",duration=.38)),(.18,.13,metal("impactSoft_heavy",duration=.32)))
    if cue == "TankLoader":
        return layer((.32,.62,latch()),(.61,.51,metal("impactTin_medium",duration=.42,speed=.74)),
                     (1.10,.46,take("drawKnife1",duration=.45,speed=.68)),
                     (1.69,.72,metal("impactMetal_heavy",duration=.39,speed=.65)),
                     (2.27,.44,take("clothBelt",duration=.43,speed=.78)),
                     (2.81,.72,take("metalLatch",duration=.28,speed=.65)),
                     (3.40,.87,metal("impactMetal_medium",duration=.16,speed=.82)),(3.53,.38,take("metalClick",duration=.07)))
    if cue == "Reload":
        return layer((0,.56,cloth()),(.10,.82,latch()),(.37,.48,take("handleSmallLeather",duration=.36)),
                     (.75,.92,metal("impactMetal_light",duration=.16)),(.96,.64,take("drawKnife1",duration=.21,speed=1.24)))
    if cue == "Harvest":
        return layer((0,.65,cloth()),(.07,.76,take(["beltHandle1","beltHandle2","dropLeather"][v],duration=.38)))
    if cue == "Build":
        return layer((0,.89,metal("impactWood_heavy",duration=.40)),(.11,.47,metal("impactMetal_light",duration=.27)))
    if cue in ("Debris","Masonry","Splinter","Metal","Tile","Earth"):
        base={"Debris":"impactSoft_heavy","Masonry":"impactMining","Splinter":"impactPlank_medium",
              "Metal":"impactMetal_heavy","Tile":"impactGlass_medium","Earth":"impactSoft_heavy"}[cue]
        return layer((0,1,metal(base,duration=.45)),(.041,.28,metal("impactPlank_medium" if cue=="Splinter" else "impactMining",duration=.20,speed=1.4)))
    if cue == "Hurt":
        return layer((0,1,metal("impactPunch_heavy",duration=.26,hz=4200)),(.019,.37,cloth()))
    if cue == "Dry":
        return layer((0,.72,take("metalClick",duration=.069,speed=varied*1.16)),(.015,.25,cloth()[:1900]))
    if cue == "Throw":
        return layer((0,.65,cloth()),(.053,.72,latch()))
    if cue == "Crash":
        return layer((0,.9,metal("impactMetal_heavy",duration=.59,speed=.7)),(.032,.78,metal("impactWood_heavy",duration=.33,speed=.68)),(.18,.34,metal("impactGlass_medium",duration=.33)))
    if cue in ("ProductionStart","ProductionReady"):
        return layer((0,.7,latch()),(.13,.85,metal("impactMetal_medium",duration=.36,speed=1.21 if cue=="ProductionReady" else .82)))
    if cue == "BootRubble":
        return layer((0, .86, metal("footstep_concrete", duration=.39)),
                     (.018, .66, metal("impactMining", duration=.26, speed=1.06)),
                     (.145, .19, metal("impactGlass_medium", duration=.13, speed=1.31)))
    if cue in ("BootGround","BootHard"):
        ground = "footstep_grass" if cue=="BootGround" else "footstep_concrete"
        return layer((0,1,metal(ground,duration=.43)), (.027,.17,cloth()))
    raise ValueError(cue)


def loop(key, start, length, blend, hz):
    source = read(key)[round(RATE*start):round(RATE*(start+length))]
    x = normalize(low(high(source, 34), hz))
    n = round(blend*RATE)
    cross = np.linspace(0.,1.,n)
    return np.r_[x[n:-n], x[-n:]*(1-cross) + x[:n]*cross]


def encode(data):
    return np.round(np.clip(data, -.99, .99) * 32767).astype("<i2").tobytes()


def bake():
    with ThreadPoolExecutor(max_workers=8) as pool:
        list(pool.map(verify,manifest.items()))
    names = "Shot Distant Reload Harvest Build Debris Hurt Dry Patrol Smg Cannon Throw Blast Masonry Splinter Metal Mounted ProductionStart ProductionReady Crash Apc ShellBlast TankLoader Tile Earth BootGround BootHard BootRubble".split()
    def finish(cue, variation):
        x = normalize(make(cue, variation)) * .91
        n = min(len(x), round(.006 * RATE))
        x[-n:] *= np.linspace(1., 0., n)
        return encode(x)
    groups = {name: [finish(name, v) for v in range(3)] for name in names}
    groups["Diesel"] = [encode(loop(21187, .01, 1.19, .09, 8500))]
    groups["Rotor"] = [encode(loop(329523, 18., 5.5, .42, 6400))]
    groups["Wind"] = [encode(loop(528944, .1, 9.25, .65, 6800))]
    payload = bytearray(b"FSBK\x01" + struct.pack("<IH",RATE,len(groups)))
    for name,clips in groups.items():
        payload.extend(bytes([len(name)]) + name.encode() + bytes([len(clips)]))
        for clip in clips:
            payload.extend(struct.pack("<I",len(clip)//2))
            payload.extend(clip)
    args.output.write_bytes(zlib.compress(payload, 9))


if __name__ == "__main__":
    bake()
