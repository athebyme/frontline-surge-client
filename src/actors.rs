//! Native first-person arms, articulated infantry and short-lived match effects.
#[path = "actor_pose.rs"]
mod pose;
#[path = "blast_fx.rs"]
mod blast_fx;
use blast_fx::{Burst, BurstKind};
use macroquad::camera::Camera;
use macroquad::prelude::*;
use pose::{arm_pose, ReloadPose};
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

struct Rig {
    parts: HashMap<String, Mesh>,
    original: HashMap<String, Vec<[u8; 4]>>,
    light_level: u8,
    thermal: bool,
    fabric: Texture2D,
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
        let swatch = texture.width() / 4.;
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
                    (tile as f32 * swatch + 0.5 + (swatch - 1.) * u) / texture.width(),
                    (0.5 + (texture.height() - 1.) * v) / texture.height(),
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
            thermal: false,
            fabric: texture,
        }
    }
    fn set_daylight(&mut self, light: f32) {
        let level = (light.clamp(0., 1.) * 48.).round() as u8;
        if level == self.light_level {
            return;
        }
        self.light_level = level;
        if !self.thermal {
            self.update_colors();
        }
    }

    fn set_thermal(&mut self, enabled: bool) {
        if self.thermal == enabled {
            return;
        }
        self.thermal = enabled;
        for mesh in self.parts.values_mut() {
            mesh.texture = if enabled {
                None
            } else {
                Some(self.fabric.clone())
            };
        }
        self.update_colors();
    }

    fn update_colors(&mut self) {
        let d = self.light_level as f32 / 48.;
        let gains = [0.090 + 0.910 * d, 0.116 + 0.884 * d, 0.157 + 0.843 * d];
        for (name, mesh) in &mut self.parts {
            let person =
                name.starts_with("a/") || name.starts_with("e/") || name.ends_with("_hand");
            let exposed = name.ends_with("/head") || name.ends_with("_hand");
            for (vertex, base) in mesh.vertices.iter_mut().zip(&self.original[name]) {
                if self.thermal {
                    let relief = base[1] as f32 / 255.;
                    let skin = exposed && base[0] as u16 > base[2] as u16 + 22;
                    let heat = if name == "flash" {
                        255
                    } else if skin {
                        (217. + 28. * relief) as u8
                    } else if person {
                        (163. + 37. * relief) as u8
                    } else {
                        (47. + 35. * relief) as u8
                    };
                    vertex.color = [heat, heat, heat, base[3]];
                } else if name == "flash" || name.ends_with("/mark") {
                    vertex.color = *base;
                } else {
                    for i in 0..3 {
                        vertex.color[i] = (base[i] as f32 * gains[i]).round() as u8;
                    }
                }
            }
        }
    }

    fn draw(&self, key: &str, transform: Mat4) {
        if self.thermal && key.ends_with("/mark") {
            return;
        }
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
    pitch: f32,
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
            pitch: num(p, "pitch", 0.),
            phase: (id_of(p) % 53) as f32 * 0.87,
            movement: 0.,
            hp: num(p, "hp", 100.),
            down: 0.,
            down_sign: if id_of(p) % 2 == 0 { 1. } else { -1. },
            hurt: 0.,
        }
    }
    fn update(&mut self, p: &Value, dt: f32, pose: crate::motion::Pose) {
        let target = pose.position;
        let yaw = pose.yaw;
        let hp = num(p, "hp", 100.);
        if hp < self.hp {
            self.hurt = 1.;
        }
        self.hp = hp;
        self.hurt = (self.hurt - dt * 4.5).max(0.);
        self.location = target;
        self.yaw = yaw;
        self.pitch = pose.pitch;
        if !alive(p) {
            self.down = (self.down + dt * 1.25).min(1.);
            return;
        }
        self.down = 0.;
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
    fn draw(&self, rig: &Rig, transform: Mat4, cycle: f32, pose: &ReloadPose) {
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
        let feed = self.feed_position + pose.feed_offset;
        rig.draw(self.feed, transform * translate(feed) * rz(pose.feed_twist));
    }
}
fn seated(p: &Value) -> bool {
    p["vehicleId"].is_number()
}

fn reload_phase(p: &Value, snapshot_age: f32) -> f32 {
    let remaining = num(p, "reload", 0.) - snapshot_age;
    if remaining <= 0. {
        0.
    } else {
        1. - (remaining / num(p, "reloadTime", 2.).max(0.01)).clamp(0., 1.)
    }
}

fn uniform(friend: bool) -> [&'static str; 9] {
    if friend {
        [
            "a/torso",
            "a/head",
            "a/l_upper",
            "a/l_lower",
            "a/r_upper",
            "a/r_lower",
            "a/mark",
            "a/hip",
            "a/shin",
        ]
    } else {
        [
            "e/torso",
            "e/head",
            "e/l_upper",
            "e/l_lower",
            "e/r_upper",
            "e/r_lower",
            "e/mark",
            "e/hip",
            "e/shin",
        ]
    }
}

fn draw_buggy_driver(rig: &Rig, pose: crate::motion::Pose, friend: bool, camera: Vec3) {
    use frontline_surge::buggy::{EYE, SEAT};
    let parts = uniform(friend);
    let chassis = translate(pose.position) * ry(std::f32::consts::PI - pose.yaw);
    // The infantry rig faces -Z, while the authored vehicle faces +Z.
    let body = chassis * translate(Vec3::from_array(SEAT)) * ry(std::f32::consts::PI)
        * Mat4::from_scale(Vec3::splat(0.76)) * translate(vec3(0., -0.92, 0.));
    rig.draw(parts[0], body);
    let eye = chassis.transform_point3(Vec3::from_array(EYE));
    if eye.distance_squared(camera) > 0.075 {
        rig.draw(parts[1], body);
    }
    let shoulders = body * translate(vec3(0., 1.325, 0.));
    for (left, upper, lower, wrist) in [
        (true, parts[2], parts[3], vec3(-0.061, -0.047, -0.431)),
        (false, parts[4], parts[5], vec3(0.090, -0.057, -0.246)),
    ] {
        let side = if left { -1. } else { 1. };
        let wheel = vec3(side * 0.20, -0.07 + side * pose.steering * 0.05, -0.41);
        let (arm, forearm) = arm_pose(left, wheel - wrist, 0.);
        rig.draw(upper, shoulders * arm);
        rig.draw(lower, shoulders * forearm);
        let thigh = body * translate(vec3(side * 0.121, 0.92, 0.)) * rx(1.33);
        rig.draw(parts[7], thigh);
        rig.draw(parts[8], thigh * translate(vec3(0., -0.421, 0.)) * rx(-1.62));
    }
}

fn draw_remote(rig: &Rig, r: &Remote, p: &Value, friend: bool, snapshot_age: f32) {
    if r.down >= 0.99 {
        return;
    }
    let parts = uniform(friend);
    let sway = r.phase.sin() * r.movement;
    let step_height = (0.5 - 0.5 * (2. * r.phase).cos()) * r.movement;
    let fall = smooth(r.down / 0.77);
    let root = translate(r.location + vec3(0., 0.024 * step_height - 0.33 * fall, 0.))
        * ry(-r.yaw)
        * rz(r.down_sign * 1.36 * fall + r.hurt * 0.09)
        * rx(0.24 * fall);
    let torso = root
        * translate(vec3(0.025 * sway, 0., 0.))
        * translate(vec3(0., 0.91, 0.))
        * ry(-0.065 * sway)
        * rz(0.04 * sway)
        * rx(-0.067 * r.movement)
        * translate(vec3(0., -0.91, 0.));
    rig.draw(parts[0], torso);
    let gun = Gun::equipped(p);
    let reload = ReloadPose::sample(gun.body, reload_phase(p, snapshot_age));
    let recoil = (num(p, "recoil", 0.) - 6.8 * snapshot_age).max(0.);
    let pilot = p["droneId"].is_number();
    let pitch = if pilot {
        -0.67
    } else {
        r.pitch.clamp(-0.96, 0.96)
    };
    let neck = vec3(0., 1.484, 0.);
    rig.draw(
        parts[1],
        torso
            * translate(neck)
            * ry(0.055 * sway)
            * rx(pitch * 0.53 - reload.ready * 0.19)
            * translate(-neck),
    );
    let arms = torso * translate(vec3(0., 1.325, 0.)) * rx(pitch + recoil * 0.07);
    let bind =
        translate(vec3(0.056, 0.005, -0.313)) * ry(0.315) * Mat4::from_scale(Vec3::splat(0.76));
    let posed = translate(vec3(0., 0.012, 0.031) * recoil) * bind * reload.weapon(0.78);
    let launch = (num(p, "droneLaunch", 0.) - snapshot_age).max(0.);
    let toss = ((0.92 - launch) / 0.32).clamp(0., 1.) * (launch / 0.60).min(1.);
    let gun_delta = posed * bind.inverse();
    for (left, wrist, upper, lower) in [
        (true, vec3(-0.061, -0.047, -0.431), parts[2], parts[3]),
        (false, vec3(0.090, -0.057, -0.246), parts[4], parts[5]),
    ] {
        let rest = gun_delta.transform_point3(wrist);
        let target = if left {
            let grasp =
                posed.transform_point3(vec3(-0.048, -0.062, -0.349) + reload.support_offset);
            rest.lerp(grasp, reload.grasp)
        } else {
            rest
        };
        let target = if launch > 0. {
            let hand = if left { vec3(0.06, -0.08, -0.34) } else { vec3(0.18, -0.05, -0.32) };
            hand + vec3(0., toss * 0.19, -toss * 0.21)
        } else {
            target
        };
        let offset = target - wrist;
        let (shoulder, elbow) = arm_pose(left, offset, 0.024 * reload.ready);
        rig.draw(upper, arms * shoulder);
        rig.draw(lower, arms * elbow);
    }
    if launch <= 0. {
        gun.draw(rig, arms * posed, recoil.max(reload.cycle), &reload);
    }
    if r.down == 0. {
        rig.draw(parts[6], root * translate(vec3(0., 2.05, 0.)));
        if recoil > 0.80 {
            rig.draw("flash", arms * posed * translate(gun.muzzle));
        }
    }
    let stride = if yes(p, "sprinting", false) {
        0.67
    } else {
        0.46
    };
    for side in [-1., 1.] {
        let motion = sway * side;
        let recovery = (r.phase.cos() * side).max(0.) * r.movement;
        let hip = root
            * translate(vec3(side * 0.121 + 0.010 * sway, 0.92, 0.))
            * rx(stride * motion + 0.08 * recovery);
        rig.draw(parts[7], hip);
        rig.draw(
            parts[8],
            hip * translate(vec3(0., -0.421, 0.))
                * rx(-0.68 * recovery * recovery - 0.04 * r.movement),
        );
    }
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

struct VehicleFlash {
    vehicle: u64,
    cannon: bool,
    remaining: f32,
}

impl VehicleFlash {
    fn transform(&self, v: &Value, state: &Value, motion: &crate::motion::Motion) -> Option<Mat4> {
        let mount = crate::vehicle_camera::GunMount::for_vehicle(v["kind"].as_str()?)?;
        let pose = motion.vehicle(v);
        let turret = mount.turret(pose.position, pose.yaw, pose.turret);
        let pitch = crate::vehicle_camera::gun_pitch(v, Some(state), motion);
        let tip = if self.cannon && v["kind"] == "mbt" {
            Vec3::from_array(mount.0.primary_tip)
        } else {
            Vec3::from_array(mount.0.mg_tip)
        };
        Some(mount.barrel(turret, pitch) * translate(tip) * ry(PI))
    }
}

struct Tracer {
    start: Vec3,
    end: Vec3,
    age: f32,
    duration: f32,
    width: f32,
    color: Color,
}
impl Tracer {
    fn segment(&self) -> (Vec3, Vec3) {
        let path = self.end - self.start;
        let len = path.length();
        let dir = path / len;
        let streak = (len * 0.16).clamp(0.16, 8.).min(len);
        let head = streak + (len - streak) * (self.age / self.duration).clamp(0., 1.);
        (self.start + dir * (head - streak), self.start + dir * head)
    }
}

/// A depth-tested streak with a visible end-on profile in the driver's sight.
pub(crate) fn draw_streak(tail: Vec3, head: Vec3, eye: Vec3, width: f32, color: Color) {
    let span = head - tail;
    let length = span.length();
    if length < 0.001 {
        return;
    }
    let center = (head + tail) * 0.5;
    let width = width.max((center.distance(eye) * 0.0016).min(0.24));
    let orient = Quat::from_rotation_arc(Vec3::Y, span / length);
    with_transform(Mat4::from_rotation_translation(orient, center), || {
        draw_cube(Vec3::ZERO, vec3(width, length, width), None, color);
        draw_cube(
            vec3(0., length * 0.34, 0.),
            vec3(width * 1.42, length * 0.30, width * 1.42),
            None,
            Color::from_rgba(255, 236, 183, 255),
        );
    });
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
    blast_ribbons: Mesh,
    vehicle_flashes: Vec<VehicleFlash>,
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
            blast_ribbons: Mesh {
                vertices: Vec::with_capacity(140),
                indices: Vec::with_capacity(430),
                texture: None,
            },
            vehicle_flashes: Vec::with_capacity(4),
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
    pub fn set_thermal(&mut self, enabled: bool) {
        self.rig.set_thermal(enabled);
    }

    pub fn set_daylight(&mut self, value: f32) {
        self.rig.set_daylight(value);
    }
    pub fn clear(&mut self) {
        self.remote.clear();
        self.tracers.clear();
        self.chips.clear();
        self.bursts.clear();
        self.vehicle_flashes.clear();
        self.grenade_positions.clear();
        self.throw_time = 0.;
        self.last_event = None;
        self.round = None;
        self.clock = 0.;
        self.aim = 0.;
        self.movement = 0.;
        self.kick = 0.;
    }
    pub fn draw_world(
        &mut self,
        state: &Value,
        local: &Value,
        dt: f32,
        motion: &crate::motion::Motion,
        camera: Vec3,
    ) {
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
                if let Some(id) = p["vehicleId"].as_u64() {
                    let buggy = state["vehicles"].as_array().and_then(|vs| vs.iter().find(|v|
                        v["id"].as_u64() == Some(id) && v["kind"] == "scout_buggy"
                        && v["driver"] == p["id"] && v["destroyed"] != true));
                    if let Some(v) = buggy.filter(|_| alive(p)) {
                        draw_buggy_driver(&self.rig, motion.vehicle(v), p["team"] == local["team"], camera);
                    }
                    continue;
                }
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
                r.update(p, dt, motion.player(p));
                draw_remote(&self.rig, r, p, p["team"] == local["team"], motion.age);
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
                self.event(event, uid, state, motion);
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
        self.vehicle_flashes.retain_mut(|flash| {
            let Some(v) = state["vehicles"]
                .as_array()
                .and_then(|list| list.iter().find(|v| id_of(v) == flash.vehicle))
            else {
                return false;
            };
            if motion.optic_vehicle != Some(flash.vehicle) {
                if let Some(transform) = flash.transform(v, state, motion) {
                    let scale = if flash.cannon { 3.1 } else { 1.65 };
                    self.rig.draw(
                        "flash",
                        transform * rz(flash.remaining * 36.) * Mat4::from_scale(Vec3::splat(scale)),
                    );
                }
            }
            flash.remaining -= dt;
            flash.remaining > 0.
        });
        self.draw_effects(dt, camera);
        self.draw_bursts(camera, dt);
    }
    fn event(&mut self, e: &Value, uid: u64, state: &Value, motion: &crate::motion::Motion) {
        let cannon = e["type"] == "tankFire";
        let mounted_mg = e["type"] == "shot" && e["weapon"] == "vehicle_mg";
        let mut drawn_muzzle = None;
        if cannon || mounted_mg {
            if let Some(v) = state["vehicles"].as_array().and_then(|list| {
                list.iter().find(|v| {
                    let vehicle = e["vehicleId"].as_u64();
                    vehicle.is_some_and(|id| id == id_of(v))
                        || (e["actor"].is_number() && v["driver"] == e["actor"])
                })
            }) {
                let flash = VehicleFlash {
                    vehicle: id_of(v),
                    cannon,
                    remaining: if cannon { 0.092 } else { 0.058 },
                };
                if cannon {
                    if let Some(muzzle) = flash.transform(v, state, motion) {
                        self.add_burst(muzzle.transform_point3(Vec3::ZERO), 0.52, BurstKind::Muzzle);
                    }
                } else {
                    drawn_muzzle = flash.transform(v, state, motion)
                        .map(|pose| pose.transform_point3(Vec3::ZERO));
                }
                self.vehicle_flashes.push(flash);
            }
        }
        match e["type"].as_str().unwrap_or("") {
            "shot" => {
                let mine = e["actor"].as_u64() == Some(uid);
                if mine && !mounted_mg {
                    self.kick = 1.;
                }
                let a = pos(e);
                let b = dest(e);
                let delta = b - a;
                let dist = delta.length();
                let tank = matches!(e["weapon"].as_str(), Some("tank" | "mbt"));
                if dist > 0.55 {
                    let dir = delta / dist;
                    let start = drawn_muzzle.unwrap_or(a + dir * (dist * 0.2).min(1.2));
                    // Cover inside the barrel still receives the server impact.
                    // A visible tracer can only travel forward from the muzzle.
                    if (b - start).dot(dir) > 0.1 {
                        self.tracers.push(Tracer {
                            start,
                            end: b,
                            age: 0.,
                            duration: (start.distance(b) / 750.).clamp(0.055, 0.16),
                            width: if mounted_mg { 0.044 } else { 0.026 },
                            color: if mine {
                                Color::from_rgba(244, 164, 77, 255)
                            } else {
                                Color::from_rgba(243, 133, 70, 255)
                            },
                        });
                    }
                }
                if let Some(material) = e["impactMaterial"].as_str() {
                    self.scatter(b, material, true);
                    if tank {
                        self.add_burst(b, 0.67, BurstKind::Impact);
                    }
                }
            }
            "shellImpact" => {
                let hit = pos(e);
                // A nearby shell can launch and hit between network snapshots.
                // Draw its final confirmed segment even in that case.
                if let Ok(origin) = serde_json::from_value::<[f32; 3]>(e["trailFrom"].clone()) {
                    let start = Vec3::from_array(origin);
                    if start.distance_squared(hit) > 0.01 {
                        self.tracers.push(Tracer {
                            start,
                            end: hit,
                            age: 0.,
                            duration: 0.040,
                            width: 0.10,
                            color: Color::from_rgba(244, 146, 57, 255),
                        });
                    }
                }
                self.impact_burst(
                    hit,
                    e["impactMaterial"].as_str().unwrap_or("earth"),
                    if e["weapon"] == "apc_he" { 0.78 } else { 1.13 },
                );
            }
            "vehicleImpact" if num(e, "strength", 0.) > 0.25 => {
                self.scatter(pos(e), e["material"].as_str().unwrap_or("metal"), true);
            }
            "throw" if e["actor"].as_u64() == Some(uid) => {
                self.throw_time = 0.41;
            }
            "explosion" => {
                self.impact_burst(pos(e), "earth", 1.);
            }
            "destroy"
                if !matches!(
                    e["kind"].as_str(),
                    Some("house" | "barn" | "ruin" | "wall" | "barricade")
                ) =>
            {
                let wreck = vec3(num(e, "ex", 0.), 0.76, num(e, "ez", 0.));
                if e["vehicleId"].is_number() {
                    // A killing shell has already flashed on the hull. In that case
                    // just vent a column of smoke from the disabled vehicle.
                    let flash = !self.bursts.iter().any(|b| b.kind == BurstKind::Impact
                        && b.age < 0.20 && b.origin.distance_squared(wreck) < 20.25);
                    self.add_burst(wreck, 0.75, BurstKind::Wreck { flash });
                }
                self.scatter(
                    wreck,
                    e["kind"].as_str().unwrap_or("stone"),
                    false,
                );
            }
            _ => {}
        }
    }
    fn impact_burst(&mut self, at: Vec3, material: &str, scale: f32) {
        self.add_burst(at, scale, BurstKind::Impact);
        self.scatter(at + Vec3::Y * 0.06, material, false);
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
                | "plank"
                | "tower"
                | "fence"
        );
        let palette = if metal {
            [[100, 96, 85], [132, 99, 80]]
        } else if timber {
            [[159, 125, 83], [98, 77, 56]]
        } else if matches!(material, "brick" | "tile") {
            [[166, 94, 66], [121, 77, 60]]
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
    fn add_burst(&mut self, origin: Vec3, scale: f32, kind: BurstKind) {
        if self.bursts.len() >= 10 {
            self.bursts.remove(0);
        }
        self.bursts.push(Burst {
            origin,
            age: 0.,
            scale,
            seed: self.rng.range(0., TAU),
            kind,
        });
    }
    fn draw_bursts(&mut self, eye: Vec3, dt: f32) {
        self.bursts.retain(|b| b.age < b.duration());
        self.bursts.sort_by(|a, b| {
            b.origin.distance_squared(eye).total_cmp(&a.origin.distance_squared(eye))
        });
        for b in &mut self.bursts {
            if b.origin.distance_squared(eye) < 220. * 220. {
                b.draw(eye, &mut self.smoke, &mut self.blast_ribbons, self.rig.thermal);
            }
            b.age += dt;
        }
    }
    fn draw_effects(&mut self, dt: f32, camera: Vec3) {
        self.tracers.retain_mut(|t| {
            t.age = (t.age + dt).min(t.duration);
            let (tail, head) = t.segment();
            draw_streak(tail, head, camera, t.width, t.color);
            t.age < t.duration
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
        let gun = Gun::equipped(p);
        let reload = ReloadPose::sample(gun.body, reload_phase(p, 0.));
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
            + phase.sin() * 0.012 * self.movement * (1. - self.aim);
        let y = -0.206
            + (0.206 - gun.sight) * self.aim
            + 0.007 * (phase * 2.).cos() * self.movement * (1. - self.aim)
            + (self.clock * 1.55).sin() * 0.0014
            + 0.021 * recoil
            - 0.17 * throw;
        let walk = self.movement * (1. - self.aim);
        let tf = translate(vec3(
            x,
            y + 0.119 * reload.ready,
            -0.535 + 0.058 * recoil - 0.099 * reload.ready,
        )) * ry(0.11 * (1. - self.aim) + 0.015 * phase.cos() * walk)
            * rx(0.020 * (1. - self.aim) + 0.063 * recoil)
            * rz(-0.038 * (1. - self.aim)
                + 0.014 * phase.sin() * walk
                + num(p, "hitFlash", 0.) * 0.04)
            * reload.weapon(1.);
        let projection = Mat4::perspective_rh_gl(66f32.to_radians(), aspect.max(0.15), 0.025, 5.);
        let depth = Mat4::from_cols(
            Vec4::X,
            Vec4::Y,
            vec4(0., 0., 0.021, 0.),
            vec4(0., 0., -0.979, 1.),
        );
        set_camera(&SightCamera(depth * projection));
        let launch = num(p, "droneLaunch", 0.);
        if launch > 0. {
            let phase = (0.92 - launch).clamp(0., 0.92);
            let lift = smooth((phase / 0.32).clamp(0., 1.));
            let lower = smooth(((phase - 0.32) / 0.5).clamp(0., 1.));
            let palm = vec3(0.13 - 0.027 * lift, -0.29 + 0.15 * lift - 0.28 * lower,
                -0.47 - 0.19 * lift + 0.15 * lower);
            let right = translate(palm) * rx(0.18 - 0.46 * lift)
                * translate(-vec3(0.04, -0.05, -0.14));
            let left = translate(palm + vec3(-0.12, -0.025 - 0.13 * lift, 0.02))
                * translate(-vec3(-0.061, -0.082, -0.344));
            self.rig.draw("right_hand", right);
            self.rig.draw("left_hand", left);
            if phase <= 0.32 {
                crate::drone_view::draw_airframe(
                    palm + vec3(-0.046, 0.042, -0.065), 0., 0.03,
                    p["team"] == "blue", self.clock, 0.48,
                );
            }
            set_default_camera();
            return;
        }
        gun.draw(&self.rig, tf, impulse.max(reload.cycle), &reload);
        let mut rest = if gun.body == "vx9" {
            vec3(0., 0., -0.013)
        } else {
            Vec3::ZERO
        };
        if gun.grip == Some("grip_stable") {
            rest += vec3(0.013, -0.039, 0.016);
        }
        let wrist = vec3(-0.061, -0.082, -0.344);
        let support =
            tf * translate(
                reload.support_offset
                    + rest * (1. - reload.ready)
                    + vec3(-0.024, 0.31, -0.16) * throw,
            ) * translate(wrist)
                * rz(reload.wrist_roll)
                * translate(-wrist);
        self.rig.draw("left_hand", support);
        self.rig.draw("right_hand", tf);
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
        if self.aim > 0.78 && reload.ready < 0.07 && throw < 0.03 && gun.optic != "optic_irons" {
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
