//! Render on a short, shared server-time timeline. Gameplay keeps the latest snapshot.
use macroquad::prelude::{vec3, Vec2, Vec3};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};

const BUFFER: f64 = 0.065;

#[derive(Clone, Copy, Default)]
pub struct Pose {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub turret: f32,
    pub steering: f32,
    pub travel: f32,
    pub roll: f32,
    pub bob: f32,
    pub stride: f32,
    deaths: u64,
    seat: Option<u64>,
    live: bool,
}
fn scalar(v: &Value, field: &str) -> f32 {
    v[field].as_f64().unwrap_or_default() as f32
}

impl Pose {
    fn read(v: &Value) -> Self {
        let yaw = scalar(v, "yaw");
        Self {
            position: vec3(scalar(v, "x"), scalar(v, "y"), scalar(v, "z")),
            yaw,
            pitch: scalar(v, "pitch"),
            turret: v["turretYaw"].as_f64().map_or(yaw, |s| s as f32),
            steering: scalar(v, "steering"),
            travel: scalar(v, "travel"),
            roll: scalar(v, "roll"),
            bob: scalar(v, "bob"),
            stride: if v["moving"] == true { 1. } else { 0. },
            deaths: v["deaths"].as_u64().unwrap_or(0),
            seat: v["vehicleId"].as_u64(),
            live: v["alive"] != false && v["destroyed"] != true,
        }
    }
    fn continuous(self, next: Self, jump: f32) -> bool {
        self.deaths == next.deaths
            && self.seat == next.seat
            && self.live == next.live
            && self.position.distance_squared(next.position) <= jump * jump
    }
    fn between(self, next: Self, t: f32) -> Self {
        let lerp = |a, b| a + (b - a) * t;
        let angle = |a: f32, b: f32| a + (b - a).sin().atan2((b - a).cos()) * t;
        Self {
            position: self.position.lerp(next.position, t),
            yaw: angle(self.yaw, next.yaw),
            pitch: lerp(self.pitch, next.pitch),
            turret: angle(self.turret, next.turret),
            bob: angle(self.bob, next.bob),
            stride: lerp(self.stride, next.stride),
            roll: lerp(self.roll, next.roll),
            travel: lerp(self.travel, next.travel),
            steering: lerp(self.steering, next.steering),
            ..next
        }
    }
}

type Poses = HashMap<u64, Pose>;
struct Frame {
    at: f64,
    players: Poses,
    vehicles: Poses,
    drones: Poses,
}
impl Frame {
    fn read(state: &Value, at: f64) -> Self {
        let group = |key: &str| {
            state[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| Some((v["id"].as_u64()?, Pose::read(v))))
                .collect()
        };
        Self {
            at,
            players: group("players"),
            vehicles: group("vehicles"),
            drones: group("drones"),
        }
    }
}

#[derive(Default)]
pub struct Motion {
    frames: VecDeque<Frame>,
    round: Option<u64>,
    cursor: f64,
    speed: f64,
    pub age: f32,
    pub local_gun_pitch: Option<(u64, f32)>,
}
impl Motion {
    pub fn receive(&mut self, state: &Value) {
        let Some(at) = state["elapsed"].as_f64() else {
            return;
        };
        let round = state["round"].as_u64();
        if self.round != round || self.frames.back().is_none_or(|f| at - f.at > 0.25) {
            self.frames.clear();
            self.round = round;
            self.cursor = at - BUFFER;
            self.speed = 1.;
        } else if self.frames.back().is_some_and(|f| at <= f.at) {
            return;
        } else {
            // Trim clock drift gradually. Arrival jitter must not rewind a visible frame.
            self.speed = (1. + (at - BUFFER - self.cursor) * 2.).clamp(0.90, 1.10);
        }
        self.age = 0.;
        self.frames.push_back(Frame::read(state, at));
        while self.frames.len() > 8 {
            self.frames.pop_front();
        }
    }
    pub fn advance(&mut self, dt: f32) {
        if let Some(last) = self.frames.back() {
            self.cursor = (self.cursor + f64::from(dt) * self.speed).min(last.at);
            self.age += dt;
        }
    }
    fn sample(&self, v: &Value, group: fn(&Frame) -> &Poses, jump: f32) -> Pose {
        let Some(id) = v["id"].as_u64() else {
            return Pose::read(v);
        };
        let Some(latest) = self.frames.back().and_then(|f| group(f).get(&id)).copied() else {
            return Pose::read(v);
        };
        let mut ahead = latest;
        let mut end = self.frames.back().unwrap().at;
        for frame in self.frames.iter().rev().skip(1) {
            let Some(&behind) = group(frame).get(&id) else {
                break;
            };
            if !behind.continuous(ahead, jump) {
                break;
            }
            if frame.at <= self.cursor {
                let t = ((self.cursor - frame.at) / (end - frame.at)).clamp(0., 1.) as f32;
                return behind.between(ahead, t);
            }
            ahead = behind;
            end = frame.at;
        }
        ahead
    }
    pub fn player(&self, v: &Value) -> Pose {
        self.sample(v, |f| &f.players, 3.)
    }
    pub fn vehicle(&self, v: &Value) -> Pose {
        self.sample(v, |f| &f.vehicles, 9.)
    }
    pub fn drone(&self, v: &Value) -> Pose {
        self.sample(v, |f| &f.drones, 9.)
    }
}

// While grabbed, Macroquad accumulates raw OS motion events into this pixel position.
// Forget the free pointer when a menu takes focus; resizing must not produce a turn.
#[derive(Default)]
pub struct LookDelta(Option<Vec2>);
impl LookDelta {
    pub fn sample(&mut self, captured: Option<Vec2>) -> Vec2 {
        let delta = self
            .0
            .zip(captured)
            .map_or(Vec2::ZERO, |(previous, now)| now - previous);
        self.0 = captured;
        delta
    }
}

pub fn view_angles(forward: Vec3) -> (f32, f32) {
    (
        forward.x.atan2(-forward.z),
        forward.y.atan2(forward.x.hypot(forward.z)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Same 100 Hz camera sweep and walk, same 20 Hz server samples.
    #[test]
    fn frame_pacing_at_100_fps_with_20_hz_snapshots() {
        let mut motion = Motion::default();
        let mut p = Value::Null;
        let mut old_x = 0.0_f32;
        let mut last = [0.0; 4];
        let mut changed = [0; 4];
        let mut min_dx = [f32::MAX; 2];
        let mut max_dx = [0.0_f32; 2];
        for frame in 0..=200 {
            let time = f64::from(frame) / 100.;
            motion.advance(0.01);
            if frame % 5 == 0 {
                p = json!({"id":1, "x":time*4., "yaw":time});
                motion.receive(&json!({"elapsed":time, "round":1, "players":[p]}));
            }
            let cam_yaw = time as f32;
            let direction = crate::dir(cam_yaw, 0.4);
            old_x += (scalar(&p, "x") - old_x) * (1. - (-0.26_f32).exp());
            let values = [
                scalar(&p, "yaw"),
                view_angles(direction).0,
                old_x,
                motion.player(&p).position.x,
            ];
            if frame > 100 {
                for i in 0..4 {
                    let delta = values[i] - last[i];
                    changed[i] += usize::from(delta.abs() > 0.00001);
                    if i >= 2 {
                        min_dx[i - 2] = min_dx[i - 2].min(delta);
                        max_dx[i - 2] = max_dx[i - 2].max(delta);
                    }
                }
            }
            last = values;
        }
        println!("compass updates per 100 frames: snapshot {}, live {}; step distance in metres: chasing snapshots {:.4}–{:.4}, timestamped interpolation {:.4}–{:.4}", changed[0], changed[1], min_dx[0], max_dx[0], min_dx[1], max_dx[1]);
        assert_eq!(changed, [20, 100, 100, 100]);
        assert!(max_dx[0] - min_dx[0] > 0.025);
        assert!((min_dx[1] - 0.04).abs() < 0.0005 && (max_dx[1] - 0.04).abs() < 0.0005);
    }

    #[test]
    fn jitter_stalls_respawns_and_reused_ids_do_not_blend_across_breaks() {
        let mut motion = Motion::default();
        let p = |x, deaths| json!({"id":1,"x":x,"deaths":deaths});
        let snap = |at, players: Vec<Value>| json!({"round":1, "elapsed":at,"players":players});
        let mut last = 0.;
        for frame in 0..40 {
            motion.advance(0.01);
            let arrival = [
                (0, 0.),
                (4, 0.05),
                (11, 0.10),
                (15, 0.15),
                (23, 0.2),
                (30, 0.25),
            ];
            if let Some(&(_, at)) = arrival.iter().find(|(n, _)| *n == frame) {
                motion.receive(&snap(at, vec![p(at, 0)]));
            }
            let x = motion.player(&p(0.25, 0)).position.x;
            assert!(x >= last && x <= 0.25001);
            last = x;
        }
        motion.advance(2.);
        assert_eq!(motion.player(&p(0.25, 0)).position.x, 0.25);
        motion.receive(&snap(0.30, vec![p(75., 1)]));
        assert_eq!(motion.player(&p(75., 1)).position.x, 75.);
        motion.receive(&snap(0.35, vec![]));
        motion.receive(&snap(0.40, vec![p(16., 0)]));
        assert_eq!(motion.player(&p(16., 0)).position.x, 16.);
        motion.receive(&json!({"elapsed":0., "round":2, "players":[p(9.,0)]}));
        assert_eq!(motion.player(&p(9., 0)).position.x, 9.);
    }

    #[test]
    fn mouse_uses_only_motion_since_capture() {
        let mut look = LookDelta::default();
        let initial = Some(Vec2::new(2000., -240.));
        assert_eq!(look.sample(initial), Vec2::ZERO);
        assert_eq!(
            look.sample(Some(Vec2::new(2003., -244.))),
            Vec2::new(3., -4.)
        );
        assert_eq!(look.sample(None), Vec2::ZERO);
        assert_eq!(look.sample(Some(Vec2::new(8., 4.))), Vec2::ZERO);
        assert_eq!(look.sample(Some(Vec2::new(18., 4.))), Vec2::new(10., 0.));
    }

    #[test]
    fn angles_cross_north_and_wrap_on_shortest_arc() {
        let a = Pose {
            turret: 179_f32.to_radians(),
            bob: 6.2,
            ..Default::default()
        };
        let b = Pose {
            turret: -179_f32.to_radians(),
            bob: 0.1,
            ..Default::default()
        };
        let mid = a.between(b, 0.5);
        assert!((mid.turret.to_degrees().abs() - 180.).abs() < 0.01);
        assert!(mid.bob.cos() > 0.99);
    }
}
