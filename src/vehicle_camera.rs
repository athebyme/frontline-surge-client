//! The chase camera and the gunner's seat use the same interpolated vehicle as the model.
use macroquad::prelude::*;
use serde_json::Value;

use frontline_surge::vehicle_gun::{Mount, TURN_RATE};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sensor {
    #[default]
    Day,
    Night,
    Thermal,
}
impl Sensor {
    pub fn next(self) -> Self {
        match self {
            Self::Day => Self::Night,
            Self::Night => Self::Thermal,
            Self::Thermal => Self::Day,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Day => "ДЕНЬ",
            Self::Night => "ПНВ",
            Self::Thermal => "ТЕПЛО",
        }
    }
}

pub struct VehicleCamera {
    pub onboard: bool,
    pub sensor: Sensor,
    zoom: f32,
    shown_zoom: f32,
    exit_wheel: f32,
    turret: Option<(u64, f32)>,
}
impl Default for VehicleCamera {
    fn default() -> Self {
        Self {
            onboard: false,
            sensor: Sensor::Day,
            zoom: 1.,
            shown_zoom: 1.,
            exit_wheel: 0.,
            turret: None,
        }
    }
}
impl VehicleCamera {
    pub fn toggle(&mut self) {
        self.onboard = !self.onboard;
        self.exit_wheel = 0.;
    }
    pub fn scroll(&mut self, wheel: f32) {
        if !wheel.is_finite() || wheel == 0. {
            return;
        }
        if self.onboard {
            self.exit_wheel = (self.exit_wheel - wheel).clamp(0., 1.);
            if self.exit_wheel >= 1. {
                self.toggle();
                self.zoom = self.zoom.max(0.71);
            }
        } else {
            let next = self.zoom * 0.86_f32.powf(wheel.clamp(-4., 4.));
            if next < 0.57 && wheel > 0. {
                self.toggle();
                self.zoom = 0.60;
            } else {
                // Preserve fractional trackpad input at the close threshold.
                self.zoom = next.clamp(0.57, 1.85);
            }
        }
    }
    pub fn chase_distance(&mut self, light: bool, dt: f32) -> f32 {
        self.shown_zoom += (self.zoom.max(0.60) - self.shown_zoom) * (1. - (-14. * dt).exp());
        self.shown_zoom * if light { 5.2 } else { 7.6 }
    }

    // Command and joint yaw are headings in world space. Hull translation and
    // chassis steering must never be fed back into the stabilized sight axis.
    pub fn turret(&mut self, id: u64, latest: f32, aim: f32, age: f32, dt: f32) -> f32 {
        let prior = self.turret.filter(|e| e.0 == id).map_or(latest, |e| e.1);
        let step = TURN_RATE * dt.clamp(0., 0.1);
        let next = prior + (aim - prior).clamp(-step, step);
        // Bound local prediction by input transit and the most recent sample.
        let budget = TURN_RATE * (age.clamp(0., 0.13) + 0.075);
        let corrected = next.clamp(latest - budget, latest + budget);
        // A jittered confirmation can narrow the prediction budget. Hold until
        // it catches up instead of rotating backwards against a held command.
        let result = if (corrected - prior) * (aim - prior) < 0. && (latest - prior).abs() < 0.65 {
            prior
        } else {
            corrected
        };
        self.turret = Some((id, result));
        result
    }
}

/// Keep only a short movement command ahead of the physical optic. A fast
/// mouse sweep should stop after a fraction of a second, even after several turns.
pub fn optic_command(previous: f32, joint: f32, delta: f32) -> f32 {
    let lead = previous - joint;
    let start = if lead * delta < 0. { joint } else { previous };
    (start + delta).clamp(joint - 0.48, joint + 0.48)
}

pub struct GunMount(pub Mount);
impl GunMount {
    pub fn for_vehicle(kind: &str) -> Option<Self> {
        Mount::for_kind(kind).map(Self)
    }
    pub fn turret(&self, body: Vec3, hull_yaw: f32, turret_yaw: f32) -> Mat4 {
        let face = |yaw| Quat::from_rotation_y(std::f32::consts::PI - yaw);
        Mat4::from_rotation_translation(
            face(turret_yaw),
            body + face(hull_yaw) * Vec3::from_array(self.0.pivot),
        )
    }
    pub fn barrel(&self, turret: Mat4, pitch: f32) -> Mat4 {
        turret
            * Mat4::from_translation(Vec3::from_array(self.0.hinge))
            * Mat4::from_rotation_x(-self.0.pitch(pitch))
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
fn point(v: &Value) -> Option<Vec3> {
    let xyz = v.as_array()?;
    if xyz.len() != 3 {
        return None;
    }
    Some(vec3(
        xyz[0].as_f64()? as f32,
        xyz[1].as_f64()? as f32,
        xyz[2].as_f64()? as f32,
    ))
}
pub fn reconcile(origin: Vec3, direction: Vec3, contact: Vec3, player: &Value, age: f32) -> Vec3 {
    let Some(confirmed) = point(&player["mgSight"]) else {
        return contact;
    };
    let old_origin = point(&player["mgOrigin"]).unwrap_or(origin);
    // A confirmation only fine-tunes an already matching surface at almost the
    // same angle. Crossing an edge always publishes the fresh visible contact.
    let saved_ray = confirmed - old_origin;
    let distance = origin.distance(contact);
    let tangent_error = (confirmed - origin).cross(direction).length();
    let along = (confirmed - origin).dot(direction);
    if saved_ray.dot(direction) <= 0. || (distance - along).abs() >= 0.45 {
        return contact;
    }
    let weight = (1. - tangent_error / 0.09).clamp(0., 1.) * (1. - age / 0.11).clamp(0., 1.);
    origin + direction * (distance - (distance - along).clamp(0., 0.12) * weight)
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
        let ray = mount
            .0
            .ray(body.x, body.z, hull_yaw, turret_yaw, pitch, false);
        return (
            Vec3::from_array(ray.origin) + Vec3::Y * body.y,
            Vec3::from_array(ray.direction),
            58.,
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
    fn server_bores_pass_through_the_rendered_muzzles_at_every_pose() {
        let body = vec3(5., 0., -12.);
        for kind in ["mbt", "apc"] {
            let mount = GunMount::for_vehicle(kind).unwrap();
            for (hull, turret, pitch) in [(0., 0., -1.), (1.9, -0.72, 0.28), (-2.99, 16.3, 0.98)] {
                let barrel = mount.barrel(mount.turret(body, hull, turret), pitch);
                for mg in [false, true] {
                    let ray = mount.0.ray(body.x, body.z, hull, turret, pitch, mg);
                    let socket = if mg {
                        mount.0.mg_tip
                    } else {
                        mount.0.primary_tip
                    };
                    let tip =
                        Vec3::from_array(ray.origin) + Vec3::from_array(ray.direction) * socket[2];
                    assert!(
                        tip.distance(barrel.transform_point3(Vec3::from_array(socket))) < 0.00001
                    );
                }
            }
        }
    }
    #[test]
    fn sustained_mouse_sweeps_do_not_reverse_at_half_or_complete_turns() {
        for sign in [-1., 1.] {
            let mut camera = VehicleCamera::default();
            let dt = 0.01;
            let mut latest = sign * 2.95;
            let mut last = latest;
            let mut packet = 0;
            for frame in 1..=1300 {
                // 100 fps rendering with deliberately uneven confirmations.
                if frame % 6 == 0 && frame % 42 != 0 {
                    latest += sign * (frame - packet) as f32 * dt * TURN_RATE;
                    packet = frame;
                }
                let age = (frame - packet) as f32 * dt;
                let aim = sign * (2.95 + frame as f32 * 0.066);
                let next = camera.turret(5, latest, aim, age, dt);
                assert!((next - last) * sign >= -0.00001);
                assert!((next - latest).abs() < 0.27);
                last = next;
            }
            assert!(last.abs() > 3. * std::f32::consts::TAU);
        }
    }
    #[test]
    fn violent_mouse_sweep_stops_promptly_and_can_reverse_immediately() {
        for sign in [-1_f32, 1.] {
            let start = sign * 3.08;
            let (mut ray, mut command) = (start, start);
            for _ in 0..1200 {
                command = optic_command(command, ray, sign * 0.2);
                let prior = ray;
                ray += (command - ray).clamp(-TURN_RATE * 0.01, TURN_RATE * 0.01);
                assert!((ray - prior) * sign >= 0.);
            }
            assert!((ray - start).abs() > 2. * std::f32::consts::TAU);
            let reverse = optic_command(command, ray, -sign * 0.08);
            assert!((reverse - ray) * sign < 0.);
            let released = ray;
            for _ in 0..50 {
                ray += (command - ray).clamp(-TURN_RATE * 0.01, TURN_RATE * 0.01);
            }
            assert!((ray - command).abs() < 0.0001);
            assert!((ray - released).abs() <= 0.48);
        }
    }
    #[test]
    fn wheel_crosses_into_optics_and_small_reverse_noise_stays_inside() {
        let mut camera = VehicleCamera::default();
        for _ in 0..3 {
            camera.scroll(1.);
        }
        assert!(!camera.onboard);
        camera.scroll(1.);
        assert!(camera.onboard);
        for _ in 0..8 {
            camera.scroll(-0.08);
            camera.scroll(0.08);
        }
        assert!(camera.onboard);
        camera.scroll(-1.);
        assert!(!camera.onboard);
        camera.scroll(0.08);
        assert!(!camera.onboard);
        camera.toggle();
        assert!(camera.onboard);
        assert_eq!(Sensor::Day.next().next().next(), Sensor::Day);
    }
    #[test]
    fn confirmation_stays_on_current_bore_and_never_pushes_past_cover() {
        let p = serde_json::json!({"mgOrigin":[0,1,0], "mgSight":[0,1,-40]});
        let start = vec3(0., 1., 0.);
        let near = vec3(0., 1., -4.);
        assert_eq!(reconcile(start, -Vec3::Z, near, &p, 0.), near);
        let far = vec3(0., 1., -41.);
        assert_eq!(reconcile(start, -Vec3::Z, far, &p, 0.), far);
        let almost = vec3(0., 1., -40.15);
        let result = reconcile(start, -Vec3::Z, almost, &p, 0.);
        assert!(result.distance(almost) < 0.121 && result.z < -40.);
        // After translating and turning the chassis, a stabilized world heading
        // and pitch still map to the same image center on the optic.
        for (x, yaw) in [(0., 0.), (1., 0.5), (4., 3.13), (5., -3.12)] {
            let (eye, axis, _) = onboard("mbt", vec3(x, 0., -x), yaw, 2.1, -1.9, -0.04);
            assert!(eye.is_finite());
            assert!(axis.distance(crate::dir(2.1, -0.04)) < 0.0001);
        }
    }
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
                assert!(
                    look.distance(super::super::dir(
                        2.25,
                        Mount::for_kind(kind).unwrap().pitch(pitch)
                    )) < 0.0001
                );
            }
        }
    }
}
