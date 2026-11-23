// Prevent a console window when double-clicking the Windows application.
#![cfg_attr(all(target_os = "windows", not(test)), windows_subsystem = "windows")]
mod actors;
mod chase;
use ui::debug_ui;
mod drone_view;
mod motion;
mod settings;
mod sound;
mod ui;
mod vehicle_camera;
mod vehicle_sensor;
mod world;

use frontline_surge::net::{Client, Connecting};
use macroquad::prelude::*;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use ui::{Action, Ui};

fn window_conf() -> Conf {
    Conf {
        window_title: "Линия фронта • Frontline Surge".into(),
        window_width: 1280,
        window_height: 720,
        high_dpi: false,
        sample_count: 1,
        window_resizable: true,
        icon: Some(game_icon()),
        ..Default::default()
    }
}
fn game_icon() -> macroquad::miniquad::conf::Icon {
    fn fill(out: &mut [u8], n: usize) {
        for y in 0..n {
            for x in 0..n {
                let u = (x as f32 + 0.5) / n as f32;
                let v = (y as f32 + 0.5) / n as f32;
                let dx = (u - 0.5).abs();
                let gold = dx < 0.32
                    && ((v - (0.23 + dx * 0.7)).abs() < 0.045
                        || (v - (0.46 + dx * 0.7)).abs() < 0.045
                        || (v - (0.69 + dx * 0.7)).abs() < 0.038 && dx < 0.15);
                let c = if gold {
                    [222, 191, 120, 255]
                } else {
                    [36, 49, 44, 255]
                };
                out[(y * n + x) * 4..(y * n + x + 1) * 4].copy_from_slice(&c);
            }
        }
    }
    let mut i = macroquad::miniquad::conf::Icon {
        small: [0; 16 * 16 * 4],
        medium: [0; 32 * 32 * 4],
        big: [0; 64 * 64 * 4],
    };
    fill(&mut i.small, 16);
    fill(&mut i.medium, 32);
    fill(&mut i.big, 64);
    i
}
fn num(v: &Value, key: &str) -> f32 {
    v[key].as_f64().unwrap_or(0.0) as f32
}
fn grab(on: bool) {
    set_cursor_grab(on);
    show_mouse(!on);
}
#[cfg(target_os = "windows")]
fn has_focus() -> bool {
    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> *mut std::ffi::c_void;
        fn GetWindowThreadProcessId(window: *mut std::ffi::c_void, pid: *mut u32) -> u32;
    }
    let mut pid = 0u32;
    // Both calls access only Windows' foreground-window information for this process.
    unsafe {
        GetWindowThreadProcessId(GetForegroundWindow(), &mut pid);
    }
    pid == std::process::id()
}
#[cfg(not(target_os = "windows"))]
fn has_focus() -> bool {
    true
}

fn dir(yaw: f32, pitch: f32) -> Vec3 {
    vec3(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    )
}

struct Play {
    connecting: Option<Connecting>,
    client: Option<Client>,
    state: Value,
    local: Value,
    mode: &'static str,
    status: String,
    endpoint: String,
    yaw: f32,
    pitch: f32,
    body: Vec3,
    position_ready: bool,
    vehicle_camera: vehicle_camera::VehicleCamera,
    aim_scene: chase::Scene,
    drone_zoom: bool,
    operator_view: Option<(f32, f32)>,
    motion: motion::Motion,
    render_local: Value,
    ads: f32,
    next_input: Instant,
    last_input: Instant,
    ads_rate: f32,
    pointer: motion::LookDelta,
    edges: Value,
    fire_tap: bool,
    drawn_build: Option<(Option<u64>, Option<u64>)>,
    secondary_tap: bool,
    wait_trigger_release: bool,
    seq: u64,
}
impl Play {
    fn new() -> Self {
        Self {
            connecting: None,
            client: None,
            state: Value::Null,
            local: Value::Null,
            mode: "menu",
            status: String::new(),
            endpoint: String::new(),
            yaw: 0.,
            pitch: 0.,
            body: Vec3::ZERO,
            position_ready: false,
            vehicle_camera: Default::default(),
            aim_scene: Default::default(),
            drone_zoom: false,
            operator_view: None,
            motion: motion::Motion::default(),
            render_local: Value::Null,
            ads: 0.,
            next_input: Instant::now(),
            last_input: Instant::now(),
            ads_rate: 13.,
            pointer: Default::default(),
            edges: json!({}),
            fire_tap: false,
            drawn_build: None,
            secondary_tap: false,
            wait_trigger_release: true,
            seq: 0,
        }
    }
    fn reset(&mut self, why: String) {
        grab(false);
        *self = Self::new();
        self.status = why;
    }
    fn connect(&mut self, name: String, address: String, port: u16, team: String) {
        self.status = format!("Подключение к {address}:{port}…");
        self.connecting = Some(Connecting::new(address, port, name, team));
        self.mode = "connecting";
    }
    fn poll(&mut self) -> Result<(), String> {
        if let Some(c) = &self.connecting {
            if let Some(new) = c.poll()? {
                self.client = Some(new);
                self.connecting = None;
            }
        }
        let Some(link) = &mut self.client else {
            return Ok(());
        };
        let Some(new) = link.poll()? else {
            return Ok(());
        };
        let Some(id) = link.id else {
            return Ok(());
        };
        let Some(player) = new["players"]
            .as_array()
            .and_then(|a| a.iter().find(|p| p["id"] == id))
        else {
            return Ok(());
        };
        let new_life = self.local["alive"] != true && player["alive"] == true;
        if !self.position_ready
            || new_life
            || self.local["deaths"] != player["deaths"]
            || new["round"] != self.state["round"]
        {
            self.yaw = num(player, "yaw");
            self.pitch = num(player, "pitch");
            self.body = vec3(num(player, "x"), 0., num(player, "z"));
            self.position_ready = true;
            self.ads = num(player, "aim");
            self.edges = json!({});
            self.fire_tap = false;
            self.secondary_tap = false;
        }
        if self.local["droneId"] != player["droneId"]
            || self.local["vehicleId"] != player["vehicleId"]
        {
            self.drone_zoom = false;
            self.vehicle_camera = Default::default();
            self.ads = 0.;
            if player["droneId"].is_number() {
                self.operator_view = Some((self.yaw, self.pitch));
                if let Some(d) = new["drones"].as_array().into_iter().flatten().find(|d| d["id"] == player["droneId"]) {
                    self.yaw = num(d,"yaw");
                    self.pitch = num(d,"pitch");
                }
            } else if self.local["droneId"].is_number() {
                if let Some((yaw,pitch)) = self.operator_view.take().filter(|_| player["alive"] == true && !new_life) {
                    self.yaw = yaw;
                    self.pitch = pitch;
                }
            } else if let Some(v) = new["vehicles"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|v| v["id"] == player["vehicleId"])
            {
                self.yaw = v["turretUnwrapped"].as_f64().or(v["turretYaw"].as_f64())
                    .map_or(num(v, "yaw"), |v| v as f32);
                self.pitch = -0.035;
            }
            self.wait_trigger_release = true;
        }
        if self.local["weapon"] != player["weapon"]
            || self.local["optic"] != player["optic"]
            || self.local["grip"] != player["grip"]
        {
            let mut kit = frontline_surge::arsenal::Arsenal::default();
            let slot = match player["weapon"].as_str() {
                Some("vx9") => 1,
                Some("ar4") => 2,
                _ => 0,
            };
            for part in ["optic", "grip"] {
                if let Some(key) = player[part].as_str() {
                    kit.fit(slot, part, key);
                }
            }
            self.ads_rate = 13. * kit.equipped(slot).aim_speed();
        }
        self.aim_scene.receive(&new);
        self.motion.receive(&new);
        self.render_local = player.clone();
        self.local = player.clone();
        self.state = new;
        if self.state["phase"] == "ended" && self.mode != "pause" {
            if self.mode != "ended" {
                grab(false);
            }
            self.mode = "ended";
        } else if self.mode == "connecting"
            || (self.mode == "ended" && self.state["phase"] == "playing")
        {
            self.mode = "playing";
            self.next_input = Instant::now();
            self.status.clear();
            self.wait_trigger_release = true;
            grab(true);
        }
        Ok(())
    }
    fn pause(&mut self) {
        self.mode = "pause";
        grab(false);
        self.edges = json!({});
        self.fire_tap = false;
        self.secondary_tap = false;
        if let Some(c) = self.client.as_mut() {
            self.seq += 1;
            let _ = c.input(json!({"seq":self.seq,"yaw":self.yaw,"pitch":self.pitch}));
        }
    }
    fn resume(&mut self) {
        if self.state["phase"] == "ended" {
            self.mode = "ended";
            grab(false);
        } else {
            self.mode = "playing";
            self.next_input = Instant::now();
            self.wait_trigger_release = true;
            grab(true);
        }
    }
    fn look_and_send(&mut self, dt: f32, active: bool, mouse_speed: f32) -> Result<(), String> {
        let captured = (self.mode == "playing" && active).then(|| Vec2::from(mouse_position()));
        let delta = self.pointer.sample(captured);
        if captured.is_some() {
            if !is_mouse_button_down(MouseButton::Left)
                && !is_key_down(KeyCode::Space)
                && (!self.local["vehicleId"].is_number()
                    || !is_mouse_button_down(MouseButton::Right))
            {
                self.wait_trigger_release = false;
            }
            if self.local["vehicleId"].is_number()
                && self.local["alive"] == true
                && !self.local["droneId"].is_number()
            {
                let was_onboard = self.vehicle_camera.onboard;
                if is_key_pressed(KeyCode::Y) {
                    self.vehicle_camera.toggle();
                }
                let armored = self.in_armored_optics();
                self.vehicle_camera.scroll(mouse_wheel().1, armored);
                if !was_onboard && self.in_armored_optics() {
                    if let Some((_, joint)) = self.motion.local_turret { self.yaw = joint; }
                }
                if self.in_armored_optics() && is_key_pressed(KeyCode::N) {
                    self.vehicle_camera.sensor = self.vehicle_camera.sensor.next();
                }
                if self.in_armored_optics() && is_key_pressed(KeyCode::Z) {
                    self.vehicle_camera.cycle_magnification();
                }
            }
            let sensitivity = if self.in_armored_optics() {
                0.0022 / self.vehicle_camera.magnification()
            } else if self.drone_zoom || is_mouse_button_down(MouseButton::Right) {
                0.00135
            } else {
                0.0022
            } * mouse_speed;
            let previous_yaw = self.yaw;
            self.yaw += delta.x * sensitivity;
            self.pitch = (self.pitch - delta.y * sensitivity).clamp(-1.28, 1.28);
            self.yaw += (is_key_down(KeyCode::Right) as i8 - is_key_down(KeyCode::Left) as i8)
                as f32
                * dt
                * 1.75;
            if self.local["vehicleId"].is_null() {
                self.yaw = self.yaw.sin().atan2(self.yaw.cos());
            }
            if self.in_armored_optics() {
                if let Some((_, joint)) = self.motion.local_turret {
                    self.yaw = vehicle_camera::optic_command(previous_yaw, joint, self.yaw-previous_yaw);
                }
                let mount = self.driver_vehicle().and_then(|v| frontline_surge::vehicle_gun::Mount::for_kind(v["kind"].as_str()?));
                if let Some(mount) = mount { self.pitch = mount.pitch(self.pitch); }
            }
            if is_key_pressed(KeyCode::N) && !self.in_armored_optics() {
                self.edges["cycleMode"] = json!(true);
            }
            for (key, name) in [
                (KeyCode::E, "interact"),
                (KeyCode::B, "build"),
                (KeyCode::R, "reload"),
                (KeyCode::Q, "cycleBuild"),
                (KeyCode::F, "vehicle"),
                (KeyCode::G, "service"),
                (KeyCode::V, "upgradeSelect"),
                (KeyCode::U, "upgrade"),

                (KeyCode::X, "pulse"),
                (KeyCode::C, "ammoCraft"),
                (KeyCode::H, "grenade"),

            ] {
                if is_key_pressed(key) {
                    self.edges[name] = json!(true);
                }
            }
            if is_key_pressed(KeyCode::J) {
                // The legacy drone edge also launches or recalls on older protocol-5 hosts.
                self.edges["drone"] = json!(true);
                self.edges["droneLink"] = json!(true);
                self.edges["droneRecall"] = json!(is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift));
            }
            for (key, id) in [
                (KeyCode::Key1, "mr7"),
                (KeyCode::Key2, "vx9"),
                (KeyCode::Key3, "ar4"),
            ] {
                if is_key_pressed(key) {
                    self.edges["weapon"] = json!(id);
                }
            }
            if !self.wait_trigger_release
                && self.local["vehicleId"].is_number()
                && is_mouse_button_pressed(MouseButton::Right)
            {
                self.secondary_tap = true;
            }
            if !self.wait_trigger_release
                && (is_mouse_button_pressed(MouseButton::Left) || is_key_pressed(KeyCode::Space))
            {
                if self.local["droneId"].is_number() {
                    self.edges["ping"] = json!(true);
                } else {
                    self.fire_tap = true;
                    if let Some((preview, support)) = self.drawn_build {
                        self.edges["place"] = json!(true);
                        if let Some(id) = preview {
                            self.edges["previewId"] = id.into();
                        }
                        self.edges["supportBuildId"] = json!(support);
                    }
                }
            }
        }
        let Some(link) = self.client.as_mut() else {
            return Ok(());
        };
        if link.id.is_none() {
            return Ok(());
        }
        let period = Duration::from_millis(if self.mode == "playing" { 32 } else { 600 });
        let now = Instant::now();
        let trigger_edge = self.fire_tap
            || self.secondary_tap
            || (active && is_mouse_button_pressed(MouseButton::Right));
        if now < self.next_input
            && (!trigger_edge || now.duration_since(self.last_input) < Duration::from_millis(8))
        {
            return Ok(());
        }
        // Send the trigger promptly, and carry the regular cadence across render frames.
        self.next_input = if now < self.next_input || now - self.next_input >= period {
            now + period
        } else {
            self.next_input + period
        };
        self.last_input = now;
        self.seq += 1;
        let mut v = std::mem::replace(&mut self.edges, json!({}));
        v["seq"] = json!(self.seq);
        v["yaw"] = json!(self.yaw.sin().atan2(self.yaw.cos()));
        if self.local["vehicleId"].is_number() {
            v["turretAim"] = json!(self.yaw);
        }
        v["pitch"] = json!(self.pitch);
        if self.mode == "playing" && active && self.local["alive"] == true {
            let f = (is_key_down(KeyCode::W) || is_key_down(KeyCode::Up)) as i8
                - (is_key_down(KeyCode::S) || is_key_down(KeyCode::Down)) as i8;
            let s = is_key_down(KeyCode::D) as i8 - is_key_down(KeyCode::A) as i8;
            v["forward"] = json!(f);
            v["strafe"] = json!(s);
            v["shoot"] = json!(
                !self.wait_trigger_release
                    && !self.local["droneId"].is_number()
                    && (self.fire_tap
                        || is_mouse_button_down(MouseButton::Left)
                        || is_key_down(KeyCode::Space))
            );
            let right_down = is_mouse_button_down(MouseButton::Right);
            let seated = self.local["vehicleId"].is_number();
            v["aim"] = json!(right_down && (!seated || !self.wait_trigger_release));
            v["apcRounds"] = json!(seated);
            v["secondary"] =
                json!(seated && !self.wait_trigger_release && (right_down || self.secondary_tap));
            v["sprint"] =
                json!(is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift));
        }
        self.fire_tap = false;
        self.secondary_tap = false;
        link.input(v)
            .map_err(|_| "Связь с сервером прервалась.".into())
    }
    fn driver_vehicle(&self) -> Option<&Value> {
        let id = self.local["vehicleId"].as_u64()?;
        self.state["vehicles"].as_array()?.iter().find(|v| v["id"].as_u64() == Some(id))
    }
    fn in_armored_optics(&self) -> bool {
        self.vehicle_camera.onboard && self.driver_vehicle().is_some_and(|v|
            matches!(v["kind"].as_str(), Some("mbt" | "apc")))
    }
    fn active_sensor(&self) -> vehicle_camera::Sensor {
        if self.in_armored_optics() { self.vehicle_camera.sensor } else { vehicle_camera::Sensor::Day }
    }
    fn update_vehicle_sight(&mut self, dt: f32) {
        let id = self.local["vehicleId"].as_u64();
        self.motion.local_turret = None;
        self.motion.optic_vehicle = None;
        let Some(v) = self.state["vehicles"].as_array().and_then(|vs| vs.iter()
            .find(|v| id.is_some() && v["id"].as_u64() == id)) else { return };
        let Some(mount) = frontline_surge::vehicle_gun::Mount::for_kind(v["kind"].as_str().unwrap_or("")) else {return};
        let yaw = if let Some(latest) = v["turretUnwrapped"].as_f64() {
            self.vehicle_camera.turret(id.unwrap(), latest as f32, self.yaw, self.motion.age, dt)
        } else {
            self.motion.vehicle(v).turret
        };
        self.motion.local_turret = Some((id.unwrap(), yaw));
        if self.vehicle_camera.onboard { self.motion.optic_vehicle = id; }
        let pose = self.motion.vehicle(v);
        let pitch = mount.pitch(self.pitch);
        let ray = mount.ray(pose.position.x, pose.position.z, pose.yaw, yaw, pitch, true);
        let origin = Vec3::from_array(ray.origin);
        let direction = Vec3::from_array(ray.direction);
        let predicted = self.aim_scene.machinegun_contact(origin, direction,
            frontline_surge::vehicle_gun::MG_RANGE, &self.state, &self.local, &self.motion);
        let contact = vehicle_camera::reconcile(origin, direction, predicted, &self.local, self.motion.age);
        self.render_local["mgSight"] = json!(contact.to_array());
        self.render_local["sightRange"] = json!(origin.distance(contact));
        self.render_local["gunYaw"] = json!(yaw);
        self.render_local["gunPitch"] = json!(pitch);
        self.render_local["onboardOptic"] = json!(self.vehicle_camera.onboard);
        self.render_local["opticSensor"] = json!(self.vehicle_camera.sensor.label());
        self.render_local["opticZoom"] = json!(self.vehicle_camera.magnification());
    }
    fn update_presentation(&mut self, dt: f32, active: bool) {
        if !self.local.is_object() {
            return;
        }
        self.motion.local_gun_pitch = self.local["vehicleId"].as_u64().map(|id| (id, self.pitch));
        self.update_vehicle_sight(dt);
        let aim = active
            && is_mouse_button_down(MouseButton::Right)
            && self.local["alive"] == true
            && self.local["buildMode"] != true
            && self.local["vehicleId"].is_null()
            && self.local["droneId"].is_null()
            && num(&self.local, "reload") <= 0.;
        let target = if aim { 1. } else { 0. };
        self.ads += (target - self.ads) * (1. - (-self.ads_rate * dt).exp());
        self.render_local["aim"] = json!(self.ads);
        self.render_local["bob"] = json!(self.motion.player(&self.local).bob);
        for (key, decay) in [
            ("droneLaunch", 1.),
            ("reload", 1.),
            ("recoil", 6.),
            ("hitFlash", 2.7),
            ("hitConfirm", 3.8),
        ] {
            let value = (num(&self.local, key) - decay * self.motion.age).max(0.);
            self.render_local[key] = json!(value);
        }
    }
    fn camera(&mut self, dt: f32) -> Camera3D {
        let drone = self.state["drones"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|v| v["id"] == self.local["droneId"]);
        let car = self.state["vehicles"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|v| v["id"] == self.local["vehicleId"]);
        let mut world_up = Vec3::Y;
        let (eye, look, fov) = if let Some(v) = drone {
            let rendered = self.motion.drone(v);
            let eye = rendered.position;
            let look = dir(self.yaw, self.pitch);
            world_up = Quat::from_axis_angle(
                look,
                rendered.roll * if self.drone_zoom { 0.35 } else { 1. },
            ) * Vec3::Y;
            (eye, look, if self.drone_zoom { 43.4 } else { 77. })
        } else if let Some(v) = car {
            let pose = self.motion.vehicle(v);
            self.body = pose.position;
            if self.vehicle_camera.onboard {
                vehicle_camera::onboard(
                    v["kind"].as_str().unwrap_or(""),
                    self.body,
                    pose.yaw,
                    pose.turret,
                    self.yaw,
                    self.pitch,
                    self.vehicle_camera.magnification(),
                )
            } else {
                let height = match v["kind"].as_str() {
                    Some("scout_buggy") => 1.35,
                    Some("patrol4x4") => 1.75,
                    Some("apc") => 2.45,
                    _ => 2.9,
                };
                let focus = self.body + Vec3::Y * (height + 0.65);
                let look = dir(self.yaw, self.pitch);
                let distance = self.vehicle_camera.chase_distance(height < 2., dt);
                let desired = focus - look * distance + Vec3::Y * 1.35;
                let eye = self.aim_scene.clip(
                    focus,
                    desired.max(vec3(f32::NEG_INFINITY, 0.32, f32::NEG_INFINITY)),
                );
                (eye, (focus + look * 15. - eye).normalize_or_zero(), 75.)
            }
        } else if self.position_ready {
            let pose = self.motion.player(&self.local);
            self.body = pose.position;
            let bob = (pose.bob * 2.).sin() * 0.024 * pose.stride;
            let height = if self.local["alive"] == false {
                0.9
            } else {
                1.68 + bob
            };
            (
                self.body + Vec3::Y * height,
                dir(
                    self.yaw,
                    self.pitch + num(&self.render_local, "recoil") * 0.008,
                ),
                73. + (self.local["adsFov"].as_f64().unwrap_or(53.) as f32 - 73.) * self.ads,
            )
        } else {
            let t = get_time() as f32;
            (
                vec3(20. + (t * 0.11).sin() * 0.12, 2., -2.5),
                dir(-1.31 + (t * 0.07).sin() * 0.012, 0.02),
                66.,
            )
        };
        Camera3D {
            position: eye,
            target: eye + look,
            up: world_up,
            fovy: fov.to_radians(),
            z_near: 0.035,
            z_far: 820.,
            ..Default::default()
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let font =
        load_ttf_font_from_bytes(include_bytes!("../assets/ui.ttf")).expect("Embedded UI font");
    let mut screen = Ui::new();
    let mut field = world::World::new(screen.settings.graphics);
    let mut people = actors::Actors::new();
    let mut audio = sound::Sound::new();
    let mut play = Play::new();
    let mut fullscreen = false;
    let mut debug = debug_ui::DebugUi::new();
    let mut spectator: Option<debug_ui::FreeCamera> = None;
    let mut video = drone_view::Feed::new();
    let mut optic_display = vehicle_sensor::Display::new();
    loop {
        let frame_time = get_frame_time().max(0.);
        let dt = frame_time.min(0.1);
        play.motion.advance(frame_time);
        if let Err(e) = play.poll() {
            play.reset(e);
            people.clear();
            audio.clear();
        }
        if play.mode != "playing" {
            screen.set_inventory(false);
            screen.set_map(false);
            screen.set_crafting(false);
            debug.open = false;
            spectator = None;
        }
        let before_overlay = debug.open || screen.modal_open();
        let operator = play.client.as_ref().is_some_and(|c| c.developer);
        if play.mode == "playing" {
            if !has_focus() {
                screen.set_inventory(false);
                screen.set_map(false);
                screen.set_crafting(false);
                debug.open = false;
                spectator = None;
                play.pause();
            } else if is_key_pressed(KeyCode::F2) {
                debug.open = !debug.open;
                screen.set_inventory(false);
                screen.set_map(false);
                screen.set_crafting(false);
            } else if !debug.open && spectator.is_none() {
                if is_key_pressed(KeyCode::K) {
                    screen.set_crafting(!screen.crafting_open());
                } else if is_key_pressed(KeyCode::M) {
                    screen.set_map(!screen.map_open());
                } else if (is_key_pressed(KeyCode::I) || is_key_pressed(KeyCode::T))
                    && !play.local["droneId"].is_number()
                {
                    screen.set_inventory(!screen.inventory_open());
                } else if !screen.modal_open() && play.local["alive"] == true {
                    if is_key_pressed(KeyCode::E) {
                        screen.interact_station(&play.state, &play.local);
                    }
                    if play.local["droneId"].is_number() && is_key_pressed(KeyCode::Z) {
                        play.drone_zoom = !play.drone_zoom;
                    }
                }
            }
        }
        if is_key_pressed(KeyCode::Escape) {
            if debug.open {
                debug.open = false;
            } else if spectator.is_some() {
                spectator = None;
                play.wait_trigger_release = true;
            } else if !screen.handle_back() {
                match play.mode {
                    "playing" => play.pause(),
                    "pause" => play.resume(),
                    "connecting" => {
                        play.reset(String::new());
                        people.clear();
                        audio.clear();
                    }
                    _ => {}
                }
            }
        }
        let overlay = debug.open || screen.modal_open();
        if before_overlay != overlay && play.mode == "playing" {
            grab(!overlay);
            play.fire_tap = false;
            play.secondary_tap = false;
            play.wait_trigger_release = true;
            play.edges = json!({});
        }
        if let Err(e) = play.look_and_send(
            dt,
            !overlay && spectator.is_none(),
            screen.settings.sensitivity as f32 / 100.,
        ) {
            play.reset(e);
            debug.open = false;
            spectator = None;
            screen.set_inventory(false);
            screen.set_map(false);
            screen.set_crafting(false);
            people.clear();
            audio.clear();
        }
        if (debug.open || spectator.is_some()) && is_key_pressed(KeyCode::F3) {
            screen.set_fps_visible(!screen.fps_visible());
        }
        if is_key_pressed(KeyCode::F11) {
            screen.settings.fullscreen = !screen.settings.fullscreen;
            screen.save_preferences();
        }
        if fullscreen != screen.settings.fullscreen {
            fullscreen = screen.settings.fullscreen;
            set_fullscreen(fullscreen);
        }
        play.update_presentation(
            dt,
            play.mode == "playing" && !overlay && spectator.is_none(),
        );
        let mut cam = if let Some(f) = spectator.as_mut() {
            f.camera(dt, !overlay)
        } else {
            play.camera(dt)
        };
        let in_feed = play.local["droneId"].is_number() && spectator.is_none();
        let sensor = if spectator.is_none() && !in_feed { play.active_sensor() } else { vehicle_camera::Sensor::Day };
        let thermal = sensor == vehicle_camera::Sensor::Thermal;
        if in_feed {
            cam.render_target = Some(video.target());
        } else if sensor != vehicle_camera::Sensor::Day {
            cam.render_target = Some(optic_display.target());
        }
        set_camera(&cam);
        clear_background(Color::from_rgba(153, 174, 177, 255));
        let snapshot = if play.state.is_null() {
            None
        } else {
            Some(&play.state)
        };
        let ghost = play
            .local
            .get("preview")
            .filter(|p| p.is_object() && play.local["alive"] == true);
        if spectator.is_some() { play.motion.optic_vehicle = None; }
        field.set_thermal(thermal);
        field.set_graphics(screen.settings.graphics);
        field.draw(snapshot, cam.position, &play.motion);
        play.drawn_build = None;
        if snapshot.is_some() {
            people.set_thermal(thermal);
            people.set_daylight(field.daylight());
            if spectator.is_some() {
                let mut local = play.local.clone();
                let head = vec3(num(&local, "x"), 1.55, num(&local, "z"));
                let inside_operator = (cam.position.x - head.x).hypot(cam.position.z - head.z)
                    < 0.57
                    && cam.position.y < 2.15;
                local["freeCamera"] = json!(!inside_operator);
                people.draw_world(&play.state, &local, dt, &play.motion, cam.position);
            } else {
                people.draw_world(&play.state, &play.render_local, dt, &play.motion, cam.position);
            }
            drone_view::draw_pulses(&play.state);
            drone_view::draw(&play.state, &play.local, cam.position, &play.motion);
            if let Some(p) = ghost {
                field.draw_build_preview(p);
                play.drawn_build = Some((p["previewId"].as_u64(), p["supportBuildId"].as_u64()));
            }
            if let Some(id) = play.client.as_ref().and_then(|c| c.id) {
                let heading = motion::view_angles(cam.target - cam.position).0;
                audio.update(
                    &play.state,
                    id,
                    (cam.position.x, cam.position.y, cam.position.z, heading),
                );
            }
            if spectator.is_none()
                && play.local["alive"] == true
                && play.mode != "ended"
                && !play.local["vehicleId"].is_number()
                && !play.local["droneId"].is_number()
            {
                people.draw_weapon(
                    &play.render_local,
                    screen_width() / screen_height().max(1.),
                    dt,
                );
            }
        }
        set_default_camera();
        if in_feed {
            let signal = play.state["drones"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|v| v["id"] == play.local["droneId"])
                .map_or(1., |v| num(v, "signal"));
            video.present(signal);
        } else if sensor != vehicle_camera::Sensor::Day {
            optic_display.present(sensor);
        }
        screen.set_drone_zoom(play.drone_zoom);
        screen.set_scoreboard(play.mode == "playing" && !overlay && is_key_down(KeyCode::Tab));
        let action = if debug.open {
            match debug.draw(
                &font,
                &play.state,
                &play.local,
                operator,
                play.client.as_ref().is_some_and(|c| c.vehicle_service),
                spectator.is_some(),
                screen.fps_visible(),
            ) {
                debug_ui::DebugAction::None => {}
                debug_ui::DebugAction::Close => debug.open = false,
                debug_ui::DebugAction::Fps => screen.set_fps_visible(!screen.fps_visible()),
                debug_ui::DebugAction::Flight => {
                    if operator {
                        spectator = if spectator.is_some() {
                            None
                        } else {
                            Some(debug_ui::FreeCamera::at(&cam))
                        };
                        debug.open = false;
                        play.wait_trigger_release = true;
                    }
                }
                debug_ui::DebugAction::Command(value) => {
                    if let Some(client) = play.client.as_mut() {
                        if let Err(e) = client.debug(value) {
                            play.status = e.to_string();
                        }
                    }
                }
            }
            Action::None
        } else if spectator.is_some() {
            draw_rectangle(14., 14., 385., 33., Color::new(0.05, 0.08, 0.07, 0.78));
            let note = if screen.fps_visible() {
                format!("Камера  ·  Esc к бойцу  ·  F2 меню  ·  {} FPS", get_fps())
            } else {
                "Камера  ·  Esc к бойцу  ·  F2 меню".to_string()
            };
            draw_text_ex(
                &note,
                24.,
                36.,
                TextParams {
                    font: Some(&font),
                    font_size: 20,
                    color: WHITE,
                    ..Default::default()
                },
            );
            Action::None
        } else {
            screen.draw(
                play.mode,
                &play.state,
                &play.render_local,
                &font,
                &play.status,
                &play.endpoint,
                &cam,
            )
        };
        match action {
            Action::Input(v) => {
                if let Some(fields) = v.as_object() {
                    for (k, value) in fields {
                        play.edges[k] = value.clone();
                    }
                }
                play.wait_trigger_release = true;
            }
            Action::None => {}
            Action::Quit => break,
            Action::Craft(id) => {
                play.edges["craft"] = json!(id);
                play.wait_trigger_release = true;
            }
            Action::Join(name, host, port, team) => {
                play.endpoint = format!("{host}:{port}");
                play.connect(name, host, port, team);
            }
            Action::Resume => play.resume(),
            Action::Leave => {
                screen.set_crafting(false);
                screen.set_map(false);
                play.reset(String::new());
                people.clear();
                audio.clear();
            }
        }
        if play.mode == "playing" && overlay != (debug.open || screen.modal_open()) {
            grab(!debug.open && !screen.modal_open());
            play.wait_trigger_release = true;
            play.fire_tap = false;
            play.secondary_tap = false;
        }
        next_frame().await;
    }
}
