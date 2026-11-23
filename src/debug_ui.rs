//! Local development panel. Only the dedicated process can enable match commands.
use super::stash_ui::material_symbol;
use super::{alpha, count, friendly_vehicle, Surface, BLUE_TEAM, EDGE, GOLD, INK, MATERIALS, MUTED, PAPER};
use macroquad::prelude::*;
use serde_json::{json, Value};

pub enum DebugAction {
    None,
    Close,
    Flight,
    Fps,
    Command(Value),
}
pub struct DebugUi {
    pub open: bool,
    selected: usize,
}
impl DebugUi {
    pub fn new() -> Self {
        Self {
            open: false,
            selected: 1,
        }
    }
    pub fn draw(
        &mut self,
        font: &Font,
        state: &Value,
        player: &Value,
        admin: bool,
        vehicle_service: bool,
        flight: bool,
        fps: bool,
    ) -> DebugAction {
        let s = Surface::new(font);
        let (x, y) = s.panel(816., 540., if admin { "Локальный разработчик" } else { "Публичный матч" }, "Отладка матча");
        if s.button(x + 658., y + 25., 130., 34., "Закрыть   F2", false) {
            return DebugAction::Close;
        }
        let control = |left: f32, top: f32, width: f32, label: &str, enabled: bool| {
            if enabled {
                s.button(x + left, y + top, width, 35., label, false)
            } else {
                s.disabled(x + left, y + top, width, 35., label);
                false
            }
        };
        s.text("ОТОБРАЖЕНИЕ", x + 28., y + 128., 12., MUTED);
        if control(28., 145., 147., if fps {"FPS включён"} else {"FPS выключен"}, true) {
            return DebugAction::Fps;
        }
        if control(184., 145., 179., if flight {"Вернуться к бойцу"} else {"Свободная камера"}, admin) {
            return DebugAction::Flight;
        }
        if admin {
            s.text("WASD  ход    Q / E  высота    Shift  быстрее", x + 28., y + 201., 13., MUTED);
        }
        let players = state["players"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        let bots = players.iter().filter(|p| p["bot"] == true).count();
        let humans = players.len() - bots;
        s.line(x + 389., y + 119., x + 389., y + 205., 1., EDGE);
        s.text("УЧАСТНИКИ", x + 413., y + 128., 12., MUTED);
        s.right(&format!("{} / 12", players.len()), x + 787., y + 128., 15., PAPER);
        for i in 0..12 {
            let tint = if i < humans { PAPER } else if i < players.len() { BLUE_TEAM } else { EDGE };
            s.rect(x + 413. + i as f32 * 31.5, y + 137., 26., 4., tint);
        }
        s.text(&format!("Игроки  {humans}"), x + 413., y + 170., 16., PAPER);
        s.text(&format!("Боты  {bots}"), x + 413., y + 193., 15., BLUE_TEAM);
        if control(555., 155., 110., "−  Бот", admin && bots > 0) {
            return DebugAction::Command(json!({"op":"bots", "count": bots - 1 }));
        }
        if control(676., 155., 111., "+  Бот", admin && bots < 10 && players.len() < 12) {
            return DebugAction::Command(json!({"op":"bots", "count": bots + 1 }));
        }

        s.line(x + 28., y + 219., x + 787., y + 219., 1., EDGE);
        s.text("РЮКЗАК", x + 28., y + 246., 12., MUTED);
        for (i, (id, name)) in MATERIALS.iter().enumerate() {
            let left = x + 28. + i as f32 * 153.;
            let top = y + 259.;
            let value = count(player, id, 0);
            let hover = s.over(left, top, 146., 54.);
            let selected = i == self.selected;
            s.rect(left, top, 146., 54., if selected || hover { alpha(PAPER, 0.09)} else { alpha(INK, 0.56)});
            if selected { s.rect(left, top + 52., 146., 2., GOLD); }
            material_symbol(&s, i, left + 20., top + 26., if selected { GOLD } else { MUTED });
            s.text(name, left + 39., top + 20., 13., MUTED);
            s.text(&value.to_string(), left + 39., top + 42., 20., PAPER);
            if hover && s.click { self.selected = i; }
        }
        let (id, label) = MATERIALS[self.selected];
        let alive = player["alive"] == true;
        let room = (32 - count(player, id, 0)).max(0);
        s.text(&format!("{label}   ·   свободно {room}"), x + 28., y + 347., 16., PAPER);
        for (i, amount) in [1, 16, 32].into_iter().enumerate() {
            if control(425. + i as f32 * 124., 323., 113., &format!("+ {amount}"), admin && alive && room > 0) {
                return DebugAction::Command(json!({"op":"grant", "resource":id, "amount":amount}));
            }
        }
        s.line(x + 28., y + 375., x + 787., y + 375., 1., EDGE);
        s.text("ВОССТАНОВЛЕНИЕ", x + 28., y + 402., 12., MUTED);
        if control(28., 414., 231., "Здоровье и боезапас", admin && alive) {
            return DebugAction::Command(json!({"op":"refill"}));
        }
        let vehicle_hint = if !admin {
            Some("Хост включает команды при запуске с --dev-local.")
        } else if !vehicle_service {
            Some("Для обслуживания машины нужен сервер с поддержкой F2-сервиса.")
        } else if !alive {
            Some("Восстановление доступно после возрождения.")
        } else if friendly_vehicle(state, player).is_none() {
            Some("Для ремонта и заправки подойди к союзной машине.")
        } else { None };
        if control(270., 414., 385., "Машина   ·   ремонт, топливо и боезапас", vehicle_hint.is_none()) {
            return DebugAction::Command(json!({"op":"vehicleService"}));
        }
        if let Some(hint) = vehicle_hint {
            s.text(hint, x + 28., y + 472., 14., MUTED);
        }
        if let Some(feedback) = player["message"].as_str().filter(|_| player["messageT"].as_f64().unwrap_or(0.) > 0.) {
            s.wrap(feedback, x + 28., y + 503., 757., 14., PAPER);
        }
        DebugAction::None
    }
}
pub struct FreeCamera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}
impl FreeCamera {
    pub fn at(cam: &Camera3D) -> Self {
        let v = (cam.target - cam.position).normalize_or_zero();
        Self {
            position: cam.position,
            yaw: v.x.atan2(-v.z),
            pitch: v.y.clamp(-1., 1.).asin(),
        }
    }
    pub fn camera(&mut self, dt: f32, active: bool) -> Camera3D {
        if active {
            let mouse = mouse_delta_position();
            self.yaw -= mouse.x * screen_width() * 0.0011;
            self.pitch = (self.pitch + mouse.y * screen_height() * 0.0011).clamp(-1.5, 1.5);
        }
        let dir = vec3(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        );
        if active {
            let pair = |a, b| is_key_down(a) as i8 as f32 - is_key_down(b) as i8 as f32;
            let right = vec3(self.yaw.cos(), 0., self.yaw.sin());
            let movement = dir * pair(KeyCode::W, KeyCode::S)
                + right * pair(KeyCode::D, KeyCode::A)
                + Vec3::Y * pair(KeyCode::E, KeyCode::Q);
            self.position += movement.normalize_or_zero()
                * dt
                * if is_key_down(KeyCode::LeftShift) {
                    45.
                } else {
                    14.
                };
            self.position = self
                .position
                .clamp(vec3(-380., 0.24, -340.), vec3(380., 150., 340.));
        }
        Camera3D {
            position: self.position,
            target: self.position + dir,
            up: Vec3::Y,
            z_far: 850.,
            fovy: 70_f32.to_radians(),
            ..Default::default()
        }
    }
}
