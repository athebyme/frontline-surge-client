//! Airborne observation drone and short-lived friendly location marks.
use macroquad::prelude::*;
use serde_json::Value;
fn get(o: &Value, k: &str) -> f32 {
    o[k].as_f64().unwrap_or(0.) as f32
}

fn airframe_bar(start: Vec3, end: Vec3, width: f32, height: f32, color: Color) {
    let along = end - start;
    let mid = (start + end) * 0.5;
    let model = Mat4::from_translation(mid) * Mat4::from_rotation_y(-along.z.atan2(along.x));
    unsafe {
        get_internal_gl().quad_gl.push_model_matrix(model);
    }
    draw_cube(Vec3::ZERO, vec3(along.length(), height, width), None, color);
    unsafe {
        get_internal_gl().quad_gl.pop_model_matrix();
    }
}

/// One compact airframe is visible in the hand, in flight and while hovering.
pub fn draw_airframe(pos: Vec3, yaw: f32, roll: f32, blue: bool, clock: f32, scale: f32) {
    let rotation = Quat::from_rotation_y(-yaw) * Quat::from_rotation_z(roll);
    let model = Mat4::from_scale_rotation_translation(Vec3::splat(scale), rotation, pos);
    let dark = Color::from_rgba(42, 50, 51, 255);
    let accent = if blue {
        Color::from_rgba(72, 165, 199, 255)
    } else {
        ORANGE
    };
    let spar = Color::from_rgba(83, 97, 99, 255);
    unsafe {
        get_internal_gl().quad_gl.push_model_matrix(model);
    }
    draw_cube(Vec3::ZERO, vec3(0.22, 0.09, 0.27), None, dark);
    draw_cube(
        vec3(0., 0.048, 0.008),
        vec3(0.15, 0.018, 0.17),
        None,
        Color::from_rgba(122, 134, 121, 255),
    );
    draw_cube(
        vec3(0., -0.055, -0.117),
        vec3(0.09, 0.07, 0.075),
        None,
        dark,
    );
    draw_sphere(vec3(0., -0.055, -0.156), 0.028, None, accent);
    for side in [-1., 1.] {
        draw_cube(
            vec3(side * 0.079, -0.059, 0.025),
            vec3(0.021, 0.046, 0.17),
            None,
            dark,
        );
    }
    for (index, (x, z)) in [(-0.24, -0.23), (0.24, -0.23), (-0.24, 0.23), (0.24, 0.23)]
        .into_iter()
        .enumerate()
    {
        let arm = vec3(x, 0.005, z);
        airframe_bar(vec3(x * 0.32, 0.005, z * 0.37), arm, 0.032, 0.022, spar);
        draw_cube(arm, vec3(0.05, 0.047, 0.05), None, dark);
        draw_cube(
            arm + Vec3::Y * 0.024,
            vec3(0.036, 0.008, 0.036),
            None,
            accent,
        );
        let angle = clock * if index % 3 == 0 { 47. } else { -47. } + index as f32;
        let blade = vec3(angle.cos(), 0., angle.sin()) * 0.124;
        let hub = arm + Vec3::Y * 0.028;
        airframe_bar(
            hub - blade,
            hub + blade,
            0.021,
            0.007,
            Color::from_rgba(133, 144, 144, 255),
        );
    }
    unsafe {
        get_internal_gl().quad_gl.pop_model_matrix();
    }
}

pub fn draw(state: &Value, local: &Value, camera: Vec3, motion: &crate::motion::Motion) {
    let t = get_time() as f32;
    for v in state["drones"].as_array().into_iter().flatten() {
        if local["droneId"] == v["id"]
            || (v["owner"] == local["id"]
                && get(v, "launchRemaining") - motion.age > 0.60
                && local["freeCamera"] != true)
        {
            continue;
        }
        let pose = motion.drone(v);
        if pose.position.distance_squared(camera) <= 180. * 180. {
            draw_airframe(
                pose.position,
                pose.yaw,
                pose.roll,
                v["team"] == "blue",
                t,
                1.,
            );
        }
    }
    for v in state["markers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v["team"] == local["team"])
    {
        let p = vec3(get(v, "x"), 0.14, get(v, "z"));
        let c = Color::new(0.2, 0.91, 0.83, (get(v, "remaining") / 1.5).clamp(0.15, 1.));
        draw_line_3d(p, p + Vec3::Y * 3., c);
        let r = 0.95 + (t * 4.).sin() * 0.13;
        for i in 0..12 {
            let a = i as f32 * std::f32::consts::TAU / 12.;
            let b = (i + 1) as f32 * std::f32::consts::TAU / 12.;
            draw_line_3d(
                p + vec3(a.sin() * r, 0., a.cos() * r),
                p + vec3(b.sin() * r, 0., b.cos() * r),
                c,
            );
        }
    }
}

/// Offscreen video feed. The HUD is drawn at native window resolution afterwards.
pub struct Feed {
    target: Option<RenderTarget>,
    material: Option<Material>,
}
impl Feed {
    pub fn new() -> Self {
        let material = load_material(
            ShaderSource::Glsl {
                vertex: include_str!("shaders/drone_feed.vert"),
                fragment: include_str!("shaders/drone_feed.frag"),
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("Reception", UniformType::Float1),
                    UniformDesc::new("Clock", UniformType::Float1),
                    UniformDesc::new("Pixel", UniformType::Float2),
                ],
                ..Default::default()
            },
        )
        .ok();
        Self {
            target: None,
            material,
        }
    }
    pub fn target(&mut self) -> RenderTarget {
        let (w, h) = (screen_width().max(1.), screen_height().max(1.));
        let scale = (960. / w).min(1.);
        let (width, height) = (
            (w * scale).round().max(1.) as u32,
            (h * scale).round().max(1.) as u32,
        );
        if !self.target.as_ref().is_some_and(|t| {
            t.texture.width() as u32 == width && t.texture.height() as u32 == height
        }) {
            let t = render_target_ex(
                width,
                height,
                RenderTargetParams {
                    sample_count: 1,
                    depth: true,
                },
            );
            t.texture.set_filter(FilterMode::Linear);
            self.target = Some(t);
        }
        self.target.as_ref().unwrap().clone()
    }
    pub fn present(&self, signal: f32) {
        let Some(t) = &self.target else {
            return;
        };
        if let Some(mat) = &self.material {
            mat.set_uniform("Reception", signal.clamp(0., 1.));
            mat.set_uniform("Clock", (get_time() % 900.) as f32);
            mat.set_uniform(
                "Pixel",
                vec2(1. / t.texture.width(), 1. / t.texture.height()),
            );
            gl_use_material(mat);
        }
        draw_texture_ex(
            &t.texture,
            0.,
            0.,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(screen_width(), screen_height())),
                flip_y: true,
                ..Default::default()
            },
        );
        gl_use_default_material();
    }
}
pub fn draw_pulses(state: &Value) {
    let now = get(state, "elapsed");
    for event in state["events"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v["type"] == "dronePulse")
    {
        let age = now - get(event, "at");
        if !(0.0..0.55).contains(&age) {
            continue;
        }
        let p = vec3(get(event, "x"), get(event, "y"), get(event, "z"));
        let phase = (age / 0.55).min(1.);
        let r = get(event, "radius") * phase.sqrt();
        let tint = Color::new(0.2, 0.9, 0.88, (1. - phase) * 0.8);
        for i in 0..32 {
            let a = std::f32::consts::TAU * i as f32 / 32.;
            let b = a + std::f32::consts::TAU / 32.;
            draw_line_3d(
                p + vec3(a.cos(), 0., a.sin()) * r,
                p + vec3(b.cos(), 0., b.sin()) * r,
                tint,
            );
            draw_line_3d(
                p + vec3(0., a.cos(), a.sin()) * r,
                p + vec3(0., b.cos(), b.sin()) * r,
                tint,
            );
        }
    }
}
