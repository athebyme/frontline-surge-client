//! Recorded field audio. All voices and continuous sources share the same stereo bus.
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
    Apc,
    ShellBlast,
    TankLoader,
    Tile,
    Earth,
    BootGround,
    BootHard,
    BootRubble,
}
const BANK: [Cue; 28] = [
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
    Cue::Apc,
    Cue::ShellBlast,
    Cue::TankLoader,
    Cue::Tile,
    Cue::Earth,
    Cue::BootGround,
    Cue::BootHard,
    Cue::BootRubble,
];
const COUNT: usize = BANK.len();
#[cfg(any(target_os = "windows", test))]
const RATE: usize = 32000;

#[derive(Clone, Copy)]
#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
struct Trigger {
    cue: Cue,
    gain: f32,
    pan: f32,
    priority: u8,
    distance: f32,
    offset: f32,
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
const ROTOR: u8 = 4;
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
    // Hovering, newly tossed and remotely piloted craft are all physical emitters.
    let mut rotors = Vec::new();
    for d in state["drones"].as_array().into_iter().flatten() {
        let Some(id) = d["id"].as_u64() else { continue };
        if d["battery"].as_f64().is_some_and(|v| v <= 0.) {
            continue;
        }
        let dx = n(d, "x") - n(me, "x");
        let dy = n(d, "y") - n(me, "y");
        let dz = n(d, "z") - n(me, "z");
        let distance = dx.hypot(dz).hypot(dy);
        if distance >= 55. {
            continue;
        }
        let pan = if distance < 0.2 {
            0.
        } else {
            (dx * yaw.cos() + dz * yaw.sin()) / distance * 0.87
        };
        let spool = 1. - n(d, "launchRemaining").clamp(0., 0.8);
        let load = (n(d, "speed") / 12.).clamp(0., 1.) * 0.65
            + (n(d, "vSpeed") / 4.).clamp(-1., 1.) * 0.37;
        let level = 0.49 * spool * (1. - distance / 55.) / (1. + distance * 0.095);
        rotors.push(Motor {
            id,
            kind: ROTOR,
            speed: load,
            pan,
            level,
        });
    }
    rotors.sort_by(|a, b| b.level.total_cmp(&a.level));
    rotors.truncate(3);
    result.extend(rotors);
    result
}

struct Steps {
    roads: Vec<[f32; 6]>,
    walkers: HashMap<u64, (f32, f32, f32)>,
}
impl Steps {
    fn new() -> Self {
        let level: Value =
            serde_json::from_str(include_str!("../assets/level.json")).expect("bundled roads");
        let roads = level["roads"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| {
                let n = |k: &str| r[k].as_f64().unwrap_or(0.) as f32;
                let (sin, cos) = n("yaw").sin_cos();
                [n("x"), n("z"), n("w") * 0.5, n("d") * 0.5, sin, cos]
            })
            .collect();
        Self {
            roads,
            walkers: HashMap::new(),
        }
    }
    fn advance(&mut self, state: &Value, ear: &Value, local: u32) -> Vec<Trigger> {
        let mut next = HashMap::new();
        let mut effects = Vec::new();
        for p in state["players"].as_array().into_iter().flatten() {
            let Some(id) = p["id"].as_u64() else {
                continue;
            };
            if p["alive"] != true || p["vehicleId"].is_number() || p["moving"] != true {
                continue;
            }
            let (x, z) = (
                p["x"].as_f64().unwrap_or(0.) as f32,
                p["z"].as_f64().unwrap_or(0.) as f32,
            );
            let (lx, lz, mut travelled) = self.walkers.get(&id).copied().unwrap_or((x, z, 0.38));
            let dist = (x - lx).hypot(z - lz);
            let stride = if p["sprinting"] == true { 1.82 } else { 1.34 };
            if dist >= 2.5 {
                travelled = 0.;
            } else {
                travelled += dist;
            }
            if travelled >= stride {
                travelled %= stride;
                let road = self.roads.iter().any(|&[rx, rz, w, d, sin, cos]| {
                    let (dx, dz) = (x - rx, z - rz);
                    (dx * cos - dz * sin).abs() <= w && (dx * sin + dz * cos).abs() <= d
                });
                let cue = if p["looseFooting"] == true {
                    Cue::BootRubble
                } else if road {
                    Cue::BootHard
                } else {
                    Cue::BootGround
                };
                if let Some(t) = place(
                    cue,
                    x - ear["x"].as_f64().unwrap_or(0.) as f32,
                    z - ear["z"].as_f64().unwrap_or(0.) as f32,
                    ear,
                    id == local as u64,
                ) {
                    effects.push(t);
                }
            }
            next.insert(id, (x, z, travelled));
        }
        self.walkers = next;
        effects
    }
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
    sampled: Option<f64>,
    steps: Steps,
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
            sampled: None,
            steps: Steps::new(),
            #[cfg(target_os = "windows")]
            output: device::Output::new(),
        }
    }
    pub fn clear(&mut self) {
        self.last_event = None;
        self.round = None;
        self.sampled = None;
        self.steps.walkers.clear();
        self.stations.last.clear();
        #[cfg(target_os = "windows")]
        {
            self.motor_at = f64::NEG_INFINITY;
        }
        self.last_cue.fill(f64::NEG_INFINITY);
        #[cfg(target_os = "windows")]
        self.output.clear();
    }
    pub fn update(&mut self, state: &Value, local_id: u32, listener: (f32, f32, f32, f32)) {
        let round = state["round"].as_u64();
        if self.round.is_some() && round != self.round {
            self.clear();
        }
        self.round = round;
        let time = state["elapsed"].as_f64().unwrap_or(0.);
        if self.sampled == Some(time) {
            return;
        }
        self.sampled = Some(time);
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
        let ear = serde_json::json!({"id":local_id,"team":me["team"],"x":listener.0,"y":listener.1,"z":listener.2,"yaw":listener.3});
        let footsteps = self.steps.advance(state, &ear, local_id);
        for effect in self
            .stations
            .changes(state, &ear)
            .into_iter()
            .chain(footsteps)
        {
            #[cfg(target_os = "windows")]
            self.output.enqueue(effect);
            #[cfg(not(target_os = "windows"))]
            let _ = effect;
        }
        #[cfg(target_os = "windows")]
        if time - self.motor_at >= 0.075 {
            self.output.motors(
                motors_near(state, &ear),
                if me["vehicleId"].is_number() {
                    0.028
                } else {
                    0.065
                },
            );
            self.motor_at = time;
        }
        for event in events {
            let id = event["id"].as_u64().unwrap_or(0);
            if id <= cursor || time - event["at"].as_f64().unwrap_or(time) > 0.40 {
                continue;
            }
            for trigger in [
                from_event(event, &ear, local_id),
                detail(event, &ear, local_id, time),
            ]
            .into_iter()
            .flatten()
            {
                self.last_cue[trigger.cue as usize] = time;
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
        "tankFire" if event["weapon"] == "apc_he" => Cue::Apc,
        "tankFire" => Cue::Cannon,
        "shellImpact" if event["weapon"] == "apc_he" => Cue::Blast,
        "shellImpact" => Cue::ShellBlast,
        "shot" if event["weapon"].as_str() == Some("vehicle_mg") => Cue::Mounted,
        "vehicleImpact" if event["strength"].as_f64().unwrap_or(0.) > 0.12 => Cue::Crash,
        "productionStart" => Cue::ProductionStart,
        "productionReady" => Cue::ProductionReady,
        "inventoryMove" if own => Cue::Harvest,
        "shot" if matches!(event["weapon"].as_str(), Some("tank" | "mbt")) => Cue::Cannon,
        "shot" if event["weapon"].as_str() == Some("vx9") => Cue::Smg,
        "shot" if event["weapon"].as_str() == Some("ar4") => Cue::Patrol,
        "shot" if event["weapon"] == "mr7" => Cue::Shot,
        "shot" if distance > 18. && !own => Cue::Distant,
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
    place(cue, dx, dz, me, centered)
}

fn place(cue: Cue, dx: f32, dz: f32, me: &Value, centered: bool) -> Option<Trigger> {
    let distance = dx.hypot(dz);
    let range = match cue {
        Cue::Mounted => 130.,
        Cue::Crash => 58.,
        Cue::ProductionStart | Cue::ProductionReady => 27.,
        Cue::Cannon | Cue::ShellBlast => 200.,
        Cue::Apc => 160.,
        Cue::TankLoader => 32.,
        Cue::Blast => 145.,
        Cue::Masonry | Cue::Splinter | Cue::Metal | Cue::Tile | Cue::Earth => 52.,
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
        Cue::Blast
            | Cue::ShellBlast
            | Cue::Cannon
            | Cue::Apc
            | Cue::Mounted
            | Cue::Shot
            | Cue::Smg
            | Cue::Patrol
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
        distance: if centered { 0. } else { distance },
        offset: 0.,
    })
}

fn detail(event: &Value, me: &Value, local: u32, now: f64) -> Option<Trigger> {
    let coord = |p: &Value, key: &str| p[key].as_f64().unwrap_or(0.) as f32;
    match event["type"].as_str()? {
        "tankFire" if event["weapon"] != "apc_he" => {
            let own = event["actor"].as_u64() == Some(local as u64);
            let mut effect = place(
                Cue::TankLoader,
                coord(event, "x") - coord(me, "x"),
                coord(event, "z") - coord(me, "z"),
                me,
                own,
            )?;
            effect.offset = (now - event["at"].as_f64().unwrap_or(now)).clamp(0., 0.4) as f32;
            Some(effect)
        }
        "shot" => {
            let cue = match event["impactMaterial"].as_str()? {
                "metal" | "barrels" | "jeep" | "truck" | "tank" => Cue::Metal,
                "plank" | "fence" | "crate" | "tree" | "barn" | "barricade" | "tower" => {
                    Cue::Splinter
                }
                "tile" | "glass" => Cue::Tile,
                "brick" | "stone" | "wall" | "ruin" | "house" => Cue::Masonry,
                "earth" | "ground" | "sandbag" => Cue::Earth,
                _ => Cue::Debris,
            };
            let mut effect = place(
                cue,
                coord(event, "ex") - coord(me, "x"),
                coord(event, "ez") - coord(me, "z"),
                me,
                false,
            )?;
            effect.gain *= 0.48;
            Some(effect)
        }
        _ => None,
    }
}

#[cfg(any(target_os = "windows", test))]
#[path = "sound_mixer.rs"]
mod mixer;

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
    const FRAMES: usize = 384; // 12 ms of audio per block
    const SIZE: u32 = size_of::<Header>() as u32;
    enum Message {
        Play(u64, Trigger),
        Motors(u64, Vec<Motor>, f32),
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
        pub(super) fn motors(&self, values: Vec<Motor>, wind: f32) {
            if let Some(tx) = &self.tx {
                let _ = tx.try_send(Message::Motors(
                    self.epoch.load(Ordering::Acquire),
                    values,
                    wind,
                ));
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
                    Ok(Message::Motors(g, v, wind)) if g == current => {
                        mixer.engines(&v);
                        mixer.outdoors(wind);
                    }
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
    fn guns_impacts_and_loader_use_their_own_origins() {
        let listener = json!({"x":0, "z":0, "yaw":0});
        let tank = json!({"actor":3, "at":10., "weapon":"tank", "type":"tankFire","x":3,"z":0});
        let load = detail(&tank, &listener, 3, 10.16).unwrap();
        assert_eq!(load.cue, Cue::TankLoader);
        assert!(load.pan.abs() < 0.001 && (load.offset - 0.16).abs() < 0.001);
        assert_eq!(from_event(&tank, &listener, 3).unwrap().cue, Cue::Cannon);
        let apc = json!({"actor":3,"type":"tankFire","weapon":"apc_he"});
        assert_eq!(from_event(&apc, &listener, 3).unwrap().cue, Cue::Apc);
        assert!(detail(&apc, &listener, 3, 10.).is_none());
        for (tag, cue) in [
            ("plank", Cue::Splinter),
            ("brick", Cue::Masonry),
            ("stone", Cue::Masonry),
            ("metal", Cue::Metal),
            ("tile", Cue::Tile),
            ("earth", Cue::Earth),
        ] {
            let shot = json!({"type":"shot","actor":3,"weapon":"vehicle_mg","x":-12,"z":0,"ex":7,"ez":0,"impactMaterial":tag});
            assert_eq!(from_event(&shot, &listener, 1).unwrap().cue, Cue::Mounted);
            assert!(from_event(&shot, &listener, 1).unwrap().pan < -0.7);
            let hit = detail(&shot, &listener, 1, 10.).unwrap();
            assert_eq!(hit.cue, cue);
            assert!(hit.pan > 0.7);
        }
    }
    #[test]
    fn parked_rotor_stays_world_relative_and_flying_changes_its_load() {
        let ear = json!({"x":0,"y":2,"z":0,"yaw":0});
        let mut craft = json!({"id":4,"owner":1,"x":4,"y":3,"z":0,"piloted":false,"battery":50,"speed":0,"vSpeed":0});
        let rotor = |d: &Value| motors_near(&json!({"drones":[d]}), &ear);
        let hover = rotor(&craft)[0];
        assert_eq!(hover.kind, ROTOR);
        assert!(hover.pan > 0.8 && hover.level > 0.2);
        craft["y"] = 25.into();
        let high = rotor(&craft)[0];
        assert!(high.level < hover.level && high.pan < hover.pan);
        craft["piloted"] = true.into();
        craft["speed"] = 8.into();
        craft["vSpeed"] = 3.into();
        let climbing = rotor(&craft)[0];
        assert!(climbing.speed > hover.speed);
        craft["speed"] = 0.into();
        craft["vSpeed"] = (-3).into();
        assert!(rotor(&craft)[0].speed < hover.speed);
        craft["battery"] = 0.into();
        assert!(rotor(&craft).is_empty());
    }
    #[test]
    fn footsteps_follow_displacement_and_footing() {
        let mut steps = Steps {
            roads: vec![[0., 0., 10., 10., 0., 1.]],
            walkers: HashMap::new(),
        };
        let ear = json!({"x":0,"z":0,"yaw":0});
        let mut at = 0.;
        for (loose, expected) in [
            (false, Cue::BootHard),
            (true, Cue::BootRubble),
            (false, Cue::BootGround),
        ] {
            if expected == Cue::BootGround {
                steps.roads.clear();
            }
            let player = |x| json!({"players":[{"id":1,"alive":true,"moving":true,"x":x,"z":0,"looseFooting":loose}]});
            assert!(steps.advance(&player(at), &ear, 1).is_empty());
            at += 1.36;
            assert_eq!(steps.advance(&player(at), &ear, 1)[0].cue, expected);
            assert!(steps.advance(&player(at), &ear, 1).is_empty());
        }
        assert!(steps
            .advance(
                &json!({"players":[{"id":1,"alive":true,"moving":true,"vehicleId":1,"x":10}]}),
                &ear,
                1
            )
            .is_empty());
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
        s.update(
            &json!({"players":[p],"elapsed":1,"events":[remote]}),
            3,
            (0., 0., 0., 0.),
        );
        assert_eq!(s.last_event, Some(2));
        assert!(s.last_cue[Cue::Distant as usize].is_infinite());
        let newest = json!({"id":3,"type":"shot","actor":3,"at":2,"x":0,"z":0});
        let state = json!({"players":[p],"elapsed":2,"events":[newest]});
        s.update(&state, 3, (0., 0., 0., 0.));
        assert_eq!(s.last_cue[Cue::Shot as usize], 2.);
        s.update(
            &json!({"players":[p],"elapsed":2.1,"events":[newest]}),
            3,
            (0., 0., 0., 0.),
        );
        assert_eq!(s.last_cue[Cue::Shot as usize], 2.);
        s.clear();
        assert!(s.last_event.is_none());
    }
}
