//! Per-building surfaces from the *same* layout as the authoritative simulation.
//! Each quad remembers its cell and its neighbor. Removing a cell both makes a hole
//! and exposes the cut faces behind it. Only that building's index buffers change.
use super::{dead, in_frustum, mq, number, prop_model, Page, PageKind};
use frontline_surge::fracture::{Cell, Layout, Material};
use macroquad::prelude::*;
use serde_json::Value;
use std::collections::HashMap;

const NONE: u16 = u16::MAX;
const VERTEX_LIMIT: usize = 60_000;
const MAX_FLYING: usize = 85;
pub(super) fn supports(kind: &str) -> bool {
    matches!(kind, "house" | "barn" | "ruin" | "wall" | "barricade")
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    p: [f32; 3],
    n: [f32; 3],
    uv: [f32; 2],
    style: [u8; 4],
}
#[derive(Clone, Copy)]
struct Face {
    base: u16,
    cell: u16,
    neighbor: u16,
}
struct Part {
    page: Page,
    faces: Vec<Face>,
}
struct Writer {
    vertices: Vec<Vertex>,
    faces: Vec<Face>,
}
const AXIS: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];
fn key(cell: &Cell) -> [i32; 3] {
    std::array::from_fn(|i| (2. * cell.center[i] / cell.size[i]).round() as i32)
}
fn faces_for(box_center: Vec3, size: Vec3, i: usize) -> [Vec3; 4] {
    let h = size * 0.5;
    let (a, b, c, d) = match i {
        0 => (
            vec3(h.x, -h.y, h.z),
            vec3(h.x, -h.y, -h.z),
            vec3(h.x, h.y, -h.z),
            vec3(h.x, h.y, h.z),
        ),
        1 => (
            vec3(-h.x, -h.y, -h.z),
            vec3(-h.x, -h.y, h.z),
            vec3(-h.x, h.y, h.z),
            vec3(-h.x, h.y, -h.z),
        ),
        2 => (
            vec3(-h.x, h.y, -h.z),
            vec3(-h.x, h.y, h.z),
            vec3(h.x, h.y, h.z),
            vec3(h.x, h.y, -h.z),
        ),
        3 => (
            vec3(-h.x, -h.y, h.z),
            vec3(-h.x, -h.y, -h.z),
            vec3(h.x, -h.y, -h.z),
            vec3(h.x, -h.y, h.z),
        ),
        4 => (
            vec3(-h.x, -h.y, h.z),
            vec3(h.x, -h.y, h.z),
            vec3(h.x, h.y, h.z),
            vec3(-h.x, h.y, h.z),
        ),
        _ => (
            vec3(h.x, -h.y, -h.z),
            vec3(-h.x, -h.y, -h.z),
            vec3(-h.x, h.y, -h.z),
            vec3(h.x, h.y, -h.z),
        ),
    };
    [
        a + box_center,
        b + box_center,
        c + box_center,
        d + box_center,
    ]
}
fn tone(style: [u8; 4], factor: f32) -> [u8; 4] {
    [
        (style[0] as f32 * factor).clamp(0., 255.) as u8,
        (style[1] as f32 * factor).clamp(0., 255.) as u8,
        (style[2] as f32 * factor).clamp(0., 255.) as u8,
        style[3],
    ]
}
fn mat_style(mat: Material) -> [u8; 4] {
    match mat {
        Material::Wood => [132, 102, 71, 4],
        Material::Brick => [151, 93, 69, 5],
        Material::Stone => [143, 137, 117, 7],
        Material::Metal => [102, 110, 102, 9],
        Material::Tile => [148, 91, 64, 8],
    }
}
impl Writer {
    fn new() -> Self {
        Self {
            vertices: vec![],
            faces: vec![],
        }
    }
    fn quad(&mut self, pts: [Vec3; 4], face: usize, style: [u8; 4], cell: u16, neighbor: u16) {
        let dir = AXIS[face];
        let n = [dir[0] as f32, dir[1] as f32, dir[2] as f32];
        let base = self.vertices.len() as u16;
        for p in pts {
            let uv = match face {
                0 | 1 => [p.z, p.y],
                2 | 3 => [p.x, p.z],
                _ => [p.x, p.y],
            };
            self.vertices.push(Vertex {
                p: p.to_array(),
                n,
                uv,
                style,
            });
        }
        self.faces.push(Face {
            base,
            cell,
            neighbor,
        });
    }
    fn cuboid(&mut self, center: Vec3, size: Vec3, style: [u8; 4], cell: u16) {
        for i in 0..6 {
            self.quad(faces_for(center, size, i), i, style, cell, NONE);
        }
    }
    fn upload(&mut self, gpu: &mut dyn mq::RenderingBackend) -> Part {
        let buffer = gpu.new_buffer(
            mq::BufferType::VertexBuffer,
            mq::BufferUsage::Immutable,
            mq::BufferSource::slice(&self.vertices),
        );
        let idx = gpu.new_buffer(
            mq::BufferType::IndexBuffer,
            mq::BufferUsage::Dynamic,
            mq::BufferSource::empty::<u16>(self.faces.len() * 6),
        );
        self.vertices.clear();
        Part {
            page: Page {
                vertices: buffer,
                indices: idx,
                count: 0,
                low: Vec3::ZERO,
                high: Vec3::ZERO,
                kind: PageKind::Template,
            },
            faces: std::mem::take(&mut self.faces),
        }
    }
}
fn update_part(part: &mut Part, broken: &[bool], gpu: &mut dyn mq::RenderingBackend) {
    let mut indices = Vec::with_capacity(part.faces.len() * 6);
    for f in &part.faces {
        if f.cell != NONE && broken[f.cell as usize] {
            continue;
        }
        if f.neighbor != NONE && !broken[f.neighbor as usize] {
            continue;
        }
        let k = f.base;
        indices.extend_from_slice(&[k, k + 1, k + 2, k, k + 2, k + 3]);
    }
    part.page.count = indices.len() as i32;
    if !indices.is_empty() {
        gpu.buffer_update(part.page.indices, mq::BufferSource::slice(&indices));
    }
}
fn world_bounds(model: Mat4, size: Vec3) -> (Vec3, Vec3) {
    let middle = model.transform_point3(vec3(0., size.y * 0.5, 0.));
    let half = (model.x_axis.truncate().abs() * size.x
        + model.y_axis.truncate().abs() * size.y
        + model.z_axis.truncate().abs() * size.z)
        * 0.5
        + Vec3::splat(0.22);
    (middle - half, middle + half)
}
// A single dynamic page per marked building. Neither unmarked buildings nor
// unchanged snapshots allocate or upload scar vertices.
const MAX_SCAR_QUADS: usize = 48 * 6;
fn scar_geometry(
    cells: &[Cell],
    neighbors: &[[u16; 6]],
    broken: &[bool],
    marks: &Value,
) -> (Vec<Vertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for code in marks
        .as_array()
        .into_iter()
        .flatten()
        .take(48)
        .filter_map(Value::as_u64)
    {
        let id = (code >> 8) as usize;
        let Some(c) = cells.get(id) else { continue };
        let stage = (code & 3) as usize;
        if stage == 0 || broken[id] {
            continue;
        }
        let mut center = Vec3::from_array(c.center);
        let size = Vec3::from_array(c.size);
        let seed = (id as u32).wrapping_mul(53);
        let jitter = vec3(
            random(seed + 1) - 0.5,
            random(seed + 2) - 0.5,
            random(seed + 3) - 0.5,
        ) * 0.12;
        center += jitter * size;
        for (face, axis) in AXIS.iter().enumerate() {
            if code & (4 << face) == 0 {
                continue;
            }
            let next = neighbors[id][face];
            if next != NONE && !broken[next as usize] {
                continue;
            }
            let n = Vec3::new(axis[0] as f32, axis[1] as f32, axis[2] as f32);
            let half = [0., 0.48, 0.72, 0.92][stage];
            let face_center = center - n * (center - Vec3::from_array(c.center)).dot(n) + n * 0.005;
            let extent = size * (Vec3::splat(half) + n.abs() * (1. - half));
            let pts = faces_for(face_center, extent, face);
            let color = if stage == 1 {
                [125, 106, 86]
            } else {
                [89, 72, 57]
            };
            let tile = if (id + face) % 2 == 0 { 25 } else { 29 };
            let uv = [
                [0.005, 0.005],
                [0.995, 0.005],
                [0.995, 0.995],
                [0.005, 0.995],
            ];
            let base = vertices.len() as u16;
            for (i, p) in pts.iter().enumerate() {
                vertices.push(Vertex {
                    p: p.to_array(),
                    n: n.to_array(),
                    uv: uv[(i + (id + face) % 4) % 4],
                    style: [color[0], color[1], color[2], tile],
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    (vertices, indices)
}
struct Building {
    parts: Vec<Part>,
    scar_page: Option<Page>,
    scar_revision: u64,
    neighbors: Vec<[u16; 6]>,
    cells: Vec<Cell>,
    model: Mat4,
    low: Vec3,
    high: Vec3,
    dead: bool,
    revision: u64,
    broken: Vec<bool>,
    lod: bool,
}
impl Building {
    fn new(p: &Value, gpu: &mut dyn mq::RenderingBackend, has_old_lod: bool) -> Option<Self> {
        let kind = p["kind"].as_str()?;
        let w = number(p, "w", 3.2);
        let h = number(p, "h", 1.2);
        let d = number(p, "d", 0.8);
        let variant = p["variant"].as_u64().unwrap_or(0) as u32;
        let layout = Layout::new(kind, w, h, d, variant)?;
        let cells = layout.cells;
        let map: HashMap<[i32; 3], u16> = cells
            .iter()
            .enumerate()
            .map(|(id, c)| (key(c), id as u16))
            .collect();
        let size = vec3(w, h, d);
        let cottage = kind == "house";
        let barn = kind == "barn";
        let mut writer = Writer::new();
        let mut neighbors = Vec::with_capacity(cells.len());
        let mut parts = Vec::new();
        let plaster = if p["style"].as_str() == Some("dispatch") {
            [165, 174, 162, 19]
        } else {
            match variant % 3 {
                0 => [179, 170, 146, 19],
                1 => [195, 184, 158, 19],
                _ => [185, 163, 139, 19],
            }
        };
        let shutter = match variant % 3 {
            0 => [91, 115, 94, 24],
            1 => [95, 112, 123, 24],
            _ => [131, 96, 77, 24],
        };
        for (id, c) in cells.iter().enumerate() {
            if writer.vertices.len() > VERTEX_LIMIT - 800 {
                parts.push(writer.upload(gpu));
            }
            let ix = key(c);
            let id = id as u16;
            let local = Vec3::from_array(c.center);
            let block = Vec3::from_array(c.size);
            let mut adjacent = [NONE; 6];
            let tint = 0.97 + 0.055 * random(id as u32 * 73 + variant);
            for (face, dir) in AXIS.iter().enumerate() {
                let at = std::array::from_fn(|k| ix[k] + dir[k] * 2);
                adjacent[face] = *map.get(&at).unwrap_or(&NONE);
                let outer = match face {
                    0 => local.x + block.x * 0.5 > w * 0.5 - 0.02,
                    1 => local.x - block.x * 0.5 < -w * 0.5 + 0.02,
                    4 => local.z + block.z * 0.5 > d * 0.5 - 0.02,
                    5 => local.z - block.z * 0.5 < -d * 0.5 + 0.02,
                    _ => false,
                };
                let mut style = mat_style(c.material);
                match c.material {
                    Material::Brick if outer && cottage => style = plaster,
                    Material::Brick
                        if !outer
                            && adjacent[face] == NONE
                            && face != 2
                            && face != 3
                            && cottage =>
                    {
                        style = [176, 164, 132, 20]
                    }
                    Material::Wood if barn && outer => {
                        style = if variant % 2 == 1 {
                            [142, 88, 59, 4]
                        } else {
                            [126, 100, 70, 4]
                        }
                    }
                    Material::Tile if face == 2 => {
                        style = if variant % 3 == 0 {
                            [94, 105, 102, 8]
                        } else {
                            [150, 90, 65, 8]
                        }
                    }
                    Material::Tile if face == 3 => style = [91, 72, 48, 3],
                    Material::Metal if barn && face == 2 => style = [107, 116, 111, 21],
                    Material::Stone if kind == "ruin" => style = [127, 125, 112, 7],
                    _ => {}
                }
                writer.quad(
                    faces_for(local, block, face),
                    face,
                    tone(style, tint),
                    id,
                    adjacent[face],
                );
            }
            // Shallow facade details remain attached to their owning masonry cell.
            // They dress the edge of the opening without bridging a traversable door or a window.
            neighbors.push(adjacent);
            if (barn || cottage) && local.y < h * 0.70 {
                for &face in &[0usize, 1, 4, 5] {
                    let dir = AXIS[face];
                    let along_x = face >= 4;
                    let outer = if along_x {
                        dir[2] as f32 * local.z + block.z * 0.5 > d * 0.5 - 0.02
                    } else {
                        dir[0] as f32 * local.x + block.x * 0.5 > w * 0.5 - 0.02
                    };
                    if !outer {
                        continue;
                    }
                    let lip = vec3(
                        dir[0] as f32 * (block.x * 0.5 + 0.021),
                        0.,
                        dir[2] as f32 * (block.z * 0.5 + 0.021),
                    );
                    let span = if along_x { block.x } else { block.z };
                    let dim = |width: f32, height: f32, depth: f32| {
                        if along_x {
                            vec3(width, height, depth)
                        } else {
                            vec3(depth, height, width)
                        }
                    };
                    if adjacent[2] == NONE && local.y < h * 0.44 && local.y > 0.3 {
                        writer.cuboid(
                            local + lip + Vec3::Y * (block.y * 0.5 - 0.052),
                            dim(span + 0.07, 0.10, 0.112),
                            [185, 176, 148, 7],
                            id,
                        );
                    }
                    let underside = local.y - block.y * 0.5;
                    if cottage
                        && adjacent[3] == NONE
                        && (1.6..=3.6).contains(&underside)
                        && matches!(c.material, Material::Brick | Material::Wood)
                    {
                        // Each lintel segment sits within the solid cell above the original opening.
                        writer.cuboid(
                            local + lip - Vec3::Y * (block.y * 0.5 - 0.075),
                            dim(span - 0.012, 0.15, 0.13),
                            [163, 155, 132, 7],
                            id,
                        );
                    }
                    let sides = if along_x { [1, 0] } else { [5, 4] };
                    for (which, other) in sides.into_iter().enumerate() {
                        if adjacent[other] != NONE || local.y < 0.24 || local.y > 3.2 {
                            continue;
                        }
                        let side = if which == 0 { -1. } else { 1. };
                        let tang = if along_x { Vec3::X } else { Vec3::Z };
                        // Narrow shutters and oak door cheeks are mounted on solid wall cells.
                        let width = if cottage { 0.22 } else { 0.096 };
                        let edge = tang * (side * (span * 0.5 - width * 0.57));
                        // A real window has masonry below the opening. Door jambs stay plain timber.
                        let dir = AXIS[other];
                        let below_opening = [ix[0] + 2 * dir[0], 1, ix[2] + 2 * dir[2]];
                        let style = if cottage && map.contains_key(&below_opening) {
                            shutter
                        } else {
                            [97, 71, 47, 3]
                        };
                        writer.cuboid(
                            local + lip + edge,
                            dim(width, block.y * 0.98, 0.055),
                            style,
                            id,
                        );
                    }
                }
            }
            if c.material == Material::Wood && kind == "barricade" && id % 4 == 0 {
                writer.cuboid(
                    local + vec3(0., 0., block.z * 0.51),
                    vec3(block.x * 0.14, block.y, 0.027),
                    [100, 108, 100, 9],
                    id,
                );
            }
        }
        if (cottage || barn || kind == "ruin") && w > 2. && d > 2. {
            let floor_style = if barn {
                [133, 119, 88, 2]
            } else {
                [142, 115, 81, 4]
            };
            writer.quad(
                faces_for(vec3(0., 0.015, 0.), vec3(w, 0.012, d), 2),
                2,
                floor_style,
                NONE,
                NONE,
            );
        }
        if !writer.vertices.is_empty() {
            parts.push(writer.upload(gpu));
        }
        let broken = vec![false; cells.len()];
        for part in &mut parts {
            update_part(part, &broken, gpu);
        }
        let model = prop_model(p, Vec3::ONE);
        let (low, high) = world_bounds(model, size);
        Some(Self {
            parts,
            scar_page: None,
            scar_revision: 0,
            neighbors,
            cells,
            model,
            low,
            high,
            dead: false,
            revision: 0,
            broken,
            lod: has_old_lod && (cottage || barn),
        })
    }
    fn snapshot(&mut self, prop: &Value, gpu: &mut dyn mq::RenderingBackend) -> bool {
        let revision = prop["revision"].as_u64().unwrap_or(0);
        let gone = dead(prop);
        let scar_rev = prop["scarRevision"].as_u64().unwrap_or(0);
        let geometry_changed = self.revision != revision || self.dead != gone;
        if !geometry_changed && self.scar_revision == scar_rev {
            return false;
        }
        self.dead = gone;
        if self.revision != revision {
            self.broken.fill(false);
            for id in prop["broken"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_u64)
            {
                if let Some(b) = self.broken.get_mut(id as usize) {
                    *b = true;
                }
            }
            for part in &mut self.parts {
                update_part(part, &self.broken, gpu);
            }
            self.revision = revision;
        }
        let (vertices, indices) =
            scar_geometry(&self.cells, &self.neighbors, &self.broken, &prop["scars"]);
        if !vertices.is_empty() {
            let page = self.scar_page.get_or_insert_with(|| Page {
                vertices: gpu.new_buffer(
                    mq::BufferType::VertexBuffer,
                    mq::BufferUsage::Dynamic,
                    mq::BufferSource::empty::<Vertex>(MAX_SCAR_QUADS * 4),
                ),
                indices: gpu.new_buffer(
                    mq::BufferType::IndexBuffer,
                    mq::BufferUsage::Dynamic,
                    mq::BufferSource::empty::<u16>(MAX_SCAR_QUADS * 6),
                ),
                count: 0,
                low: self.low,
                high: self.high,
                kind: PageKind::Template,
            });
            gpu.buffer_update(page.vertices, mq::BufferSource::slice(&vertices));
            gpu.buffer_update(page.indices, mq::BufferSource::slice(&indices));
            page.count = indices.len() as i32;
        } else if let Some(page) = &mut self.scar_page {
            page.count = 0;
        }
        self.scar_revision = scar_rev;
        geometry_changed
    }
    fn release(&mut self, gpu: &mut dyn mq::RenderingBackend) {
        for p in &self.parts {
            gpu.delete_buffer(p.page.vertices);
            gpu.delete_buffer(p.page.indices);
        }
        if let Some(p) = self.scar_page.take() {
            gpu.delete_buffer(p.vertices);
            gpu.delete_buffer(p.indices);
        }
    }
}
pub(super) fn barricade_template(size: Vec3, gpu: &mut dyn mq::RenderingBackend) -> Vec<Page> {
    let p = serde_json::json!({"kind":"barricade","w":size.x,"h":size.y,"d":size.z});
    Building::new(&p, gpu, false)
        .expect("valid construction template")
        .parts
        .into_iter()
        .map(|part| part.page)
        .collect()
}

fn random(mut x: u32) -> f32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x45d9f3b);
    x ^= x >> 16;
    x as f32 / u32::MAX as f32
}
fn material_no(m: Material) -> usize {
    match m {
        Material::Wood => 0,
        Material::Brick => 1,
        Material::Stone => 2,
        Material::Metal => 3,
        Material::Tile => 4,
    }
}
struct Chunk {
    pos: Vec3,
    vel: Vec3,
    spin: Vec3,
    rotation: Vec3,
    size: Vec3,
    material: usize,
    age: f32,
    life: f32,
}
pub(super) struct BreakableScenery {
    authored: Vec<Option<Building>>,
    built: Vec<Option<Building>>,
    build_ids: Vec<u64>,
    chunk_meshes: Vec<Part>,
    chunks: Vec<Chunk>,
    round: Option<u64>,
    last_event: Option<u64>,
    last_time: f64,
}
impl BreakableScenery {
    pub(super) fn new(level: &Value, gpu: &mut dyn mq::RenderingBackend) -> Self {
        let authored = level["props"]
            .as_array()
            .expect("map props")
            .iter()
            .map(|p| {
                if supports(p["kind"].as_str().unwrap_or("")) {
                    Building::new(p, gpu, true)
                } else {
                    None
                }
            })
            .collect();
        let mut chunk_meshes = Vec::new();
        for mat in [
            Material::Wood,
            Material::Brick,
            Material::Stone,
            Material::Metal,
            Material::Tile,
        ] {
            let mut w = Writer::new();
            w.cuboid(Vec3::ZERO, Vec3::ONE, mat_style(mat), NONE);
            let mut part = w.upload(gpu);
            update_part(&mut part, &[], gpu);
            chunk_meshes.push(part);
        }
        Self {
            authored,
            built: vec![],
            build_ids: vec![],
            chunk_meshes,
            chunks: vec![],
            round: None,
            last_event: None,
            last_time: 0.,
        }
    }
    pub(super) fn owns(&self, i: usize) -> bool {
        self.authored.get(i).is_some_and(Option::is_some)
    }
    pub(super) fn intact_lod(&self, i: usize) -> bool {
        self.authored
            .get(i)
            .and_then(Option::as_ref)
            .is_some_and(|b| b.lod && b.revision == 0 && !b.dead)
    }
    fn clear_dynamic(&mut self, gpu: &mut dyn mq::RenderingBackend) {
        for b in self.built.iter_mut().flatten() {
            b.release(gpu);
        }
        self.built.clear();
        self.build_ids.clear();
    }
    pub(super) fn sync(
        &mut self,
        state: Option<&Value>,
        gpu: &mut dyn mq::RenderingBackend,
        now: f64,
    ) -> bool {
        let dt = ((now - self.last_time) as f32).clamp(0., 0.08);
        self.last_time = now;
        let round = state.and_then(|s| s["round"].as_u64());
        let mut changed = false;
        if self.round != round {
            self.clear_dynamic(gpu);
            self.chunks.clear();
            self.last_event = None;
            for b in self.authored.iter_mut().flatten() {
                b.snapshot(&Value::Null, gpu);
            }
            self.round = round;
            changed = true;
        }
        if let Some(s) = state {
            for p in s["propChanges"].as_array().into_iter().flatten() {
                if let Some(b) = p["index"]
                    .as_u64()
                    .and_then(|n| self.authored.get_mut(n as usize))
                    .and_then(Option::as_mut)
                {
                    changed |= b.snapshot(p, gpu);
                }
            }
            let built = s["builtProps"].as_array().map(Vec::as_slice).unwrap_or(&[]);
            for (i, p) in built.iter().enumerate() {
                let id = p["buildId"].as_u64().unwrap_or(i as u64 + 1);
                if self.built.len() <= i {
                    self.built.push(None);
                    self.build_ids.push(u64::MAX);
                }
                if self.build_ids[i] != id {
                    if let Some(mut old) = self.built[i].take() {
                        old.release(gpu);
                        changed = true;
                    }
                    self.build_ids[i] = id;
                    self.built[i] = if !dead(p) && supports(p["kind"].as_str().unwrap_or("")) {
                        Building::new(p, gpu, false)
                    } else {
                        None
                    };
                    changed |= self.built[i].is_some();
                }
                if let Some(b) = &mut self.built[i] {
                    changed |= b.snapshot(p, gpu);
                }
            }
            while self.built.len() > built.len() {
                self.build_ids.pop();
                if let Some(mut b) = self.built.pop().flatten() {
                    b.release(gpu);
                    changed = true;
                }
            }
            let events = s["events"].as_array().map(Vec::as_slice).unwrap_or(&[]);
            if let Some(last) = self.last_event {
                for e in events {
                    let id = e["id"].as_u64().unwrap_or(0);
                    if id > last && e["type"].as_str() == Some("fracture") {
                        self.spawn_chunks(e, id);
                    }
                }
            }
            let high = events
                .iter()
                .filter_map(|e| e["id"].as_u64())
                .max()
                .unwrap_or(0);
            self.last_event = Some(self.last_event.unwrap_or(0).max(high));
        }
        for c in &mut self.chunks {
            c.age += dt;
            c.vel.y -= 11.6 * dt;
            c.pos += c.vel * dt;
            c.rotation += c.spin * dt;
            let floor = c.size.y.min(c.size.x) * 0.25;
            if c.pos.y < floor {
                c.pos.y = floor;
                c.vel.y = c.vel.y.abs() * 0.19;
                c.vel.x *= 0.68;
                c.vel.z *= 0.68;
                c.spin *= 0.65;
            }
        }
        self.chunks.retain(|c| c.age < c.life);
        changed
    }
    fn spawn_chunks(&mut self, e: &Value, event_id: u64) {
        let Some(index) = e["index"].as_u64().map(|n| n as usize) else {
            return;
        };
        let b = if index < self.authored.len() {
            self.authored[index].as_ref()
        } else {
            let slot = index - self.authored.len();
            if let Some(requested) = e["buildId"].as_u64() {
                if self.build_ids.get(slot).copied() != Some(requested) {
                    return;
                }
            }
            self.built.get(slot).and_then(Option::as_ref)
        };
        let Some(b) = b else { return };
        let Some(ids) = e["cells"].as_array() else {
            return;
        };
        let center = vec3(number(e, "x", 0.), number(e, "y", 1.), number(e, "z", 0.));
        let capacity = if e["collapsed"].as_bool() == Some(true) {
            46
        } else {
            22
        };
        let step = ids.len().div_ceil(capacity).max(1);
        for n in ids.iter().step_by(step) {
            let Some(i) = n.as_u64() else { continue };
            let Some(c) = b.cells.get(i as usize) else {
                continue;
            };
            let seed = (event_id as u32)
                .wrapping_mul(313)
                .wrapping_add(i as u32 * 97);
            let pos = b.model.transform_point3(Vec3::from_array(c.center));
            let kick = (pos - center).normalize_or_zero();
            let vel = kick * (1.5 + 3.5 * random(seed))
                + vec3(
                    random(seed + 1) - 0.5,
                    1.2 + random(seed + 2) * 2.8,
                    random(seed + 3) - 0.5,
                );
            let fraction = if e["collapsed"].as_bool() == Some(true) {
                0.70
            } else {
                0.47
            };
            let size = Vec3::from_array(c.size) * fraction * (0.68 + 0.56 * random(seed + 4));
            self.chunks.push(Chunk {
                pos,
                vel,
                rotation: Vec3::ZERO,
                spin: vec3(
                    random(seed + 5) - 0.5,
                    random(seed + 6) - 0.5,
                    random(seed + 7) - 0.5,
                ) * 7.,
                size,
                material: material_no(c.material),
                age: 0.,
                life: 2.3 + random(seed + 8) * 1.15,
            });
        }
        if self.chunks.len() > MAX_FLYING {
            let r = self.chunks.len() - MAX_FLYING;
            self.chunks.drain(..r);
        }
    }
    pub(super) fn visit(
        &self,
        planes: &[Vec4; 6],
        eye: Vec3,
        shadow: bool,
        model_range_sq: f32,
        draw: &mut impl FnMut(&Page, Mat4, f32),
    ) {
        for b in self.authored.iter().chain(self.built.iter()).flatten() {
            if b.dead || !in_frustum(planes, b.low, b.high) {
                continue;
            }
            if b.lod
                && b.revision == 0
                && ((b.low + b.high) * 0.5).distance_squared(eye) >= model_range_sq
            {
                continue;
            }
            for part in &b.parts {
                if part.page.count > 0 {
                    draw(&part.page, b.model, 1.);
                }
            }
            if !shadow && ((b.low + b.high) * 0.5).distance_squared(eye) < 65.0_f32.powi(2) {
                if let Some(p) = &b.scar_page {
                    if p.count > 0 {
                        draw(p, b.model, 1.);
                    }
                }
            }
        }
        if !shadow {
            for c in &self.chunks {
                if (c.pos - eye).length_squared() > 115. * 115.
                    || !in_frustum(planes, c.pos - Vec3::ONE, c.pos + Vec3::ONE)
                {
                    continue;
                }
                let m = Mat4::from_scale_rotation_translation(
                    c.size,
                    Quat::from_euler(EulerRot::YXZ, c.rotation.y, c.rotation.x, c.rotation.z),
                    c.pos,
                );
                draw(
                    &self.chunk_meshes[c.material].page,
                    m,
                    ((c.life - c.age) / 0.65).clamp(0., 1.),
                );
            }
        }
    }
}

#[cfg(test)]
mod damage_render_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn marks_only_cover_intact_exposed_faces_and_are_removed_with_their_cell() {
        let cell = Cell {
            center: [0., 0.5, 0.],
            size: [1., 1., 1.],
            material: Material::Brick,
        };
        let cells = [cell.clone(), cell];
        let mut neighbors = [[NONE; 6]; 2];
        neighbors[0][4] = 1;
        let marks = json!([(4 << 4) | (4 << 5) | 2]);
        let (v, i) = scar_geometry(&cells, &neighbors, &[false, false], &marks);
        assert_eq!((v.len(), i.len()), (4, 6));
        assert!(v.iter().all(|v| v.p[2] < -0.5 && v.n == [0., 0., -1.]));
        assert_eq!(
            scar_geometry(&cells, &neighbors, &[false, true], &marks)
                .0
                .len(),
            8
        );
        assert!(scar_geometry(&cells, &neighbors, &[true, false], &marks)
            .0
            .is_empty());
    }
}
