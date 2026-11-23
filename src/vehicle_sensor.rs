//! Native-resolution, depth-backed optic channel. The interface is drawn after this pass.
use crate::vehicle_camera::Sensor;
use macroquad::prelude::*;

pub struct Display {
    target: Option<RenderTarget>,
    material: Material,
}

impl Display {
    pub fn new() -> Self {
        let material = load_material(
            ShaderSource::Glsl {
                vertex: include_str!("shaders/drone_feed.vert"),
                fragment: include_str!("shaders/vehicle_sensor.frag"),
            },
            MaterialParams {
                uniforms: vec![
                    UniformDesc::new("Thermal", UniformType::Float1),
                    UniformDesc::new("Clock", UniformType::Float1),
                ],
                ..Default::default()
            },
        )
        .expect("Vehicle optic shader");
        Self {
            target: None,
            material,
        }
    }

    pub fn target(&mut self) -> RenderTarget {
        let width = screen_width().max(1.) as u32;
        let height = screen_height().max(1.) as u32;
        if self.target.as_ref().is_none_or(|surface| {
            surface.texture.width() as u32 != width || surface.texture.height() as u32 != height
        }) {
            self.target = Some(render_target_ex(
                width,
                height,
                RenderTargetParams {
                    depth: true,
                    sample_count: 1,
                },
            ));
        }
        self.target.as_ref().unwrap().clone()
    }

    pub fn present(&self, sensor: Sensor) {
        let Some(target) = &self.target else { return };
        self.material.set_uniform(
            "Thermal",
            if sensor == Sensor::Thermal {
                1.0f32
            } else {
                0.0
            },
        );
        self.material
            .set_uniform("Clock", (get_time() % 60.) as f32);
        gl_use_material(&self.material);
        draw_texture_ex(
            &target.texture,
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
