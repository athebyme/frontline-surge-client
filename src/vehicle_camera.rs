//! The chase camera and the gunner's seat use the same interpolated vehicle as the model.
use macroquad::prelude::*;
use serde_json::Value;

pub struct VehicleCamera {
    pub onboard: bool,
    zoom: f32,
    shown_zoom: f32,
}
impl Default for VehicleCamera {
    fn default() -> Self {
        Self {
            onboard: false,
            zoom: 1.,
            shown_zoom: 1.,
        }
    }
}
impl VehicleCamera {
    pub fn scroll(&mut self, wheel: f32) {
        if wheel.is_finite() && wheel != 0. && !self.onboard {
            self.zoom = (self.zoom * 0.86_f32.powf(wheel.clamp(-4., 4.))).clamp(0.60, 1.85);
        }
    }
    pub fn chase_distance(&mut self, light: bool, dt: f32) -> f32 {
        self.shown_zoom += (self.zoom - self.shown_zoom) * (1. - (-14. * dt).exp());
        self.shown_zoom * if light { 5.2 } else { 7.6 }
    }
}
// Model-local axes point forward along +Z and left along +X, with +Y up.
pub struct GunMount {
    pub pivot: Vec3,
    hinge: Vec3,
    eye: Vec3,
    pub mg_tip: Vec3,
}

impl GunMount {
    pub fn for_vehicle(kind: &str) -> Option<Self> {
        let (pivot, hinge, eye, mg_tip) = match kind {
            "mbt" => (
                vec3(0., 1.73, 0.34),
                vec3(0., 0.24, 0.96),
                vec3(-0.22, 0.18, 0.21),
                vec3(0.265, -0.065, 0.95),
            ),
            "apc" => (
                vec3(0., 2., 0.35),
                vec3(0.12, 0.26, 0.22),
                vec3(-0.22, 0.14, 0.16),
                vec3(0., 0., 0.96),
            ),
            _ => return None,
        };
        Some(Self {
            pivot,
            hinge,
            eye,
            mg_tip,
        })
    }
    pub fn turret(&self, body: Vec3, hull_yaw: f32, turret_yaw: f32) -> Mat4 {
        let face = |yaw| Quat::from_rotation_y(std::f32::consts::PI - yaw);
        Mat4::from_rotation_translation(face(turret_yaw), body + face(hull_yaw) * self.pivot)
    }
    pub fn barrel(&self, turret: Mat4, pitch: f32) -> Mat4 {
        turret
            * Mat4::from_translation(self.hinge)
            * Mat4::from_rotation_x(-pitch.clamp(-0.70, 0.70))
    }
}
pub fn gun_pitch(vehicle: &Value, state: Option<&Value>, motion: &crate::motion::Motion) -> f32 {
    if let Some((id, pitch)) = motion.local_gun_pitch {
        if vehicle["id"].as_u64() == Some(id) {
            return pitch;
        }
    }
    let driver = vehicle["driver"].as_u64();
    state
        .and_then(|s| s["players"].as_array())
        .and_then(|players| {
            players
                .iter()
                .find(|p| driver.is_some() && p["id"].as_u64() == driver)
        })
        .map_or(0., |p| motion.player(p).pitch)
}
pub fn onboard(
    kind: &str,
    body: Vec3,
    hull_yaw: f32,
    turret_yaw: f32,
    aim_yaw: f32,
    pitch: f32,
) -> (Vec3, Vec3, f32) {
    if let Some(mount) = GunMount::for_vehicle(kind) {
        let barrel = mount.barrel(mount.turret(body, hull_yaw, turret_yaw), pitch);
        return (
            barrel.transform_point3(mount.eye),
            barrel.transform_vector3(Vec3::Z),
            66.,
        );
    }
    let eye = match kind {
        "scout_buggy" => vec3(0.29, 1.42, 0.53),
        "patrol4x4" => vec3(0.32, 1.73, 0.76),
        _ => vec3(0.43, 2.56, 1.72),
    };
    let hull = Quat::from_rotation_y(std::f32::consts::PI - hull_yaw);
    (body + hull * eye, super::dir(aim_yaw, pitch), 77.)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_is_bounded_and_reaches_the_same_distance_at_different_frame_rates() {
        let run = |wheel: f32, hz: u32| {
            let mut camera = VehicleCamera::default();
            for _ in 0..40 {
                camera.scroll(wheel);
            }
            let mut dist = 0.;
            for _ in 0..hz {
                dist = camera.chase_distance(false, 1. / hz as f32);
            }
            dist
        };
        assert!((run(1., 30) - 4.56).abs() < 0.001);
        assert!((run(-1., 144) - 14.06).abs() < 0.001);
        assert!((run(-1., 30) - run(-1., 144)).abs() < 0.001);
    }
    #[test]
    fn optic_axis_follows_the_articulated_barrel_even_across_hull_and_turret_turns() {
        for kind in ["mbt", "apc"] {
            for pitch in [-1., 0., 0.65] {
                let (eye, look, _) = onboard(kind, vec3(9., 0., -2.), -1.5, 2.25, 0.8, pitch);
                assert!(eye.is_finite());
                assert!(look.distance(super::super::dir(2.25, pitch.clamp(-0.7, 0.7))) < 0.0001);
            }
        }
    }
}
