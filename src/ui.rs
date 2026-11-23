//! Native immediate-mode menus, scoreboard and field HUD.
use macroquad::prelude::*;
use crate::settings::Settings;
#[path = "settings_ui.rs"]
mod settings_ui;
use serde_json::Value;
#[path = "tactical_map.rs"]
mod tactical_map;
use tactical_map::TacticalMap;
#[path = "crafting.rs"]
mod crafting;
use crafting::CraftMenu;
pub(crate) use crafting::friendly_vehicle;
#[path = "stash_ui.rs"]
mod stash_ui;
use stash_ui::Stash;

const PAPER: Color = Color::new(0.93, 0.96, 0.94, 1.0);
const GOLD: Color = Color::new(0.98, 0.72, 0.31, 1.0);
const MUTED: Color = Color::new(0.70, 0.78, 0.76, 1.0);
const BLUE_TEAM: Color = Color::new(0.57, 0.82, 0.85, 1.0);
const RED_TEAM: Color = Color::new(0.93, 0.63, 0.51, 1.0);
const INK: Color = Color::new(0.064, 0.094, 0.102, 1.0);
const EDGE: Color = Color::new(0.75, 0.89, 0.85, 0.36);
const PLATE: Color = Color::new(0.04, 0.082, 0.09, 0.83);

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

struct Surface<'a> {
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
    fn inventory(font: &'a Font) -> Self {
        let w = screen_width();
        let h = screen_height();
        // Give the three inventory columns more room on a small desktop window.
        let standard = (w / 1140.).min(h / 680.).clamp(0.9, 3.);
        Self::scaled(font, standard.min(w / 1020.).min(h / 572.).max(0.01))
    }
    fn scaled(font: &'a Font, scale: f32) -> Self {
        let (mx, my) = mouse_position();
        Self {
            font,
            scale,
            w: screen_width() / scale,
            h: screen_height() / scale,
            pointer: vec2(mx / scale, my / scale),
            click: is_mouse_button_pressed(MouseButton::Left),
        }
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
    fn button(&self, x: f32, y: f32, w: f32, h: f32, label: &str, primary: bool) -> bool {
        let hover = self.over(x, y, w, h);
        let bg = if primary {
            if hover {
                Color::new(1., 0.81, 0.48, 1.)
            } else {
                GOLD
            }
        } else if hover {
            Color::new(0.20, 0.24, 0.19, 0.96)
        } else {
            PLATE
        };
        self.rect(x, y, w, h, bg);
        if !primary {
            self.border(x, y, w, h, if hover { GOLD } else { EDGE });
        }
        self.mid(
            label,
            x + w * 0.5,
            y + h * 0.5 + 5.,
            15.,
            if primary { INK } else { PAPER },
        );
        hover && self.click
    }
    fn disabled(&self, x: f32, y: f32, w: f32, h: f32, label: &str) {
        self.rect(x, y, w, h, alpha(INK, 0.7));
        self.border(x, y, w, h, EDGE);
        self.mid(label, x + w * 0.5, y + h * 0.5 + 5., 14., MUTED);
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
        self.rect(0., 0., self.w, self.h, alpha(INK, 0.69));
        let x = (self.w - w) * 0.5;
        let y = (self.h - h) * 0.5;
        self.rect(x, y, w, h, Color::new(0.095, 0.14, 0.15, 0.98));
        self.border(x, y, w, h, EDGE);
        self.rect(x, y, w, 3., GOLD);
        self.text(kicker, x + 27., y + 37., 12., GOLD);
        self.text(title, x + 26., y + 89., 39., PAPER);
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
            s.measure("ЛИНИЯ ФРОНТА", 83.);
            s.measure("FRONTLINE SURGE", 16.);
            let letters = "АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯабвгдеёжзийклмнопрстуфхцчшщъыьэюя0123456789.: / →·";
            for size in [11., 12., 14., 15., 17.] {
                s.measure(letters, size);
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
        // Deliberately offset the second strike for a heavy screen-printed title.
        for (label, base, col) in [("ЛИНИЯ", y + 87., PAPER), ("ФРОНТА", y + 161., GOLD)]
        {
            s.text(label, x - 2., base, 83., col);
            s.text(label, x, base, 83., col);
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
        for (i, label) in ["Пехота", "Снабжение", "Техника", "Разведка"]
            .iter()
            .enumerate()
        {
            let xx = x + 28. + i as f32 * 150.;
            if s.button(xx, y + 104., 140., 33., label, false) {
                self.help_page = i as u8;
            }
            if self.help_page == i as u8 {
                s.rect(xx, y + 135., 140., 2., GOLD);
            }
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
                ("ЛКМ / ПКМ в танке", "Пушка / пулемёт"),
                ("G у техники", "Заправить или отремонтировать машину"),
                (
                    "V  /  U",
                    "Выбрать комплект / установить у своей мастерской",
                ),
                (
                    "T у грузовика",
                    "Выбери материал и количество для погрузки или выгрузки",
                ),
                ("Y / колесо", "Сменить вид / приблизить или отдалить камеру"),
            ],
            3 => &[
                ("J", "Выпустить или отозвать личный разведдрон"),
                ("W A S D", "Горизонтальный полёт"),
                ("Shift / ПКМ", "Подняться / снизиться"),
                ("Мышь / ЛКМ", "Повернуть камеру / оставить командную метку"),
                ("Z / X", "Включить цифровой зум ×2 / дать импульс"),
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
    fn compass(s: &Surface, yaw: f32) {
        let heading = yaw.to_degrees();
        let center = s.w * 0.5;
        for step in ((heading - 50.) / 5.).floor() as i32..=((heading + 50.) / 5.).ceil() as i32 {
            let deg = step * 5;
            let a = deg.rem_euclid(360);
            let x = center + (deg as f32 - heading) * 3.1;
            let major = a % 45 == 0;
            s.line(
                x,
                32.,
                x,
                if major { 40. } else { 35. },
                1.2,
                alpha(PAPER, 0.85),
            );
            if major || a % 15 == 0 {
                let text = if major {
                    ["С", "СВ", "В", "ЮВ", "Ю", "ЮЗ", "З", "СЗ"][(a / 45) as usize].to_owned()
                } else {
                    a.to_string()
                };
                s.mid(&text, x + 1., 26., 12., INK);
                s.mid(&text, x, 25., 12., PAPER);
            }
        }
        s.line(center - 3., 45., center, 40., 1.8, GOLD);
        s.line(center, 40., center + 3., 45., 1.8, GOLD);
    }
    fn drone_hud(s: &Surface, state: &Value, p: &Value, d: &Value, zoom: bool, camera: &Camera3D) {
        let a = Color::new(0.41, 0.91, 0.81, 1.);
        let cx = s.w * 0.5;
        let cy = s.h * 0.5;
        let (yaw, pitch) = crate::motion::view_angles(camera.target - camera.position);
        Self::compass(s, yaw);
        for (x, y, dx, dy) in [
            (24., 23., 1., 1.),
            (s.w - 24., 23., -1., 1.),
            (24., s.h - 23., 1., -1.),
            (s.w - 24., s.h - 23., -1., -1.),
        ] {
            s.line(x, y, x + dx * 31., y, 1.4, a);
            s.line(x, y, x, y + dy * 31., 1.4, a);
        }
        for (dx, dy) in [(-1., -1.), (1., -1.), (-1., 1.), (1., 1.)] {
            let x = cx + dx * 16.;
            let y = cy + dy * 12.;
            s.line(x, y, x - dx * 6., y, 1.3, a);
            s.line(x, y, x, y - dy * 6., 1.3, a);
        }
        s.rect(cx - 1., cy - 1., 2., 2., a);
        s.rect(36., 33., 201., 67., PLATE);
        s.rect(47., 47., 5., 5., a);
        s.text("КАНАЛ ДРОНА", 61., 54., 13., a);
        s.text(
            if zoom { "FPV   ×2" } else { "FPV   ×1" },
            47.,
            83.,
            23.,
            PAPER,
        );
        let signal = (number(d, "signal", 1.).clamp(0., 1.) * 100.).round() as i32;
        s.rect(s.w - 230., 33., 194., 67., PLATE);
        s.text("ВИДЕОСВЯЗЬ", s.w - 217., 55., 11., a);
        s.text(&format!("{}%", signal), s.w - 217., 83., 20., PAPER);
        for i in 0..5 {
            let h = 5. + i as f32 * 3.;
            s.rect(
                s.w - 125. + i as f32 * 11.,
                83. - h,
                7.,
                h,
                if i * 20 < signal { a } else { EDGE },
            );
        }
        let forward = (camera.target - camera.position).normalize_or_zero();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let roll = camera.up.dot(right).atan2(camera.up.dot(right.cross(forward)));
        let horizon = cy + pitch * 54.;
        let (sin, cos) = (-roll).sin_cos();
        for sign in [-1., 1.] {
            for offset in [35., 55.] {
                let far = offset + if offset == 35. { 13. } else { 8. };
                s.line(
                    cx + sign * offset * cos,
                    horizon + sign * offset * sin,
                    cx + sign * far * cos,
                    horizon + sign * far * sin,
                    1.3,
                    alpha(a, 0.85),
                );
            }
        }
        let speed = number(d, "speed", 0.).max(0.);
        let climb = number(d, "vSpeed", 0.);
        let tx = 36.;
        let ty = s.h * 0.5 - 29.;
        s.rect(tx, ty, 120., 52., PLATE);
        s.text("СКОРОСТЬ / ВЕРТ.", tx + 8., ty + 16., 10., MUTED);
        s.text(
            &format!("{:.0}  /  {:+.0} м/с", speed, climb),
            tx + 8.,
            ty + 39.,
            15.,
            a,
        );
        let battery = number(d, "battery", 0.).clamp(0., 60.);
        let altitude = number(d, "y", 0.).max(0.);
        let range = (number(d, "x", 0.) - number(p, "x", 0.))
            .hypot(number(d, "z", 0.) - number(p, "z", 0.));
        let y = s.h - 120.;
        s.rect(36., y, 274., 83., PLATE);
        s.text("БАТАРЕЯ", 48., y + 19., 10., MUTED);
        s.right(
            &clock(battery),
            s.w.min(296.),
            y + 21.,
            16.,
            if battery < 12. { RED_TEAM } else { a },
        );
        s.rect(48., y + 29., 248., 3., EDGE);
        s.rect(48., y + 29., 248. * battery as f32 / 60., 3., a);
        s.text(
            &format!("Высота   {:.0} м", altitude),
            48.,
            y + 55.,
            14.,
            PAPER,
        );
        s.right(
            &format!("Удаление   {:.0} м", range),
            296.,
            y + 55.,
            14.,
            PAPER,
        );
        s.text("Shift   вверх     ПКМ   вниз", 48., y + 75., 12., a);
        let newest = state["markers"].as_array().into_iter().flatten().find(|m| {
            m["team"] == p["team"] && m["owner"] == p["id"] && number(m, "remaining", 0.) > 0.
        });
        let x = s.w - 310.;
        s.rect(x, y, 274., 83., PLATE);
        s.text("КОМАНДНАЯ МЕТКА", x + 12., y + 19., 10., MUTED);
        s.text(
            &newest.map_or_else(
                || "ЛКМ  Отметить точку".to_owned(),
                |m| format!("Отмечено   ·   {} с", count(m, "remaining", 0)),
            ),
            x + 12.,
            y + 46.,
            16.,
            PAPER,
        );
        s.text(
            "Z  Зум   ·   M  Карта   ·   J  Отозвать",
            x + 12.,
            y + 72.,
            12.,
            a,
        );
        let charges = count(d, "charges", 0);
        let pulse = if charges == 0 {
            "Импульс израсходован"
        } else if battery < 14. {
            "Для импульса мало энергии"
        } else {
            "X  Импульс  ·  14 с батареи"
        };
        s.rect(x, y - 32., 274., 26., PLATE);
        s.text(
            pulse,
            x + 12.,
            y - 14.,
            13.,
            if charges > 0 && battery >= 14. {
                a
            } else {
                MUTED
            },
        );
    }
    fn vehicle_sight(s: &Surface, camera: &Camera3D, v: &Value, p: &Value) {
        let (Some(yaw), Some(pitch)) = (v["turretYaw"].as_f64(), p["pitch"].as_f64()) else {
            return;
        };
        let matrix = camera.matrix();
        let viewport = vec2(s.w, s.h);
        let yaw = yaw as f32;
        let pitch = pitch as f32;
        let cannon = v["kind"] == "mbt" && !is_mouse_button_down(MouseButton::Right);
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
            sight_position(matrix, crate::dir(yaw, pitch.clamp(-0.6, 0.6)).extend(0.), viewport)
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
        if let Some(d) = own_drone(state, p) {
            Self::drone_hud(s, state, p, d, self.drone_zoom, camera);
            if self.scoreboard {
                Self::board(s, state, p);
            }
            return;
        }
        let players = &state["players"];
        let score = &state["scores"];
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
        let team_color = if local_team == "blue" {
            BLUE_TEAM
        } else {
            RED_TEAM
        };
        let team = if local_team == "blue" {
            "Синие"
        } else {
            "Красные"
        };
        let online = players.as_array().map_or(0, Vec::len);
        let cap = count(state, "fragLimit", 0);
        let detail = if online < 2 {
            "Ждём соперника".into()
        } else if cap > 0 {
            format!("До {cap} очков")
        } else {
            format!("В сети {online}")
        };
        s.rect(22., 22., 210., 62., PLATE);
        s.rect(22., 22., 3., 62., team_color);
        s.text(team, 35., 42., 13., team_color);
        s.right(&detail, 220., 42., 12., MUTED);
        let (other_team, other_color) = if local_team == "blue" {
            ("red", RED_TEAM)
        } else {
            ("blue", BLUE_TEAM)
        };
        s.text(
            &count(score, local_team, 0).to_string(),
            35.,
            73.,
            26.,
            team_color,
        );
        s.right(
            &count(score, other_team, 0).to_string(),
            220.,
            73.,
            26.,
            other_color,
        );
        s.mid(
            &clock(number(state, "remaining", 420.)),
            127.,
            70.,
            20.,
            PAPER,
        );
        Self::compass(s, crate::motion::view_angles(camera.target - camera.position).0);
        let armed = vehicle.filter(|v| matches!(string(v, "kind", ""), "mbt" | "apc"));
        let vehicle_gun = armed.is_some();
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
        let edge = 22.;
        let bottom = s.h - edge;
        let hp = count(p, "hp", 100).max(0);
        let health_color = if hp <= 30 { RED_TEAM } else { team_color };
        let stamina = number(p, "stamina", 100.) as f32;
        let health_y = bottom - 83.;
        s.rect(edge, health_y, 206., 83., PLATE);
        s.rect(edge, health_y, 3., 83., health_color);
        s.text("ЗДОРОВЬЕ", 34., health_y + 22., 12., MUTED);
        s.right(&hp.to_string(), 214., health_y + 29., 27., health_color);
        s.rect(34., health_y + 35., 180., 5., EDGE);
        s.rect(
            34.,
            health_y + 35.,
            180. * (hp as f32 / 100.).clamp(0., 1.),
            5.,
            health_color,
        );
        if let Some(v) = vehicle {
            s.text("Курс", 34., bottom - 24., 13., MUTED);
            s.text(&heading_label(v, "yaw"), 76., bottom - 24., 16., PAPER);
            if vehicle_gun {
                s.text("Башня", 122., bottom - 24., 13., MUTED);
                s.right(
                    &heading_label(v, "turretYaw"),
                    214.,
                    bottom - 24.,
                    16.,
                    PAPER,
                );
            }
            s.text("I  Инвентарь", 34., bottom - 8., 11., MUTED);
        } else {
            s.text("Выносливость", 34., health_y + 58., 13., MUTED);
            s.rect(138., health_y + 52., 76., 3., EDGE);
            s.rect(
                138.,
                health_y + 52.,
                76. * (stamina / 100.).clamp(0., 1.),
                3.,
                GOLD,
            );
            s.text(
                "I Рюкзак   K Крафт   B Стройка",
                34.,
                bottom - 7.,
                11.,
                MUTED,
            );
            let gathering = building || factory.is_some() || string(p, "hint", "").contains("[E]");
            if gathering {
                let by = health_y - 57.;
                s.rect(edge, by, 303., 52., PLATE);
                for (i, (key, name)) in MATERIALS.iter().enumerate() {
                    let xx = edge + 10. + i as f32 * 57.;
                    s.text(name, xx, by + 17., 12., MUTED);
                    s.text(&count(p, key, 0).to_string(), xx, by + 41., 20., PAPER);
                }
            }
        }
        if let Some(v) = vehicle {
            let capacity = count(v, "cargoCapacity", 0);
            let kind = string(v, "kind", "");
            let kit = kit_name(string(v, "upgrade", ""));
            let level = match count(v, "upgradeLevel", 0) {
                1 => " I",
                2 => " II",
                _ => "",
            };
            let w = 326.;
            let x = s.w - edge - w;
            let y = bottom
                - 122.
                - if capacity > 0 { 25. } else { 0. }
                - if vehicle_gun { 28. } else { 0. };
            s.rect(x, y, w, bottom - y, PLATE);
            s.rect(x, y, 3., bottom - y, BLUE_TEAM);
            s.text(vehicle_name(kind), x + 13., y + 26., 21., PAPER);
            let speed = (number(v, "speed", 0.).abs() * 3.6).round();
            s.right(
                &format!("{speed} км/ч"),
                x + w - 13.,
                y + 25.,
                19.,
                BLUE_TEAM,
            );
            for (i, (label, key, max_key, color)) in [
                ("Корпус", "hp", "maxHp", BLUE_TEAM),
                ("Топливо", "fuel", "maxFuel", GOLD),
            ]
            .iter()
            .enumerate()
            {
                let xx = x + 13. + i as f32 * 154.;
                let val = number(v, key, 0.);
                let max = number(v, max_key, 100.).max(1.);
                let ratio = (val / max).clamp(0., 1.) as f32;
                s.text(label, xx, y + 52., 12., MUTED);
                s.right(
                    &format!("{} / {}", val.ceil() as i32, max.ceil() as i32),
                    xx + 144.,
                    y + 52.,
                    14.,
                    PAPER,
                );
                s.rect(xx, y + 59., 144., 4., EDGE);
                s.rect(
                    xx,
                    y + 59.,
                    144. * ratio,
                    4.,
                    if ratio <= 0.2 { RED_TEAM } else { *color },
                );
            }
            let mut row = y + 84.;
            if vehicle_gun {
                if kind == "mbt" {
                    s.text("ЛКМ  Пушка", x + 13., row, 13., MUTED);
                    s.right(
                        &count(v, "shells", 0).to_string(),
                        x + 146.,
                        row,
                        20.,
                        PAPER,
                    );
                }
                let xx = x + if kind == "mbt" { 167. } else { 13. };
                s.text(
                    if kind == "mbt" {
                        "ПКМ  Пулемёт"
                    } else {
                        "ЛКМ  Пулемёт"
                    },
                    xx,
                    row,
                    13.,
                    MUTED,
                );
                s.right(
                    &count(v, "mgAmmo", 0).to_string(),
                    x + w - 13.,
                    row,
                    20.,
                    PAPER,
                );
                row += 28.;
            }
            if capacity > 0 {
                let cargo = &v["cargo"];
                let total = MATERIALS
                    .iter()
                    .map(|(key, _)| count(cargo, key, 0))
                    .sum::<i32>();
                let mut goods = MATERIALS.iter().filter(|(key, _)| count(cargo, key, 0) > 0);
                let manifest = match goods.next() {
                    Some((key, name)) => match goods.count() {
                        0 => format!("{name} ×{}", count(cargo, key, 0)),
                        other => format!("{name} ×{} · ещё {other}", count(cargo, key, 0)),
                    },
                    None => "Пусто".into(),
                };
                s.text(
                    &format!("T  Кузов   {total} / {capacity}"),
                    x + 13.,
                    row,
                    15.,
                    PAPER,
                );
                s.right(&manifest, x + w - 13., row, 13., MUTED);
                row += 25.;
            }
            s.text(
                &format!("Комплект · {kit}{level}"),
                x + 13.,
                row,
                13.,
                MUTED,
            );
            s.line(x + 13., bottom - 27., x + w - 13., bottom - 27., 1., EDGE);
            s.text(
                "Y  Вид   ·   Колесо  Зум   ·   M  Карта",
                x + 13.,
                bottom - 9.,
                12.,
                MUTED,
            );
            s.right("F  Выйти", x + w - 13., bottom - 9., 12., PAPER);
        } else {
            let x = s.w - edge - 234.;
            let y = bottom - 103.;
            let weapon = match string(p, "weapon", "mr7") {
                "vx9" => "VX-9",
                "ar4" => "AR-4",
                _ => "MR-7",
            };
            let ammo = count(p, "ammo", 0);
            let reserve = count(p, "reserve", 0);
            let reload = number(p, "reload", 0.);
            let ammo_color = if ammo == 0 { RED_TEAM } else { PAPER };
            s.rect(x, y, 234., 103., PLATE);
            s.rect(x, y, 3., 103., GOLD);
            s.text(weapon, x + 13., y + 23., 19., PAPER);
            s.right(
                &format!("N  {}", crafting::mode_name(string(p, "fireMode", "semi"))),
                x + 221.,
                y + 22.,
                12.,
                MUTED,
            );
            s.text(&ammo.to_string(), x + 12., y + 63., 40., ammo_color);
            s.text(
                &format!("/  {reserve}"),
                x + 19. + s.measure(&ammo.to_string(), 40.),
                y + 61.,
                20.,
                MUTED,
            );
            s.right(
                &format!("H  Гранаты ×{}", count(p, "grenades", 0)),
                x + 221.,
                y + 59.,
                13.,
                PAPER,
            );
            if reload > 0. {
                let duration = number(p, "reloadTime", 2.).max(0.01);
                s.rect(x + 13., y + 70., 208., 3., EDGE);
                s.rect(
                    x + 13.,
                    y + 70.,
                    208. * (1. - reload / duration).clamp(0., 1.) as f32,
                    3.,
                    GOLD,
                );
                s.text("Перезарядка", x + 13., bottom - 9., 13., GOLD);
            } else {
                s.text(
                    if ammo == 0 && reserve > 0 {
                        "R  Перезарядить"
                    } else {
                        "1–3  Оружие"
                    },
                    x + 13.,
                    bottom - 9.,
                    12.,
                    if ammo == 0 { GOLD } else { MUTED },
                );
                let cooldown = count(p, "droneCooldown", 0);
                let drone = if cooldown > 0 {
                    format!("M Карта   J {cooldown} с")
                } else {
                    "M Карта   J Дрон".into()
                };
                s.right(&drone, x + 221., bottom - 9., 12., MUTED);
            }
        }
        let mut sy = if vehicle.is_some() { health_y - 15. } else { s.h * 0.655 };
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
            seated_hint = hint.split('·').map(str::trim)
                .filter(|part| !matches!(*part, "[F] Выйти" | "[ЛКМ] Пушка" | "[ПКМ] Пулемёт" | "[ЛКМ] Пулемёт" | "[T] Груз"))
                .collect::<Vec<_>>().join("   ·   ");
            seated_hint.as_str()
        } else { hint };
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
                let seated = vehicle.is_some();
                let max_w = if seated { s.w * 0.46 } else { s.w - 80. };
                let w = (s.measure(tx, 15.) + 22.).min(max_w);
                let x = if seated { edge } else { cx - w / 2. };
                s.rect(x, sy - 18., w, 25., PLATE);
                s.rect(x, sy - 18., 2., 25., col);
                s.text(&s.shorten(tx, 15., w - 18.), x + 11., sy, 15., col);
                sy += if seated { -29. } else { 29. };
            }
        } else {
            s.rect(cx - 183., cy - 52., 366., 104., PLATE);
            s.mid("БОЕЦ ВЫБЫЛ", cx, cy - 14., 12., GOLD);
            s.mid(
                &format!(
                    "Возвращение в бой через {} с",
                    count(p, "respawnIn", 0).max(0)
                ),
                cx,
                cy + 23.,
                20.,
                PAPER,
            );
        }
        if let Some(events) = state["events"].as_array() {
            let mut line = 0;
            let elapsed = number(state, "elapsed", 0.);
            for e in events
                .iter()
                .rev()
                .filter(|e| {
                    string(e, "type", "") == "kill" && elapsed - number(e, "at", elapsed) < 5.5
                })
                .take(3)
            {
                let a = string(e, "actorName", "Боец");
                let b = string(e, "targetName", "Боец");
                let ally = string(e, "team", "blue") == "blue";
                let source = s.shorten(kill_source(e), 12., 142.);
                let sw = s.measure(&source, 12.);
                let max_name = ((s.w * 0.37 - sw - 38.) * 0.5).clamp(43., 148.);
                let a = s.shorten(a, 13., max_name);
                let b = s.shorten(b, 13., max_name);
                let aw = s.measure(&a, 13.);
                let bw = s.measure(&b, 13.);
                let width = aw + sw + bw + 34.;
                let x = s.w - width - 24.;
                let y = 24. + line as f32 * 25.;
                line += 1;
                s.rect(x, y, width, 22., PLATE);
                s.text(
                    &a,
                    x + 7.,
                    y + 16.,
                    13.,
                    if ally { BLUE_TEAM } else { RED_TEAM },
                );
                s.text(&source, x + aw + 16., y + 16., 12., GOLD);
                s.text(
                    &b,
                    x + aw + sw + 26.,
                    y + 16.,
                    13.,
                    if ally { RED_TEAM } else { BLUE_TEAM },
                );
            }
        }
        if self.scoreboard {
            Self::board(s, state, p);
        }
    }
    fn board(s: &Surface, state: &Value, local: &Value) {
        let (x, y) = s.panel(964., 492., "КОМАНДНЫЙ БОЙ", "Таблица счёта");
        s.text(
            "Когда истечёт время, победит команда с большим числом фрагов.",
            x + 28.,
            y + 116.,
            14.,
            MUTED,
        );
        let players = state["players"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for (i, (team, title, tint)) in [("blue", "Синие", BLUE_TEAM), ("red", "Красные", RED_TEAM)]
            .into_iter()
            .enumerate()
        {
            let xx = x + 28. + i as f32 * 460.;
            s.text(
                &format!("{}   /   {} фр.", title, count(&state["scores"], team, 0)),
                xx,
                y + 157.,
                22.,
                tint,
            );
            s.text("ПОЗЫВНОЙ", xx, y + 188., 10., MUTED);
            for (right, label) in [
                (230., "ОЧКИ"),
                (298., "ФРАГИ"),
                (361., "ПОМОЩЬ"),
                (428., "СМЕРТИ"),
            ] {
                s.right(label, xx + right, y + 188., 10., MUTED);
            }
            s.line(xx, y + 197., xx + 428., y + 197., 1., EDGE);
            let mut members: Vec<_> = players
                .iter()
                .filter(|p| string(p, "team", "") == team)
                .collect();
            members.sort_by_key(|p| {
                (
                    std::cmp::Reverse(count(p, "points", 0)),
                    std::cmp::Reverse(count(p, "kills", 0)),
                    count(p, "deaths", 0),
                )
            });
            for (j, p) in members.iter().take(10).enumerate() {
                let by = y + 220. + j as f32 * 23.;
                let tint = if p["id"] == local["id"] { GOLD } else { PAPER };
                s.text(
                    &s.shorten(string(p, "name", "Боец"), 14., 168.),
                    xx,
                    by,
                    14.,
                    tint,
                );
                for (right, key) in [
                    (230., "points"),
                    (298., "kills"),
                    (361., "assists"),
                    (428., "deaths"),
                ] {
                    s.right(
                        &count(p, key, 0).to_string(),
                        xx + right,
                        by,
                        14.,
                        if key == "points" { tint } else { PAPER },
                    );
                }
            }
        }
        s.text(
            "Личный счёт: 100 за фраг, 50 за помощь.",
            x + 28.,
            y + 470.,
            13.,
            MUTED,
        );
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
            s.rect(x, y, 65., 17., PLATE);
            s.right(
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
        let s = if mode == "playing" && self.inventory_open {
            Surface::inventory(font)
        } else {
            Surface::new(font)
        };
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
                    self.hud(&s, state, local, camera);
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
