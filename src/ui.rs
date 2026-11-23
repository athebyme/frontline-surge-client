//! Native immediate-mode menus, scoreboard and field HUD.
use macroquad::prelude::*;
use crate::settings::Settings;
#[path = "settings_ui.rs"]
mod settings_ui;
use serde_json::Value;
#[path = "tactical_map.rs"]
mod tactical_map;
#[path = "armor_optics.rs"]
mod armor_optics;
use tactical_map::TacticalMap;
#[path = "crafting.rs"]
mod crafting;
use crafting::CraftMenu;
pub(crate) use crafting::friendly_vehicle;
#[path = "stash_ui.rs"]
mod stash_ui;
#[path = "field_hud.rs"]
mod field_hud;
#[path = "debug_ui.rs"]
pub(crate) mod debug_ui;
use stash_ui::Stash;

const PAPER: Color = Color::new(0.910, 0.914, 0.863, 1.);
const GOLD: Color = Color::new(0.839, 0.694, 0.443, 1.);
const MUTED: Color = Color::new(0.663, 0.690, 0.635, 1.);
const BLUE_TEAM: Color = Color::new(0.537, 0.682, 0.682, 1.);
const RED_TEAM: Color = Color::new(0.765, 0.561, 0.502, 1.);
const INK: Color = Color::new(0.078, 0.098, 0.094, 1.);
const PLATE: Color = Color::new(0.102, 0.125, 0.118, 0.96);
const EDGE: Color = Color::new(0.494, 0.541, 0.490, 0.51);

#[derive(Debug, PartialEq)]
pub enum Action {
    None,
    Join(String, String, u16, String),
    Craft(String),
    Input(Value),
    Resume,
    Leave,
    Quit,
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    Nick,
    Address,
}

pub struct Ui {
    field_font: Font,
    nickname: String,
    choice: &'static str,
    craft_open: bool,
    craft: CraftMenu,
    address: String,
    editing: Option<Field>,
    select_all: bool,
    help: bool,
    scoreboard: bool,
    map_open: bool,
    map: TacticalMap,
    inventory_open: bool,
    inventory: Stash,
    pub(crate) settings: Settings,
    settings_open: bool,
    settings_write_failed: bool,
    fps_elapsed: f32,
    fps_frames: u32,
    fps: f32,
    drone_zoom: bool,
    help_page: u8,
    error: String,
    copied: f64,
    menu_font_scale: f32,
}

#[derive(Clone, Copy)]
struct Surface<'a> {
    title_font: Option<&'a Font>,
    font: &'a Font,
    scale: f32,
    w: f32,
    h: f32,
    pointer: Vec2,
    click: bool,
}

fn alpha(mut color: Color, a: f32) -> Color {
    color.a = a;
    color
}
fn number(v: &Value, name: &str, fallback: f64) -> f64 {
    v.get(name).and_then(Value::as_f64).unwrap_or(fallback)
}
fn string<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or(default)
}
fn flag(v: &Value, key: &str, default: bool) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(default)
}
fn sight_position(matrix: Mat4, point: Vec4, viewport: Vec2) -> Option<Vec2> {
    let clip = matrix * point;
    if !clip.is_finite() || clip.w <= 0. || clip.x.abs() > clip.w || clip.y.abs() > clip.w {
        return None;
    }
    let ndc = vec2(clip.x, -clip.y) / clip.w;
    Some((ndc + Vec2::ONE) * 0.5 * viewport)
}
fn mg_position(matrix: Mat4, player: &Value, yaw: f32, pitch: f32, viewport: Vec2) -> Option<Vec2> {
    let point = match player["mgSight"].as_array() {
        Some(coords) if coords.len() == 3 => {
            vec3(coords[0].as_f64()? as f32, coords[1].as_f64()? as f32, coords[2].as_f64()? as f32).extend(1.)
        }
        // Older protocol-5 servers only publish the barrel angles.
        _ => crate::dir(yaw, pitch.clamp(-0.7, 0.7)).extend(0.),
    };
    sight_position(matrix, point, viewport)
}
fn kill_source(event: &Value) -> &str {
    match (event["weapon"].as_str(), event["vehicleKind"].as_str()) {
        (Some("mr7"), _) => "MR-7",
        (Some("vx9"), _) => "VX-9",
        (Some("ar4"), _) => "AR-4",
        (Some("apc_he"), _) => "ОФ БТР",
        (Some("tank"), _) => "Пушка танка",
        (Some("vehicle_mg"), Some("mbt")) => "Пулемёт танка",
        (Some("vehicle_mg"), Some("apc")) => "Пулемёт БТР",
        (Some("vehicle_mg"), _) => "Бортовой пулемёт",
        (Some("ram"), Some("mbt")) => "Наезд танка",
        (Some("ram"), Some("apc")) => "Наезд БТР",
        (Some("ram"), Some("cargo6x6")) => "Наезд грузовика",
        (Some("ram"), Some("patrol4x4")) => "Наезд 4×4",
        (Some("ram"), Some("scout_buggy")) => "Наезд багги",
        (Some("ram"), _) => "Наезд",
        (Some("grenade"), _) => "Граната",
        (Some("drone_pulse"), _) => "Импульс дрона",
        (Some(other), _) if !other.is_empty() => other,
        _ => "›",
    }
}

fn heading_label(v: &Value, key: &str) -> String {
    v[key].as_f64().map_or_else(|| "—".into(), |yaw| {
        format!("{:03}°", (yaw.to_degrees().round() as i32).rem_euclid(360))
    })
}

fn count(v: &Value, key: &str, fallback: i32) -> i32 {
    number(v, key, fallback as f64).ceil() as i32
}
const MATERIALS: [(&str, &str); 5] = [
    ("wood", "Дерево"),
    ("metal", "Металл"),
    ("oil", "Нефть"),
    ("powder", "Порох"),
    ("fuel", "Топливо"),
];
fn vehicle_name(kind: &str) -> &str {
    match kind {
        "patrol4x4" => "Патрульный 4×4",
        "cargo6x6" => "Грузовой 6×6",
        "mbt" => "Основной танк",
        "apc" => "Бронетранспортёр",
        "scout_buggy" => "Разведбагги",
        _ => "Машина",
    }
}
fn kit_name(kind: &str) -> &str {
    match kind {
        "mobility" => "Ходовая",
        "utility" => "Оснащение",
        _ => "Штатный",
    }
}
fn plan_name(kind: &str) -> &str {
    match kind {
        "refinery" => "Нефтеперегонка",
        "workshop" => "Мастерская",
        _ => "Стена",
    }
}
fn plan_cost(kind: &str) -> &str {
    match kind {
        "refinery" => "8 дерева · 6 металла",
        "workshop" => "6 дерева · 8 металла",
        _ => "3 дерева · 1 металл",
    }
}
fn nearest<'a>(
    list: &'a Value,
    player: &Value,
    range: f64,
    allowed: impl Fn(&Value) -> bool,
) -> Option<&'a Value> {
    let px = number(player, "x", 0.);
    let pz = number(player, "z", 0.);
    list.as_array()?
        .iter()
        .filter(|v| allowed(v))
        .filter_map(|v| {
            let dx = number(v, "x", 0.) - px;
            let dz = number(v, "z", 0.) - pz;
            let d = dx * dx + dz * dz;
            (d <= range * range).then_some((d, v))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, v)| v)
}
fn own_drone<'a>(state: &'a Value, p: &Value) -> Option<&'a Value> {
    let id = &p["droneId"];
    if id.is_null() {
        return None;
    }
    state["drones"]
        .as_array()?
        .iter()
        .find(|d| d["id"] == *id && d["owner"] == p["id"] && d["team"] == p["team"])
}
fn clock(seconds: f64) -> String {
    let s = seconds.max(0.).ceil() as i32;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

impl<'a> Surface<'a> {
    fn new(font: &'a Font) -> Self {
        let scale = (screen_width() / 1140.)
            .min(screen_height() / 680.)
            .clamp(0.32, 3.);
        Self::scaled(font, scale)
    }
    fn field(font: &'a Font) -> Self {
        let w = screen_width();
        let h = screen_height();
        // Give the three inventory columns more room on a small desktop window.
        let standard = (w / 1140.).min(h / 680.).clamp(0.9, 3.);
        Self::scaled(font, standard.min(w / 1020.).min(h / 572.).max(0.01))
    }
    fn scaled(font: &'a Font, scale: f32) -> Self {
        let (mx, my) = mouse_position();
        Self {
            title_font: None,
            font,
            scale,
            w: screen_width() / scale,
            h: screen_height() / scale,
            pointer: vec2(mx / scale, my / scale),
            click: is_mouse_button_pressed(MouseButton::Left),
        }
    }
    fn bold(&self) -> Self {
        Self {
            font: self.title_font.unwrap_or(self.font),
            ..*self
        }
    }
    fn menu_ribbon(&self, x: f32, y: f32, w: f32) {
        self.rect(x, y, w, 1., EDGE);
    }
    fn rect(&self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        draw_rectangle(
            x * self.scale,
            y * self.scale,
            w * self.scale,
            h * self.scale,
            c,
        );
    }
    fn border(&self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        draw_rectangle_lines(
            x * self.scale,
            y * self.scale,
            w * self.scale,
            h * self.scale,
            1.2 * self.scale,
            c,
        );
    }
    fn line(&self, x: f32, y: f32, bx: f32, by: f32, th: f32, c: Color) {
        draw_line(
            x * self.scale,
            y * self.scale,
            bx * self.scale,
            by * self.scale,
            th * self.scale,
            c,
        );
    }
    fn measure(&self, text: &str, size: f32) -> f32 {
        measure_text(
            text,
            Some(self.font),
            (size * self.scale).round().max(1.) as u16,
            1.,
        )
        .width
            / self.scale
    }
    fn text(&self, tx: &str, x: f32, baseline: f32, size: f32, c: Color) {
        let px = (size * self.scale).round().max(1.) as u16;
        draw_text_ex(
            tx,
            x * self.scale,
            baseline * self.scale,
            TextParams {
                font: Some(self.font),
                font_size: px,
                color: c,
                ..Default::default()
            },
        );
    }
    fn mid(&self, t: &str, x: f32, y: f32, size: f32, c: Color) {
        self.text(t, x - self.measure(t, size) * 0.5, y, size, c);
    }
    fn right(&self, t: &str, x: f32, y: f32, size: f32, c: Color) {
        self.text(t, x - self.measure(t, size), y, size, c);
    }
    fn over(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.pointer.x >= x
            && self.pointer.x <= x + w
            && self.pointer.y >= y
            && self.pointer.y <= y + h
    }
    fn keycap_width(&self, key: &str) -> f32 {
        (self.bold().measure(key, 12.) + 9.).max(18.)
    }
    fn keycap(&self, key: &str, x: f32, baseline: f32) -> f32 {
        let width = self.keycap_width(key);
        self.rect(x, baseline - 13., width, 17., PLATE);
        self.border(x, baseline - 13., width, 17., alpha(MUTED, 0.75));
        self.bold().mid(key, x + width / 2., baseline, 12., PAPER);
        width
    }
    fn button(&self, x: f32, y: f32, w: f32, h: f32, label: &str, primary: bool) -> bool {
        let hover = self.over(x, y, w, h);
        if primary {
            self.rect(x, y, w, h, if hover { PAPER } else { GOLD });
        } else {
            self.rect(x, y, w, h, if hover { Color::new(0.20, 0.23, 0.20, 1.) } else { PLATE });
            self.border(x, y, w, h, if hover { MUTED } else { EDGE });
        }
        self.bold().mid(
            label,
            x + w * 0.5,
            y + h * 0.5 + 5.,
            15.,
            if primary { INK } else { PAPER },
        );
        hover && self.click
    }
    fn disabled(&self, x: f32, y: f32, w: f32, h: f32, label: &str) {
        self.rect(x, y, w, h, alpha(INK, 0.42));
        self.border(x, y, w, h, alpha(MUTED, 0.20));
        self.bold().mid(
            label,
            x + w * 0.5,
            y + h * 0.5 + 5.,
            14.,
            alpha(MUTED, 0.87),
        );
    }
    fn wrap(&self, text: &str, x: f32, baseline: f32, width: f32, size: f32, c: Color) {
        let mut line = String::new();
        let mut y = baseline;
        for word in text.split_whitespace() {
            let next = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && self.measure(&next, size) > width {
                self.text(&line, x, y, size, c);
                y += size * 1.45;
                line = word.to_owned();
            } else {
                line = next;
            }
        }
        if !line.is_empty() {
            self.text(&line, x, y, size, c);
        }
    }
    fn shorten(&self, text: &str, size: f32, width: f32) -> String {
        if self.measure(text, size) <= width {
            return text.to_owned();
        }
        let mut out = String::new();
        for c in text.chars() {
            out.push(c);
            if self.measure(&out, size) > width - self.measure("…", size) {
                out.pop();
                break;
            }
        }
        out.push('…');
        out
    }
    fn vignette(&self) {
        let w = self.w * self.scale;
        let h = self.h * self.scale;
        let mut mesh = Mesh {
            vertices: Vec::with_capacity(16),
            indices: Vec::with_capacity(42),
            texture: None,
        };
        // Shared edges and per-vertex alpha keep the vignette smooth at any resolution.
        for (i, (fraction, opacity)) in [
            (0., 0.96),
            (0.21, 0.84),
            (0.43, 0.43),
            (0.68, 0.10),
            (0.91, 0.),
        ]
        .into_iter()
        .enumerate()
        {
            for y in [0., h] {
                mesh.vertices.push(Vertex::new(
                    fraction * w,
                    y,
                    0.,
                    0.,
                    0.,
                    alpha(INK, opacity),
                ));
            }
            if i > 0 {
                let k = (i * 2) as u16;
                mesh.indices
                    .extend_from_slice(&[k - 2, k - 1, k, k, k - 1, k + 1]);
            }
        }
        let k = mesh.vertices.len() as u16;
        for (y, opacity) in [(h - 82. * self.scale, 0.), (h, 0.47)] {
            for x in [0., w] {
                mesh.vertices
                    .push(Vertex::new(x, y, 0., 0., 0., alpha(INK, opacity)));
            }
        }
        mesh.indices
            .extend_from_slice(&[k, k + 1, k + 2, k + 2, k + 1, k + 3]);
        draw_mesh(&mesh);
    }
    fn logo(&self, x: f32, y: f32) {
        let pts = [
            (1., 8.),
            (18., 1.),
            (35., 8.),
            (35., 24.),
            (18., 36.),
            (1., 24.),
            (1., 8.),
        ];
        for p in pts.windows(2) {
            self.line(x + p[0].0, y + p[0].1, x + p[1].0, y + p[1].1, 1.8, GOLD);
        }
        for gap in [0., 6.] {
            self.line(x + 9., y + 16. + gap, x + 18., y + 11. + gap, 2.9, GOLD);
            self.line(x + 18., y + 11. + gap, x + 27., y + 16. + gap, 2.9, GOLD);
        }
        self.text("FRONTLINE  SURGE", x + 47., y + 25., 16., PAPER);
    }
    fn panel(&self, w: f32, h: f32, kicker: &str, title: &str) -> (f32, f32) {
        self.rect(0., 0., self.w, self.h, alpha(INK, 0.57));
        let x = (self.w - w) * 0.5;
        let y = (self.h - h) * 0.5;
        self.rect(x, y, w, h, PLATE);
        self.border(x, y, w, h, EDGE);
        self.text(kicker, x + 27., y + 33., 12., MUTED);
        self.bold().text(title, x + 25., y + 78., 34., PAPER);
        self.menu_ribbon(x + 27., y + 98., w - 54.);
        (x, y)
    }
}

// A bare IPv6 address uses the default port. Brackets disambiguate an explicit port.
fn parse_endpoint(value: &str) -> Result<(String, u16), String> {
    let raw = value.trim();
    if raw.is_empty() || raw.contains('/') || raw.chars().any(char::is_whitespace) {
        return Err("Введи IP или имя сервера, например 192.168.1.10:8787.".into());
    }
    let (host, port) = if let Some(rest) = raw.strip_prefix('[') {
        let (addr, suffix) = rest
            .split_once(']')
            .ok_or("Для IPv6 с портом используй [адрес]:порт.")?;
        if suffix.is_empty() {
            (addr, "8787")
        } else {
            (
                addr,
                suffix
                    .strip_prefix(':')
                    .ok_or("После ] укажи порт через двоеточие.")?,
            )
        }
    } else if raw.matches(':').count() == 1 {
        raw.split_once(':').unwrap()
    } else {
        (raw, "8787")
    };
    if host.is_empty() {
        return Err("Укажи IP или имя сервера.".into());
    }
    Ok((host.to_string(), parse_port(port)?))
}
fn parse_port(value: &str) -> Result<u16, String> {
    value
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|p| *p > 0)
        .ok_or_else(|| "Порт должен быть числом от 1 до 65535.".into())
}

impl Ui {
    pub fn new() -> Self {
        Self {
            field_font: load_ttf_font_from_bytes(include_bytes!("../assets/field-labels.ttf")).expect("Embedded field font"),
            nickname: "Беркут".into(),
            choice: "auto",
            craft_open: false,
            craft: CraftMenu::new(),
            address: "127.0.0.1:8787".into(),
            editing: None,
            select_all: false,
            help: false,
            scoreboard: false,
            map_open: false,
            map: TacticalMap::new(),
            inventory_open: false,
            inventory: Stash::new(),
            settings: Settings::load(),
            settings_open: false,
            settings_write_failed: false,
            fps_elapsed: 0.,
            fps_frames: 0,
            fps: 0.,
            drone_zoom: false,
            help_page: 0,
            error: String::new(),
            copied: -10.,
            menu_font_scale: 0.,
        }
    }
    pub fn set_scoreboard(&mut self, active: bool) {
        self.scoreboard = active;
    }
    pub fn set_map(&mut self, active: bool) {
        if active {
            self.inventory_open = false;
            self.craft_open = false;
        }
        if active && !self.map_open {
            self.map.recenter();
        }
        self.map_open = active;
    }
    pub fn set_crafting(&mut self, active: bool) {
        self.craft_open = active;
        if active {
            self.map_open = false;
            self.inventory_open = false;
            self.craft.browse();
        }
    }
    pub fn set_fps_visible(&mut self, on: bool) {
        self.settings.show_fps = on;
        self.save_preferences();
    }
    pub fn fps_visible(&self) -> bool {
        self.settings.show_fps
    }
    pub fn modal_open(&self) -> bool {
        self.inventory_open || self.craft_open || self.map_open
    }
    pub fn set_inventory(&mut self, active: bool) {
        self.inventory_open = active;
        if active {
            self.map_open = false;
            self.craft_open = false;
        }
    }
    pub fn inventory_open(&self) -> bool {
        self.inventory_open
    }
    pub fn set_drone_zoom(&mut self, active: bool) {
        self.drone_zoom = active
    }
    pub fn available_station(&self, state: &Value, p: &Value) -> Option<&'static str> {
        crafting::available_station(state, p)
    }
    pub fn open_station(&mut self, kind: &str) {
        self.set_crafting(true);
        self.craft.open_station(kind);
    }
    pub fn interact_station(&mut self, state: &Value, p: &Value) -> bool {
        if let Some(kind) = self.available_station(state, p) {
            self.open_station(kind);
            true
        } else {
            false
        }
    }
    pub fn crafting_open(&self) -> bool {
        self.craft_open
    }
    pub fn map_open(&self) -> bool {
        self.map_open
    }
    pub fn handle_back(&mut self) -> bool {
        if self.inventory_open {
            self.inventory_open = false;
            return true;
        }
        if self.craft_open {
            self.craft_open = false;
            return true;
        }

        if self.map_open {
            self.map_open = false;
            return true;
        }

        if self.settings_open {
            self.settings_open = false;
            return true;
        }
        if self.help {
            self.help = false;
            return true;
        }
        if self.editing.take().is_some() {
            self.select_all = false;
            return true;
        }
        false
    }
    fn submit(&mut self) -> Action {
        let name = self.nickname.trim();
        let name = if name.is_empty() {
            "Беркут"
        } else {
            name
        }
        .chars()
        .take(18)
        .collect::<String>();
        let result = parse_endpoint(&self.address)
            .map(|(h, p)| Action::Join(name, h, p, self.choice.into()));
        match result {
            Ok(a) => {
                self.editing = None;
                self.error.clear();
                a
            }
            Err(e) => {
                self.error = e;
                Action::None
            }
        }
    }
    fn input_field(&mut self, s: &Surface, field: Field, x: f32, y: f32, w: f32, label: &str) {
        let is_selected = self.editing == Some(field);
        let value = match field {
            Field::Nick => &mut self.nickname,
            Field::Address => &mut self.address,
        };
        s.text(label, x, y - 6., 11., MUTED);
        s.rect(x, y, w, 36., alpha(INK, 0.82));
        s.border(x, y, w, 36., if is_selected { GOLD } else { EDGE });
        let mut display = value.clone();
        while s.measure(&display, 15.) > w - 29. && !display.is_empty() {
            display.remove(0);
        }
        let width = s.measure(&display, 15.);
        if is_selected && self.select_all && !display.is_empty() {
            s.rect(x + 8., y + 6., width + 3., 23., alpha(GOLD, 0.4));
        }
        s.text(&display, x + 9., y + 23., 15., PAPER);
        if is_selected && (get_time() * 2.) as i32 % 2 == 0 {
            s.rect(x + 11. + width, y + 9., 1.4, 18., GOLD);
        }
        if s.click && s.over(x, y, w, 36.) {
            self.editing = Some(field);
            self.select_all = true;
        }
    }
    fn handle_text(&mut self) {
        let Some(field) = self.editing else {
            while get_char_pressed().is_some() {}
            return;
        };
        let value = match field {
            Field::Nick => &mut self.nickname,
            Field::Address => &mut self.address,
        };
        let limit = match field {
            Field::Nick => 18,
            Field::Address => 180,
        };
        let ctrl = is_key_down(KeyCode::LeftControl)
            || is_key_down(KeyCode::RightControl)
            || is_key_down(KeyCode::LeftSuper)
            || is_key_down(KeyCode::RightSuper);
        if ctrl && is_key_pressed(KeyCode::A) {
            self.select_all = true;
        }
        if is_key_pressed(KeyCode::Backspace) || is_key_pressed(KeyCode::Delete) {
            if self.select_all {
                value.clear();
            } else {
                value.pop();
            }
            self.select_all = false;
        }
        let mut incoming = String::new();
        if ctrl && is_key_pressed(KeyCode::V) {
            incoming = macroquad::miniquad::window::clipboard_get().unwrap_or_default();
        }
        while let Some(c) = get_char_pressed() {
            if !ctrl {
                incoming.push(c);
            }
        }
        for ch in incoming.chars().filter(|c| !c.is_control()) {
            if self.select_all {
                value.clear();
                self.select_all = false;
            }
            if value.chars().count() < limit {
                value.push(ch);
            }
        }
    }
    fn menu(&mut self, s: &Surface, status: &str) -> Action {
        if self.menu_font_scale != s.scale {
            // Growing Macroquad's glyph atlas mid-frame invalidates the UVs of
            // earlier draw calls. Warm the title and UI text before drawing.
            s.bold().measure("ЛИНИЯ ФРОНТА", 83.);
            s.measure("FRONTLINE SURGE", 16.);
            let letters = "АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯабвгдеёжзийклмнопрстуфхцчшщъыьэюя0123456789.: / →·";
            for size in [11., 12., 14., 15., 17.] {
                s.measure(letters, size);
                s.bold().measure(letters, size);
            }
            self.menu_font_scale = s.scale;
        }
        s.vignette();
        let x = 52.;
        let y = (s.h - 484.) * 0.46;
        s.logo(x, 30.);
        if s.button(s.w - 240., 29., 114., 34., "Настройки", false) {
            self.settings_open = true;
            self.editing = None;
            return Action::None;
        }
        if s.button(s.w - 116., 29., 66., 34., "Выход", false) {
            return Action::Quit;
        }
        s.line(x, y + 1., x + 25., y + 1., 2., GOLD);
        s.text(
            "СЕТЕВАЯ ИГРА   /   КОМАНДНЫЙ БОЙ",
            x + 36.,
            y + 5.,
            12.,
            GOLD,
        );
        let display = s.bold();
        for (label, base, col) in [("ЛИНИЯ", y + 87., PAPER), ("ФРОНТА", y + 161., GOLD)]
        {
            display.ink(label, x, base, 83., col);
        }
        s.text(
            "Промышленная долина, сектор Сен-Лоран. Две команды",
            x,
            y + 205.,
            15.,
            PAPER,
        );
        s.text(
            "строят базы, снабжают технику и ведут разведку с дронов.",
            x,
            y + 228.,
            15.,
            PAPER,
        );
        self.input_field(s, Field::Nick, x, y + 280., 170., "ПОЗЫВНОЙ");
        self.input_field(s, Field::Address, x + 181., y + 280., 239., "АДРЕС СЕРВЕРА");
        s.text("КОМАНДА", x, y + 336., 11., MUTED);
        for (idx, (key, label, color)) in [
            ("blue", "Синие", BLUE_TEAM),
            ("auto", "Авто", GOLD),
            ("red", "Красные", RED_TEAM),
        ]
        .iter()
        .enumerate()
        {
            let bx = x + idx as f32 * 99.;
            if s.button(bx, y + 343., 92., 30., label, false) {
                self.choice = key;
                self.editing = None;
            }
            if self.choice == *key {
                s.rect(bx, y + 371., 92., 2., *color);
            }
        }
        if is_key_pressed(KeyCode::Tab) {
            self.editing = Some(if self.editing == Some(Field::Nick) {
                Field::Address
            } else {
                Field::Nick
            });
            self.select_all = true;
        }
        self.handle_text();
        if s.button(x, y + 386., 224., 45., "Вступить в бой   →", true)
            || is_key_pressed(KeyCode::Enter)
        {
            return self.submit();
        }
        if s.button(x + 235., y + 386., 154., 45., "Управление", false) {
            self.help = true;
            self.help_page = 0;
            self.editing = None;
        }
        if s.button(x, y + 440., 224., 35., "На локальный сервер", false) {
            self.address = "127.0.0.1:8787".into();
            return self.submit();
        }
        let message = if self.error.is_empty() {
            status
        } else {
            &self.error
        };
        s.wrap(message, x, y + 492., 510., 14., RED_TEAM);
        s.rect(x, s.h - 46., 7., 7., GOLD);
        s.text(
            "СЕН-ЛОРАН   /   ПРОМЫШЛЕННАЯ ДОЛИНА",
            x + 16.,
            s.h - 39.,
            12.,
            MUTED,
        );
        Action::None
    }
    fn controls(&mut self, s: &Surface) -> Action {
        let (x, y) = s.panel(684., 542., "СПРАВКА ОПЕРАТОРА", "Управление");
        for (i, label) in ["Пехота", "Снабжение", "Техника", "Разведка", "Фраги"]
            .iter()
            .enumerate()
        {
            let xx = x + 28. + i as f32 * 127.;
            if s.button(xx, y + 104., 121., 33., label, false) {
                self.help_page = i as u8;
            }
            if self.help_page == i as u8 {
                s.rect(xx, y + 135., 121., 2., GOLD);
            }
        }
        if self.help_page == 4 {
            field_hud::legend(s, x + 30., y + 185.);
            if s.button(x + 27., y + 493., 141., 35., "Назад", true) {
                self.help = false;
            }
            return Action::None;
        }
        let rows: &[(&str, &str)] = match self.help_page {
            1 => &[
                ("E у залежи", "Добыть древесину, металл, нефть или порох"),
                ("K", "Каталог рецептов, стоимость и производство"),
                ("B / Q / ЛКМ", "Открыть чертёж / сменить план / поставить"),
                (
                    "Нефтеперегонка",
                    "8 дерева + 6 металла; E: 4 нефти → 6 топлива за 12 с",
                ),
                ("Мастерская", "Постройка за 6 дерева и 8 металла"),
                (
                    "C у мастерской",
                    "Изготовить боеприпасы за 2 пороха и 1 металл",
                ),
                ("E у стены", "Отремонтировать своё укрытие"),
            ],
            2 => &[
                ("F", "Занять место водителя / выйти из машины"),
                ("W S / A D", "Газ и тормоз / поворот"),
                (
                    "ЛКМ / ПКМ",
                    "Танк: пушка / пулемёт. БТР: очередь ОФ / пулемёт",
                ),
                ("G у техники", "Заправить или отремонтировать машину"),
                (
                    "V  /  U",
                    "Выбрать комплект / установить у своей мастерской",
                ),
                (
                    "T у грузовика",
                    "Выбери материал и количество для погрузки или выгрузки",
                ),
                (
                    "Y / колесо / Z / N",
                    "Вид / дистанция и зум / зум 1–2–4× / канал оптики",
                ),
            ],
            3 => &[
                (
                    "J / Shift+J",
                    "Запуск, зависание или возврат в камеру / полный отзыв",
                ),
                ("W A S D", "Горизонтальный полёт"),
                ("Shift / ПКМ", "Подняться / снизиться"),
                ("Мышь / ЛКМ", "Повернуть камеру / оставить командную метку"),
                (
                    "Z / X",
                    "Зум ×2 / урон бойцам и технике в радиусе 6,8 м",
                ),
                (
                    "Запас хода",
                    "Батареи хватает на 60 секунд. Дальность от оператора 65 м.",
                ),
                (
                    "Повторный запуск",
                    "После отзыва — 18 с. После потери аппарата — 45 с.",
                ),
            ],
            _ => &[
                ("W A S D / Shift", "Движение / бег"),
                (
                    "ЛКМ / ПКМ / R / N",
                    "Огонь / прицел / перезарядка / режим огня",
                ),
                (
                    "1 / 2 / 3   ·   H",
                    "MR-7 / VX-9 / AR-4   ·   бросок гранаты",
                ),
                (
                    "E рядом с объектом",
                    "Добыть ресурс или воспользоваться производством",
                ),
                ("B / Q / ЛКМ", "Чертёж / сменить постройку / разместить"),
                ("M / I / K", "Карта / инвентарь / крафт"),
                ("Tab / F3 / Esc", "Таблица бойцов / FPS / меню"),
            ],
        };
        for (i, (key, desc)) in rows.iter().enumerate() {
            let by = y + 172. + i as f32 * 33.;
            s.text(key, x + 29., by, 14., GOLD);
            s.text(desc, x + 221., by, 14., PAPER);
            s.line(x + 28., by + 9., x + 655., by + 9., 0.7, alpha(EDGE, 0.22));
        }
        s.text("СВОЙ СЕРВЕР", x + 28., y + 426., 11., GOLD);
        s.wrap("Запусти frontline-server отдельно. На этом компьютере подключайся к 127.0.0.1:8787. Для друзей в одной сети нужен адрес компьютера с сервером.",x+28.,y+451.,618.,14.,MUTED);
        if s.button(x + 28., y + 493., 141., 35., "Назад", true) {
            self.help = false;
        }
        Action::None
    }
    fn vehicle_sight(s: &Surface, camera: &Camera3D, v: &Value, p: &Value) {
        let (Some(yaw), Some(pitch)) = (v["turretYaw"].as_f64(), p["pitch"].as_f64()) else {
            return;
        };
        let matrix = camera.matrix();
        let viewport = vec2(s.w, s.h);
        let yaw = number(p, "gunYaw", yaw) as f32;
        let pitch = number(p, "gunPitch", pitch) as f32;
        let primary = v["kind"] == "mbt" || number(v, "shellCapacity", 0.) > 0.;
        let cannon = primary && !is_mouse_button_down(MouseButton::Right);
        if p["onboardOptic"] == true {
            armor_optics::draw(s, camera, p, v);
        }
        let mg = mg_position(matrix, p, yaw, pitch, viewport);
        if cannon && !p["mgSight"].is_null() {
            // The secondary marker stays visible while lining up the first MG shot.
            if let Some(at) = mg {
                for (stroke, ink) in [(3.4, INK), (1.4, PAPER)] {
                    draw_circle_lines(at.x * s.scale, at.y * s.scale, 5. * s.scale, stroke * s.scale, ink);
                }
            }
        }
        let at = if cannon {
            sight_position(matrix, crate::dir(yaw, pitch).extend(0.), viewport)
        } else {
            mg
        };
        let Some(at) = at else { return; };
        let color = if number(p, "hitConfirm", 0.) > 0. {
            GOLD
        } else {
            PAPER
        };
        for (stroke, ink) in [(3.6, INK), (1.6, color)] {
            for sign in [-1., 1.] {
                s.line(at.x + sign * 10., at.y, at.x + sign * 24., at.y, stroke, ink);
                if cannon {
                    s.line(at.x, at.y, at.x + sign * 6., at.y + 7., stroke, ink);
                } else {
                    s.line(at.x, at.y + sign * 5., at.x, at.y + sign * 12., stroke, ink);
                }
            }
            if !cannon {
                draw_circle_lines(
                    at.x * s.scale, at.y * s.scale, 8. * s.scale, stroke * s.scale, ink,
                );
            }
        }
    }
    fn hud(&mut self, s: &Surface, state: &Value, p: &Value, camera: &Camera3D) {
        if self.scoreboard {
            field_hud::scoreboard(s, state, p);
            return;
        }
        field_hud::shade(s);
        if let Some(d) = own_drone(state, p) {
            field_hud::drone(s, state, p, d, self.drone_zoom, camera);
            field_hud::feed(s, state, 91.);
            return;
        }
        let alive = flag(p, "alive", true);
        let building = flag(p, "buildMode", false);
        let local_team = string(p, "team", "blue");
        let vehicles = &state["vehicles"];
        let vehicle = vehicles.as_array().into_iter().flatten().find(|v| {
            !p["vehicleId"].is_null() && v["id"] == p["vehicleId"] && !flag(v, "destroyed", false)
        });
        let parked = nearest(vehicles, p, 6.0, |v| {
            !flag(v, "destroyed", false)
                && (string(v, "team", "") == local_team
                    || string(v, "team", "") == "neutral"
                    || string(v, "team", "").is_empty())
        });
        let factory = nearest(&state["factories"], p, 6.0, |v| {
            string(v, "team", "") == local_team
        });
        let cx = s.w / 2.;
        let cy = s.h / 2.;
        let hurt = number(p, "hitFlash", 0.) as f32;
        if hurt > 0. {
            let band = s.h * 0.009;
            for i in 0..10 {
                let v = i as f32;
                let c = alpha(RED_TEAM, 0.45 * hurt.min(1.) * (1. - v / 10.).powi(2));
                let inset = band * v;
                s.rect(inset, inset, s.w - 2. * inset, band + 0.5, c);
                s.rect(inset, s.h - inset - band, s.w - 2. * inset, band + 0.5, c);
                s.rect(
                    inset,
                    inset + band,
                    band + 0.5,
                    s.h - 2. * inset - 2. * band,
                    c,
                );
                s.rect(
                    s.w - inset - band,
                    inset + band,
                    band + 0.5,
                    s.h - 2. * inset - 2. * band,
                    c,
                );
            }
        }
        field_hud::match_score(s, state, p);
        field_hud::compass(s, crate::motion::view_angles(camera.target - camera.position).0);
        let armed = vehicle.filter(|v| matches!(string(v, "kind", ""), "mbt" | "apc"));
        if alive && !building {
            if let Some(v) = armed {
                Self::vehicle_sight(s, camera, v, p);
            }
        }
        if alive && !building && vehicle.is_none() {
            if number(p, "hitConfirm", 0.) > 0. {
                for (dx, dy) in [(-1., -1.), (1., -1.), (-1., 1.), (1., 1.)] {
                    s.line(
                        cx + dx * 5.,
                        cy + dy * 5.,
                        cx + dx * 9.,
                        cy + dy * 9.,
                        2.,
                        GOLD,
                    );
                }
            } else if number(p, "reload", 0.) <= 0. && number(p, "aim", 0.) < 0.75 {
                let spread = 5.
                    + number(p, "recoil", 0.) as f32 * 10.
                    + if flag(p, "moving", false) { 2. } else { 0. };
                for (dx, dy) in [(-1., 0.), (1., 0.), (0., -1.), (0., 1.)] {
                    s.line(
                        cx + dx * spread,
                        cy + dy * spread,
                        cx + dx * (spread + 4.),
                        cy + dy * (spread + 4.),
                        2.6,
                        alpha(INK, 0.7),
                    );
                    s.line(
                        cx + dx * spread,
                        cy + dy * spread,
                        cx + dx * (spread + 4.),
                        cy + dy * (spread + 4.),
                        1.3,
                        PAPER,
                    );
                }
            }
        }
        let gathering = building || factory.is_some() || string(p, "hint", "").contains("[E]");
        field_hud::soldier(s, p, vehicle, gathering);
        if let Some(v) = vehicle { field_hud::vehicle(s, v); } else { field_hud::kit(s, p); }
        let mut sy = if vehicle.is_some() { s.h - 111. } else { s.h - 130. };
        let station = if building {
            None
        } else {
            self.available_station(state, p)
        };
        let hint = if building {
            ""
        } else {
            match station {
                Some("workshop") => "[E] Открыть мастерскую",
                Some("refinery") => "[E] Открыть нефтеперегонку",
                _ => string(p, "hint", ""),
            }
        };
        let seated_hint;
        let hint = if vehicle.is_some() && hint.starts_with("[F] Выйти") {
            // The vehicle card already lists these persistent controls.
            seated_hint = hint
                .split('·')
                .map(str::trim)
                .filter(|part| {
                    !matches!(*part, "[F] Выйти" | "[T] Груз")
                        && !part.starts_with("[ЛКМ]")
                        && !part.starts_with("[ПКМ]")
                })
                .collect::<Vec<_>>()
                .join("   ·   ");
            seated_hint.as_str()
        } else {
            hint
        };
        let msg = if number(p, "messageT", 0.) > 0. {
            string(p, "message", "")
        } else {
            ""
        };
        let build_text = if building {
            let k = string(p, "buildKind", "wall");
            format!(
                "{}  ·  {}  |  Q  Выбор    {}    B  Отмена",
                plan_name(k),
                plan_cost(k),
                if flag(&p["preview"], "valid", false) {
                    if p["preview"]["level"] == 2 {
                        "ЛКМ  Второй ярус"
                    } else {
                        "ЛКМ  Поставить"
                    }
                } else {
                    "Нет места или ресурсов"
                }
            )
        } else {
            String::new()
        };
        let build = build_text.as_str();
        let mut context = String::new();
        if alive && !building {
            if let Some(f) = factory {
                if string(f, "kind", "") == "workshop" {
                    if vehicle.is_some() || parked.is_some() {
                        let selected = string(p, "upgradeChoice", "mobility");
                        context = format!(
                            "Мастерская   ·   V  Комплект: {}    U  Установить",
                            kit_name(selected)
                        );
                    } else if hint.is_empty() {
                        context = "Мастерская   ·   C  Боеприпасы".into();
                    }
                } else if hint.is_empty() {
                    let time = number(f, "remaining", 0.);
                    context = if time > 0. {
                        format!("Нефтеперегонка   ·   Готово через {}", clock(time))
                    } else if number(f, "ready", 0.) > 0. {
                        "Можно забрать готовое топливо".into()
                    } else {
                        "Нефтеперегонка готова к запуску".into()
                    };
                }
            } else if let Some(v) = parked {
                if vehicle.is_none() {
                    if hint.is_empty() {
                        context = format!(
                            "{}   ·   F  Сесть   ·   G  Обслужить",
                            vehicle_name(string(v, "kind", ""))
                        );
                    } else if hint.contains("[F]") {
                        context = vehicle_name(string(v, "kind", "")).into();
                    }
                }
            }
        }
        if alive {
            for (tx, col) in [
                (build, GOLD),
                (hint, PAPER),
                (context.as_str(), MUTED),
                (msg, MUTED),
            ] {
                if tx.is_empty() {
                    continue;
                }
                field_hud::prompt(s, tx, sy, col, vehicle.is_some());
                sy -= 26.;
            }
        } else {
            s.ink_mid("Боец выбыл", cx, cy - 12., 29., PAPER);
            s.ink_mid(
                &format!("До возрождения   {} с", count(p, "respawnIn", 0).max(0)),
                cx,
                cy + 18.,
                18.,
                GOLD,
            );
        }
        field_hud::feed(s, state, 45.);
    }
    pub fn draw(
        &mut self,
        mode: &str,
        state: &Value,
        local: &Value,
        font: &Font,
        status: &str,
        endpoint: &str,
        camera: &Camera3D,
    ) -> Action {
        let action = self.draw_scene(mode, state, local, font, status, endpoint, camera);
        if is_key_pressed(KeyCode::F3) {
            self.set_fps_visible(!self.fps_visible());
        }
        self.fps_elapsed += get_frame_time();
        self.fps_frames += 1;
        if self.fps_elapsed >= 0.45 {
            self.fps = self.fps_frames as f32 / self.fps_elapsed;
            self.fps_frames = 0;
            self.fps_elapsed = 0.;
        }
        if self.fps_visible() {
            let s = Surface::new(font);
            let x = s.w - 88.;
            let y = if mode == "menu" { s.h - 47. } else { 4. };
            s.ink_right(
                &format!(
                    "{} FPS",
                    if self.fps > 0. {
                        self.fps.round() as i32
                    } else {
                        get_fps()
                    }
                ),
                x + 58.,
                y + 12.,
                11.,
                MUTED,
            );
        }
        action
    }
    fn draw_scene(
        &mut self,
        mode: &str,
        state: &Value,
        local: &Value,
        font: &Font,
        status: &str,
        endpoint: &str,
        camera: &Camera3D,
    ) -> Action {
        set_default_camera();
        let title = self.field_font.clone();
        let mut s = if mode == "playing" && self.inventory_open {
            Surface::field(font)
        } else {
            Surface::new(font)
        };
        s.title_font = Some(&title);
        if self.settings_open {
            if mode == "menu" || mode == "pause" {
                return self.preferences(&s);
            }
            self.settings_open = false;
        }
        if self.help && (mode == "menu" || mode == "pause") {
            return self.controls(&s);
        }
        if mode != "playing" {
            self.inventory_open = false;
            self.map_open = false;
            self.craft_open = false;
        }
        if mode != "menu" {
            self.editing = None;
            self.error.clear();
        }
        match mode {
            "menu" => self.menu(&s, status),
            "connecting" => {
                let (x, y) = s.panel(470., 238., "СЕТЕВАЯ ИГРА", "Подключение");
                s.wrap(
                    if status.is_empty() {
                        "Ждём ответа сервера…"
                    } else {
                        status
                    },
                    x + 27.,
                    y + 127.,
                    410.,
                    15.,
                    MUTED,
                );
                if s.button(x + 27., y + 174., 145., 39., "Отмена", false) {
                    Action::Leave
                } else {
                    Action::None
                }
            }
            "playing" => {
                if self.inventory_open {
                    let (action, close) = self.inventory.draw(&s, state, local);
                    if close {
                        self.inventory_open = false;
                    }
                    return action;
                }
                if self.craft_open {
                    let (action, close) = self.craft.draw(&s, state, local);
                    if close {
                        self.craft_open = false;
                    }
                    return action;
                } else if self.map_open {
                    if self.map.draw(&s, state, local) {
                        self.map_open = false;
                    }
                } else {
                    let mut hud = Surface::field(font);
                    hud.title_font = Some(&title);
                    self.hud(&hud, state, local, camera);
                }
                Action::None
            }
            "pause" => {
                let (x, y) = s.panel(490., 352., "СЕТЕВОЙ МАТЧ", "Меню боя");
                s.text("Матч продолжается.", x + 27., y + 128., 15., MUTED);
                if s.button(x + 27., y + 149., 235., 43., "Вернуться в бой   →", true)
                {
                    return Action::Resume;
                }
                if s.button(x + 272., y + 149., 190., 43., "Управление", false) {
                    self.help = true;
                }
                if s.button(x + 27., y + 202., 235., 39., "Настройки", false) {
                    self.settings_open = true;
                }
                if s.button(x + 272., y + 202., 190., 39., "Покинуть сервер", false) {
                    return Action::Leave;
                }
                s.text("АДРЕС ДЛЯ ИГРОКОВ", x + 27., y + 277., 11., GOLD);
                let address = if endpoint.is_empty() {
                    "Адрес ещё не получен"
                } else {
                    endpoint
                };
                s.text(
                    &s.shorten(address, 15., 285.),
                    x + 27.,
                    y + 307.,
                    15.,
                    PAPER,
                );
                if !endpoint.is_empty()
                    && s.button(
                        x + 333.,
                        y + 282.,
                        129.,
                        37.,
                        if get_time() - self.copied < 2. {
                            "Скопировано"
                        } else {
                            "Копировать"
                        },
                        false,
                    )
                {
                    macroquad::miniquad::window::clipboard_set(endpoint);
                    self.copied = get_time();
                }
                Action::None
            }
            "ended" => {
                let title = match string(state, "winner", "draw") {
                    "red" => "Победа красных",
                    "blue" => "Победа синих",
                    _ => "Ничья",
                };
                let (x, y) = s.panel(500., 300., "РАУНД ЗАВЕРШЁН", title);
                s.text(
                    &format!(
                        "Синие   {} : {}   Красные",
                        count(&state["scores"], "blue", 0),
                        count(&state["scores"], "red", 0)
                    ),
                    x + 27.,
                    y + 149.,
                    22.,
                    PAPER,
                );
                s.text(
                    &format!(
                        "Следующий раунд через {} с",
                        count(state, "nextRoundIn", 0).max(0)
                    ),
                    x + 27.,
                    y + 187.,
                    15.,
                    MUTED,
                );
                if s.button(x + 27., y + 226., 206., 43., "Покинуть сервер", false) {
                    Action::Leave
                } else {
                    Action::None
                }
            }
            _ => Action::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn turret_marker_tracks_view_rotation_and_rejects_offscreen_directions() {
        let viewport = vec2(1280., 720.);
        let perspective = Mat4::perspective_rh_gl(1.2, 16. / 9., 0.1, 1000.);
        let eye = vec3(50., 3., -12.);
        let yaw = 0.9;
        let pitch = 0.2;
        let view = Mat4::look_at_rh(eye, eye + crate::dir(yaw, pitch), Vec3::Y);
        let matrix = perspective * view;
        let marker = |y, p| sight_position(matrix, crate::dir(y, p).extend(0.), viewport);
        assert!(marker(yaw, pitch).unwrap().distance(viewport * 0.5) < 0.01);
        assert!(marker(yaw + 0.15, pitch).unwrap().x > viewport.x * 0.5);
        assert!(marker(yaw, pitch + 0.1).unwrap().y < viewport.y * 0.5);
        assert!(marker(yaw + 1.4, 0.).is_none());
        assert!(marker(yaw + std::f32::consts::PI, 0.).is_none());
    }

    #[test]
    fn mounted_mg_marks_the_visible_point_from_offset_and_zoomed_views() {
        let viewport = vec2(1280., 720.);
        let muzzle = vec3(0., 2.08, 20.);
        // The near/front surface of the same upright hitbox traced by the server.
        for range in [7., 28., 74.] {
            let pitch = (1.2_f32 - 2.08).atan2(range);
            let direction = crate::dir(0., pitch);
            let hit = muzzle + direction * ((range - 0.39) / pitch.cos());
            let player = serde_json::json!({"mgSight":[hit.x,hit.y,hit.z]});
            for (eye, fov) in [(vec3(0., 4.9, 27.6), 75_f32), (vec3(0.6, 2.5, 20.), 38.)] {
                let view = Mat4::look_at_rh(eye, eye - Vec3::Z, Vec3::Y);
                let matrix = Mat4::perspective_rh_gl(fov.to_radians(), 16. / 9., 0.035, 820.) * view;
                // In camera space the front of the hitbox has this angular offset.
                let depth = eye.z - hit.z;
                let focal_px = viewport.y * 0.5 / (fov.to_radians() * 0.5).tan();
                let expected = viewport * 0.5 + vec2(hit.x - eye.x, eye.y - hit.y) * (focal_px / depth);
                let actual = mg_position(matrix, &player, 0.8, 0., viewport).unwrap();
                assert!(actual.distance(expected) < 0.01);
                let infinite = mg_position(matrix, &Value::Null, 0., pitch, viewport).unwrap();
                assert!(infinite.distance(expected) > 1.);
            }
        }
    }
    #[test]
    fn kill_feed_names_all_fatal_sources_and_accepts_older_events() {
        for (weapon, kind, expected) in [
            ("mr7", "", "MR-7"), ("ar4", "", "AR-4"), ("vx9", "", "VX-9"),
            ("vehicle_mg", "mbt", "Пулемёт танка"), ("vehicle_mg", "apc", "Пулемёт БТР"),
            ("vehicle_mg", "", "Бортовой пулемёт"), ("tank", "", "Пушка танка"),
            ("grenade", "", "Граната"), ("drone_pulse", "", "Импульс дрона"),
            ("ram", "mbt", "Наезд танка"), ("ram", "scout_buggy", "Наезд багги"),
            ("ram", "", "Наезд"), ("", "", "›"),
        ] {
            assert_eq!(kill_source(&serde_json::json!({"weapon":weapon, "vehicleKind":kind})), expected);
        }
    }
    #[test]
    fn invitations_parse_ipv4_dns_ipv6_and_custom_ports() {
        for (input, host, port) in [
            (" game.local ", "game.local", 8787),
            ("192.168.1.20:9001", "192.168.1.20", 9001),
            ("[::1]:9090", "::1", 9090),
            ("::1", "::1", 8787),
        ] {
            assert_eq!(parse_endpoint(input).unwrap(), (host.to_string(), port));
        }
        for bad in ["http://bad", ":123", "host:0", "host:90000", "[::1]:", ""] {
            assert!(parse_endpoint(bad).is_err());
        }
    }
}
