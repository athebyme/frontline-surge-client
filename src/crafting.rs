//! Context-sensitive field fabrication catalogue. The server validates each submitted action.
use super::{
    alpha, count, flag, kit_name, number, string, Action, Surface, EDGE, GOLD, INK, MATERIALS,
    MUTED, PAPER, RED_TEAM,
};
use macroquad::prelude::Color;
use serde_json::{json, Value};

pub(super) struct CraftMenu {
    tab: usize,
    pick: usize,
    station: Option<&'static str>,
}
struct Recipe {
    id: &'static str,
    name: &'static str,
    place: &'static str,
    outcome: String,
    cost: Vec<(&'static str, i32)>,
    button: &'static str,
    lock: Option<String>,
}
impl Recipe {
    fn new(
        id: &'static str,
        name: &'static str,
        place: &'static str,
        outcome: impl Into<String>,
        cost: Vec<(&'static str, i32)>,
        button: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            place,
            outcome: outcome.into(),
            cost,
            button,
            lock: None,
        }
    }
    fn require(&mut self, ok: bool, why: impl Into<String>) {
        if !ok && self.lock.is_none() {
            self.lock = Some(why.into());
        }
    }
    fn check_cost(&mut self, p: &Value) {
        let mut missing = Vec::new();
        for (k, n) in &self.cost {
            let short = n - count(p, k, 0);
            if short > 0 {
                missing.push(format!("{} +{}", material(k), short));
            }
        }
        self.require(
            missing.is_empty(),
            format!("Не хватает: {}.", missing.join(", ")),
        );
    }
}
fn material(key: &str) -> &str {
    MATERIALS
        .iter()
        .find_map(|(k, n)| (*k == key).then_some(*n))
        .unwrap_or(key)
}
pub(super) fn footprint(p: &Value, site: &Value, w: f64, d: f64) -> f64 {
    let x = number(p, "x", 0.) - number(site, "x", 0.);
    let z = number(p, "z", 0.) - number(site, "z", 0.);
    let a = number(site, "yaw", 0.);
    let (si, co) = a.sin_cos();
    let dx = ((x * co + z * si).abs() - w * 0.5).max(0.);
    let dz = ((-x * si + z * co).abs() - d * 0.5).max(0.);
    dx * dx + dz * dz
}
fn building_size(k: &str) -> (f64, f64) {
    if k == "refinery" {
        (4.4, 3.3)
    } else {
        (4.8, 3.8)
    }
}
fn by_facility<'a>(p: &Value, state: &'a Value, kind: &str, reach: f64) -> Option<&'a Value> {
    let team = string(p, "team", "blue");
    let source = state["factories"].as_array()?;
    let (w, d) = building_size(kind);
    source
        .iter()
        .filter(|f| {
            string(f, "kind", "") == kind
                && string(f, "team", "") == team
                && footprint(p, f, number(f, "w", w), number(f, "d", d)) <= reach * reach
        })
        .min_by(|a, b| {
            footprint(p, a, number(a, "w", w), number(a, "d", d)).total_cmp(&footprint(
                p,
                b,
                number(b, "w", w),
                number(b, "d", d),
            ))
        })
}
fn car<'a>(state: &'a Value, p: &Value) -> Option<&'a Value> {
    let team = string(p, "team", "blue");
    let cars = state["vehicles"].as_array()?;
    let current = &p["vehicleId"];
    if !current.is_null() {
        return cars
            .iter()
            .find(|v| v["id"] == *current && !flag(v, "destroyed", false));
    }
    cars.iter()
        .filter(|v| string(v, "team", "") == team && !flag(v, "destroyed", false))
        .filter_map(|v| {
            let (w, d) = match string(v, "kind", "") {
                "cargo6x6" => (2.36, 6.),
                "mbt" => (2.9, 5.9),
                "apc" => (2.8, 5.45),
                "scout_buggy" => (1.75, 3.3),
                _ => (1.65, 3.5),
            };
            let dd = footprint(p, v, w, d);
            (dd <= 2.65 * 2.65).then_some((dd, v))
        })
        .min_by(|(a, _), (b, _)| a.total_cmp(b))
        .map(|(_, v)| v)
}
fn refinery_remaining(f: &Value) -> f64 {
    if count(f, "ready", 0) > 0 || f["pending"].as_i64() == Some(0) {
        return 0.;
    }
    number(f, "remaining", 0.).max(0.)
}
fn catalogue(tab: usize, state: &Value, p: &Value) -> Vec<Recipe> {
    let carried = count(p, "fuel", 0);
    let seated = !p["vehicleId"].is_null();
    let vehicle = car(state, p);
    let workshop = by_facility(p, state, "workshop", if seated { 8. } else { 3. }).is_some();
    let motor_workshop = by_facility(p, state, "workshop", 8.).is_some();
    let tank = seated && vehicle.is_some_and(|v| string(v, "kind", "") == "mbt");
    let mut rows = Vec::new();
    match tab{
        0=>{
            for (id,name,wood,metal,what) in [
                ("wall","Стена",3,1,"Укрытие для пехоты. Выбери чертёж, найди свободное место и подтверди размещение ЛКМ."),
                ("refinery","Нефтеперегонка",8,6,"Перерабатывает четыре единицы нефти в шесть единиц топлива за 12 секунд."),
                ("workshop","Мастерская",6,8,"Своя точка снабжения. Здесь изготавливают патроны и гранаты, обслуживают и улучшают машины.")
            ] {
                let mut r=Recipe::new(id,name,"Пешком, на свободном участке.",what,vec![("wood",wood),("metal",metal)],"Взять чертёж");
                r.require(!seated,"Выйди из машины для строительства.");rows.push(r);
            }
        }
        1=>{
            let refinery=by_facility(p,state,"refinery",2.6);
            let ready=refinery.map_or(0,|f|count(f,"ready",0));
            let remaining=refinery.map_or(0,|f|refinery_remaining(f).ceil() as i32);
            let mut refine=if ready>0{
                Recipe::new("refine","Забрать топливо","Рядом со своей нефтеперегонкой.",format!("В приёмнике {} ед. Места для топлива в рюкзаке: {}.",ready,(32-carried).max(0)),vec![],"Забрать топливо")
            }else{
                Recipe::new("refine","Переработать нефть","Рядом со своей нефтеперегонкой.","Через 12 секунд можно забрать 6 топлива. Заправляй технику прямо из рюкзака.",vec![("oil",4)],"Запустить цикл")
            };
            refine.require(!seated,"Выйди из машины для работы с нефтеперегонкой.");
            refine.require(refinery.is_some(),"Подойди к своей нефтеперегонке.");
            refine.require(remaining==0,format!("Переработка уже идёт. Осталось {remaining} с."));
            if ready>0{refine.require(carried<32,"Освободи место в рюкзаке.");}
            rows.push(refine);
            let gun_car = seated && vehicle.is_some_and(|v|count(v,"mgCapacity",0)>0);
            let (name, detail) = if tank {
                ("Боекомплект танка","До 3 снарядов для пушки и 28 патронов для пулемёта.")
            } else if gun_car {
                ("Пулемётная лента","До 28 патронов для бортового пулемёта.")
            } else {
                ("Патроны","Добавляет 24 патрона в запас бойца.")
            };
            let mut ammo=Recipe::new("ammo",name,"Рядом со своей мастерской.",detail,
                vec![("powder",2),("metal",1)],"Изготовить");
            ammo.require(workshop,"Подойди или подведи машину к своей мастерской.");
            let full = gun_car && vehicle.is_some_and(|v| (!tank || count(v,"shells",0)>=12) && count(v,"mgAmmo",0)>=count(v,"mgCapacity",0));
            ammo.require(!full,"Боекомплект машины полон.");
            rows.push(ammo);
            let mut grenade=Recipe::new("grenade","Ручная граната","Рядом со своей мастерской.","Добавляет одну гранату в снаряжение бойца. H — бросок в бою.",vec![("powder",2),("metal",1)],"Изготовить");
            grenade.require(!seated,"Выйди из машины для изготовления ручной гранаты.");grenade.require(workshop,"Подойди к своей мастерской.");grenade.require(count(p,"grenades",0)<4,"Подсумок полон: четыре гранаты.");rows.push(grenade);
        }
        _=>{
            let fuel_needed=vehicle.map_or(0,|v|(number(v,"maxFuel",0.)-number(v,"fuel",0.)).max(0.).ceil() as i32);
            let fuel=count(p,"fuel",0).min(fuel_needed);
            let damaged=vehicle.is_some_and(|v|number(v,"hp",0.)<number(v,"maxHp",0.));
            let repair=damaged&&motor_workshop&&count(p,"metal",0)>=2;
            let mut cost=Vec::new();if fuel>0{cost.push(("fuel",fuel));}if repair{cost.push(("metal",2));}
            let mut service=Recipe::new("service","Обслуживание","У своей машины. Для ремонта нужна мастерская.",
                if fuel>0||repair{format!("Заправка: +{fuel}. Восстановление корпуса: {}.",if repair{"до 80"}else{"0"})}else{"Топливо из рюкзака идёт в бак. Два металла восстанавливают до 80 прочности у мастерской.".into()},
                cost,"Обслужить");
            service.require(vehicle.is_some(),"Подойди к своей машине.");
            service.require(fuel>0||repair,if damaged&&!motor_workshop{"Для ремонта нужна своя мастерская. Для заправки возьми топливо."}else if damaged{"Для ремонта нужны 2 металла."}else if fuel_needed>0{"Для заправки возьми топливо."}else{"Корпус цел, бак полный."});
            rows.push(service);
            let choice=string(p,"upgradeChoice","mobility");
            let tier=vehicle.map_or(1,|v|if string(v,"upgrade","")==choice {count(v,"upgradeLevel",0)+1}else{1});
            let name=kit_name(choice);let level=if tier==1{"I"}else{"II"};
            let effect=if choice=="mobility"{"Повышает скорость машины."}else{
                match vehicle.map(|v|string(v,"kind","")).unwrap_or("") {
                    "patrol4x4"|"scout_buggy"=>"Топливный бак получает дополнительные 42% объёма за уровень.",
                    "cargo6x6"=>"Грузовой отсек получает 32 дополнительных места за уровень.",
                    "apc"=>"Грузовой отсек получает 12 дополнительных мест за уровень.",
                    "mbt"=>"Корпус получает 22% дополнительной прочности за уровень.",
                    _=>"Улучшает бак, кузов или прочность корпуса в зависимости от машины."
                }
            };
            let mut up=Recipe::new("upgrade","Установить комплект","У своей машины возле своей мастерской.",
                format!("{name} {level}. {effect} При смене комплекта прогресс новой ветки начинается с I."),
                vec![("wood",tier.min(2)*2),("metal",tier.min(2)*4)],"Установить");
            up.require(vehicle.is_some(),"Подойди к своей машине.");
            up.require(motor_workshop,"Подведи машину к своей мастерской.");
            up.require(tier<=2,"На машине уже установлен комплект II.");
            if let Some(v)=vehicle {
                let base=match string(v,"kind",""){"cargo6x6"=>96,"apc"=>24,_=>0};
                let total:i32=MATERIALS.iter().map(|(k,_)|count(&v["cargo"],k,0)).sum();
                up.require(!(base>0 && choice=="mobility" && total>base),format!("Выгрузи часть груза. Штатный отсек вмещает {base} ед."));
            }
            rows.push(up);
        }
    }
    for r in &mut rows {
        r.require(
            p["droneId"].is_null(),
            "Отзови разведдрон перед работой со снаряжением.",
        );
        r.require(flag(p, "alive", true), "Дождись возвращения в бой.");
        r.check_cost(p);
    }
    rows
}
pub(super) fn available_station(state: &Value, p: &Value) -> Option<&'static str> {
    if !flag(p, "alive", true) || flag(p, "buildMode", false) || !p["droneId"].is_null() {
        return None;
    }
    let seated = !p["vehicleId"].is_null();
    // Use the server's line-of-sight checked pickup target, so a pile on the
    // other side of a wall cannot block the station interaction.
    if !seated && string(p, "hint", "").contains("подобрать") {
        return None;
    }
    ["refinery", "workshop"]
        .into_iter()
        .filter(|kind| !seated || *kind == "workshop")
        .filter_map(|kind| {
            let reach = if seated {
                8.
            } else if kind == "workshop" {
                3.
            } else {
                2.6
            };
            let f = by_facility(p, state, kind, reach)?;
            let (w, d) = building_size(kind);
            Some((kind, footprint(p, f, number(f, "w", w), number(f, "d", d))))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|v| v.0)
}
pub(super) fn mode_name(id: &str) -> &'static str {
    match id {
        "burst" => "Отсечка",
        "auto" => "Авто",
        _ => "Одиночный",
    }
}
impl CraftMenu {
    pub(super) fn new() -> Self {
        Self {
            tab: 0,
            pick: 0,
            station: None,
        }
    }
    pub(super) fn browse(&mut self) {
        self.station = None;
    }
    pub(super) fn open_station(&mut self, kind: &str) {
        self.station = match kind {
            "refinery" => Some("refinery"),
            "workshop" => Some("workshop"),
            _ => None,
        };
        self.tab = if kind == "workshop" { 3 } else { 1 };
        self.pick = 0;
    }
    fn loadout(&self, s: &Surface, x: f32, y: f32, state: &Value, p: &Value) -> Action {
        let weapon = string(p, "weapon", "mr7");
        let valid = flag(p, "alive", true) && p["vehicleId"].is_null() && p["droneId"].is_null();
        let fitting = valid && by_facility(p, state, "workshop", 3.).is_some();
        s.text("ОСНОВНОЕ ОРУЖИЕ", x + 29., y + 204., 11., GOLD);
        let choose =
            |left: f32, top: f32, width: f32, name: &str, selected: bool, enabled: bool| {
                let click = if enabled {
                    s.button(left, top, width, 37., name, false)
                } else {
                    s.disabled(left, top, width, 37., name);
                    false
                };
                if selected {
                    s.rect(left, top + 34., width, 3., GOLD);
                }
                click && !selected
            };
        for (i, (key, label)) in [
            ("mr7", "1   MR-7"),
            ("vx9", "2   VX-9"),
            ("ar4", "3   AR-4"),
        ]
        .iter()
        .enumerate()
        {
            if choose(
                x + 28. + i as f32 * 158.,
                y + 216.,
                146.,
                label,
                weapon == *key,
                valid,
            ) {
                return Action::Input(json!({"weapon":key}));
            }
        }
        s.text("РЕЖИМ ОГНЯ", x + 536., y + 204., 11., GOLD);
        let caption = format!("{}    N  ›", mode_name(string(p, "fireMode", "semi")));
        if valid {
            if s.button(x + 535., y + 216., 182., 37., &caption, false) {
                return Action::Input(json!({"cycleMode":true}));
            }
        } else {
            s.disabled(x + 535., y + 216., 182., 37., &caption);
        }
        s.text("ПРИЦЕЛ", x + 28., y + 293., 11., GOLD);
        s.text("ЦЕВЬЁ", x + 468., y + 293., 11., GOLD);
        for (col, (field, input, opts)) in [
            (
                "optic",
                "fitOptic",
                [
                    ("irons", "Открытый"),
                    ("reflex", "Коллиматор"),
                    ("scope", "Оптика"),
                ],
            ),
            (
                "grip",
                "fitGrip",
                [
                    ("standard", "Штатное"),
                    ("light", "Лёгкое"),
                    ("stable", "Устойчивое"),
                ],
            ),
        ]
        .iter()
        .enumerate()
        {
            let bx = x + 28. + col as f32 * 440.;
            let current = string(p, field, opts[0].0);
            for (i, (id, label)) in opts.iter().enumerate() {
                if choose(
                    bx + i as f32 * 137.,
                    y + 308.,
                    130.,
                    label,
                    current == *id,
                    fitting,
                ) {
                    return Action::Input(json!({(*input):id}));
                }
            }
        }
        s.wrap(
            "Коллиматор даёт широкий обзор. Оптика приближает цель.",
            x + 28.,
            y + 375.,
            397.,
            14.,
            MUTED,
        );
        s.wrap(
            "Лёгкое цевьё повышает подвижность. Устойчивое уменьшает разброс.",
            x + 468.,
            y + 375.,
            394.,
            14.,
            MUTED,
        );
        s.line(x + 28., y + 429., x + 880., y + 429., 1., EDGE);
        let note = if !valid {
            "Для подгонки снаряжения нужен боец пешком."
        } else if !fitting {
            "Подойди к своей мастерской, чтобы сменить прицел или цевьё."
        } else {
            "Подгонка бесплатна. Прицел и цевьё сохраняются для каждого оружия."
        };
        s.wrap(
            note,
            x + 28.,
            y + 462.,
            828.,
            15.,
            if fitting { PAPER } else { RED_TEAM },
        );
        if number(p, "messageT", 0.) > 0. {
            s.wrap(string(p, "message", ""), x + 28., y + 521., 828., 14., GOLD);
        }
        Action::None
    }
    pub(super) fn draw(&mut self, s: &Surface, state: &Value, p: &Value) -> (Action, bool) {
        let title = match self.station {
            Some("workshop") => "Мастерская",
            Some("refinery") => "Нефтеперегонка",
            _ => "Крафт",
        };
        let (x, y) = s.panel(908., 572., "ПОЛЕВОЕ СНАБЖЕНИЕ", title);
        if s.button(x + 762., y + 26., 120., 33., "Закрыть  K", false) {
            return (Action::None, true);
        }
        let materials = MATERIALS
            .iter()
            .map(|(k, n)| format!("{n} {}", count(p, k, 0)))
            .collect::<Vec<_>>()
            .join("    ·    ");
        s.text(
            &format!("В рюкзаке    {materials}"),
            x + 29.,
            y + 117.,
            14.,
            PAPER,
        );
        let tabs: &[(usize, &str)] = match self.station {
            Some("workshop") => &[(3, "Снаряжение"), (1, "Боеприпасы"), (2, "Техника")],
            Some("refinery") => &[(1, "Производство")],
            _ => &[
                (0, "Строительство"),
                (1, "Производство"),
                (2, "Техника"),
                (3, "Снаряжение"),
            ],
        };
        for (i, (tab, name)) in tabs.iter().enumerate() {
            let left = x + 28. + i as f32 * 179.;
            if s.button(left, y + 140., 168., 36., name, false) {
                self.tab = *tab;
                self.pick = 0;
            }
            if self.tab == *tab {
                s.rect(left, y + 174., 168., 2., GOLD);
            }
        }
        if self.tab == 3 {
            return (self.loadout(s, x, y, state, p), false);
        }
        let mut rows = catalogue(self.tab, state, p);
        if self.station == Some("refinery") {
            rows.retain(|r| r.id == "refine");
        }
        if self.station == Some("workshop") && self.tab == 1 {
            rows.retain(|r| r.id != "refine");
        }
        self.pick = self.pick.min(rows.len() - 1);
        for (i, r) in rows.iter().enumerate() {
            let left = x + 28.;
            let top = y + 202. + i as f32 * 75.;
            let chosen = i == self.pick;
            let hovered = s.over(left, top, 289., 66.);
            let c = if chosen {
                Color::new(0.25, 0.28, 0.20, 1.)
            } else if hovered {
                Color::new(0.18, 0.21, 0.17, 1.)
            } else {
                INK
            };
            s.rect(left, top, 289., 66., c);
            s.border(left, top, 289., 66., if chosen { GOLD } else { EDGE });
            s.text(r.name, left + 13., top + 26., 17., PAPER);
            s.text(
                if r.lock.is_none() {
                    "Доступно"
                } else {
                    "Недоступно"
                },
                left + 13.,
                top + 50.,
                12.,
                if r.lock.is_none() { GOLD } else { MUTED },
            );
            if hovered && s.click {
                self.pick = i;
            }
        }
        if self.station == Some("refinery") {
            let nearby = by_facility(p, state, "refinery", 2.6);
            let remaining = nearby.map_or(0., refinery_remaining);
            let ready = nearby.map_or(0, |v| count(v, "ready", 0));
            let bx = x + 29.;
            let by = y + 314.;
            s.text("СОСТОЯНИЕ УСТАНОВКИ", bx, by, 11., GOLD);
            let label = if nearby.is_none() {
                "Вне досягаемости".to_owned()
            } else if remaining > 0. {
                format!("В работе   ·   {} с", remaining.ceil() as i32)
            } else if ready > 0 {
                format!("К выдаче   ·   {ready} топлива")
            } else {
                "Готова к запуску".to_owned()
            };
            s.text(&label, bx, by + 32., 19., PAPER);
            s.rect(bx, by + 47., 262., 4., EDGE);
            let progress = if ready > 0 {
                1.
            } else if remaining > 0. {
                (1. - remaining / 12.).clamp(0., 1.)
            } else {
                0.
            };
            s.rect(bx, by + 47., 262. * progress as f32, 4., GOLD);
            s.text("Один цикл    4 нефти → 6 топлива", bx, by + 81., 13., MUTED);
        }
        let r = &rows[self.pick];
        let dx = x + 345.;
        let right = x + 879.;
        s.text(r.name, dx, y + 228., 25., PAPER);
        s.wrap(&r.outcome, dx, y + 259., right - dx, 15., MUTED);
        s.text("ГДЕ", dx, y + 333., 11., GOLD);
        s.text(r.place, dx, y + 355., 14., PAPER);
        s.text("МАТЕРИАЛЫ  ·  ЕСТЬ / НУЖНО", dx, y + 389., 11., GOLD);
        if r.cost.is_empty() {
            s.text("Материалы не расходуются.", dx, y + 412., 14., PAPER);
        } else {
            let mut item_x = dx;
            for (key, n) in &r.cost {
                let have = count(p, key, 0);
                let text = format!("{}   {} / {n}", material(key), have);
                s.text(
                    &text,
                    item_x,
                    y + 412.,
                    14.,
                    if have >= *n { PAPER } else { RED_TEAM },
                );
                item_x += s.measure(&text, 14.) + 28.;
            }
        }
        if let Some(why) = &r.lock {
            s.wrap(why, dx, y + 452., right - dx, 14., RED_TEAM);
        } else if number(p, "messageT", 0.) > 0. {
            s.wrap(
                string(p, "message", ""),
                dx,
                y + 452.,
                right - dx,
                14.,
                MUTED,
            );
        }
        let by = y + 496.;
        if r.lock.is_none() {
            if s.button(dx, by, 198., 45., r.button, true) {
                return (Action::Craft(r.id.to_owned()), self.tab == 0);
            }
        } else {
            s.rect(dx, by, 198., 45., alpha(INK, 0.72));
            s.border(dx, by, 198., 45., EDGE);
            s.mid(r.button, dx + 99., by + 28., 15., MUTED);
        }
        if r.id == "upgrade" && s.button(dx + 211., by, 217., 45., "Сменить комплект", false)
        {
            return (Action::Craft("upgradeSelect".into()), false);
        }
        (Action::None, false)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn finished_refinery_keeps_pickup_and_next_cycle_available() {
        let p = json!({"team":"blue","fuel":0,"oil":4,"alive":true,"x":0,"z":0});
        let mut state = json!({"factories":[{"team":"blue","kind":"refinery","x":0,"z":0,"remaining":0.00003,"ready":6,"pending":0}]});
        assert!(catalogue(1, &state, &p)[0].lock.is_none());
        state["factories"][0]["ready"] = json!(0);
        assert!(catalogue(1, &state, &p)[0].lock.is_none());
    }
    #[test]
    fn station_prompts_follow_team_range_and_ground_target() {
        let state = json!({"factories":[
            {"team":"red","kind":"refinery","x":0,"z":0},
            {"team":"blue","kind":"workshop","x":5,"z":0,"w":4,"d":3}
        ]});
        let mut p = json!({"team":"blue","alive":true,"x":0,"z":0});
        assert_eq!(available_station(&state, &p), Some("workshop"));
        p["hint"] = json!("[E] Металл ×2 · подобрать");
        assert_eq!(available_station(&state, &p), None);
        p["hint"] = json!("");
        p["x"] = json!(-1);
        assert_eq!(available_station(&state, &p), None);
    }
    #[test]
    fn full_main_gun_allows_reloading_partial_secondary() {
        let mut state = json!({"factories":[{"team":"blue","kind":"workshop","x":0,"z":0}],
            "vehicles":[{"id":8,"kind":"mbt","team":"blue","shells":12,"mgAmmo":11,"mgCapacity":80}]});
        let p = json!({"team":"blue","alive":true,"vehicleId":8,"x":0,"z":0,"powder":2,"metal":1});
        assert!(catalogue(1, &state, &p)[1].lock.is_none());
        state["vehicles"][0]["mgAmmo"] = json!(80);
        assert!(catalogue(1, &state, &p)[1].lock.is_some());
    }
    #[test]
    fn crafting_respects_pack_and_allied_station() {
        let mut p = json!({"alive":true,"team":"blue","wood":3,"metal":1,"powder":2,"x":0,"z":0});
        let mut state = json!({"factories":[{"x":0,"z":0,"kind":"workshop","team":"red"}]});
        assert!(catalogue(0, &state, &p)[0].lock.is_none());
        assert!(catalogue(0, &state, &p)[1].lock.is_some());
        assert!(catalogue(1, &state, &p)[1].lock.is_some());
        state["factories"][0]["team"] = json!("blue");
        assert!(catalogue(1, &state, &p)[1].lock.is_none());
        p["powder"] = json!(1);
        assert!(catalogue(1, &state, &p)[1].lock.is_some());
    }
}
