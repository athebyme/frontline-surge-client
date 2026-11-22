//! Wheels and track links are attached to the authored chassis in vehicle-local space.
//! Server travel is signed: driving backwards reverses both the running gear and belt.
use super::*;

struct RunningGear {
    axles: &'static [f32],
    track: f32,
    radius: f32,
}
fn running_gear(kind: &str) -> Option<RunningGear> {
    let (axles, track, radius): (&[f32], f32, f32) = match kind {
        "scout_buggy" => (&[-1.05, 1.06], 0.84, 0.426),
        "patrol4x4" => (&[-1.12, 1.16], 0.80, 0.431),
        "cargo6x6" => (&[-2.05, -0.93, 1.93], 1.02, 0.523),
        "apc" => (&[-2.08, -0.74, 0.68, 2.03], 1.19, 0.523),
        "mbt" => (&[-2.13, -1.44, -0.75, -0.06, 0.63, 1.32, 2.01], 1.38, 0.336),
        _ => return None,
    };
    Some(RunningGear {
        axles,
        track,
        radius,
    })
}
impl World {
    pub(super) fn place_drivetrain(
        &self,
        kind: &str,
        pose: VehiclePose,
        body: Mat4,
        out: &mut Vec<Placement>,
    ) {
        let Some(gear) = running_gear(kind) else {
            return;
        };
        let wheel = format!("wheel_{kind}");
        let tracked = kind == "mbt";
        for side in [-1., 1.] {
            let travel = pose.travel + side * gear.track * pose.differential;
            let spin =
                Mat4::from_rotation_x((travel / gear.radius).rem_euclid(std::f32::consts::TAU));
            let mirror = Mat4::from_scale(vec3(side, 1., 1.));
            for &z in gear.axles {
                let steering = if !tracked && z > 0. {
                    -pose.steer * (if kind == "apc" { 0.32 } else { 0.51 })
                } else {
                    0.
                };
                let center = vec3(
                    side * gear.track,
                    if tracked { 0.52 } else { gear.radius },
                    z,
                );
                let matrix = body
                    * Mat4::from_translation(center)
                    * Mat4::from_rotation_y(steering)
                    * spin
                    * mirror;
                self.place(&wheel, matrix, out);
            }
            if tracked {
                let pitch = (2. * 4.26 + std::f32::consts::TAU * 0.324) / 42.;
                let frame = ((travel / pitch).rem_euclid(1.) * 12.).floor() as usize;
                self.place(
                    &format!("belt_{frame}"),
                    body * Mat4::from_translation(vec3(side * 1.20, 0., 0.)),
                    out,
                );
            }
        }
    }
    pub(super) fn place_industry(
        &self,
        s: &Value,
        out: &mut Vec<Placement>,
        lights: &mut Vec<(Vec3, f32)>,
    ) {
        let Some(factories) = s["factories"].as_array() else {
            return;
        };
        for f in factories {
            if f["kind"] != "refinery" {
                continue;
            }
            let center = vec3(number(f, "x", 0.), 0., number(f, "z", 0.));
            let size = vec3(
                number(f, "w", 3.4) / 3.4,
                number(f, "h", 2.6) / 3.,
                number(f, "d", 3.1) / 3.1,
            );
            let matrix = Mat4::from_scale_rotation_translation(
                size,
                Quat::from_rotation_y(-number(f, "yaw", 0.)),
                center,
            );
            self.place("refinery_hardware", matrix, out);
            let remainder = number(f, "remaining", 0.);
            let operating = remainder > 0. && number(f, "pending", 0.) > 0.;
            let fan_turn = if operating {
                number(s, "elapsed", 0.) * 9.5
            } else {
                0.
            };
            self.place(
                "refinery_fan",
                matrix
                    * Mat4::from_translation(vec3(0.49, 0.766, 1.158))
                    * Mat4::from_rotation_z(fan_turn),
                out,
            );
            let progress = if operating {
                (1. - remainder / number(f, "duration", 12.).max(0.01)).clamp(0., 1.)
            } else if number(f, "ready", 0.) > 0. {
                1.
            } else {
                0.
            };
            let bars = (progress * 4.).ceil() as usize;
            for bar in 0..bars {
                self.place(
                    "refinery_gauge",
                    matrix * Mat4::from_translation(vec3(1.19, 0.79 + bar as f32 * 0.076, 0.823)),
                    out,
                );
            }
            let mode = if operating {
                "working"
            } else if number(f, "ready", 0.) > 0. {
                "ready"
            } else {
                "idle"
            };
            self.place(
                &format!("lamp_{mode}"),
                matrix * Mat4::from_translation(vec3(1.19, 1.29, 0.64)),
                out,
            );
            if operating {
                lights.push((matrix.transform_point3(vec3(1.19, 1.64, 0.86)), 4.5));
            }
        }
    }
}

pub(super) fn draw_shells(state: Option<&Value>, eye: Vec3) {
    let Some(shells) = state.and_then(|s| s["shells"].as_array()) else {
        return;
    };
    for shell in shells {
        let at = vec3(
            number(shell, "x", 0.),
            number(shell, "y", 1.),
            number(shell, "z", 0.),
        );
        let velocity = vec3(
            number(shell, "vx", 0.),
            number(shell, "vy", 0.),
            number(shell, "vz", 0.),
        );
        let dir = velocity.normalize_or_zero();
        if dir.length_squared() < 0.5 {
            continue;
        }
        let cross = dir.cross(eye - at).normalize_or_zero() * 0.052;
        let tail = at - dir * (velocity.length() * 0.045).clamp(0.6, 2.8);
        draw_line_3d(tail, at, Color::from_rgba(246, 171, 89, 246));
        draw_line_3d(at - cross, at + cross, Color::from_rgba(255, 223, 161, 255));
        draw_sphere(at, 0.066, None, Color::from_rgba(255, 235, 188, 255));
    }
}
