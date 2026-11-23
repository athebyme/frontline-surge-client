//! Map collision for the chase camera and the mounted sight. Authored geometry is
//! immutable; only reliable map updates change the cells used in render-frame queries.
use crate::motion::Motion;
use frontline_surge::fracture::{FractureState, Layout};
use macroquad::prelude::*;
use serde_json::Value;
use std::sync::{Arc, OnceLock};

fn number(v: &Value, key: &str) -> f32 {
    v[key].as_f64().unwrap_or_default() as f32
}

struct Hull {
    base: Vec3,
    half: Vec3,
    sin: f32,
    cos: f32,
    extent: Vec2,
}
impl Hull {
    fn new(base: Vec3, size: Vec3, yaw: f32) -> Self {
        let (sin, cos) = yaw.sin_cos();
        let half = size * 0.5;
        let extent = vec2(
            half.x * cos.abs() + half.z * sin.abs(),
            half.x * sin.abs() + half.z * cos.abs(),
        );
        Self {
            base,
            half,
            sin,
            cos,
            extent,
        }
    }
    fn local(&self, v: Vec3) -> Vec3 {
        vec3(
            v.x * self.cos + v.z * self.sin,
            v.y,
            v.z * self.cos - v.x * self.sin,
        )
    }
    fn local_segment(&self, origin: Vec3, dir: Vec3, limit: f32) -> Option<(Vec3, Vec3, f32)> {
        let from = origin - self.base;
        let to = from + dir * limit;
        if from.x.min(to.x) > self.extent.x
            || from.x.max(to.x) < -self.extent.x
            || from.z.min(to.z) > self.extent.y
            || from.z.max(to.z) < -self.extent.y
            || from.y.min(to.y) > 2. * self.half.y
            || from.y.max(to.y) < 0.
        {
            return None;
        }
        let o = self.local(from);
        let d = self.local(dir);
        let center = o - Vec3::Y * self.half.y;
        let (mut near, mut far) = (0.0_f32, limit);
        for axis in 0..3 {
            let (c, v, h) = (center[axis], d[axis], self.half[axis]);
            if v.abs() < 1e-7 {
                if c.abs() > h {
                    return None;
                }
            } else {
                let a = (-h - c) / v;
                let b = (h - c) / v;
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if near > far {
                    return None;
                }
            }
        }
        Some((o, d, near))
    }
}

struct Obstacle {
    hull: Hull,
    layout: Option<Layout>,
}
impl Obstacle {
    fn read(v: &Value) -> Option<Self> {
        if v["solid"] != true || v["dead"] == true {
            return None;
        }
        let size = vec3(number(v, "w"), number(v, "h"), number(v, "d"));
        let layout = Layout::new(
            v["kind"].as_str().unwrap_or(""),
            size.x,
            size.y,
            size.z,
            number(v, "variant") as u32,
        );
        // Cell layouts own their geometric limits. The small margin matches the
        // tolerance of shared cells at adjoining panel edges.
        let bounds = if layout.is_some() {
            size + Vec3::splat(0.0001)
        } else {
            vec3(
                v["collisionW"].as_f64().map_or(size.x, |n| n as f32),
                v["collisionH"].as_f64().map_or(size.y, |n| n as f32),
                v["collisionD"].as_f64().map_or(size.z, |n| n as f32),
            )
        };
        let hull = Hull::new(
            vec3(number(v, "x"), 0., number(v, "z")),
            bounds,
            number(v, "yaw"),
        );
        Some(Self { hull, layout })
    }
    fn distance(&self, from: Vec3, dir: Vec3, limit: f32, damage: &MapDamage) -> f32 {
        if damage.dead {
            return limit;
        }
        let Some((o, d, near)) = self.hull.local_segment(from, dir, limit) else {
            return limit;
        };
        match &self.layout {
            Some(layout) => layout
                .ray(&damage.fracture, o.to_array(), d.to_array(), limit)
                .unwrap_or(limit),
            None => near,
        }
    }
}

#[derive(Default)]
struct MapDamage {
    dead: bool,
    fracture: FractureState,
}
impl MapDamage {
    fn receive(&mut self, v: &Value) {
        self.dead = v["dead"] == true;
        let revision = v["revision"].as_u64().unwrap_or(0) as u32;
        let broken = v["broken"].as_array();
        let cuts = || v["cuts"].as_array().into_iter().flatten()
            .filter_map(|n| u32::try_from(n.as_u64()?).ok());
        // Full lists also handle joining a different match with the same round,
        // slot and revision. The reliable wire list contains sorted cell IDs.
        if revision == self.fracture.revision
            && self.fracture.packed_cuts().eq(cuts())
            && self.fracture.broken.len() == broken.map_or(0, Vec::len)
            && self
                .fracture
                .broken
                .iter()
                .copied()
                .map(u64::from)
                .eq(broken.into_iter().flatten().filter_map(Value::as_u64))
        {
            return;
        }
        self.fracture.revision = revision;
        self.fracture.replace_cuts(cuts());
        self.fracture.broken.clear();
        self.fracture.broken.extend(
            broken
                .into_iter()
                .flatten()
                .filter_map(|n| u16::try_from(n.as_u64()?).ok()),
        );
    }
}

struct BuiltObstacle {
    id: u64,
    geom: Obstacle,
    damage: MapDamage,
}

pub struct Scene {
    authored: Arc<[Option<Obstacle>]>,
    damage: Vec<MapDamage>,
    changed: Vec<bool>,
    built: Vec<Option<BuiltObstacle>>,
    round: Option<u64>,
    elapsed: f64,
}
impl Default for Scene {
    fn default() -> Self {
        static LEVEL: OnceLock<Arc<[Option<Obstacle>]>> = OnceLock::new();
        let map = LEVEL.get_or_init(|| {
            let level: Value =
                serde_json::from_str(include_str!("../assets/level.json")).expect("embedded level");
            level["props"]
                .as_array()
                .unwrap()
                .iter()
                .map(Obstacle::read)
                .collect::<Vec<_>>()
                .into()
        });
        Self::new(map.clone())
    }
}
impl Scene {
    fn new(authored: Arc<[Option<Obstacle>]>) -> Self {
        let count = authored.len();
        Self {
            authored,
            damage: (0..count).map(|_| MapDamage::default()).collect(),
            changed: vec![false; count],
            built: Vec::new(),
            round: None,
            elapsed: 0.,
        }
    }

    pub fn receive(&mut self, state: &Value) {
        let round = state["round"].as_u64();
        let elapsed = state["elapsed"].as_f64().unwrap_or(0.);
        if round != self.round || elapsed < self.elapsed {
            self.built.clear();
        }
        self.round = round;
        self.elapsed = elapsed;
        self.changed.fill(false);
        for p in state["propChanges"].as_array().into_iter().flatten() {
            let Some(i) = p["index"]
                .as_u64()
                .or_else(|| p["i"].as_u64())
                .and_then(|i| usize::try_from(i).ok())
            else {
                continue;
            };
            if let Some(damage) = self.damage.get_mut(i) {
                self.changed[i] = true;
                damage.receive(p);
            }
        }
        for (d, &changed) in self.damage.iter_mut().zip(&self.changed) {
            if !changed && (d.dead || d.fracture.revision != 0 || !d.fracture.broken.is_empty()) {
                *d = MapDamage::default();
            }
        }
        let props = state["builtProps"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        self.built.resize_with(props.len(), || None);
        for (slot, p) in self.built.iter_mut().zip(props) {
            if p["dead"] == true || p["solid"] != true {
                *slot = None;
                continue;
            }
            let id = p["buildId"].as_u64();
            if id.is_none() || slot.as_ref().map(|b| b.id) != id {
                *slot = Obstacle::read(p).map(|geom| BuiltObstacle {
                    id: id.unwrap_or(0),
                    geom,
                    damage: MapDamage::default(),
                });
            }
            if let Some(b) = slot {
                b.damage.receive(p);
            }
        }
    }

    fn map_distance(&self, from: Vec3, dir: Vec3, mut limit: f32) -> f32 {
        for (prop, d) in self.authored.iter().zip(&self.damage) {
            if let Some(p) = prop {
                limit = p.distance(from, dir, limit, d);
            }
        }
        for b in self.built.iter().flatten() {
            limit = b.geom.distance(from, dir, limit, &b.damage);
        }
        limit
    }

    pub fn clip(&self, focus: Vec3, desired: Vec3) -> Vec3 {
        let delta = desired - focus;
        let len = delta.length();
        if len < 0.02 {
            return focus;
        }
        let dir = delta / len;
        focus + dir * (self.map_distance(focus, dir, len) - 0.18).max(0.)
    }

    pub fn machinegun_contact(
        &self,
        origin: Vec3,
        direction: Vec3,
        range: f32,
        state: &Value,
        driver: &Value,
        motion: &Motion,
    ) -> Vec3 {
        let dir = direction.normalize_or_zero();
        if dir == Vec3::ZERO {
            return origin;
        }
        let floor = if dir.y < -1e-6 {
            -origin.y / dir.y
        } else {
            f32::INFINITY
        };
        let mut best = self.map_distance(origin, dir, range.min(floor).max(0.));
        for v in state["vehicles"].as_array().into_iter().flatten() {
            if driver["vehicleId"] == v["id"] || driver["id"] == v["driver"] {
                continue;
            }
            let pose = motion.vehicle(v);
            if v["kind"] == "scout_buggy" && v["destroyed"] != true {
                let ray = frontline_surge::buggy::SightLine::new(pose.position.x, pose.position.z,
                    pose.yaw, (origin - Vec3::Y * pose.position.y).to_array(), dir.to_array());
                if let Some(t) = ray.hull(best) { best = t; }
                if v["driver"].is_number() {
                    if let Some(t) = ray.driver(best) { best = t; }
                }
                continue;
            }
            let mut size = match v["kind"].as_str() {
                Some("scout_buggy") => vec3(1.75, 1.45, 3.3),
                Some("apc") => vec3(2.8, 2.35, 5.45),
                Some("patrol4x4") => vec3(1.65, 1.6, 3.5),
                Some("cargo6x6") => vec3(2.36, 2.7, 6.),
                _ => vec3(2.9, 2.85, 5.9),
            };
            if v["destroyed"] == true {
                size.y *= 0.58;
            }
            let hull = Hull::new(pose.position, size, pose.yaw);
            if let Some((_, _, t)) = hull.local_segment(origin, dir, best) {
                best = t;
            }
        }
        for drone in state["drones"].as_array().into_iter().flatten() {
            if drone["team"] == driver["team"] {
                continue;
            }
            let q = motion.drone(drone).position - origin;
            let t = q.dot(dir);
            let radial = 0.33 * 0.33 - (q - dir * t).length_squared();
            if radial >= 0. {
                let near = t - radial.sqrt();
                if near >= 0. && near < best {
                    best = near;
                }
            }
        }
        for p in state["players"].as_array().into_iter().flatten() {
            if p["id"] == driver["id"] || p["alive"] != true || !p["vehicleId"].is_null() {
                continue;
            }
            if let Some(t) = player_distance(origin - motion.player(p).position, dir, best) {
                best = t;
            }
        }
        origin + dir * best
    }
}

// The same finite upright cylinder used by the simulation, including its flat caps.
fn player_distance(origin: Vec3, dir: Vec3, limit: f32) -> Option<f32> {
    let a = dir.x * dir.x + dir.z * dir.z;
    let b = origin.x * dir.x + origin.z * dir.z;
    let c = origin.x * origin.x + origin.z * origin.z - 0.39 * 0.39;
    let (mut lo, mut hi) = (0.0_f32, limit);
    if a < 1e-9 {
        if c > 0. {
            return None;
        }
    } else {
        let disc = b * b - a * c;
        if disc < 0. {
            return None;
        }
        let root = disc.sqrt();
        lo = lo.max((-b - root) / a);
        hi = hi.min((-b + root) / a);
    }
    if dir.y.abs() < 1e-7 {
        if !(0.0..=1.84).contains(&origin.y) {
            return None;
        }
    } else {
        let a = -origin.y / dir.y;
        let b = (1.84 - origin.y) / dir.y;
        lo = lo.max(a.min(b));
        hi = hi.min(a.max(b));
    }
    (lo <= hi).then_some(lo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty() -> Scene {
        Scene::new(Vec::new().into())
    }
    fn sample(scene: &Scene, state: &Value, origin: Vec3, dir: Vec3) -> Vec3 {
        let driver = json!({"id":1,"vehicleId":7,"team":"blue"});
        scene.machinegun_contact(origin, dir, 180., state, &driver, &Motion::default())
    }

    #[test]
    fn sight_and_chase_follow_the_same_solid_cell_and_its_opening() {
        let shape = json!({"kind":"wall","x":0,"z":-6,"w":3.28,"h":2,"d":0.5,"solid":true});
        let geometry = Obstacle::read(&shape).unwrap();
        let origin = Vec3::from_array(geometry.layout.as_ref().unwrap().cells[0].center);
        let origin = vec3(origin.x, origin.y, 0.);
        let mut scene = Scene::new(vec![Some(geometry)].into());
        let dir = Vec3::NEG_Z;
        let state = json!({"round":1,"elapsed":0,"propChanges":[]});
        scene.receive(&state);
        let intact = sample(&scene, &state, origin, dir);
        assert!((intact.z + 5.75).abs() < 0.0001);
        assert!((scene.clip(origin, origin + dir * 8.).z - intact.z - 0.18).abs() < 0.0001);
        for changes in [
            json!([{"index":0,"dead":false,"broken":[0],"revision":1}]),
            json!([{"index":0,"dead":true}]),
        ] {
            let state = json!({"round":1,"elapsed":1,"propChanges":changes});
            scene.receive(&state);
            assert!((sample(&scene, &state, origin, dir).z + 180.).abs() < 0.0001);
        }
        scene.receive(&state);
        assert_eq!(sample(&scene, &state, origin, dir), intact);
    }

    #[test]
    fn built_layout_is_reused_until_replaced_and_removed_with_the_slot() {
        let mut scene = empty();
        let mut state = json!({"round":2,"builtProps":[{"kind":"wall","x":0,"z":-9,
            "w":3.28,"h":2,"d":0.5,"solid":true,"buildId":31}]});
        scene.receive(&state);
        let cells = |scene: &Scene| {
            scene.built[0]
                .as_ref()
                .unwrap()
                .geom
                .layout
                .as_ref()
                .unwrap()
                .cells
                .as_ptr()
        };
        let initial = cells(&scene);
        for tick in 0..5 {
            state["elapsed"] = json!(tick as f32 * 0.05);
            scene.receive(&state);
            assert_eq!(cells(&scene), initial);
        }
        let origin = vec3(0.37, 0.3, 0.);
        assert!((sample(&scene, &state, origin, Vec3::NEG_Z).z + 8.75).abs() < 0.0001);
        state["builtProps"][0]["z"] = json!(-15.);
        state["builtProps"][0]["buildId"] = json!(32);
        scene.receive(&state);
        assert!((sample(&scene, &state, origin, Vec3::NEG_Z).z + 14.75).abs() < 0.0001);
        state["builtProps"][0]["dead"] = json!(true);
        scene.receive(&state);
        assert_eq!(sample(&scene, &state, origin, Vec3::NEG_Z).z, -180.);
    }

    #[test]
    fn nearest_body_vehicle_drone_or_floor_respects_server_ownership_and_shape() {
        let scene = empty();
        let mut state = json!({
            "players":[{"id":1,"alive":true,"x":0,"z":0,"vehicleId":7},
                {"id":2,"alive":true,"x":0,"z":-3,"vehicleId":9},
                {"id":3,"alive":true,"x":0,"z":-18}, {"id":4,"alive":false,"x":0,"z":-2}],
            "vehicles":[{"id":7,"kind":"mbt","x":0,"z":0,"driver":1},
                {"id":9,"kind":"apc","x":0,"z":-14,"yaw":1.5707963,"driver":2}],
            "drones":[{"id":10,"team":"blue","x":0,"y":1.6,"z":-2},
                {"id":11,"team":"red","x":0,"y":1.6,"z":-8}]
        });
        let start = vec3(0., 1.6, 0.);
        let endpoint = |s: &Value| sample(&scene, s, start, Vec3::NEG_Z).z;
        assert!((endpoint(&state) + 7.67).abs() < 0.0001);
        state["drones"][1]["y"] = json!(4.);
        assert!((endpoint(&state) + 12.6).abs() < 0.0001);
        state["vehicles"][1]["destroyed"] = json!(true);
        assert!((endpoint(&state) + 17.61).abs() < 0.0001);
        let floor = sample(&scene, &state, start, vec3(0., -1., -1.));
        assert!(floor.distance(vec3(0., 0., -1.6)) < 0.0001);
    }

    #[test]
    fn moving_body_sight_advances_between_twenty_hz_snapshots() {
        let scene = empty();
        let driver = json!({"id":1,"vehicleId":7,"team":"blue"});
        let mut motion = Motion::default();
        let mut state = Value::Null;
        let mut previous = 0.;
        for frame in 0..80 {
            let time = frame as f32 * 0.01;
            if frame % 5 == 0 {
                state = json!({"round":1,"elapsed":time,
                    "players":[{"id":3,"alive":true,"x":0,"z":-18. + time * 3.}]});
                motion.receive(&state);
            }
            motion.advance(0.01);
            let z = scene
                .machinegun_contact(
                    vec3(0., 1., 0.),
                    Vec3::NEG_Z,
                    180.,
                    &state,
                    &driver,
                    &motion,
                )
                .z;
            if frame > 20 {
                assert!(
                    (0.02..0.04).contains(&(z - previous)),
                    "step {}",
                    z - previous
                );
            }
            previous = z;
        }
    }
}
