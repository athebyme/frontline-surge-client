//! Procedural field audio. Clips are generated once; the Windows device mixes
//! multiple concurrent voices in small stereo PCM buffers without game-thread I/O.
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(usize)]
enum Cue {
    Shot,
    Distant,
    Reload,
    Harvest,
    Build,
    Debris,
    Hurt,
    Dry,
    Patrol,
    Smg,
    Cannon,
    Throw,
    Blast,
    Masonry,
    Splinter,
    Metal,
    Mounted,
    ProductionStart,
    ProductionReady,
    Crash,
}
const BANK: [Cue; 20] = [
    Cue::Shot,
    Cue::Distant,
    Cue::Reload,
    Cue::Harvest,
    Cue::Build,
    Cue::Debris,
    Cue::Hurt,
    Cue::Dry,
    Cue::Patrol,
    Cue::Smg,
    Cue::Cannon,
    Cue::Throw,
    Cue::Blast,
    Cue::Masonry,
    Cue::Splinter,
    Cue::Metal,
    Cue::Mounted,
    Cue::ProductionStart,
    Cue::ProductionReady,
    Cue::Crash,
];
const COUNT: usize = BANK.len();
#[cfg(any(target_os = "windows", test))]
const RATE: usize = 22050;

#[derive(Clone, Copy)]
#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
struct Trigger {
    cue: Cue,
    gain: f32,
    pan: f32,
    priority: u8,
}

// The sample worker tracks the loudest nearby occupied vehicles. Audio rate is
// independent of render and network ticks, including during gear changes.
#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Copy, Debug)]
struct Motor {
    id: u64,
    kind: u8,
    speed: f32,
    pan: f32,
    level: f32,
}
#[cfg(any(target_os = "windows", test))]
fn motors_near(state: &Value, me: &Value) -> Vec<Motor> {
    let n = |v: &Value, k: &str| v[k].as_f64().unwrap_or(0.) as f32;
    let yaw = n(me, "yaw");
    let id = me["id"].as_u64();
    let mut result = Vec::new();
    for v in state["vehicles"].as_array().into_iter().flatten() {
        let Some(driver) = v["driver"].as_u64() else {
            continue;
        };
        if v["destroyed"].as_bool() == Some(true) {
            continue;
        }
        let (kind, top, range) = match v["kind"].as_str() {
            Some("mbt") => (3, 9.2, 94.),
            Some("apc") => (2, 12.7, 80.),
            Some("cargo6x6") => (2, 15., 78.),
            Some("scout_buggy") => (0, 28., 54.),
            _ => (1, 22., 63.),
        };
        let dx = n(v, "x") - n(me, "x");
        let dz = n(v, "z") - n(me, "z");
        let dist = dx.hypot(dz);
        if dist >= range {
            continue;
        }
        let onboard = Some(driver) == id;
        let level = if onboard {
            0.47
        } else {
            0.55 * (1. - dist / range) / (1. + dist * 0.046)
        };
        let pan = if onboard || dist < 0.02 {
            0.
        } else {
            ((dx * yaw.cos() + dz * yaw.sin()) / dist * 0.88).clamp(-1., 1.)
        };
        result.push(Motor {
            id: v["id"].as_u64().unwrap_or(0),
            kind,
            speed: (n(v, "speed").abs() / top).clamp(0., 1.3),
            pan,
            level,
        });
    }
    result.sort_by(|a, b| b.level.total_cmp(&a.level));
    result.truncate(4);
    result
}

// Factory progress has no discrete network event. Comparing consecutive snapshots
// prevents replay on joins; a new station starts with an unknown previous state.
#[derive(Default)]
struct Stations {
    last: HashMap<(u32, u32, u64), (bool, u64)>,
}
impl Stations {
    fn changes(&mut self, state: &Value, me: &Value) -> Vec<Trigger> {
        let mut now = HashMap::new();
        let mut out = Vec::new();
        for f in state["factories"].as_array().into_iter().flatten() {
            if f["team"] != me["team"] {
                continue;
            }
            let x = f["x"].as_f64().unwrap_or(0.) as f32;
            let z = f["z"].as_f64().unwrap_or(0.) as f32;
            let key = (x.to_bits(), z.to_bits(), f["id"].as_u64().unwrap_or(0));
            let active = f["remaining"].as_f64().unwrap_or(0.) > 0.;
            let ready = f["ready"].as_u64().unwrap_or(0);
            if let Some(&(was_active, was_ready)) = self.last.get(&key) {
                let cue = if ready > was_ready {
                    "productionReady"
                } else if active && !was_active {
                    "productionStart"
                } else {
                    ""
                };
                if !cue.is_empty() {
                    let event = serde_json::json!({"type":cue,"x":x,"z":z});
                    if let Some(t) = from_event(&event, me, me["id"].as_u64().unwrap_or(0) as u32) {
                        out.push(t);
                    }
                }
            }
            now.insert(key, (active, ready));
        }
        self.last = now;
        out
    }
}

pub struct Sound {
    last_event: Option<u64>,
    round: Option<u64>,
    stations: Stations,
    #[cfg(target_os = "windows")]
    motor_at: f64,
    last_cue: [f64; COUNT],
    #[cfg(target_os = "windows")]
    output: device::Output,
}

impl Sound {
    pub fn new() -> Self {
        Self {
            last_event: None,
            round: None,
            stations: Stations::default(),
            #[cfg(target_os = "windows")]
            motor_at: f64::NEG_INFINITY,
            last_cue: [f64::NEG_INFINITY; COUNT],
            #[cfg(target_os = "windows")]
            output: device::Output::new(),
        }
    }
    pub fn clear(&mut self) {
        self.last_event = None;
        self.round = None;
        self.stations.last.clear();
        #[cfg(target_os = "windows")]
        {
            self.motor_at = f64::NEG_INFINITY;
        }
        self.last_cue.fill(f64::NEG_INFINITY);
        #[cfg(target_os = "windows")]
        self.output.clear();
    }
    pub fn update(&mut self, state: &Value, local_id: u32) {
        let round = state["round"].as_u64();
        if self.round.is_some() && round != self.round {
            self.clear();
        }
        self.round = round;
        let Some(events) = state["events"].as_array() else {
            return;
        };
        let newest = events
            .iter()
            .filter_map(|e| e["id"].as_u64())
            .max()
            .unwrap_or(0);
        let Some(cursor) = self.last_event else {
            // A joining client must not replay the server's pre-join history.
            self.last_event = Some(newest);
            return;
        };
        let Some(me) = state["players"].as_array().and_then(|players| {
            players
                .iter()
                .find(|p| p["id"].as_u64() == Some(local_id.into()))
        }) else {
            return;
        };
        for effect in self.stations.changes(state, me) {
            #[cfg(target_os = "windows")]
            self.output.enqueue(effect);
            #[cfg(not(target_os = "windows"))]
            let _ = effect;
        }
        let time = state["elapsed"].as_f64().unwrap_or(0.);
        #[cfg(target_os = "windows")]
        if time - self.motor_at >= 0.075 {
            self.output.motors(motors_near(state, me));
            self.motor_at = time;
        }
        for event in events {
            let id = event["id"].as_u64().unwrap_or(0);
            if id <= cursor || time - event["at"].as_f64().unwrap_or(time) > 0.40 {
                continue;
            }
            if let Some(trigger) = from_event(event, me, local_id) {
                let previous = &mut self.last_cue[trigger.cue as usize];
                if time - *previous < 0.031 {
                    continue;
                }
                *previous = time;
                #[cfg(target_os = "windows")]
                self.output.enqueue(trigger);
            }
        }
        self.last_event = Some(cursor.max(newest));
    }
}

fn from_event(event: &Value, me: &Value, local_id: u32) -> Option<Trigger> {
    let own = event["actor"].as_u64() == Some(local_id as u64);
    let kind = event["type"].as_str()?;
    let origin = |axis, impact_axis| -> f32 {
        let value: &Value = if kind == "destroy" && event.get(impact_axis).is_some() {
            &event[impact_axis]
        } else {
            &event[axis]
        };
        value.as_f64().unwrap_or(0.) as f32
    };
    let dx = origin("x", "ex") - me["x"].as_f64().unwrap_or(0.) as f32;
    let dz = origin("z", "ez") - me["z"].as_f64().unwrap_or(0.) as f32;
    let distance = dx.hypot(dz);
    let cue = match kind {
        "tankFire" => Cue::Cannon,
        "shellImpact" => Cue::Blast,
        "shot" if event["weapon"].as_str() == Some("vehicle_mg") => Cue::Mounted,
        "vehicleImpact" if event["strength"].as_f64().unwrap_or(0.) > 0.12 => Cue::Crash,
        "productionStart" => Cue::ProductionStart,
        "productionReady" => Cue::ProductionReady,
        "inventoryMove" if own => Cue::Harvest,
        "shot" if matches!(event["weapon"].as_str(), Some("tank" | "mbt")) => Cue::Cannon,
        "shot" if distance > 18. && !own => Cue::Distant,
        "shot" if event["weapon"].as_str() == Some("vx9") => Cue::Smg,
        "shot" if event["weapon"].as_str() == Some("ar4") => Cue::Patrol,
        "shot" => Cue::Shot,
        "throw" => Cue::Throw,
        "explosion" => Cue::Blast,
        "fracture" => match event["kind"].as_str().unwrap_or("") {
            "barn" | "barricade" | "fence" | "crate" | "tower" | "workshop" | "garage" => {
                Cue::Splinter
            }
            "truck" | "tank" | "jeep" | "patrol4x4" | "cargo6x6" | "mbt" | "barrels"
            | "oilwell" | "refinery" => Cue::Metal,
            _ => Cue::Masonry,
        },
        "reload" => Cue::Reload,
        "pickup" | "harvest" => Cue::Harvest,
        "build" | "repair" => Cue::Build,
        "destroy"
            if matches!(
                event["kind"].as_str(),
                Some(
                    "patrol4x4"
                        | "cargo6x6"
                        | "mbt"
                        | "jeep"
                        | "truck"
                        | "tank"
                        | "metal"
                        | "barrels"
                )
            ) =>
        {
            Cue::Metal
        }
        "destroy"
            if matches!(
                event["kind"].as_str(),
                Some("house" | "barn" | "ruin" | "wall" | "barricade")
            ) =>
        {
            return None
        }
        "destroy" => Cue::Debris,
        "hurt" if own => Cue::Hurt,
        "dry" if own => Cue::Dry,
        _ => return None,
    };
    let centered = own
        && !matches!(
            kind,
            "destroy"
                | "fracture"
                | "explosion"
                | "shellImpact"
                | "vehicleImpact"
                | "productionReady"
        );
    let range = match cue {
        Cue::Mounted => 130.,
        Cue::Crash => 58.,
        Cue::ProductionStart | Cue::ProductionReady => 27.,
        Cue::Cannon => 200.,
        Cue::Blast => 145.,
        Cue::Masonry | Cue::Splinter | Cue::Metal => 52.,
        Cue::Shot | Cue::Distant | Cue::Smg | Cue::Patrol => 110.,
        Cue::Debris => 65.,
        _ => 20.,
    };
    if !centered && distance >= range {
        return None;
    }
    let gain = if centered {
        0.94
    } else {
        (1. - distance / range).max(0.) / (1. + distance * 0.035)
    };
    let yaw = me["yaw"].as_f64().unwrap_or(0.) as f32;
    let pan = if centered || distance < 0.05 {
        0.
    } else {
        ((dx * yaw.cos() + dz * yaw.sin()) / distance * 0.84).clamp(-1., 1.)
    };
    let priority = if centered {
        4
    } else if matches!(
        cue,
        Cue::Blast | Cue::Cannon | Cue::Mounted | Cue::Shot | Cue::Smg | Cue::Patrol
    ) {
        3
    } else if matches!(cue, Cue::Debris | Cue::Metal | Cue::Masonry | Cue::Splinter) {
        2
    } else {
        1
    };
    Some(Trigger {
        cue,
        gain,
        pan,
        priority,
    })
}

#[cfg(any(target_os = "windows", test))]
mod mixer {
    use super::*;
    use std::f32::consts::TAU;

    fn pulse(t: f32, start: f32, rate: f32) -> f32 {
        let age = t - start;
        if age < 0. {
            0.
        } else {
            (1. - (-age * 1700.).exp()) * (-age * rate).exp()
        }
    }
    fn tone(t: f32, hz: f32) -> f32 {
        (TAU * hz * t).sin()
    }
    fn make(cue: Cue) -> Vec<f32> {
        let duration = match cue {
            Cue::Shot => 0.40,
            Cue::Distant => 0.53,
            Cue::Reload => 0.34,
            Cue::Harvest => 0.20,
            Cue::Build => 0.30,
            Cue::Debris => 0.53,
            Cue::Hurt => 0.24,
            Cue::Dry => 0.075,
            Cue::Patrol => 0.27,
            Cue::Smg => 0.19,
            Cue::Cannon => 0.85,
            Cue::Throw => 0.16,
            Cue::Blast => 0.81,
            Cue::Masonry => 0.40,
            Cue::Splinter => 0.30,
            Cue::Metal => 0.55,
            Cue::Mounted => 0.30,
            Cue::ProductionStart => 0.34,
            Cue::ProductionReady => 0.51,
            Cue::Crash => 0.39,
        };
        let frames = (duration * RATE as f32) as usize;
        let mut seed = 73429u32 + cue as u32 * 7919;
        let mut filtered = 0.;
        let mut rumble = 0.;
        (0..frames)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = (seed >> 8) as f32 / 8388608. - 1.;
                filtered += 0.32 * (noise - filtered);
                rumble += 0.045 * (noise - rumble);
                let click = noise - filtered;
                let sample = match cue {
                    Cue::Shot => {
                        (click * 0.92 + filtered * 0.7) * pulse(t, 0., 64.)
                            + (tone(t, 77.) * 0.30 + rumble * 1.7) * pulse(t, 0., 17.)
                            + filtered * 0.38 * pulse(t, 0.071, 24.)
                            + rumble * 0.65 * pulse(t, 0.126, 18.)
                    }
                    Cue::Distant => {
                        (rumble * 2.7 + tone(t, 62.) * 0.18) * pulse(t, 0., 18.)
                            + (rumble * 1.6 + filtered * 0.16) * pulse(t, 0.083, 12.)
                    }
                    Cue::Reload => {
                        (click * 0.49 + tone(t, 1150.) * 0.12) * pulse(t, 0., 75.)
                            + (filtered * 0.65 + tone(t, 263.) * 0.18) * pulse(t, 0.123, 41.)
                            + (click * 0.64 + tone(t, 930.) * 0.12) * pulse(t, 0.234, 80.)
                    }
                    Cue::Harvest => {
                        (tone(t, 129.) * 0.46 + filtered * 0.64) * pulse(t, 0., 31.)
                            + click * 0.37 * pulse(t, 0.012, 47.)
                    }
                    Cue::Build => {
                        (tone(t, 98.) * 0.35 + filtered * 0.57) * pulse(t, 0., 28.)
                            + (tone(t, 660.) * 0.12 + click * 0.45) * pulse(t, 0.095, 61.)
                    }
                    Cue::Debris => {
                        (rumble * 2.4 + tone(t, 59.) * 0.24) * pulse(t, 0., 11.)
                            + filtered
                                * (pulse(t, 0.037, 32.)
                                    + 0.67 * pulse(t, 0.131, 27.)
                                    + 0.50 * pulse(t, 0.223, 32.))
                    }
                    Cue::Hurt => {
                        (tone(t, 55.) * 0.33 + rumble * 1.4) * pulse(t, 0., 15.)
                            + filtered * 0.31 * pulse(t, 0., 54.)
                    }
                    Cue::Patrol => {
                        (click * 0.78 + filtered * 0.55) * pulse(t, 0., 83.)
                            + (tone(t, 131.) * 0.23 + rumble * 1.2) * pulse(t, 0.004, 24.)
                    }
                    Cue::Smg => {
                        (click * 0.72 + filtered * 0.88) * pulse(t, 0., 74.)
                            + (tone(t, 105.) * 0.33 + rumble * 1.35) * pulse(t, 0.002, 35.)
                            + click * 0.18 * pulse(t, 0.043, 81.)
                    }
                    Cue::Cannon => {
                        (filtered * 1.4 + click * 0.65) * pulse(t, 0., 34.)
                            + (tone(t, 46.) * 0.57 + rumble * 2.75) * pulse(t, 0., 8.5)
                            + (tone(t, 62.) * 0.19 + rumble * 1.37) * pulse(t, 0.142, 9.1)
                    }
                    Cue::Throw => {
                        (click * 0.41 + tone(t, 1095.) * 0.19) * pulse(t, 0., 124.)
                            + filtered * 0.44 * pulse(t, 0.043, 39.)
                    }
                    Cue::Blast => {
                        (click * 0.69 + filtered * 1.18) * pulse(t, 0., 51.)
                            + (rumble * 2.8 + tone(t, 57.) * 0.53) * pulse(t, 0.007, 9.8)
                            + (filtered * 0.64 + rumble * 1.30) * pulse(t, 0.085, 12.)
                            + filtered * 0.25 * pulse(t, 0.208, 17.)
                    }
                    Cue::Masonry => {
                        (filtered * 1.2 + tone(t, 151.) * 0.20) * pulse(t, 0., 39.)
                            + filtered * (pulse(t, 0.052, 51.) + 0.54 * pulse(t, 0.137, 44.))
                    }
                    Cue::Splinter => {
                        (click * 0.85 + tone(t, 347.) * 0.23) * pulse(t, 0., 72.)
                            + (click * 0.47 + filtered * 0.47) * pulse(t, 0.044, 43.)
                    }
                    Cue::Metal => {
                        (filtered * 0.91 + click * 0.68) * pulse(t, 0., 63.)
                            + (tone(t, 327.) * 0.30 + tone(t, 821.) * 0.19 + tone(t, 1451.) * 0.12)
                                * pulse(t, 0.004, 14.)
                            + filtered * 0.34 * pulse(t, 0.088, 38.)
                    }
                    Cue::Mounted => {
                        (click * 0.75 + filtered * 0.76) * pulse(t, 0., 76.)
                            + (tone(t, 65.) * 0.39 + rumble * 0.9) * pulse(t, 0.002, 22.)
                            + filtered * 0.30 * pulse(t, 0.052, 38.)
                    }
                    Cue::Crash => {
                        (tone(t, 54.) * 0.49 + rumble * 1.3) * pulse(t, 0., 15.)
                            + click * 0.6 * pulse(t, 0.009, 24.)
                            + tone(t, 276.) * 0.15 * pulse(t, 0.04, 42.)
                    }
                    Cue::ProductionStart => {
                        tone(t, 384.) * 0.43 * pulse(t, 0., 17.)
                            + tone(t, 512.) * 0.34 * pulse(t, 0.096, 21.)
                    }
                    Cue::ProductionReady => {
                        tone(t, 522.) * 0.41 * pulse(t, 0., 17.)
                            + tone(t, 696.) * 0.35 * pulse(t, 0.11, 19.)
                            + tone(t, 783.) * 0.35 * pulse(t, 0.21, 16.)
                    }
                    Cue::Dry => (click * 0.57 + tone(t, 811.) * 0.16) * pulse(t, 0., 102.),
                };
                let tail = ((frames - 1 - i) as f32 / (RATE as f32 * 0.012)).min(1.);
                sample.clamp(-1., 1.) * tail
            })
            .collect()
    }
    struct MotorVoice {
        target: Motor,
        phase: f32,
        road: f32,
        speed: f32,
        level: f32,
        pan: f32,
        rng: u32,
        wind: f32,
    }
    impl MotorVoice {
        fn new(m: Motor) -> Self {
            Self {
                target: m,
                phase: (m.id % 11) as f32 / 11.,
                road: 0.,
                speed: m.speed,
                level: 0.,
                pan: m.pan,
                rng: (m.id as u32).wrapping_mul(1717).wrapping_add(1),
                wind: 0.,
            }
        }
        fn sample(&mut self) -> (f32, f32) {
            self.speed += (self.target.speed - self.speed) * 0.0020;
            self.level += (self.target.level - self.level) * 0.0015;
            self.pan += (self.target.pan - self.pan) * 0.0014;
            let acceleration = ((self.target.speed - self.speed) * 28.).clamp(-0.25, 0.65);
            let tracked = self.target.kind == 3;
            let (idle, rise) = match self.target.kind {
                0 => (51., 95.),
                1 => (37., 75.),
                2 => (29., 55.),
                _ => (26., 47.),
            };
            let speed = self.speed.min(1.);
            let gear = (speed * 3.7).fract();
            let rpm = idle + rise * (speed * 0.65 + gear * 0.35 + acceleration * 0.40);
            self.phase = (self.phase + rpm / RATE as f32).fract();
            self.road = (self.road + (16. + 80. * speed) / RATE as f32).fract();
            self.rng = self.rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = (self.rng >> 8) as f32 / 8388608. - 1.;
            self.wind += 0.045 * (noise - self.wind);
            let engine = (self.phase * TAU).sin() * 0.31
                + (self.phase * TAU * 2.).sin() * 0.13
                + (self.phase * TAU * 3.).sin() * 0.052;
            let rolling = if tracked {
                ((1. - self.road).powi(8) * (noise * 0.41 + 0.15) - 0.022) * speed
            } else {
                (self.wind * 0.60 + (noise - self.wind) * 0.038) * speed
            };
            let sample =
                (engine * (0.56 + 0.26 * speed + acceleration * 0.15) + rolling) * self.level;
            (
                sample * (1. - self.pan) * 0.7,
                sample * (1. + self.pan) * 0.7,
            )
        }
    }
    pub(super) struct Mixer {
        motors: Vec<MotorVoice>,
        bank: [Vec<f32>; COUNT],
        voices: Vec<Voice>,
    }
    struct Voice {
        effect: Trigger,
        index: usize,
        left: f32,
        right: f32,
    }
    impl Mixer {
        pub(super) fn new() -> Self {
            Self {
                bank: BANK.map(make),
                motors: Vec::with_capacity(6),
                voices: Vec::with_capacity(8),
            }
        }
        pub(super) fn clear(&mut self) {
            self.voices.clear();
            self.motors.clear();
        }
        pub(super) fn engines(&mut self, motors: &[Motor]) {
            for voice in &mut self.motors {
                voice.target.level = 0.;
            }
            for &m in motors {
                if let Some(v) = self.motors.iter_mut().find(|v| v.target.id == m.id) {
                    v.target = m;
                } else if self.motors.len() < 8 {
                    self.motors.push(MotorVoice::new(m));
                }
            }
        }
        pub(super) fn add(&mut self, effect: Trigger) {
            if self.voices.len() >= 8 {
                let (idx, victim) = self
                    .voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.effect.priority)
                    .unwrap();
                if victim.effect.priority > effect.priority {
                    return;
                }
                self.voices.swap_remove(idx);
            }
            let angle = (effect.pan + 1.) * std::f32::consts::FRAC_PI_4;
            self.voices.push(Voice {
                effect,
                index: 0,
                left: angle.cos() * effect.gain,
                right: angle.sin() * effect.gain,
            });
        }
        pub(super) fn render(&mut self, output: &mut [i16]) {
            for frame in output.chunks_exact_mut(2) {
                let (mut left, mut right) = (0., 0.);
                for motor in &mut self.motors {
                    let (l, r) = motor.sample();
                    left += l;
                    right += r;
                }
                for voice in &mut self.voices {
                    if let Some(&s) = self.bank[voice.effect.cue as usize].get(voice.index) {
                        left += s * voice.left;
                        right += s * voice.right;
                        voice.index += 1;
                    }
                }
                frame[0] = (left.clamp(-1.58, 1.58) * 20700.) as i16;
                frame[1] = (right.clamp(-1.58, 1.58) * 20700.) as i16;
            }
            self.motors
                .retain(|m| m.target.level > 0. || m.level > 0.0002);
            self.voices
                .retain(|v| v.index < self.bank[v.effect.cue as usize].len());
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn bounded_varied_clips_and_silent_tails() {
            let m = Mixer::new();
            for clip in &m.bank {
                assert!(clip.len() >= RATE / 20 && clip.len() < RATE);
                assert!(clip.iter().all(|n| n.is_finite() && n.abs() <= 1.));
                assert_eq!(clip.last(), Some(&0.));
                assert!(clip.iter().any(|n| n.abs() > 0.12));
            }
            assert_ne!(&m.bank[0][..400], &m.bank[1][..400]);
        }
        #[test]
        fn engine_pans_tracks_change_and_fade() {
            let car = Motor {
                id: 7,
                kind: 1,
                speed: 0.15,
                pan: 1.,
                level: 0.8,
            };
            let mut m = Mixer::new();
            m.engines(&[car]);
            let mut block = vec![0; RATE / 4 * 2];
            m.render(&mut block);
            assert!(block.chunks_exact(2).all(|f| f[0].abs() <= 1));
            assert!(block.chunks_exact(2).any(|f| f[1].abs() > 800));
            let road = block.clone();
            m.engines(&[Motor {
                kind: 3,
                speed: 0.8,
                ..car
            }]);
            m.render(&mut block);
            assert_ne!(road, block);
            m.engines(&[]);
            m.render(&mut vec![0; RATE]);
            assert!(m.motors.is_empty());
        }
        #[test]
        fn stereo_pan_and_voice_expiration() {
            let mut m = Mixer::new();
            m.add(Trigger {
                cue: Cue::Dry,
                gain: 1.,
                pan: 1.,
                priority: 2,
            });
            let mut block = vec![0; RATE / 3 * 2];
            m.render(&mut block);
            assert!(block.chunks_exact(2).all(|pair| pair[0].abs() <= 1));
            assert!(block.chunks_exact(2).any(|pair| pair[1].abs() > 1000));
            assert!(m.voices.is_empty());
            m.clear();
        }
    }
}

#[cfg(target_os = "windows")]
mod device {
    use super::{mixer::Mixer, Motor, Trigger, RATE};
    use std::{
        ffi::c_void,
        mem::size_of,
        ptr::{addr_of_mut, null_mut},
        sync::{
            atomic::{AtomicU64, Ordering},
            mpsc::{self, Receiver, SyncSender, TryRecvError},
            Arc,
        },
        thread::{self, JoinHandle},
        time::Duration,
    };

    // Corresponding Windows SDK types WAVEFORMATEX and WAVEHDR. All WinMM
    // calls, and the backing allocation for each header, belong to one thread.
    #[repr(C, packed)]
    struct Format {
        tag: u16,
        channels: u16,
        rate: u32,
        bytes_per_sec: u32,
        align: u16,
        bits: u16,
        extra: u16,
    }
    #[repr(C)]
    struct Header {
        data: *mut u8,
        len: u32,
        recorded: u32,
        user: usize,
        flags: u32,
        loops: u32,
        next: *mut Header,
        reserved: usize,
    }
    type Handle = *mut c_void;
    #[link(name = "winmm")]
    extern "system" {
        fn waveOutOpen(
            handle: *mut Handle,
            device: u32,
            format: *const Format,
            callback: usize,
            instance: usize,
            flags: u32,
        ) -> u32;
        fn waveOutPrepareHeader(device: Handle, header: *mut Header, size: u32) -> u32;
        fn waveOutWrite(device: Handle, header: *mut Header, size: u32) -> u32;
        fn waveOutUnprepareHeader(device: Handle, header: *mut Header, size: u32) -> u32;
        fn waveOutReset(device: Handle) -> u32;
        fn waveOutClose(device: Handle) -> u32;
    }
    const FRAMES: usize = 384; // 17.4 ms of audio per block
    const SIZE: u32 = size_of::<Header>() as u32;
    enum Message {
        Play(u64, Trigger),
        Motors(u64, Vec<Motor>),
    }
    pub(super) struct Output {
        tx: Option<SyncSender<Message>>,
        epoch: Arc<AtomicU64>,
        thread: Option<JoinHandle<()>>,
    }
    impl Output {
        pub(super) fn new() -> Self {
            let (tx, rx) = mpsc::sync_channel(32);
            let epoch = Arc::new(AtomicU64::new(0));
            let worker_epoch = Arc::clone(&epoch);
            let thread = thread::Builder::new()
                .name("frontline-audio".into())
                .spawn(move || worker(rx, worker_epoch))
                .ok();
            Self {
                tx: Some(tx),
                epoch,
                thread,
            }
        }
        pub(super) fn motors(&self, values: Vec<Motor>) {
            if let Some(tx) = &self.tx {
                let _ = tx.try_send(Message::Motors(self.epoch.load(Ordering::Acquire), values));
            }
        }
        pub(super) fn enqueue(&self, effect: Trigger) {
            if let Some(tx) = &self.tx {
                let _ = tx.try_send(Message::Play(self.epoch.load(Ordering::Acquire), effect));
            }
        }
        pub(super) fn clear(&self) {
            // Stop continuous machinery even when several network effects fill the queue.
            self.epoch.fetch_add(1, Ordering::AcqRel);
        }
    }
    impl Drop for Output {
        fn drop(&mut self) {
            self.tx.take();
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }
    struct Block {
        samples: Box<[i16; FRAMES * 2]>,
        header: Box<Header>,
        submitted: bool,
    }
    impl Block {
        fn new() -> Self {
            let mut samples = Box::new([0i16; FRAMES * 2]);
            let header = Box::new(Header {
                data: samples.as_mut_ptr().cast(),
                len: (FRAMES * 4) as u32,
                recorded: 0,
                user: 0,
                flags: 0,
                loops: 0,
                next: null_mut(),
                reserved: 0,
            });
            Self {
                samples,
                header,
                submitted: false,
            }
        }
    }
    fn worker(rx: Receiver<Message>, epoch: Arc<AtomicU64>) {
        let fmt = Format {
            tag: 1,
            channels: 2,
            rate: RATE as u32,
            bytes_per_sec: (RATE * 4) as u32,
            align: 4,
            bits: 16,
            extra: 0,
        };
        let mut dev: Handle = null_mut();
        // Open failure (muted/disconnected audio device) leaves the game functional.
        if unsafe { waveOutOpen(&mut dev, u32::MAX, &fmt, 0, 0, 0) } != 0 {
            return;
        }
        let mut mixer = Mixer::new();
        let mut blocks = [Block::new(), Block::new(), Block::new()];
        let mut next = 0;
        let mut current = epoch.load(Ordering::Acquire);
        'play: loop {
            let generation = epoch.load(Ordering::Acquire);
            if generation != current {
                current = generation;
                mixer.clear();
                unsafe {
                    waveOutReset(dev);
                }
            }
            loop {
                match rx.try_recv() {
                    Ok(Message::Play(g, e)) if g == current => mixer.add(e),
                    Ok(Message::Motors(g, v)) if g == current => mixer.engines(&v),
                    Ok(_) => {}
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break 'play,
                }
            }
            let b = &mut blocks[next];
            let header = addr_of_mut!(*b.header);
            // Unprepare is the driver's documented ownership check. When
            // WAVERR_STILLPLAYING is returned, the buffer remains untouched.
            if b.submitted {
                match unsafe { waveOutUnprepareHeader(dev, header, SIZE) } {
                    0 => b.submitted = false,
                    33 => {
                        thread::sleep(Duration::from_millis(4));
                        continue;
                    }
                    _ => break,
                }
            }
            mixer.render(b.samples.as_mut_slice());
            if unsafe { waveOutPrepareHeader(dev, header, SIZE) } != 0 {
                break;
            }
            b.submitted = true;
            if unsafe { waveOutWrite(dev, header, SIZE) } != 0 {
                break;
            }
            next = (next + 1) % blocks.len();
        }
        unsafe {
            waveOutReset(dev);
        }
        for b in &mut blocks {
            if b.submitted {
                unsafe {
                    waveOutUnprepareHeader(dev, addr_of_mut!(*b.header), SIZE);
                }
            }
        }
        unsafe {
            waveOutClose(dev);
        }
    }
}

#[cfg(test)]
mod events_test {
    use super::*;
    use serde_json::json;
    #[test]
    fn engines_and_station_arrivals_are_spatial_and_join_safe() {
        let me = json!({"id":1,"team":"blue","x":0,"z":0,"yaw":0});
        let vehicles = json!([{"id":3,"kind":"mbt","driver":2,"speed":8,"x":9,"z":0},{"id":4,"kind":"cargo6x6","driver":null,"x":0,"z":0}]);
        let v = motors_near(&json!({"vehicles":vehicles}), &me);
        assert_eq!(v.len(), 1);
        assert!(v[0].pan > 0.8);
        let scene = |remaining: f32, ready: u32| json!({"factories":[{"team":"blue","x":6,"z":0,"remaining":remaining,"ready":ready}]});
        let mut stations = Stations::default();
        assert!(stations.changes(&scene(0., 0), &me).is_empty());
        let start = stations.changes(&scene(12., 0), &me);
        assert_eq!(start[0].cue, Cue::ProductionStart);
        let end = stations.changes(&scene(0., 6), &me);
        assert_eq!(end[0].cue, Cue::ProductionReady);
        assert!(end[0].pan > 0.8);
        assert!(stations.changes(&scene(0., 6), &me).is_empty());
    }
    #[test]
    fn events_are_stereo_local_and_deduplicated() {
        let p = json!({"id":3,"x":0,"z":0,"yaw":0});
        let remote = json!({"id":2,"type":"shot","actor":4,"at":1,"x":25,"z":0});
        let e = from_event(&remote, &p, 3).unwrap();
        assert_eq!(e.cue, Cue::Distant);
        assert!(e.pan > 0.7 && e.gain > 0. && e.gain < 0.6);
        assert!(from_event(&json!({"type":"hurt","actor":4}), &p, 3).is_none());
        let mut s = Sound::new();
        s.update(&json!({"players":[p],"elapsed":1,"events":[remote]}), 3);
        assert_eq!(s.last_event, Some(2));
        assert!(s.last_cue[Cue::Distant as usize].is_infinite());
        let newest = json!({"id":3,"type":"shot","actor":3,"at":2,"x":0,"z":0});
        let state = json!({"players":[p],"elapsed":2,"events":[newest]});
        s.update(&state, 3);
        assert_eq!(s.last_cue[Cue::Shot as usize], 2.);
        s.update(&json!({"players":[p],"elapsed":2.1,"events":[newest]}), 3);
        assert_eq!(s.last_cue[Cue::Shot as usize], 2.);
        s.clear();
        assert!(s.last_event.is_none());
    }
}
