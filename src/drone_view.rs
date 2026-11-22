//! Airborne observation drone and short-lived friendly location marks.
use macroquad::prelude::*;
use serde_json::Value;
fn get(o: &Value, k: &str) -> f32 {
    o[k].as_f64().unwrap_or(0.) as f32
}
pub fn draw(state: &Value, local: &Value, camera: Vec3) {
    let t = get_time() as f32;
    for v in state["drones"].as_array().into_iter().flatten() {
        if local["droneId"] == v["id"] {
            continue;
        }
        let pos = vec3(get(v, "x"), get(v, "y"), get(v, "z"));
        if pos.distance_squared(camera) > 180. * 180. {
            continue;
        }
        let dark = Color::from_rgba(42, 50, 51, 255);
        let trim = if v["team"] == "blue" { SKYBLUE } else { ORANGE };
        draw_cube(pos, vec3(0.28, 0.12, 0.34), None, dark);
        draw_sphere(
            pos - Vec3::Y * 0.09,
            0.073,
            None,
            Color::from_rgba(52, 88, 99, 255),
        );
        for (x, z) in [(-0.3, -0.24), (0.3, -0.24), (-0.3, 0.24), (0.3, 0.24)] {
            let (s, c) = get(v, "yaw").sin_cos();
            let arm = pos + vec3(x * c - z * s, 0.04, z * c + x * s);
            draw_line_3d(pos, arm, dark);
            draw_cube(arm, vec3(0.055, 0.08, 0.055), None, trim);
            let a = t * 42. + x * 5.;
            let blade = vec3(a.cos(), 0., a.sin()) * 0.16;
            draw_line_3d(arm + blade, arm - blade, GRAY);
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
