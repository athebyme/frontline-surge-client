//! Shared poses are sampled in normalized reload time. All distances are in model space.
use super::{rx, ry, rz, smooth, translate};
use macroquad::prelude::*;

#[derive(Clone, Copy, Default)]
pub(super) struct ReloadPose {
    pub ready: f32,
    pub grasp: f32,
    pub weapon_offset: Vec3,
    pub weapon_rotation: Vec3,
    pub feed_offset: Vec3,
    pub feed_twist: f32,
    pub support_offset: Vec3,
    pub wrist_roll: f32,
    pub cycle: f32,
}

impl ReloadPose {
    pub fn sample(weapon: &str, t: f32) -> Self {
        if !(0.0..1.0).contains(&t) || t == 0. {
            return Self::default();
        }
        // The compact model has a short, straight pull. The long rifle stays braced
        // longer and the carbine swings the feed in from the side.
        let (reach, out, seat, settle, depth, side, roll) = match weapon {
            "vx9" => (0.14, 0.34, 0.63, 0.81, 0.19, -0.092, -0.36),
            "mr7" => (0.22, 0.46, 0.76, 0.90, 0.14, -0.085, -0.53),
            _ => (0.18, 0.40, 0.69, 0.85, 0.17, -0.142, -0.60),
        };
        let ease = |a, b| smooth((t - a) / (b - a));
        let present = ease(0., reach) - ease(settle - 0.07, 1.);
        let grasp = ease(0.025, reach) - ease(seat + 0.02, settle);
        let remove = ease(reach, out);
        let insert = ease(out + 0.055, seat);
        let away = remove - insert;
        let exchange = ease(out - 0.08, out + 0.09);
        let seat_tap = ease(seat - 0.06, seat) - ease(seat, seat + 0.07);
        let finish = ease(seat + 0.03, settle) - ease(settle, 0.98);
        let feed_offset = vec3(
            side * (0.45 + 0.55 * exchange) * away,
            -depth * away + 0.009 * seat_tap,
            (0.055 - 0.10 * exchange) * away,
        );
        let grip = match weapon {
            "vx9" => vec3(0.035, -0.087, 0.228),
            "mr7" => vec3(0.033, -0.051, 0.247),
            _ => vec3(0.04, -0.071, 0.256),
        };
        Self {
            ready: present,
            grasp,
            weapon_offset: vec3(
                -0.079 * present - 0.014 * away,
                0.042 * present - 0.014 * away,
                0.034 * present,
            ),
            weapon_rotation: vec3(
                0.16 * present + 0.04 * seat_tap,
                0.28 * present + 0.07 * away,
                roll * present + 0.07 * finish,
            ),
            feed_offset,
            feed_twist: (0.18 - 0.51 * exchange) * away,
            support_offset: grip * grasp + feed_offset + vec3(0.009, 0.028, 0.031) * finish,
            wrist_roll: -0.17 * grasp - 0.14 * away + 0.18 * finish,
            cycle: finish,
        }
    }
    pub fn weapon(&self, weight: f32) -> Mat4 {
        translate(self.weapon_offset * weight)
            * ry(self.weapon_rotation.y * weight)
            * rx(self.weapon_rotation.x * weight)
            * rz(self.weapon_rotation.z * weight)
    }
}

/// Keeps the upper arm at its shoulder and the forearm attached to its elbow while
/// the hand follows the animated grip. The bend plane is derived from the bind pose.
pub(super) fn arm_pose(left: bool, target_offset: Vec3, flare: f32) -> (Mat4, Mat4) {
    let (shoulder, elbow, hand) = if left {
        (
            vec3(-0.222, 0.035, 0.),
            vec3(-0.294, -0.153, -0.109),
            vec3(-0.061, -0.047, -0.431),
        )
    } else {
        (
            vec3(0.222, 0.035, 0.),
            vec3(0.294, -0.153, 0.060),
            vec3(0.090, -0.057, -0.246),
        )
    };
    let upper = elbow - shoulder;
    let lower = hand - elbow;
    let target = hand + target_offset - shoulder;
    let axis = target.normalize_or_zero();
    let upper_len = upper.length();
    let lower_len = lower.length();
    let reach = target.length().clamp(
        (upper_len - lower_len).abs() + 0.001,
        upper_len + lower_len - 0.001,
    );
    let along = (upper.length_squared() - lower.length_squared() + reach * reach) / (2. * reach);
    let pole = upper + vec3(if left { -flare } else { flare }, 0., 0.);
    let bend = (pole - axis * pole.dot(axis)).normalize_or_zero();
    let joint =
        shoulder + axis * along + bend * (upper_len * upper_len - along * along).max(0.).sqrt();
    let pivot = |from: Vec3, to: Vec3, base: Vec3, dest: Vec3| {
        translate(dest)
            * Mat4::from_quat(Quat::from_rotation_arc(from.normalize(), to.normalize()))
            * translate(-base)
    };
    (
        pivot(upper, joint - shoulder, shoulder, shoulder),
        pivot(lower, shoulder + axis * reach - joint, elbow, joint),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reloads_keep_their_endpoints_and_move_continuously() {
        for gun in ["vx9", "ar4", "mr7"] {
            let mut prev = ReloadPose::sample(gun, 0.);
            for step in 1..=1000 {
                let pose = ReloadPose::sample(gun, step as f32 / 1000.);
                assert!(pose.support_offset.distance(prev.support_offset) < 0.007);
                assert!((pose.feed_twist - prev.feed_twist).abs() < 0.012);
                assert!(pose.weapon(1.).is_finite());
                prev = pose;
            }
            assert_eq!(prev.weapon(1.), Mat4::IDENTITY);
            assert_eq!(prev.support_offset, Vec3::ZERO);
            assert_eq!(prev.feed_offset, Vec3::ZERO);
        }
        assert!(
            ReloadPose::sample("ar4", 0.5).feed_offset.x
                < ReloadPose::sample("vx9", 0.5).feed_offset.x
        );
    }

    #[test]
    fn seated_magazine_and_left_hand_travel_together_and_arms_stay_connected() {
        let a = ReloadPose::sample("ar4", 0.40);
        let b = ReloadPose::sample("ar4", 0.55);
        assert!(
            ((b.support_offset - a.support_offset) - (b.feed_offset - a.feed_offset)).length()
                < 0.0001
        );
        for left in [false, true] {
            let elbow = if left {
                vec3(-0.294, -0.153, -0.109)
            } else {
                vec3(0.294, -0.153, 0.060)
            };
            let (upper, lower) = arm_pose(left, b.support_offset * 0.76, 0.027);
            assert!(
                upper
                    .transform_point3(elbow)
                    .distance(lower.transform_point3(elbow))
                    < 0.0001
            );
        }
    }
}
