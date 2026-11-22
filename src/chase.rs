//! Clip the orbit camera against the same building openings the simulation uses.
use frontline_surge::fracture::{FractureState, Layout};
use macroquad::prelude::*;
use serde_json::Value;
use std::sync::OnceLock;
struct Obstacle {
    x: f32,
    z: f32,
    s: f32,
    c: f32,
    w: f32,
    d: f32,
    h: f32,
    layout: Option<Layout>,
}
fn f(o: &Value, k: &str) -> f32 {
    o[k].as_f64().unwrap_or(0.) as f32
}
impl Obstacle {
    fn new(o: &Value) -> Option<Self> {
        if o["solid"] != true {
            return None;
        }
        let size =
            |a: &str, b: &str| o[a].as_f64().unwrap_or_else(|| o[b].as_f64().unwrap_or(0.)) as f32;
        let (s, c) = f(o, "yaw").sin_cos();
        let l = Layout::new(
            o["kind"].as_str().unwrap_or(""),
            f(o, "w"),
            f(o, "h"),
            f(o, "d"),
            o["variant"].as_u64().unwrap_or(0) as u32,
        );
        Some(Self {
            x: f(o, "x"),
            z: f(o, "z"),
            s,
            c,
            w: size("collisionW", "w"),
            d: size("collisionD", "d"),
            h: size("collisionH", "h"),
            layout: l,
        })
    }
    fn clip(&self, eye: Vec3, n: Vec3, limit: f32, delta: Option<&Value>) -> f32 {
        if delta.is_some_and(|v| v["dead"] == true) {
            return limit;
        }
        let off = vec2(eye.x - self.x, eye.z - self.z);
        if off.length_squared() > (self.w.hypot(self.d) * 0.5 + limit).powi(2) {
            return limit;
        }
        let o = [
            off.x * self.c + off.y * self.s,
            eye.y,
            off.y * self.c - off.x * self.s,
        ];
        let d = [
            n.x * self.c + n.z * self.s,
            n.y,
            n.z * self.c - n.x * self.s,
        ];
        let mut near = 0.0_f32;
        let mut far = limit;
        for (i, lo, hi) in [
            (0, -self.w * 0.5, self.w * 0.5),
            (1, 0., self.h),
            (2, -self.d * 0.5, self.d * 0.5),
        ] {
            if d[i].abs() < 1e-6 {
                if o[i] < lo || o[i] > hi {
                    return limit;
                }
            } else {
                let a = (lo - o[i]) / d[i];
                let b = (hi - o[i]) / d[i];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if near > far {
                    return limit;
                }
            }
        }
        if let Some(layout) = &self.layout {
            let mut state = FractureState::default();
            if let Some(d) = delta {
                state.broken.extend(
                    d["broken"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_u64)
                        .map(|i| i as u16),
                );
            }
            layout.ray(&state, o, d, limit).unwrap_or(limit)
        } else {
            near
        }
    }
}
pub fn clip(eye: Vec3, desired: Vec3, state: &Value) -> Vec3 {
    static STATIC: OnceLock<Vec<Option<Obstacle>>> = OnceLock::new();
    let obs = STATIC.get_or_init(|| {
        let level: Value = serde_json::from_str(include_str!("../assets/level.json")).unwrap();
        level["props"]
            .as_array()
            .unwrap()
            .iter()
            .map(Obstacle::new)
            .collect()
    });
    let v = desired - eye;
    let len = v.length();
    if len < 0.02 {
        return eye;
    }
    let n = v / len;
    let mut limit = len;
    for (i, item) in obs.iter().enumerate() {
        let Some(o) = item else {
            continue;
        };
        if (eye.x - o.x).abs() > 14. + o.w || (eye.z - o.z).abs() > 14. + o.d {
            continue;
        }
        let delta = state["propChanges"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|v| v["index"].as_u64() == Some(i as u64) || v["i"].as_u64() == Some(i as u64));
        limit = o.clip(eye, n, limit, delta);
    }
    for p in state["builtProps"].as_array().into_iter().flatten() {
        if (f(p, "x") - eye.x).abs() < 20. && (f(p, "z") - eye.z).abs() < 20. {
            if let Some(o) = Obstacle::new(p) {
                limit = o.clip(eye, n, limit, Some(p));
            }
        }
    }
    eye + n * (limit - 0.18).max(0.)
}
