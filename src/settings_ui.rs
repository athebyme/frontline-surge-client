use super::{alpha, Action, Surface, Ui, EDGE, GOLD, MUTED, PAPER};
use crate::settings::Tier;
use macroquad::prelude::Color;

fn choices<T: Copy + PartialEq>(
    s: &Surface,
    origin: (f32, f32),
    label: &str,
    value: Option<T>,
    options: &[(T, &str)],
) -> Option<T> {
    let (x, y) = origin;
    s.text(label, x, y + 21., 15., PAPER);
    let width = 324. / options.len() as f32;
    let mut selected = None;
    for (i, &(choice, title)) in options.iter().enumerate() {
        let bx = x + 217. + i as f32 * width;
        if s.button(bx, y, width - 4., 30., title, value == Some(choice)) {
            selected = Some(choice);
        }
    }
    selected
}

fn percent(s: &Surface, origin: (f32, f32), label: &str, value: &mut u16, min: u16, max: u16) {
    let (x, y) = origin;
    s.text(label, x, y + 21., 15., PAPER);
    let bx = x + 217.;
    if s.button(bx, y, 44., 30., "−", false) {
        *value = value.saturating_sub(5).max(min);
    }
    s.mid(&format!("{} %", value), bx + 100., y + 21., 16., PAPER);
    if s.button(bx + 156., y, 44., 30., "+", false) {
        *value = value.saturating_add(5).min(max);
    }
}

impl Ui {
    pub fn save_preferences(&mut self) {
        self.settings_write_failed = self.settings.save().is_err();
    }

    pub(super) fn preferences(&mut self, s: &Surface) -> Action {
        // The open right side keeps the scene visible while changing the renderer.
        let w = 600.;
        let h = 585.;
        let x = ((s.w - w) * 0.12).max(12.);
        let y = ((s.h - h) * 0.5).max(4.);
        s.rect(x, y, w, h, Color::new(0.055, 0.09, 0.10, 0.97));
        s.menu_ribbon(x + 28., y + 66., w - 56.);
        let x = x + 28.;
        s.bold().text("Настройки", x, y + 51., 34., PAPER);
        let original = self.settings;
        let g = &mut self.settings.graphics;
        let levels = [
            (Tier::Low, "Низкая"),
            (Tier::Medium, "Средняя"),
            (Tier::High, "Высокая"),
        ];
        if let Some(tier) = choices(
            s,
            (x, y + 78.),
            "Профиль графики",
            g.matching_preset(),
            &[
                (Tier::Low, "Быстрый"),
                (Tier::Medium, "Баланс"),
                (Tier::High, "Качество"),
            ],
        ) {
            let saturation = g.saturation;
            *g = crate::settings::Graphics::preset(tier);
            g.saturation = saturation;
        }
        if g.matching_preset().is_none() {
            s.text("Свой профиль", x, y + 120., 12., GOLD);
        }
        s.line(x, y + 133., x + 540., y + 133., 1., EDGE);
        if let Some(v) = choices(
            s,
            (x, y + 151.),
            "Тени",
            Some(g.shadows),
            &[
                (Tier::Low, "Выкл."),
                (Tier::Medium, "Быстрые"),
                (Tier::High, "Мягкие"),
            ],
        ) {
            g.shadows = v;
        }
        if let Some(v) = choices(
            s,
            (x, y + 192.),
            "Детализация моделей",
            Some(g.models),
            &levels,
        ) {
            g.models = v;
        }
        if let Some(v) = choices(
            s,
            (x, y + 233.),
            "Дальность травы",
            Some(g.foliage),
            &[
                (Tier::Low, "42 м"),
                (Tier::Medium, "88 м"),
                (Tier::High, "140 м"),
            ],
        ) {
            g.foliage = v;
        }
        if let Some(v) = choices(
            s,
            (x, y + 274.),
            "Фактура материалов",
            Some(g.materials),
            &levels,
        ) {
            g.materials = v;
        }
        percent(s, (x, y + 315.), "Насыщенность", &mut g.saturation, 70, 140);
        s.line(x, y + 362., x + 540., y + 362., 1., EDGE);
        if let Some(v) = choices(
            s,
            (x, y + 380.),
            "Режим экрана",
            Some(self.settings.fullscreen),
            &[(false, "Окно"), (true, "Полный экран")],
        ) {
            self.settings.fullscreen = v;
        }
        if let Some(v) = choices(
            s,
            (x, y + 421.),
            "Счётчик FPS",
            Some(self.settings.show_fps),
            &[(false, "Выкл."), (true, "Вкл.")],
        ) {
            self.settings.show_fps = v;
        }
        percent(
            s,
            (x, y + 462.),
            "Чувствительность мыши",
            &mut self.settings.sensitivity,
            30,
            200,
        );
        if self.settings != original {
            self.save_preferences();
        }
        s.line(x, y + 513., x + 540., y + 513., 1., alpha(EDGE, 0.25));
        if s.button(x, y + 530., 134., 35., "Назад   Esc", false) {
            self.settings_open = false;
        }
        if self.settings_write_failed {
            s.text(
                "Сбой записи настроек на диск",
                x + 155.,
                y + 552.,
                14.,
                GOLD,
            );
        } else {
            s.text(
                "Изменения сохраняются сразу",
                x + 155.,
                y + 552.,
                14.,
                MUTED,
            );
        }
        Action::None
    }
}
