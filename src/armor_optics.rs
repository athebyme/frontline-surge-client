//! Instruments for the centered armored sight. Readouts use the displayed gun pose.
use super::{alpha, number, string, Surface, INK, PAPER};
use macroquad::prelude::*;
use serde_json::Value;

pub(super) fn draw(s: &Surface, camera: &Camera3D, player: &Value, vehicle: &Value) {
    let cx = s.w * 0.5;
    let cy = s.h * 0.5;
    let half = (s.w * 0.18).min(s.h * 0.29);
    let height = half * 0.56;
    let color = match string(player, "opticSensor", "ДЕНЬ") {
        "ПНВ" => Color::new(0.78, 1., 0.84, 0.96),
        "ТЕПЛО" => Color::new(1., 0.77, 0.40, 0.98),
        _ => Color::new(0.69, 0.97, 0.88, 0.97),
    };
    let pitch = number(player, "gunPitch", 0.) as f32;
    let heading = number(player, "gunYaw", 0.).to_degrees().rem_euclid(360.);
    let focal = cy / (camera.fovy * 0.5).tan();
    // Pitch ladder anchored to the real horizontal plane as the optic moves.
    for degrees in (-35..=40).step_by(5) {
        let y = cy + (pitch - (degrees as f32).to_radians()).tan() * focal;
        if (y - cy).abs() > height - 12. {
            continue;
        }
        for sign in [-1., 1.] {
            let x = cx + sign * (half - 12.);
            let width = if degrees == 0 {
                27.
            } else if degrees % 10 == 0 {
                17.
            } else {
                8.
            };
            for (th, ink) in [(2.6, alpha(INK, 0.84)), (1.15, color)] {
                s.line(x, y, x - sign * width, y, th, ink);
                if degrees == 0 {
                    s.line(x - sign * width, y, x - sign * width, y - 4., th, ink);
                }
            }
        }
    }
    for dx in [-1., 1.] {
        for dy in [-1., 1.] {
            let (x, y) = (cx + dx * half, cy + dy * height);
            for (th, ink) in [(3., INK), (1.25, color)] {
                s.line(x, y, x - dx * 18., y, th, ink);
                s.line(x, y, x, y - dy * 10., th, ink);
            }
        }
    }
    let mode = string(player, "opticSensor", "ДЕНЬ");
    let left = if vehicle["kind"] == "mbt" {
        "ТАНК"
    } else {
        "БТР"
    };
    let zoom = number(player, "opticZoom", 1.);
    let zoom_label = if (zoom - zoom.round()).abs() < 0.05 {
        format!("{zoom:.0}×")
    } else {
        format!("{zoom:.1}×")
    };
    let line = format!("{left}   {mode}   ·   {zoom_label}   Z");
    let readout = format!(
        "{:03}°   {:.0} м",
        heading.round() as i32 % 360,
        number(player, "sightRange", 0.)
    );
    let y = cy + height + 17.;
    s.rect(
        cx - half,
        y - 14.,
        s.measure(&line, 11.) + 12.,
        19.,
        alpha(INK, 0.72),
    );
    s.text(&line, cx - half + 6., y, 11., color);
    let w = s.measure(&readout, 11.) + 12.;
    s.rect(cx + half - w, y - 14., w, 19., alpha(INK, 0.72));
    s.right(&readout, cx + half - 6., y, 11., PAPER);
}
