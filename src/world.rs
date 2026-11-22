//! Native village rendering. Geometry is uploaded once into immutable GPU buffers.
//! Fields, buildings and parked vehicles share a texture atlas and a cached shadow map.
#[path = "world_fracture.rs"]
mod fracture_mesh;
#[path = "world_vehicles.rs"]
mod moving;
use fracture_mesh::BreakableScenery;
use macroquad::miniquad as mq;
use macroquad::prelude::*;
use macroquad::window::get_internal_gl;
use serde_json::Value;
use std::collections::HashMap;

const TERRAIN: &[u8] = include_bytes!("../assets/province.frmesh");
const ATLAS: &[u8] = include_bytes!("../assets/terrain-atlas.png");
const LAND_COVER: &[u8] = include_bytes!("../assets/land-cover.png");
const LOD_SWITCH_METERS: f32 = 93.0;
const FOLIAGE_RANGE_METERS: f32 = 88.0;
const SHADOW_SIZE: u32 = 1024;
const VERTEX_BYTES: usize = 36;

struct Page {
    vertices: mq::BufferId,
    indices: mq::BufferId,
    count: i32,
    kind: PageKind,
    low: Vec3,
    high: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PageKind {
    Terrain,
    Backdrop,
    Foliage,
    PropLod(usize),
    Fixtures,
    Prop(usize),
    Template,
}

#[derive(Clone, PartialEq)]
struct Placement {
    page: usize,
    transform: Mat4,
}

/// A bounded view over the bundled, versioned art pack; no native pointer casts.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> &'a [u8] {
        let slice = &self.bytes[self.at..self.at + n];
        self.at += n;
        slice
    }
    fn u32(&mut self) -> usize {
        u32::from_le_bytes(self.take(4).try_into().unwrap()) as usize
    }
}

#[repr(C)]
struct SceneUniforms {
    vp: [f32; 16],
    light: [f32; 16],
    model: [f32; 16],
    eye: [f32; 3],
    land_rect: [f32; 4],
    fade: f32,
    sky_state: [f32; 4],
    sun: [f32; 3],
    lamps: [[f32; 4]; 4],
}

#[repr(C)]
struct SkyUniforms {
    inverse: [f32; 16],
    time: f32,
    sky_state: [f32; 4],
    sun: [f32; 3],
}

#[repr(C)]
struct ShadowUniforms {
    light: [f32; 16],
    model: [f32; 16],
}

#[derive(Clone, Copy)]
struct VehiclePose {
    pos: Vec3,
    yaw: f32,
    turret: f32,
    wreck: bool,
    travel: f32,
    differential: f32,
    steer: f32,
}

pub struct World {
    pages: Vec<Page>,
    templates: HashMap<String, Vec<usize>>,
    hidden: Vec<bool>,
    dynamic: Vec<Placement>,
    has_lod: Vec<bool>,
    breakables: BreakableScenery,
    vehicles: HashMap<u64, VehiclePose>,
    round: Option<u64>,
    scene_pipeline: mq::Pipeline,
    depth_pipeline: mq::Pipeline,
    sky_pipeline: mq::Pipeline,
    sky_bindings: mq::Bindings,
    atlas: Texture2D,
    land_cover: Texture2D,
    land_rect: [f32; 4],
    sky: SkyState,
    solar_slot: i32,
    road_lights: Vec<(usize, Vec3)>,
    lamps: [[f32; 4]; 4],
    shadow_anchor: Vec3,
    last_shadow_at: f64,
    shadow: mq::TextureId,
    shadow_pass: mq::RenderPass,
    lighting: Mat4,
    shadow_dirty: bool,
}

fn shader(
    gpu: &mut dyn mq::RenderingBackend,
    vertex: &str,
    fragment: &str,
    uniforms: &[(&str, mq::UniformType)],
    images: &[&str],
) -> mq::ShaderId {
    let meta = mq::ShaderMeta {
        uniforms: mq::UniformBlockLayout {
            uniforms: uniforms
                .iter()
                .map(|(n, t)| mq::UniformDesc::new(n, *t))
                .collect(),
        },
        images: images.iter().map(|s| s.to_string()).collect(),
    };
    gpu.new_shader(mq::ShaderSource::Glsl { vertex, fragment }, meta)
        .unwrap_or_else(|e| panic!("Village shader: {e}"))
}

impl World {
    /// Create once on the native window's main thread, after graphics initialization.
    pub fn new() -> Self {
        let atlas = Texture2D::from_file_with_format(ATLAS, None);
        atlas.set_filter(FilterMode::Linear);
        let land_cover = Texture2D::from_file_with_format(LAND_COVER, None);
        land_cover.set_filter(FilterMode::Linear);
        // Macroquad owns these handles. All uses of its backend here are scoped to the
        // single render thread; no borrowed context escapes this call.
        let gl = unsafe { get_internal_gl() };
        let gpu = gl.quad_context;
        use mq::UniformType::{Float1, Float3, Float4, Mat4 as Matrix};
        let scene_shader = shader(
            gpu,
            include_str!("shaders/field.vert"),
            include_str!("shaders/field.frag"),
            &[
                ("ViewProjection", Matrix),
                ("Light", Matrix),
                ("Model", Matrix),
                ("Eye", Float3),
                ("LandRect", Float4),
                ("Fade", Float1),
                ("SkyState", Float4),
                ("Sun", Float3),
                ("LampA", Float4),
                ("LampB", Float4),
                ("LampC", Float4),
                ("LampD", Float4),
            ],
            &["Atlas", "ShadowMap", "LandCover"],
        );
        let depth_shader = shader(
            gpu,
            include_str!("shaders/shadow.vert"),
            include_str!("shaders/shadow.frag"),
            &[("Light", Matrix), ("Model", Matrix)],
            &[],
        );
        let sky_shader = shader(
            gpu,
            include_str!("shaders/sky.vert"),
            include_str!("shaders/sky.frag"),
            &[
                ("InverseView", Matrix),
                ("Time", Float1),
                ("SkyState", Float4),
                ("Sun", Float3),
            ],
            &[],
        );
        let layout = mq::BufferLayout {
            stride: VERTEX_BYTES as i32,
            ..Default::default()
        };
        use mq::{VertexAttribute as Attr, VertexFormat as Fmt};
        let attributes = [
            Attr::new("Position", Fmt::Float3),
            Attr::new("Normal", Fmt::Float3),
            Attr::new("Texcoord", Fmt::Float2),
            Attr::new("Style", Fmt::Byte4),
        ];
        let params = mq::PipelineParams {
            depth_test: mq::Comparison::LessOrEqual,
            depth_write: true,
            ..Default::default()
        };
        let scene_pipeline = gpu.new_pipeline(
            std::slice::from_ref(&layout),
            &attributes,
            scene_shader,
            params,
        );
        let depth_pipeline = gpu.new_pipeline(&[layout], &attributes[..1], depth_shader, params);
        let sky_pipeline = gpu.new_pipeline(
            &[mq::BufferLayout::default()],
            &[Attr::new("Position", Fmt::Float2)],
            sky_shader,
            Default::default(),
        );
        let sky_vertices: [[f32; 2]; 3] = [[-1., -1.], [3., -1.], [-1., 3.]];
        let sky_indices: [u16; 3] = [0, 1, 2];
        let sky_bindings = mq::Bindings {
            vertex_buffers: vec![gpu.new_buffer(
                mq::BufferType::VertexBuffer,
                mq::BufferUsage::Immutable,
                mq::BufferSource::slice(&sky_vertices),
            )],
            index_buffer: gpu.new_buffer(
                mq::BufferType::IndexBuffer,
                mq::BufferUsage::Immutable,
                mq::BufferSource::slice(&sky_indices),
            ),
            images: vec![],
        };
        let tex = mq::TextureParams {
            width: SHADOW_SIZE,
            height: SHADOW_SIZE,
            min_filter: mq::FilterMode::Nearest,
            mag_filter: mq::FilterMode::Nearest,
            ..Default::default()
        };
        let shadow = gpu.new_render_texture(tex);
        let shadow_depth = gpu.new_render_texture(mq::TextureParams {
            format: mq::TextureFormat::Depth,
            ..tex
        });
        let shadow_pass = gpu.new_render_pass(shadow, Some(shadow_depth));
        assert_eq!(&TERRAIN[..4], b"FZQ2", "Unknown compressed map");
        let expected = u32::from_le_bytes(TERRAIN[4..8].try_into().unwrap()) as usize;
        let art = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&TERRAIN[8..], expected)
            .expect("Bundled map failed to decompress");
        assert_eq!(art.len(), expected, "Incomplete map");
        let mut reader = Cursor { bytes: &art, at: 0 };
        assert_eq!(reader.take(4), b"FRQ2", "Unknown province mesh pack");
        let mut land_rect = [0.; 4];
        for n in &mut land_rect {
            *n = f32::from_le_bytes(reader.take(4).try_into().unwrap());
        }
        land_rect[2] = 1. / land_rect[2];
        land_rect[3] = 1. / land_rect[3];
        let count = reader.u32();
        let mut pages = Vec::with_capacity(count);
        let mut templates: HashMap<String, Vec<usize>> = HashMap::new();
        let mut prop_count = 0;
        for _ in 0..count {
            let name_len = reader.u32();
            let name = std::str::from_utf8(reader.take(name_len)).unwrap();
            let n = reader.u32();
            let num_indices = reader.u32();
            let data = reader.take(n * VERTEX_BYTES);
            let mut low = Vec3::splat(f32::INFINITY);
            let mut high = Vec3::splat(f32::NEG_INFINITY);
            for v in data.chunks_exact(VERTEX_BYTES) {
                let pos = vec3(
                    f32::from_le_bytes(v[0..4].try_into().unwrap()),
                    f32::from_le_bytes(v[4..8].try_into().unwrap()),
                    f32::from_le_bytes(v[8..12].try_into().unwrap()),
                );
                low = low.min(pos);
                high = high.max(pos);
            }
            let indices: Vec<u16> = reader
                .take(num_indices * 2)
                .chunks_exact(2)
                .map(|v| u16::from_le_bytes([v[0], v[1]]))
                .collect();
            let kind = match name {
                "backdrop" => PageKind::Backdrop,
                s if s.starts_with("flora/") => PageKind::Foliage,
                s if s.starts_with("lod/") => PageKind::PropLod(s[4..].parse().expect("LOD index")),
                "terrain" => PageKind::Terrain,
                "furniture" => PageKind::Fixtures,
                _ => {
                    if let Some(id) = name.strip_prefix("p/") {
                        let index = id.parse::<usize>().unwrap();
                        prop_count = prop_count.max(index + 1);
                        PageKind::Prop(index)
                    } else {
                        let key = name.strip_prefix("local/").expect("Unknown mesh kind");
                        templates
                            .entry(key.to_string())
                            .or_default()
                            .push(pages.len());
                        PageKind::Template
                    }
                }
            };
            pages.push(Page {
                kind,
                low,
                high,
                count: num_indices as i32,
                vertices: gpu.new_buffer(
                    mq::BufferType::VertexBuffer,
                    mq::BufferUsage::Immutable,
                    mq::BufferSource::slice(data),
                ),
                indices: gpu.new_buffer(
                    mq::BufferType::IndexBuffer,
                    mq::BufferUsage::Immutable,
                    mq::BufferSource::slice(&indices),
                ),
            });
        }
        assert_eq!(reader.at, art.len(), "Extra bytes in province mesh pack");
        let sky = SkyState::at(0.);
        let lighting = light_camera(Vec3::ZERO, sky.key_light());
        let level: Value =
            serde_json::from_str(include_str!("../assets/level.json")).expect("Bundled level");
        let props = level["props"].as_array().expect("map props");
        prop_count = prop_count.max(props.len());
        let mut road_lights = Vec::new();
        for (index, p) in props.iter().enumerate() {
            if p["kind"] == "pole" {
                let theta = number(p, "yaw", 0.);
                road_lights.push((
                    index,
                    vec3(
                        number(p, "x", 0.) - 1.38 * theta.cos(),
                        number(p, "h", 6.) - 0.12,
                        number(p, "z", 0.) - 1.38 * theta.sin(),
                    ),
                ));
            }
        }
        let breakables = BreakableScenery::new(&level, gpu);
        let mut has_lod = vec![false; prop_count];
        for p in &pages {
            if let PageKind::PropLod(i) = p.kind {
                has_lod[i] = true;
            }
        }
        Self {
            pages,
            templates,
            hidden: vec![false; prop_count],
            dynamic: Vec::new(),
            has_lod,
            breakables,
            vehicles: HashMap::new(),
            round: None,
            land_cover,
            land_rect,
            sky,
            solar_slot: -1,
            road_lights,
            lamps: [[0.; 4]; 4],
            shadow_anchor: Vec3::splat(f32::INFINITY),
            last_shadow_at: -1.,
            scene_pipeline,
            depth_pipeline,
            sky_pipeline,
            sky_bindings,
            atlas,
            shadow,
            shadow_pass,
            lighting,
            shadow_dirty: true,
        }
    }

    pub fn daylight(&self) -> f32 {
        self.sky.day
    }

    fn place(&self, key: &str, transform: Mat4, out: &mut Vec<Placement>) {
        if let Some(pages) = self.templates.get(key) {
            out.extend(pages.iter().map(|&page| Placement { page, transform }));
        }
    }
    fn place_team_flag(&self, item: &Value, at: Mat4, out: &mut Vec<Placement>) {
        let key = match item["team"].as_str() {
            Some("blue") => "flag_blue",
            Some("red") => "flag_red",
            _ => return,
        };
        self.place(key, at, out);
    }
    fn sync(&mut self, state: Option<&Value>, eye: Vec3) {
        let round = state.and_then(|s| s["round"].as_u64());
        if self.round != round || state.is_none() {
            self.vehicles.clear();
            self.round = round;
        }
        let mut next_hidden = vec![false; self.hidden.len()];
        let mut next_dynamic = Vec::new();
        let mut local_lights = Vec::new();
        if let Some(s) = state {
            if let Some(changes) = s["propChanges"].as_array() {
                for p in changes {
                    if let Some(index) = p["index"].as_u64() {
                        if let Some(hidden) = next_hidden.get_mut(index as usize) {
                            *hidden = dead(p);
                        }
                    }
                }
            }
            if let Some(built) = s["builtProps"].as_array() {
                for p in built {
                    if fracture_mesh::supports(p["kind"].as_str().unwrap_or("")) {
                        continue;
                    }
                    if dead(p) {
                        continue;
                    }
                    let kind = p["kind"].as_str().unwrap_or("sandbag");
                    let wreck_key;
                    let key = if kind == "wreck" {
                        wreck_key = format!("wreck_{}", p["vehicleKind"].as_str().unwrap_or("mbt"));
                        wreck_key.as_str()
                    } else {
                        kind
                    };
                    let Some(indices) = self.templates.get(key) else {
                        continue;
                    };
                    let reference = match kind {
                        "sandbag" => vec3(3.15, 1.02, 0.82),
                        "logs" => vec3(2., 0.8, 1.05),
                        "wall" => vec3(3.2, 1.2, 0.8),
                        "refinery" => vec3(3.4, 3.0, 3.1),
                        "garage" => vec3(4.4, 2.8, 4.2),
                        "workshop" => vec3(3.0, 2.4, 2.2),
                        _ => Vec3::ONE,
                    };
                    let scale = if kind == "wreck" {
                        Vec3::ONE
                    } else {
                        vec3(
                            number(p, "w", reference.x),
                            number(p, "h", reference.y),
                            number(p, "d", reference.z),
                        ) / reference
                    };
                    let transform = Mat4::from_scale_rotation_translation(
                        scale,
                        Quat::from_rotation_y(-number(p, "yaw", 0.)),
                        vec3(number(p, "x", 0.), 0., number(p, "z", 0.)),
                    );
                    for &page in indices {
                        next_dynamic.push(Placement { page, transform });
                    }
                    if matches!(kind, "refinery" | "garage" | "workshop") {
                        self.place_team_flag(
                            p,
                            transform
                                * Mat4::from_translation(vec3(
                                    -reference.x * 0.36,
                                    reference.y * 0.82,
                                    -reference.z * 0.3,
                                )),
                            &mut next_dynamic,
                        );
                    }
                }
            }
        }
        if let Some(s) = state {
            self.place_industry(s, &mut next_dynamic, &mut local_lights);
            if let Some(drops) = s["groundDrops"].as_array() {
                for item in drops {
                    let kind = match item["resource"].as_str() {
                        Some("gunpowder") => "powder",
                        Some("scrap") => "metal",
                        Some(k) => k,
                        None => continue,
                    };
                    let x = number(item, "x", 0.);
                    let z = number(item, "z", 0.);
                    if (vec2(x, z) - vec2(eye.x, eye.z)).length_squared() > 130. * 130. {
                        continue;
                    }
                    let yaw = item["id"].as_u64().unwrap_or(0) as f32 * 1.83;
                    self.place(
                        &format!("drop_{kind}"),
                        Mat4::from_rotation_translation(
                            Quat::from_rotation_y(yaw),
                            vec3(x, 0.04, z),
                        ),
                        &mut next_dynamic,
                    );
                }
            }
        }
        if let Some(vehicles) = state.and_then(|s| s["vehicles"].as_array()) {
            let dt = get_frame_time().clamp(0., 0.10);
            let t = 1. - (-dt * 22.).exp();
            let mut current = std::collections::HashSet::with_capacity(vehicles.len());
            for v in vehicles {
                let Some(id) = v["id"].as_u64() else { continue };
                let key = v["kind"].as_str().unwrap_or("");
                let pos = vec3(number(v, "x", 0.), 0., number(v, "z", 0.));
                let yaw = number(v, "yaw", 0.);
                let turret = number(v, "turretYaw", yaw);
                let wreck = dead(v)
                    || v["destroyed"].as_bool() == Some(true)
                    || v["alive"].as_bool() == Some(false);
                current.insert(id);
                let initial = VehiclePose {
                    pos,
                    yaw,
                    turret,
                    wreck,
                    travel: number(v, "travel", 0.),
                    differential: 0.,
                    steer: number(v, "steering", 0.),
                };
                let pose = self.vehicles.entry(id).or_insert(initial);
                if pose.pos.distance_squared(pos) > 81. || pose.wreck != wreck {
                    *pose = initial;
                } else {
                    let previous = pose.pos;
                    let head = angle_delta(pose.yaw, yaw) * t;
                    pose.pos = pose.pos.lerp(pos, t);
                    pose.yaw += head;
                    pose.turret += angle_delta(pose.turret, turret) * t;
                    pose.differential += head;
                    let along =
                        (pose.pos - previous).dot(vec3(pose.yaw.sin(), 0., -pose.yaw.cos()));
                    if let Some(target) = v["travel"].as_f64() {
                        pose.travel += (target as f32 - pose.travel) * t;
                    } else {
                        pose.travel += along;
                    }
                    pose.steer += (number(v, "steering", 0.) - pose.steer) * t;
                }
                let now = *pose;
                let transform = Mat4::from_rotation_translation(
                    Quat::from_rotation_y(std::f32::consts::PI - now.yaw),
                    now.pos,
                );
                if wreck {
                    self.place(&format!("wreck_{key}"), transform, &mut next_dynamic);
                } else {
                    let team = if v["team"].as_str() == Some("red") {
                        "red"
                    } else {
                        "blue"
                    };
                    self.place(&format!("{key}_{team}"), transform, &mut next_dynamic);
                    if matches!(key, "mbt" | "apc") {
                        let pivot = if key == "mbt" {
                            vec3(0., 1.73, 0.34)
                        } else {
                            vec3(0., 2., 0.35)
                        };
                        let turret = transform
                            * Mat4::from_translation(pivot)
                            * Mat4::from_rotation_y(now.yaw - now.turret);
                        self.place(&format!("{key}_turret_{team}"), turret, &mut next_dynamic);
                    }
                    self.place_drivetrain(key, now, transform, &mut next_dynamic);
                    if v["driver"].is_number() {
                        let front = match key {
                            "mbt" | "apc" => 2.85,
                            "cargo6x6" => 2.55,
                            _ => 1.58,
                        };
                        local_lights
                            .push((transform.transform_point3(vec3(0., 1.09, front + 2.3)), 9.0));
                    }
                    // Installed kits appear on the vehicle at both upgrade levels.
                    let upgrade_level = v["upgradeLevel"].as_u64().unwrap_or(0).min(2);
                    if let Some(kit @ ("mobility" | "utility")) = v["upgrade"].as_str() {
                        for level in 1..=upgrade_level {
                            self.place(
                                &format!("{key}_{kit}_{level}"),
                                transform,
                                &mut next_dynamic,
                            );
                        }
                    }
                    if key == "cargo6x6" {
                        let freight = v["cargo"]
                            .as_object()
                            .map(|bag| bag.values().filter_map(Value::as_u64).sum::<u64>())
                            .unwrap_or(0);
                        for i in 0..((freight + 23) / 24).min(4) {
                            let offset = vec3(
                                if i % 2 == 0 { -0.45 } else { 0.45 },
                                1.08,
                                -2.23 + (i / 2) as f32 * 0.73,
                            );
                            self.place(
                                "freight",
                                transform * Mat4::from_translation(offset),
                                &mut next_dynamic,
                            );
                        }
                    }
                    let flag_pos = match key {
                        "mbt" => vec3(-0.86, 1.47, -1.96),
                        "patrol4x4" => vec3(-0.61, 0.8, -1.23),
                        _ => vec3(-0.9, 1.68, -2.53),
                    };
                    self.place_team_flag(
                        v,
                        transform * Mat4::from_translation(flag_pos),
                        &mut next_dynamic,
                    );
                }
            }
            self.vehicles.retain(|id, _| current.contains(id));
        }
        local_lights.extend(
            self.road_lights
                .iter()
                .filter(|(index, _)| !next_hidden[*index])
                .map(|(_, v)| (*v, 14.)),
        );
        local_lights
            .sort_by(|(a, _), (b, _)| a.distance_squared(eye).total_cmp(&b.distance_squared(eye)));
        self.lamps = [[0.; 4]; 4];
        for (target, (position, radius)) in self.lamps.iter_mut().zip(local_lights) {
            *target = [position.x, position.y, position.z, radius];
        }
        if self.hidden != next_hidden || self.dynamic != next_dynamic {
            self.shadow_dirty = true;
            self.hidden = next_hidden;
            self.dynamic = next_dynamic;
        }
    }

    fn submit(&self, gpu: &mut dyn mq::RenderingBackend, vp: Mat4, eye: Vec3, shadow_pass: bool) {
        let texture_set = [
            self.atlas.raw_miniquad_id(),
            self.shadow,
            self.land_cover.raw_miniquad_id(),
        ];
        let samples: &[mq::TextureId] = if shadow_pass { &[] } else { &texture_set };
        let mut last_model = None;
        let mut draw = |p: &Page, model: Mat4, fade: f32| {
            if last_model != Some((model, fade)) {
                if shadow_pass {
                    gpu.apply_uniforms(mq::UniformsSource::table(&ShadowUniforms {
                        light: self.lighting.to_cols_array(),
                        model: model.to_cols_array(),
                    }));
                } else {
                    gpu.apply_uniforms(mq::UniformsSource::table(&SceneUniforms {
                        vp: vp.to_cols_array(),
                        light: self.lighting.to_cols_array(),
                        model: model.to_cols_array(),
                        eye: eye.to_array(),
                        land_rect: self.land_rect,
                        fade,
                        sky_state: self.sky.uniforms(),
                        sun: self.sky.sun.to_array(),
                        lamps: self.lamps,
                    }));
                }
                last_model = Some((model, fade));
            }
            gpu.apply_bindings_from_slice(&[p.vertices], p.indices, samples);
            gpu.draw(0, p.count, 1);
        };
        let planes = frustum(if shadow_pass { self.lighting } else { vp });
        for p in &self.pages {
            let center = (p.low + p.high) * 0.5;
            let distance = center.distance_squared(eye);
            match p.kind {
                PageKind::Prop(i) if self.breakables.owns(i) => continue,
                PageKind::PropLod(i)
                    if self.breakables.owns(i) && !self.breakables.intact_lod(i) =>
                {
                    continue
                }
                PageKind::Template => continue,
                PageKind::Terrain | PageKind::Foliage | PageKind::Backdrop if shadow_pass => {
                    continue
                }
                PageKind::Foliage if distance > FOLIAGE_RANGE_METERS * FOLIAGE_RANGE_METERS => {
                    continue
                }
                PageKind::Prop(i)
                    if self.hidden[i]
                        || (self.has_lod[i]
                            && distance >= LOD_SWITCH_METERS * LOD_SWITCH_METERS) =>
                {
                    continue
                }
                PageKind::PropLod(i)
                    if self.hidden[i] || distance < LOD_SWITCH_METERS * LOD_SWITCH_METERS =>
                {
                    continue
                }
                _ => {}
            }
            if !in_frustum(&planes, p.low, p.high) {
                continue;
            }
            draw(p, Mat4::IDENTITY, 1.);
        }
        self.breakables.visit(&planes, eye, shadow_pass, &mut draw);
        for placed in &self.dynamic {
            let p = &self.pages[placed.page];
            let center = placed.transform.transform_point3((p.low + p.high) * 0.5);
            let radius = (p.high - p.low).length()
                * 0.5
                * placed
                    .transform
                    .x_axis
                    .truncate()
                    .length()
                    .max(placed.transform.z_axis.truncate().length())
                    .max(1.);
            let margin = Vec3::splat(radius);
            if in_frustum(&planes, center - margin, center + margin) {
                draw(p, placed.transform, 1.);
            }
        }
    }

    /// Draw while a Macroquad 3D camera is active. The following actor/weapon renderer
    /// can use normal Macroquad drawing calls; this pass shares its existing depth buffer.
    pub fn draw(&mut self, state: Option<&Value>, build_preview: Option<&Value>, camera_pos: Vec3) {
        let elapsed = state.map_or(0., |s| number(s, "elapsed", 0.));
        self.sky = SkyState::at(elapsed);
        self.sync(state, camera_pos);
        let tick = get_time();
        // Keep fine shadow texels near the active camera even at the remote motor pools.
        // Snapping to metres avoids shimmering whilst walking and limits map refresh work.
        let anchor = vec3(
            (camera_pos.x / 8.).round() * 8.,
            0.,
            (camera_pos.z / 8.).round() * 8.,
        );
        let moved = anchor != self.shadow_anchor;
        // One shadow orientation per four server seconds. Sunlight, moonlight and sky
        // remain continuous between cache updates.
        let slot = (elapsed / 4.).floor() as i32;
        if moved || slot != self.solar_slot {
            self.solar_slot = slot;
            self.lighting = light_camera(anchor, self.sky.key_light());
            self.shadow_anchor = anchor;
            self.shadow_dirty = true;
        }
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            let vp = gl.quad_gl.get_projection_matrix();
            let target = gl.quad_gl.get_active_render_pass();
            let gpu = gl.quad_context;
            if self.breakables.sync(state, gpu, tick) {
                self.shadow_dirty = true;
            }
            if self.shadow_dirty && (moved || tick - self.last_shadow_at >= 0.095) {
                gpu.begin_pass(
                    Some(self.shadow_pass),
                    mq::PassAction::clear_color(1., 1., 1., 1.),
                );
                gpu.apply_pipeline(&self.depth_pipeline);
                self.submit(gpu, vp, camera_pos, true);
                gpu.end_render_pass();
                self.shadow_dirty = false;
                self.last_shadow_at = tick;
            }
            gpu.begin_pass(target, mq::PassAction::Nothing);
            gpu.apply_pipeline(&self.sky_pipeline);
            gpu.apply_bindings(&self.sky_bindings);
            gpu.apply_uniforms(mq::UniformsSource::table(&SkyUniforms {
                inverse: vp.inverse().to_cols_array(),
                time: (elapsed % 10000.) + (get_time() % 1.) as f32 * 0.01,
                sky_state: self.sky.uniforms(),
                sun: self.sky.sun.to_array(),
            }));
            gpu.draw(0, 3, 1);
            gpu.apply_pipeline(&self.scene_pipeline);
            self.submit(gpu, vp, camera_pos, false);
            gpu.end_render_pass();
        }
        moving::draw_shells(state, camera_pos);
        if let Some(p) = build_preview.filter(|p| p.get("x").is_some()) {
            draw_footprint(p);
        }
    }
}

fn angle_delta(from: f32, to: f32) -> f32 {
    (to - from).sin().atan2((to - from).cos())
}
struct SkyState {
    sun: Vec3,
    day: f32,
    twilight: f32,
    moon: f32,
}
impl SkyState {
    fn at(elapsed: f32) -> Self {
        // A synchronized 24-minute solar day, starting at 09:00 local time.
        let angle = (elapsed / 1440. + 9. / 24. - 0.5) * std::f32::consts::TAU;
        let sun = vec3(
            -angle.sin(),
            angle.cos() * 0.77 - 0.065,
            -0.39 - angle.cos() * 0.18,
        )
        .normalize();
        fn smooth(lo: f32, hi: f32, at: f32) -> f32 {
            let x = ((at - lo) / (hi - lo)).clamp(0., 1.);
            x * x * (3. - 2. * x)
        }
        let day = smooth(-0.12, 0.42, sun.y);
        let twilight = 1. - smooth(0.03, 0.45, (sun.y - 0.005).abs());
        let moon = 1. - smooth(-0.25, 0.10, sun.y);
        Self {
            sun,
            day,
            twilight,
            moon,
        }
    }
    fn key_light(&self) -> Vec3 {
        let s = if self.sun.y >= 0. {
            self.sun
        } else {
            -self.sun
        };
        vec3(s.x, s.y.max(0.14), s.z).normalize()
    }
    fn uniforms(&self) -> [f32; 4] {
        [self.day, self.twilight, self.moon, 0.]
    }
}
fn light_camera(anchor: Vec3, sun: Vec3) -> Mat4 {
    Mat4::orthographic_rh_gl(-69., 69., -69., 69., 0.1, 220.)
        * Mat4::look_at_rh(anchor + sun * 104., anchor, Vec3::Y)
}
fn number(p: &Value, key: &str, default: f32) -> f32 {
    p.get(key)
        .and_then(Value::as_f64)
        .map(|v| v as f32)
        .unwrap_or(default)
}
fn dead(p: &Value) -> bool {
    p.get("dead").and_then(Value::as_bool).unwrap_or(false)
        || p.get("hp").and_then(Value::as_f64).is_some_and(|n| n <= 0.)
}
fn frustum(matrix: Mat4) -> [Vec4; 6] {
    let t = matrix.transpose();
    [
        t.w_axis + t.x_axis,
        t.w_axis - t.x_axis,
        t.w_axis + t.y_axis,
        t.w_axis - t.y_axis,
        t.w_axis + t.z_axis,
        t.w_axis - t.z_axis,
    ]
}
fn in_frustum(planes: &[Vec4; 6], low: Vec3, high: Vec3) -> bool {
    planes.iter().all(|p| {
        let x = if p.x >= 0. { high.x } else { low.x };
        let y = if p.y >= 0. { high.y } else { low.y };
        let z = if p.z >= 0. { high.z } else { low.z };
        p.x * x + p.y * y + p.z * z + p.w >= -0.06
    })
}
fn draw_footprint(p: &Value) {
    let center = vec3(number(p, "x", 0.), 0.035, number(p, "z", 0.));
    let rot = Quat::from_rotation_y(-number(p, "yaw", 0.));
    let width = number(p, "w", 3.15) * 0.5;
    let height = number(p, "h", 1.02);
    let depth = number(p, "d", 0.82) * 0.5;
    let color = if p["valid"].as_bool().unwrap_or(false) {
        Color::from_rgba(147, 235, 184, 255)
    } else {
        Color::from_rgba(246, 119, 86, 255)
    };
    let at = |x: f32, y: f32, z: f32| center + rot * vec3(x, y, z);
    for x in [-width, width] {
        for z in [-depth, depth] {
            draw_line_3d(at(x, 0., z), at(x, height, z), color);
        }
    }
    for y in [0., height] {
        for x in [-width, width] {
            draw_line_3d(at(x, y, -depth), at(x, y, depth), color);
        }
        for z in [-depth, depth] {
            draw_line_3d(at(-width, y, z), at(width, y, z), color);
        }
    }
}
