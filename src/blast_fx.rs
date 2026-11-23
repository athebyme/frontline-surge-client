//! Short-lived world-space explosion layers. All motion is sampled from event age.
use super::{smoke_puff, smooth};
use macroquad::prelude::*;
use std::f32::consts::TAU;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum BurstKind {
    Muzzle,
    Impact,
    Wreck { flash: bool },
}

pub(super) struct Burst {
    pub origin: Vec3,
    pub age: f32,
    pub scale: f32,
    pub seed: f32,
    pub kind: BurstKind,
}

impl Burst {
    pub fn duration(&self) -> f32 {
        match self.kind {
            BurstKind::Muzzle => 0.52,
            BurstKind::Impact => 1.25 + self.scale * 0.55,
            BurstKind::Wreck { .. } => 2.15,
        }
    }

    pub fn draw(&self, eye: Vec3, smoke: &mut Mesh, ribbons: &mut Mesh, thermal: bool) {
        let mut view = (eye - self.origin).normalize_or_zero();
        if view.length_squared() < 0.1 {
            view = Vec3::Z;
        }
        let mut right = Vec3::Y.cross(view).normalize_or_zero();
        if right.length_squared() < 0.1 {
            right = Vec3::X;
        }
        let up = view.cross(right);
        let t = self.age;
        let progress = t / self.duration();
        let lift = Vec3::Y * self.scale;
        if self.kind == BurstKind::Impact && t < 0.68 {
            self.fill_ribbons(ribbons, right, up, thermal);
            if !ribbons.indices.is_empty() {
                draw_mesh(ribbons);
            }
        }

        // Draw far lobes first. With a drone looking down, the billboard basis
        // and the sort both follow the drone camera rather than its operator.
        let mut lobes = [Vec3::ZERO; 6];
        for (i, offset) in lobes.iter_mut().enumerate() {
            let angle = i as f32 * 2.39996 + self.seed;
            let spread = (0.13 + 0.72 * progress.sqrt()) * self.scale;
            *offset = vec3(angle.cos() * spread, 0., angle.sin() * spread)
                + lift * (0.12 + progress * (0.65 + 0.44 * (i % 3) as f32));
        }
        lobes.sort_by(|a, b| a.dot(view).total_cmp(&b.dot(view)));
        let fade = (1. - smooth(progress)) * (0.48 + 0.47 * smooth(t / 0.16));
        for offset in lobes {
            let soot = if matches!(self.kind, BurstKind::Wreck { .. }) { 0.12 } else { 0.25 };
            let shade = soot + 0.12 * (offset.y / self.scale).min(1.8) + 0.10 * progress;
            let smoke_color = if thermal {
                Color::new(shade, shade, shade, 0.64 * fade)
            } else {
                Color::new(shade + 0.10, shade + 0.066, shade, 0.74 * fade)
            };
            let radius = self.scale * (0.34 + 0.92 * progress.sqrt());
            smoke_puff(smoke, self.origin + offset, right, up, radius, smoke_color);
        }

        let flash_time = match self.kind {
            BurstKind::Impact => 0.23,
            BurstKind::Wreck { flash: false } => 0.,
            _ => 0.10,
        };
        if t < flash_time {
            let strength = 1. - smooth(t / flash_time);
            let center = self.origin + lift * 0.19 + view * (0.30 * self.scale);
            let radius = self.scale * (0.82 + 0.55 * smooth(t / 0.08));
            let shell = if thermal { WHITE } else { Color::new(1., 0.43, 0.11, 1.) };
            let core = if thermal { WHITE } else { Color::new(1., 0.96, 0.74, 1.) };
            smoke_puff(smoke, center, right, up, radius, Color { a: 0.92 * strength, ..shell });
            smoke_puff(smoke, center + view * (0.03 * self.scale), right, up,
                radius * (0.25 + 0.35 * strength), Color { a: strength, ..core });
        }
    }

    fn fill_ribbons(&self, mesh: &mut Mesh, right: Vec3, up: Vec3, thermal: bool) {
        mesh.vertices.clear();
        mesh.indices.clear();
        let t = self.age;
        if t < 0.44 {
            let phase = t / 0.44;
            let radius = self.scale * (0.30 + 3.85 * (1. - (1. - phase).powi(2)));
            let thickness = self.scale * (0.14 + 0.36 * phase);
            let ground = self.origin.y < 0.8;
            let (axis_x, axis_y) = if ground { (Vec3::X, Vec3::Z) } else { (right, up) };
            let center = if ground {
                vec3(self.origin.x, 0.07 + phase * 0.10, self.origin.z)
            } else { self.origin };
            let base = if thermal { Color::new(0.78, 0.78, 0.78, 1.) }
                else { Color::new(0.85, 0.73, 0.54, 1.) };
            const SECTORS: u16 = 32;
            for i in 0..SECTORS {
                let a = TAU * f32::from(i) / f32::from(SECTORS);
                let dir = axis_x * a.cos() + axis_y * a.sin();
                let drift = (a * 5. + self.seed).sin() * thickness * 0.28;
                let density = 0.47 + 0.22 * (a * 3. + self.seed).cos();
                for (edge, opacity) in [(-1., 0.), (0., density), (0.62, 0.)] {
                    let at = center + dir * (radius + drift + thickness * edge);
                    let color = Color { a: opacity * (1. - smooth(phase)), ..base };
                    mesh.vertices.push(Vertex::new2(at, Vec2::ZERO, color));
                }
                let here = i * 3;
                let next = ((i + 1) % SECTORS) * 3;
                for j in [0, 1] {
                    let (u, v) = (here + j, next + j);
                    mesh.indices.extend_from_slice(&[u, u + 1, v, v, u + 1, v + 1]);
                }
            }
        }
        let facing = right.cross(up);
        for i in 0..13 {
            let a = self.seed + i as f32 * 2.39996;
            let noise = 0.5 + 0.5 * (a * 3.51).sin();
            let life = 0.33 + noise * 0.34;
            if t >= life {
                continue;
            }
            let vel = vec3(a.cos() * (2.4 + noise * 2.8), 2.0 + 3.1 * noise, a.sin() * (2.4 + noise * 2.8));
            let point = |time: f32| {
                self.origin + self.scale * (Vec3::Y * 0.14 + vel * time - Vec3::Y * (4.2 * time * time))
            };
            let head = point(t + 0.025);
            let tail = point((t - 0.05).max(0.));
            if head.y <= 0.02 {
                continue;
            }
            let side = (head - tail).cross(facing).normalize_or_zero() * self.scale * 0.024;
            let k = mesh.vertices.len() as u16;
            let fade = 1. - smooth(t / life);
            let hot = if thermal { WHITE } else { Color::new(1., 0.78, 0.34, 1.) };
            for (p, opacity) in [(tail, 0.), (head + side, fade), (head - side, fade)] {
                mesh.vertices.push(Vertex::new2(p, Vec2::ZERO, Color { a: opacity, ..hot }));
            }
            mesh.indices.extend_from_slice(&[k, k + 1, k + 2]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impact_ribbons_expand_then_clear_with_no_dangling_indices() {
        let mut burst = Burst { origin: Vec3::ZERO, age: 0., scale: 1., seed: 1.1, kind: BurstKind::Impact };
        let mut mesh = Mesh { vertices: Vec::new(), indices: Vec::new(), texture: None };
        burst.fill_ribbons(&mut mesh, Vec3::X, Vec3::Y, false);
        let initial = mesh.vertices[1].position.x;
        burst.age = 0.25;
        burst.fill_ribbons(&mut mesh, Vec3::X, Vec3::Y, false);
        assert!(mesh.vertices[1].position.x > initial + 1.);
        assert!(mesh.indices.iter().all(|&i| usize::from(i) < mesh.vertices.len()));
        burst.age = 0.7;
        burst.fill_ribbons(&mut mesh, Vec3::X, Vec3::Y, false);
        assert!(mesh.indices.is_empty() && mesh.vertices.is_empty());
        assert!(burst.duration() > burst.age);
    }
}
