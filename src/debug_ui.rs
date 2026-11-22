//! Local development panel. Only the dedicated process can enable match commands.
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
const RESOURCES: [(&str, &str); 5] = [
    ("wood", "Дерево"),
    ("metal", "Металл"),
    ("oil", "Нефть"),
    ("fuel", "Топливо"),
    ("powder", "Порох"),
];
const INK: Color = Color::new(0.07, 0.095, 0.09, 1.);
const PANEL: Color = Color::new(0.12, 0.15, 0.14, 1.);
const LIGHT: Color = Color::new(0.93, 0.91, 0.82, 1.);
const MUTE: Color = Color::new(0.65, 0.70, 0.64, 1.);
const ACCENT: Color = Color::new(0.97, 0.73, 0.39, 1.);
struct Canvas<'a> {
    x: f32,
    y: f32,
    k: f32,
    font: &'a Font,
}
impl Canvas<'_> {
    fn plate(&self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        draw_rectangle(
            self.x + x * self.k,
            self.y + y * self.k,
            w * self.k,
            h * self.k,
            c,
        );
    }
    fn text(&self, t: &str, x: f32, y: f32, size: f32, c: Color) {
        draw_text_ex(
            t,
            self.x + x * self.k,
            self.y + y * self.k,
            TextParams {
                font: Some(self.font),
                font_size: (size * self.k).round().max(8.) as u16,
                color: c,
                ..Default::default()
            },
        );
    }
    fn button(&self, x: f32, y: f32, w: f32, label: &str, selected: bool, enabled: bool) -> bool {
        let rect = Rect::new(
            self.x + x * self.k,
            self.y + y * self.k,
            w * self.k,
            37. * self.k,
        );
        let (mx, my) = mouse_position();
        let hover = rect.contains(vec2(mx, my));
        self.plate(
            x,
            y,
            w,
            37.,
            if enabled && hover {
                Color::new(0.27, 0.31, 0.26, 1.)
            } else {
                PANEL
            },
        );
        if selected {
            self.plate(x, y + 35., w, 2., ACCENT);
        }
        self.text(
            label,
            x + 9.,
            y + 24.,
            17.,
            if !enabled {
                Color::new(0.49, 0.51, 0.48, 1.)
            } else if selected {
                ACCENT
            } else {
                LIGHT
            },
        );
        enabled && hover && is_mouse_button_pressed(MouseButton::Left)
    }
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
        flight: bool,
        fps: bool,
    ) -> DebugAction {
        let (w, h) = (screen_width(), screen_height());
        let k = ((w - 20.) / 660.).min((h - 20.) / 448.).clamp(0.38, 1.35);
        draw_rectangle(0., 0., w, h, Color::new(0.025, 0.037, 0.035, 0.72));
        let c = Canvas {
            x: (w - 660. * k) * 0.5,
            y: (h - 448. * k) * 0.5,
            k,
            font,
        };
        c.plate(0., 0., 660., 448., INK);
        c.plate(0., 0., 4., 448., ACCENT);
        c.text("Отладка матча", 24., 42., 33., LIGHT);
        if c.button(561., 16., 77., "F2  ×", false, true) {
            return DebugAction::Close;
        }
        c.text(
            if admin {
                "Локальный разработчик · изменения применяет сервер"
            } else {
                "Публичный матч · доступны настройки отображения"
            },
            25.,
            68.,
            17.,
            MUTE,
        );
        let roster = state["players"].as_array();
        let bots = roster
            .into_iter()
            .flatten()
            .filter(|p| p["bot"] == true)
            .count();
        let humans = roster.map_or(0, Vec::len) - bots;
        c.text("Отображение", 24., 105., 20., ACCENT);
        if c.button(
            24.,
            117.,
            207.,
            if fps {
                "FPS   включён"
            } else {
                "FPS   выключен"
            },
            fps,
            true,
        ) {
            return DebugAction::Fps;
        }
        if c.button(
            242.,
            117.,
            204.,
            if flight {
                "Вернуться к бойцу"
            } else {
                "Свободная камера"
            },
            flight,
            admin,
        ) {
            return DebugAction::Flight;
        }
        c.text("WASD · Q/E высота · Shift быстрее", 24., 180., 16., MUTE);
        c.text("Участники", 24., 217., 20., ACCENT);
        c.text(
            &format!("Люди {humans}     Боты {bots}     Мест 12"),
            25.,
            246.,
            18.,
            LIGHT,
        );
        if c.button(330., 221., 127., "−  Бот", false, admin && bots > 0) {
            return DebugAction::Command(json!({"op":"bots","count":bots-1}));
        }
        if c.button(
            469.,
            221.,
            128.,
            "+  Бот",
            false,
            admin && bots < 10 && humans + bots < 12,
        ) {
            return DebugAction::Command(json!({"op":"bots","count":bots+1}));
        }
        c.text("Рюкзак и состояние", 24., 297., 20., ACCENT);
        for (i, (id, name)) in RESOURCES.iter().enumerate() {
            let v = player[*id].as_u64().unwrap_or(0);
            let selected = i == self.selected;
            if c.button(
                24. + 123. * i as f32,
                310.,
                115.,
                &format!("{name}  {v}"),
                selected,
                true,
            ) {
                self.selected = i;
            }
        }
        let id = RESOURCES[self.selected].0;
        let room = player[id].as_u64().unwrap_or(0) < 32;
        let alive = player["alive"] == true;
        for (i, amount) in [1, 16, 32].into_iter().enumerate() {
            if c.button(
                24. + 94. * i as f32,
                363.,
                83.,
                &format!("+ {amount}"),
                false,
                admin && alive && room,
            ) {
                return DebugAction::Command(json!({"op":"grant","resource":id,"amount":amount}));
            }
        }
        if c.button(
            330.,
            363.,
            267.,
            "Здоровье и боезапас",
            false,
            admin && alive,
        ) {
            return DebugAction::Command(json!({"op":"refill"}));
        }
        c.text(
            if admin {
                "Esc закрывает панель. Камера оставляет бойца в матче."
            } else {
                "Хост включает команды при запуске сервера с --dev-local."
            },
            24.,
            427.,
            15.,
            MUTE,
        );
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
