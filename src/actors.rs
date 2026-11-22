//! Native first-person arms, articulated infantry and short-lived match effects.
use macroquad::camera::Camera;
use macroquad::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::io::{Cursor, Read};

const RIG: &[u8] = include_bytes!("../assets/field-actors.fra");
const CLOTH: &[u8] = include_bytes!("../assets/field-fabric.png");

fn num(value: &Value, key: &str, default: f32) -> f32 {
    value
        .get(key)
        .and_then(Value::as_f64)
        .map(|v| v as f32)
        .unwrap_or(default)
}
fn yes(value: &Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}
fn id_of(v: &Value) -> u64 {
    v["id"].as_u64().unwrap_or(0)
}
fn pos(p: &Value) -> Vec3 {
    vec3(num(p, "x", 0.), num(p, "y", 0.), num(p, "z", 0.))
}
fn dest(p: &Value) -> Vec3 {
    vec3(
        num(p, "ex", num(p, "x", 0.)),
        num(p, "ey", num(p, "y", 0.)),
        num(p, "ez", num(p, "z", 0.)),
    )
}
fn alive(p: &Value) -> bool {
    yes(p, "alive", true) && num(p, "hp", 100.) > 0.
}
fn translate(pos: Vec3) -> Mat4 {
    Mat4::from_translation(pos)
}
fn rx(v: f32) -> Mat4 {
    Mat4::from_rotation_x(v)
}
fn ry(v: f32) -> Mat4 {
    Mat4::from_rotation_y(v)
}
fn rz(v: f32) -> Mat4 {
    Mat4::from_rotation_z(v)
}
fn step(old: f32, to: f32, dt: f32, speed: f32) -> f32 {
    old + (to - old) * (1. - (-dt * speed).exp())
}
fn smooth(x: f32) -> f32 {
    let t = x.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn shortest_angle(from: f32, to: f32) -> f32 {
    (to - from + PI).rem_euclid(TAU) - PI
}

struct Rig {
    parts: HashMap<String, Mesh>,
    original: HashMap<String, Vec<[u8; 4]>>,
    light_level: u8,
}
fn read_array<const N: usize>(reader: &mut Cursor<&[u8]>) -> [u8; N] {
    let mut b = [0; N];
    reader
        .read_exact(&mut b)
        .expect("incomplete bundled infantry rig");
    b
}
impl Rig {
    fn load() -> Self {
        let texture = Texture2D::from_file_with_format(CLOTH, Some(ImageFormat::Png));
        texture.set_filter(FilterMode::Linear);
        let mut in_file = Cursor::new(RIG);
        assert_eq!(&read_array::<4>(&mut in_file), b"FRA1");
        let count = u16::from_le_bytes(read_array(&mut in_file));
        let mut parts = HashMap::new();
        for _ in 0..count {
            let mut name = vec![0; read_array::<1>(&mut in_file)[0] as usize];
            in_file.read_exact(&mut name).expect("rig name");
            let name = String::from_utf8(name).expect("rig name encoding");
            let n = u32::from_le_bytes(read_array(&mut in_file)) as usize;
            let triangles = u32::from_le_bytes(read_array(&mut in_file)) as usize;
            let mut vertices = Vec::with_capacity(n);
            for _ in 0..n {
                let x = f32::from_le_bytes(read_array(&mut in_file));
                let y = f32::from_le_bytes(read_array(&mut in_file));
                let z = f32::from_le_bytes(read_array(&mut in_file));
                let [r, g, b, tile] = read_array(&mut in_file);
                let u = u16::from_le_bytes(read_array(&mut in_file)) as f32 / 65535.;
                let v = u16::from_le_bytes(read_array(&mut in_file)) as f32 / 65535.;
                let uv = vec2(
                    (tile as f32 * 64. + 0.5 + 63. * u) / 256.,
                    (0.5 + 63. * v) / 64.,
                );
                vertices.push(Vertex::new2(
                    vec3(x, y, z),
                    uv,
                    Color::from_rgba(r, g, b, 255),
                ));
            }
            let mut indices = Vec::with_capacity(triangles);
            for _ in 0..triangles {
                indices.push(u16::from_le_bytes(read_array(&mut in_file)));
            }
            parts.insert(
                name,
                Mesh {
                    vertices,
                    indices,
                    texture: Some(texture.clone()),
                },
            );
        }
        let original = parts
            .iter()
            .map(|(name, m)| (name.clone(), m.vertices.iter().map(|v| v.color).collect()))
            .collect();
        Self {
            parts,
            original,
            light_level: 48,
        }
    }
    fn set_daylight(&mut self, light: f32) {
        let level = (light.clamp(0., 1.) * 48.).round() as u8;
        if level == self.light_level {
            return;
        }
        self.light_level = level;
        let d = level as f32 / 48.;
        let gains = [0.39 + 0.61 * d, 0.47 + 0.53 * d, 0.62 + 0.38 * d];
        for (name, mesh) in &mut self.parts {
            if name == "flash" || name.ends_with("/mark") {
                continue;
            }
            for (v, base) in mesh.vertices.iter_mut().zip(&self.original[name]) {
                for i in 0..3 {
                    v.color[i] = (base[i] as f32 * gains[i]).round() as u8;
                }
            }
        }
    }
    fn draw(&self, key: &str, transform: Mat4) {
        if let Some(mesh) = self.parts.get(key) {
            with_transform(transform, || draw_mesh(mesh));
        }
    }
}

// Macroquad's public draw_mesh uses the current model matrix. Push/pop each joint
// on the main render thread, leaving the caller's camera and matrix stack intact.
fn with_transform(transform: Mat4, f: impl FnOnce()) {
    unsafe {
        get_internal_gl().quad_gl.push_model_matrix(transform);
    }
    f();
    unsafe {
        get_internal_gl().quad_gl.pop_model_matrix();
    }
}

#[derive(Clone)]
struct Remote {
    location: Vec3,
    yaw: f32,
    phase: f32,
    movement: f32,
    hp: f32,
    down: f32,
    down_sign: f32,
    hurt: f32,
}
impl Remote {
    fn new(p: &Value) -> Self {
        Self {
            location: pos(p),
            yaw: num(p, "yaw", 0.),
            phase: 0.,
            movement: 0.,
            hp: num(p, "hp", 100.),
            down: 0.,
            down_sign: if id_of(p) % 2 == 0 { 1. } else { -1. },
            hurt: 0.,
        }
    }
    fn update(&mut self, p: &Value, dt: f32) {
        let target = pos(p);
        let yaw = num(p, "yaw", 0.);
        let hp = num(p, "hp", 100.);
        if hp < self.hp {
            self.hurt = 1.;
        }
        if alive(p) && (self.hp <= 0. || self.location.distance_squared(target) > 16.) {
            self.location = target;
            self.yaw = yaw;
            self.down = 0.;
        }
        self.hp = hp;
        self.hurt = (self.hurt - dt * 4.5).max(0.);
        if !alive(p) {
            self.down = (self.down + dt * 1.25).min(1.);
            return;
        }
        self.down = 0.;
        self.location = self.location.lerp(target, 1. - (-18. * dt).exp());
        self.yaw += shortest_angle(self.yaw, yaw) * (1. - (-18. * dt).exp());
        self.phase += dt * if yes(p, "sprinting", false) { 11. } else { 8.1 };
        self.movement = step(
            self.movement,
            if yes(p, "moving", false) { 1. } else { 0. },
            dt,
            10.,
        );
    }
}

/// Each model has its own sight line, hand placement and feed animation.
#[derive(Clone, Copy)]
struct Gun {
    body: &'static str,
    slide: &'static str,
    feed: &'static str,
    feed_position: Vec3,
    muzzle: Vec3,
    sight: f32,
    optic: &'static str,
    grip: Option<&'static str>,
    kick: f32,
}
impl Gun {
    fn equipped(p: &Value) -> Self {
        let mut gun = match p["weapon"].as_str() {
            Some("vx9") => Self {
                body: "vx9",
                slide: "vx9_slide",
                feed: "vx9_mag",
                feed_position: vec3(0., -0.075, -0.117),
                muzzle: vec3(0., 0.036, -0.553),
                sight: 0.127,
                optic: "optic_irons",
                grip: None,
                kick: 0.67,
            },
            Some("ar4") => Self {
                body: "ar4",
                slide: "ar4_slide",
                feed: "ar4_mag",
                feed_position: vec3(0., -0.067, -0.088),
                muzzle: vec3(0., 0.040, -0.688),
                sight: 0.151,
                optic: "optic_reflex",
                grip: None,
                kick: 0.58,
            },
            _ => Self {
                body: "mr7",
                slide: "mr7_slide",
                feed: "mr7_mag",
                feed_position: vec3(0., -0.067, -0.098),
                muzzle: vec3(0., 0.039, -0.913),
                sight: 0.176,
                optic: "optic_scope",
                grip: None,
                kick: 1.,
            },
        };
        (gun.optic, gun.sight) = match p["optic"].as_str() {
            Some("irons") => ("optic_irons", 0.127),
            Some("reflex") => ("optic_reflex", 0.151),
            Some("scope") => ("optic_scope", 0.176),
            _ => (gun.optic, gun.sight),
        };
        gun.grip = match p["grip"].as_str() {
            Some("light") => Some("grip_light"),
            Some("stable") => Some("grip_stable"),
            _ => None,
        };
        gun
    }
    fn draw(&self, rig: &Rig, transform: Mat4, cycle: f32, reload_phase: f32) {
        rig.draw(self.body, transform);
        rig.draw(self.optic, transform);
        if let Some(grip) = self.grip {
            rig.draw(grip, transform);
        }

        let cycling = if self.body == "vx9" { 0.083 } else { 0.058 };
        rig.draw(
            self.slide,
            transform * translate(vec3(0., 0., cycling * cycle)),
        );
        // The spent magazine clears the body, then a replacement rises into its seat.
        let (drop, turn) = if reload_phase < 0.43 {
            let t = smooth((reload_phase - 0.09) / 0.34);
            (t, t * 0.28)
        } else {
            let t = 1. - smooth((reload_phase - 0.43) / 0.38);
            (t, -0.38 * t)
        };
        let travel = match self.body {
            "vx9" => 0.29,
            "mr7" => 0.19,
            _ => 0.22,
        };
        let feed = self.feed_position + vec3(-drop * 0.085, -travel * drop, -drop * 0.052);
        rig.draw(
            self.feed,
            transform * translate(feed) * rz(turn) * rx(drop * 0.21),
        );
    }
}
fn seated(p: &Value) -> bool {
    p["vehicleId"].is_number()
}

fn draw_remote(rig: &Rig, r: &Remote, p: &Value, friend: bool) {
    if r.down >= 0.99 || seated(p) {
        return;
    }
    let prefix = if friend { "a" } else { "e" };
    let parts = if prefix == "a" {
        ["a/torso", "a/arms", "a/mark", "a/hip", "a/shin"]
    } else {
        ["e/torso", "e/arms", "e/mark", "e/hip", "e/shin"]
    };
    let swing = r.phase.sin() * r.movement;
    let fall = smooth(r.down / 0.77);
    let body = translate(r.location + vec3(0., 0.031 * swing.abs() - 0.33 * fall, 0.))
        * ry(-r.yaw)
        * rz(r.down_sign * 1.36 * fall + r.hurt * 0.09)
        * rx(0.24 * fall - 0.030 * r.movement);
    rig.draw(parts[0], body);
    let recoil = num(p, "recoil", 0.);
    let pilot = p["droneId"].is_number();
    let arms = body
        * translate(vec3(0., 1.325, 0.))
        * rx(if pilot {
            -0.67
        } else {
            num(p, "pitch", 0.).clamp(-0.96, 0.96) + 0.085 * recoil
        });
    rig.draw(parts[1], arms);
    let gun = Gun::equipped(p);
    let held = arms
        * translate(vec3(0.056, 0.005, -0.313))
        * ry(0.315)
        * Mat4::from_scale(Vec3::splat(0.76));
    let reload = num(p, "reload", 0.);
    let phase = if reload > 0. {
        1. - (reload / num(p, "reloadTime", 2.).max(0.01)).clamp(0., 1.)
    } else {
        0.
    };
    gun.draw(rig, held, recoil, phase);
    if r.down == 0. {
        rig.draw(parts[2], body * translate(vec3(0., 2.05, 0.)));
        if recoil > 0.80 {
            rig.draw("flash", held * translate(gun.muzzle));
        }
    }
    for side in [-1., 1.] {
        let motion = swing * side;
        let hip = body * translate(vec3(side * 0.121, 0.92, 0.)) * rx(0.48 * motion);
        rig.draw(parts[3], hip);
        rig.draw(
            parts[4],
            hip * translate(vec3(0., -0.421, 0.)) * rx(-(-0.45 * motion).max(0.)),
        );
    }
}

struct Burst {
    origin: Vec3,
    age: f32,
    scale: f32,
    seed: f32,
}

/// A soft textured billboard written once. Transparent edges blend into the map;
/// the dark core briefly obscures the impact point without covering a whole street.
fn smoke_disc() -> Mesh {
    let mut vertices = vec![Vertex::new2(Vec3::ZERO, Vec2::ZERO, WHITE)];
    let mut indices = Vec::new();
    for layer in 0..2 {
        for i in 0..10 {
            let angle = TAU * i as f32 / 10.;
            let radius = if layer == 0 { 0.46 } else { 1. };
            let alpha = if layer == 0 { 0.65 } else { 0. };
            vertices.push(Vertex::new2(
                vec3(angle.cos() * radius, angle.sin() * radius, 0.),
                Vec2::ZERO,
                Color::new(1., 1., 1., alpha),
            ));
            let a = 1 + i as u16;
            let b = 1 + ((i + 1) % 10) as u16;
            if layer == 0 {
                indices.extend_from_slice(&[0, a, b]);
            } else {
                indices.extend_from_slice(&[a, a + 10, b, b, a + 10, b + 10]);
            }
        }
    }
    Mesh {
        vertices,
        indices,
        texture: None,
    }
}

fn smoke_puff(mesh: &mut Mesh, at: Vec3, right: Vec3, up: Vec3, size: f32, tint: Color) {
    // Preserve the radial opacity profile when changing colors.
    for (i, v) in mesh.vertices.iter_mut().enumerate() {
        let opacity = if i == 0 {
            1.
        } else if i < 11 {
            0.65
        } else {
            0.
        };
        v.color = Color::new(tint.r, tint.g, tint.b, tint.a * opacity).into();
    }
    let rot = Mat4::from_cols(
        (right * size).extend(0.),
        (up * size).extend(0.),
        right.cross(up).extend(0.),
        at.extend(1.),
    );
    with_transform(rot, || draw_mesh(mesh));
}
fn draw_grenade_at(p: Vec3, age: f32, remaining: f32) {
    let spin = age * 8.;
    with_transform(translate(p) * rz(spin * 0.73) * rx(spin), || {
        with_transform(Mat4::from_scale(vec3(1., 1.20, 1.)), || {
            draw_sphere_ex(
                Vec3::ZERO,
                0.092,
                None,
                Color::from_rgba(85, 94, 66, 255),
                DrawSphereParams {
                    rings: 4,
                    slices: 8,
                    ..Default::default()
                },
            )
        });
        draw_cube(
            vec3(0., 0.099, 0.),
            vec3(0.071, 0.037, 0.077),
            None,
            Color::from_rgba(128, 124, 96, 255),
        );
        draw_cube(
            vec3(0.048, 0.061, 0.),
            vec3(0.026, 0.091, 0.039),
            None,
            Color::from_rgba(91, 93, 79, 255),
        );
        let light = if remaining < 0.65 && (age * 18.).cos() > -0.2 {
            Color::from_rgba(255, 148, 80, 255)
        } else {
            Color::from_rgba(187, 167, 99, 255)
        };
        draw_cylinder(vec3(0., 0.019, 0.), 0.090, 0.092, 0.021, None, light);
    });
}

struct Tracer {
    a: Vec3,
    b: Vec3,
    life: f32,
    width: f32,
    color: Color,
}
struct Chip {
    pos: Vec3,
    velocity: Vec3,
    spin: Vec3,
    life: f32,
    total: f32,
    size: Vec3,
    color: Color,
    dust: bool,
}
struct Rng(u32);
impl Rng {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / 16777216.
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
}

// Preserve the complete scene's color while reserving the closest depth band for the rifle.
struct SightCamera(Mat4);
impl Camera for SightCamera {
    fn matrix(&self) -> Mat4 {
        self.0
    }
    fn depth_enabled(&self) -> bool {
        true
    }
    fn render_pass(&self) -> Option<RenderPass> {
        None
    }
    fn viewport(&self) -> Option<(i32, i32, i32, i32)> {
        None
    }
}

pub struct Actors {
    rig: Rig,
    remote: HashMap<u64, Remote>,
    tracers: Vec<Tracer>,
    chips: Vec<Chip>,
    bursts: Vec<Burst>,
    smoke: Mesh,
    grenade_positions: HashMap<u64, Vec3>,
    throw_time: f32,
    last_event: Option<u64>,
    round: Option<i64>,
    clock: f32,
    aim: f32,
    movement: f32,
    kick: f32,
    rng: Rng,
}
impl Actors {
    pub fn new() -> Self {
        Self {
            rig: Rig::load(),
            remote: HashMap::new(),
            tracers: Vec::new(),
            chips: Vec::new(),
            bursts: Vec::with_capacity(8),
            smoke: smoke_disc(),
            grenade_positions: HashMap::new(),
            throw_time: 0.,
            last_event: None,
            round: None,
            clock: 0.,
            aim: 0.,
            movement: 0.,
            kick: 0.,
            rng: Rng(0x762a_19c5),
        }
    }
    pub fn set_daylight(&mut self, value: f32) {
        self.rig.set_daylight(value);
    }
    pub fn clear(&mut self) {
        self.remote.clear();
        self.tracers.clear();
        self.chips.clear();
        self.bursts.clear();
        self.grenade_positions.clear();
        self.throw_time = 0.;
        self.last_event = None;
        self.round = None;
        self.clock = 0.;
        self.aim = 0.;
        self.movement = 0.;
        self.kick = 0.;
    }
    pub fn draw_world(&mut self, state: &Value, local: &Value, dt: f32) {
        let dt = dt.clamp(0., 0.12);
        let round = state["round"].as_i64();
        if self.round.is_some() && round != self.round {
            self.clear()
        }
        self.round = round;
        self.clock += dt;
        self.throw_time = (self.throw_time - dt).max(0.);
        let uid = id_of(local);
        if let Some(list) = state["players"].as_array() {
            self.remote
                .retain(|id, _| list.iter().any(|p| id_of(p) == *id));
            for p in list {
                if id_of(p) == uid
                    && !local["droneId"].is_number()
                    && local["freeCamera"].as_bool() != Some(true)
                {
                    continue;
                }
                let r = self
                    .remote
                    .entry(id_of(p))
                    .or_insert_with(|| Remote::new(p));
                r.update(p, dt);
                draw_remote(&self.rig, r, p, p["team"] == local["team"]);
            }
        }
        if let Some(events) = state["events"].as_array() {
            if self.last_event.is_none() {
                self.last_event = Some(events.iter().map(id_of).max().unwrap_or(0));
            }
            for event in events {
                let id = id_of(event);
                if id <= self.last_event.unwrap_or(0) {
                    continue;
                }
                self.last_event = Some(id);
                self.event(event, uid);
            }
        }
        if let Some(list) = state["grenades"].as_array() {
            self.grenade_positions
                .retain(|id, _| list.iter().any(|p| id_of(p) == *id));
            for g in list {
                let gp = pos(g);
                let location = self.grenade_positions.entry(id_of(g)).or_insert(gp);
                *location = location.lerp(gp, 1. - (-48. * dt).exp());
                let left = num(g, "remaining", 1.8);
                draw_grenade_at(*location, num(g, "total", 1.8) - left, left);
            }
        }
        self.draw_effects(dt);
        self.draw_bursts(pos(local) + Vec3::Y * 1.67, dt);
    }
    fn event(&mut self, e: &Value, uid: u64) {
        match e["type"].as_str().unwrap_or("") {
            "shot" => {
                let mine = e["actor"].as_u64() == Some(uid);
                if mine {
                    self.kick = 1.;
                }
                let a = pos(e);
                let b = dest(e);
                let delta = b - a;
                let dist = delta.length();
                let tank = matches!(e["weapon"].as_str(), Some("tank" | "mbt"));
                if dist > 0.55 {
                    if tank {
                        self.add_burst(a + delta / dist * 3.45, 0.34);
                    }
                    let len = (dist * 0.71).min(if tank { 8.4 } else { 4.1 });
                    self.tracers.push(Tracer {
                        a: b - delta / dist * len,
                        b,
                        life: if tank { 0.105 } else { 0.065 },
                        width: if tank { 0.055 } else { 0.017 },
                        color: if mine {
                            Color::from_rgba(255, 211, 133, 255)
                        } else {
                            Color::from_rgba(250, 165, 101, 255)
                        },
                    });
                }
                if let Some(material) = e["impactMaterial"].as_str() {
                    self.scatter(b, material, true);
                    if tank {
                        self.add_burst(b, 0.67);
                    }
                }
            }
            "tankFire" => {
                let yaw = num(e, "yaw", 0.);
                let pitch = num(e, "pitch", 0.);
                let direction = vec3(
                    yaw.sin() * pitch.cos(),
                    pitch.sin(),
                    -yaw.cos() * pitch.cos(),
                );
                self.add_burst(pos(e) + direction * 3.1, 0.65);
            }
            "shellImpact" => {
                let hit = pos(e);
                self.add_burst(hit, 1.13);
                self.scatter(hit, e["impactMaterial"].as_str().unwrap_or("earth"), true);
            }
            "vehicleImpact" if num(e, "strength", 0.) > 0.25 => {
                self.scatter(pos(e), e["material"].as_str().unwrap_or("metal"), true);
            }
            "throw" if e["actor"].as_u64() == Some(uid) => {
                self.throw_time = 0.41;
            }
            "explosion" => {
                self.add_burst(pos(e), 1.);
            }
            "destroy"
                if !matches!(
                    e["kind"].as_str(),
                    Some("house" | "barn" | "ruin" | "wall" | "barricade")
                ) =>
            {
                self.scatter(
                    vec3(num(e, "ex", 0.), 0.58, num(e, "ez", 0.)),
                    e["kind"].as_str().unwrap_or("stone"),
                    false,
                );
            }
            _ => {}
        }
    }
    fn scatter(&mut self, origin: Vec3, material: &str, impact: bool) {
        let metal = matches!(
            material,
            "tank"
                | "jeep"
                | "truck"
                | "patrol4x4"
                | "cargo6x6"
                | "mbt"
                | "apc"
                | "scout_buggy"
                | "oilwell"
                | "refinery"
                | "barrels"
                | "wreck"
                | "metal"
        );
        let timber = matches!(
            material,
            "house"
                | "barn"
                | "tree"
                | "pine"
                | "crate"
                | "logs"
                | "pole"
                | "wood"
                | "tower"
                | "fence"
        );
        let palette = if metal {
            [[100, 96, 85], [132, 99, 80]]
        } else if timber {
            [[159, 125, 83], [98, 77, 56]]
        } else {
            [[168, 159, 139], [125, 120, 105]]
        };
        let count = if impact { 5 } else { 20 };
        let keep = 160usize.saturating_sub(count);
        if self.chips.len() > keep {
            self.chips.drain(..self.chips.len() - keep);
        }
        for i in 0..count {
            let dust = i % 5 == 0;
            let size = if impact {
                self.rng.range(0.028, 0.066)
            } else {
                self.rng.range(0.074, 0.171)
            };
            let life = if impact {
                self.rng.range(0.14, 0.28)
            } else {
                self.rng.range(0.4, 0.83)
            };
            let speed = if dust {
                0.29
            } else if impact {
                self.rng.range(0.6, 1.35)
            } else {
                self.rng.range(1.15, 2.85)
            };
            let v = vec3(
                self.rng.range(-1., 1.),
                self.rng.range(0.45, 1.45),
                self.rng.range(-1., 1.),
            ) * speed;
            let spin = vec3(
                self.rng.range(-6., 6.),
                self.rng.range(-6., 6.),
                self.rng.range(-6., 6.),
            );
            let [r, g, b] = if dust {
                [149, 143, 124]
            } else {
                palette[i % 2]
            };
            self.chips.push(Chip {
                pos: origin,
                velocity: v,
                spin,
                life,
                total: life,
                size: if dust {
                    Vec3::splat(size * 1.6)
                } else {
                    vec3(
                        size * (if timber { 1.7 } else { 1. }),
                        size * (if metal { 0.35 } else { 0.67 }),
                        size,
                    )
                },
                color: Color::from_rgba(r, g, b, 255),
                dust,
            });
        }
    }
    fn add_burst(&mut self, origin: Vec3, scale: f32) {
        if self.bursts.len() >= 10 {
            self.bursts.remove(0);
        }
        self.bursts.push(Burst {
            origin,
            age: 0.,
            scale,
            seed: self.rng.range(0., TAU),
        });
    }
    fn draw_bursts(&mut self, eye: Vec3, dt: f32) {
        self.bursts.retain(|b| b.age < 0.89);
        self.bursts.sort_by(|a, b| {
            b.origin
                .distance_squared(eye)
                .total_cmp(&a.origin.distance_squared(eye))
        });
        for b in &mut self.bursts {
            let t = b.age;
            b.age += dt;
            if b.origin.distance_squared(eye) > 185. * 185. {
                continue;
            }
            let view = (eye - b.origin).normalize_or_zero();
            let mut right = Vec3::Y.cross(view).normalize_or_zero();
            if right.length_squared() < 0.1 {
                right = Vec3::X;
            }
            let up = view.cross(right).normalize_or_zero();
            for j in 0..6 {
                let a = j as f32 * 2.39996 + b.seed;
                let span = b.scale * (0.19 + 1.18 * t);
                let at = b.origin
                    + vec3(
                        a.cos() * span,
                        (0.22 + 0.10 * (j % 3) as f32 + t * 1.12) * b.scale,
                        a.sin() * span,
                    );
                let alpha = 0.66 * (1. - smooth(t / 0.87));
                let dark = 0.30 + 0.078 * (j % 3) as f32;
                smoke_puff(
                    &mut self.smoke,
                    at,
                    right,
                    up,
                    b.scale * (0.44 + 0.72 * t),
                    Color::new(dark + 0.075, dark + 0.059, dark + 0.01, alpha),
                );
            }
            if t < 0.16 {
                let strength = 1. - t / 0.16;
                let front = b.origin + Vec3::Y * (0.25 * b.scale) + view * (0.35 * b.scale);
                smoke_puff(
                    &mut self.smoke,
                    front,
                    right,
                    up,
                    b.scale * (0.48 + 0.98 * strength),
                    Color::new(1., 0.59, 0.22, 0.97 * strength),
                );
                smoke_puff(
                    &mut self.smoke,
                    front + view * 0.02,
                    right,
                    up,
                    b.scale * (0.20 + 0.49 * strength),
                    Color::new(1., 0.93, 0.70, strength),
                );
            }
        }
    }
    fn draw_effects(&mut self, dt: f32) {
        self.tracers.retain_mut(|t| {
            if t.life <= 0. {
                return false;
            }
            let span = t.b - t.a;
            let q = Quat::from_rotation_arc(Vec3::Y, span.normalize());
            let transform = Mat4::from_rotation_translation(q, (t.a + t.b) * 0.5);
            with_transform(transform, || {
                draw_cube(
                    Vec3::ZERO,
                    vec3(t.width, span.length(), t.width),
                    None,
                    t.color,
                )
            });
            t.life -= dt;
            true
        });
        self.chips.retain_mut(|p| {
            p.life -= dt;
            if p.life <= 0. {
                return false;
            }
            if !p.dust {
                p.velocity.y -= 5.8 * dt;
            }
            p.pos += p.velocity * dt;
            let elapsed = p.total - p.life;
            let tf = translate(p.pos) * rx(p.spin.x * elapsed) * rz(p.spin.z * elapsed);
            let s = p.size * smooth(p.life / (p.total * 0.55)).max(0.02);
            with_transform(tf, || draw_cube(Vec3::ZERO, s, None, p.color));
            true
        });
    }
    pub fn draw_weapon(&mut self, p: &Value, aspect: f32, dt: f32) {
        if !p.is_object() || !alive(p) || seated(p) || p["droneId"].is_number() {
            self.kick = 0.;
            set_default_camera();
            return;
        }
        let dt = dt.clamp(0., 0.12);
        self.aim = step(self.aim, num(p, "aim", 0.), dt, 16.);
        self.movement = step(
            self.movement,
            if yes(p, "moving", false) { 1. } else { 0. },
            dt,
            10.,
        );
        let rem = num(p, "reload", 0.);
        let duration = num(p, "reloadTime", 2.).max(0.01);
        let rp = if rem > 0. {
            1. - (rem / duration).clamp(0., 1.)
        } else {
            0.
        };
        let loading = (rp * PI).sin();
        let gun = Gun::equipped(p);
        let impulse = self.kick.max(num(p, "recoil", 0.));
        let recoil = impulse
            * gun.kick
            * match gun.grip {
                Some("grip_stable") => 0.80,
                Some("grip_light") => 1.07,
                _ => 1.,
            };
        self.kick = (self.kick - 6.8 * dt).max(0.);
        let phase = num(p, "bob", self.clock * 8.);
        let throw = (self.throw_time / 0.41 * PI).sin();
        let x = 0.195 * (1. - self.aim)
            + throw * 0.088
            + phase.sin() * 0.012 * self.movement * (1. - self.aim)
            + 0.093 * loading;
        let y = -0.206
            + (0.206 - gun.sight) * self.aim
            + 0.007 * (phase * 2.).cos() * self.movement * (1. - self.aim)
            + (self.clock * 1.55).sin() * 0.0014
            + 0.021 * recoil
            - 0.113 * loading
            - 0.17 * throw;
        let tf = translate(vec3(x, y, -0.535 + 0.058 * recoil))
            * ry(0.11 * (1. - self.aim) - 0.36 * loading)
            * rx(0.020 * (1. - self.aim) + 0.063 * recoil + 0.24 * loading)
            * rz(-0.038 * (1. - self.aim) - 0.63 * loading + num(p, "hitFlash", 0.) * 0.04);
        let projection = Mat4::perspective_rh_gl(66f32.to_radians(), aspect.max(0.15), 0.025, 5.);
        let depth = Mat4::from_cols(
            Vec4::X,
            Vec4::Y,
            vec4(0., 0., 0.021, 0.),
            vec4(0., 0., -0.979, 1.),
        );
        set_camera(&SightCamera(depth * projection));
        let travel =
            (((rp / 0.31).clamp(0., 1.) * PI * 0.5).sin()) * (1. - smooth((rp - 0.68) / 0.28));
        gun.draw(&self.rig, tf, impulse.max(travel), rp);
        let hand_move = if gun.body == "mr7" {
            vec3(0.081, -0.055, 0.194)
        } else if gun.body == "vx9" {
            vec3(0.094, -0.081, 0.229)
        } else {
            vec3(0.083, -0.049, 0.239)
        };
        let mut rest_offset = if gun.body == "vx9" {
            vec3(0., 0., -0.013)
        } else {
            Vec3::ZERO
        };
        if gun.grip == Some("grip_stable") {
            rest_offset += vec3(0.013, -0.039, 0.016);
        }
        self.rig.draw(
            "left_hand",
            tf * translate(hand_move * loading + rest_offset + vec3(-0.024, 0.31, -0.16) * throw)
                * rz(0.17 * loading),
        );
        self.rig.draw(
            "right_hand",
            tf * translate(vec3(0.011, 0.007, 0.013) * travel + rest_offset),
        );
        if impulse > 0.78 {
            self.rig.draw(
                "flash",
                tf * translate(gun.muzzle)
                    * rz(self.clock * 41.)
                    * Mat4::from_scale(Vec3::splat(0.83 + 0.17 * gun.kick)),
            );
        }
        set_default_camera();
        // The sight image stays anchored to the optical axis once the gun is shouldered.
        if self.aim > 0.78 && loading < 0.07 && throw < 0.03 && gun.optic != "optic_irons" {
            let opacity = smooth((self.aim - 0.78) / 0.22);
            let (cx, cy) = (screen_width() * 0.5, screen_height() * 0.5);
            let unit = (screen_height() / 720.).clamp(0.7, 2.2);
            if gun.optic == "optic_scope" {
                let etched = Color::new(0.13, 0.16, 0.16, opacity * 0.9);
                for sign in [-1., 1.] {
                    draw_line(
                        cx + 3. * unit * sign,
                        cy,
                        cx + 14. * unit * sign,
                        cy,
                        unit,
                        etched,
                    );
                }
                draw_line(cx, cy + 3. * unit, cx, cy + 14. * unit, unit, etched);
                for i in 1..=2 {
                    let y = cy + (5. + i as f32 * 4.) * unit;
                    draw_line(cx - 2. * unit, y, cx + 2. * unit, y, unit, etched);
                }
            } else {
                let tint = Color::new(0.35, 0.93, 0.80, 0.80 * opacity);
                for sign in [-1., 1.] {
                    draw_line(
                        cx + sign * 4. * unit,
                        cy + 3. * unit,
                        cx + sign * 7. * unit,
                        cy + 5. * unit,
                        unit,
                        tint,
                    );
                }
            }
            draw_circle(cx, cy, 1.6 * unit, Color::new(0.99, 0.37, 0.16, opacity));
        }
    }
}
