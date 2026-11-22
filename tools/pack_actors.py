#!/usr/bin/env python3
"""Pack the named meshes of a GLB into the native game's compact vertex stream.

Usage: python3 tools/pack_actors.py [source.glb] [field-actors.fra]
Blender: import tools/art-source/field-actors.glb, edit mesh data, preserve material/UVs
and vertex colors, then export glTF Binary with vertex colors enabled. Bone pivots are
stored in each part's local mesh coordinates. The Rust renderer animates those pivots.
"""
import json
import math
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
source = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE / 'art-source/field-actors.glb'
destination = Path(sys.argv[2]) if len(sys.argv) > 2 else HERE.parent / 'assets/field-actors.fra'
raw = source.read_bytes()
magic, version, length = struct.unpack_from('<4sII', raw)
if magic != b'glTF' or version != 2 or length != len(raw):
    raise ValueError('Expected an uncompressed glTF 2.0 binary (.glb)')
chunks = {}
offset = 12
while offset < length:
    size, kind = struct.unpack_from('<I4s', raw, offset)
    chunks[kind] = raw[offset+8:offset+8+size]
    offset += 8+size
doc = json.loads(chunks[b'JSON'])
binary = chunks[b'BIN\0']
types = {5121:'B', 5123:'H', 5125:'I', 5126:'f'}
components = {'SCALAR':1, 'VEC2':2, 'VEC3':3, 'VEC4':4}


def values(index):
    accessor = doc['accessors'][index]
    view = doc['bufferViews'][accessor['bufferView']]
    component = accessor['componentType']
    fmt = '<'+types[component]*components[accessor['type']]
    stride = view.get('byteStride', struct.calcsize(fmt))
    offset = view.get('byteOffset', 0) + accessor.get('byteOffset', 0)
    normalized = accessor.get('normalized', False)
    scale = {5121:255, 5123:65535}[component] if normalized else 1
    for i in range(accessor['count']):
        row = struct.unpack_from(fmt, binary, offset+i*stride)
        yield tuple(v/scale for v in row) if normalized else row


def srgb(channel):
    value = 12.92*channel if channel <= .0031308 else 1.055*channel**(1/2.4)-.055
    return round(255*max(0, min(1, value)))


def uv_u16(value):
    return max(0, min(65535, round(65535*value)))


keys = ['left_hand','right_hand','flash','optic_irons','optic_reflex','optic_scope','grip_light','grip_stable']
keys += [f'{team}/{part}' for team in ('a','e') for part in ('torso','arms','mark','hip','shin')]
keys += [kind+part for kind in ('mr7','vx9','ar4') for part in ('','_mag','_slide')]
wanted = {mesh.get('name'): mesh for mesh in doc['meshes']}
output = bytearray(b'FRA1'+struct.pack('<H',len(keys)))
for key in keys:
    if key not in wanted:
        raise ValueError(f'Missing mesh {key!r}. Keep mesh data names when editing.')
    encoded = bytearray()
    indices = []
    vertex_base = 0
    for primitive in wanted[key]['primitives']:
        if primitive.get('mode',4) != 4:
            raise ValueError(f'{key}: triangles required')
        attr = primitive['attributes']
        positions = list(values(attr['POSITION']))
        colors = values(attr['COLOR_0'])
        uvs = values(attr['TEXCOORD_0'])
        for point, color, (u,v) in zip(positions, colors, uvs, strict=True):
            tile = max(0,min(3,math.floor(u*4)))
            a, b = uv_u16((u*256 - tile*64 - .5)/63), uv_u16((v*64-.5)/63)
            encoded.extend(struct.pack('<3f4B2H',*point,*map(srgb,color[:3]),tile,a,b))
        elements = [n[0] for n in values(primitive['indices'])] if 'indices' in primitive else list(range(len(positions)))
        if len(elements)%3 or max(elements)>=len(positions):
            raise ValueError(f'Invalid triangles in {key}')
        for j in range(0,len(elements),3):
            i1,i2,i3 = elements[j:j+3]
            indices.extend([vertex_base+i1,vertex_base+i3,vertex_base+i2])
        vertex_base += len(positions)
    if vertex_base>65535:
        raise ValueError(f'{key}: exceeds the 16-bit index budget')
    name=key.encode('ascii')
    output.extend(bytes([len(name)])+name+struct.pack('<II',vertex_base,len(indices))+encoded)
    output.extend(struct.pack(f'<{len(indices)}H',*indices))
destination.write_bytes(output)
print(f'Packed {len(keys)} mesh parts into {destination} ({len(output):,} bytes).')
