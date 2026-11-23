//! Context-sensitive field fabrication catalogue. The server validates each submitted action.
use super::{
    alpha, count, flag, kit_name, number, string, Action, Surface, EDGE, GOLD, MATERIALS, MUTED,
    PAPER, RED_TEAM,
};
#[path = "fabrication_art.rs"]
mod fabrication_art;
use fabrication_art::product;
use serde_json::{json, Value};

pub(super) struct CraftMenu {
    pictures: fabrication_art::LoadoutPictures,
    tab: usize,
    pick: usize,
    station: Option<&'static str>,
    receipt: Option<Receipt>,
}
struct Receipt {
    previous: String,
    previous_time: f64,
    deadline: f64,
    shown: Option<String>,
}
impl Receipt {
    fn pending(p: &Value, now: f64) -> Self {
        Self {
            previous: string(p, "message", "").into(),
            previous_time: number(p, "messageT", 0.),
            deadline: now + 3.,
            shown: None,
        }
    }
    fn message<'a>(&'a mut self, p: &Value, now: f64) -> Option<&'a str> {
        let t = number(p, "messageT", 0.);
        let msg = string(p, "message", "");
        if self.shown.is_none()
            && now < self.deadline
            && t > 0.
            && !msg.is_empty()
            && (msg != self.previous || t > self.previous_time + 0.04)
        {
            self.shown = Some(msg.into());
        }
        self.previous_time = t;
        self.shown
            .as_deref()
            .filter(|result| *result == msg && t > 0.)
    }
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
pub(crate) fn friendly_vehicle<'a>(state: &'a Value, p: &Value) -> Option<&'a Value> {
    let team = string(p, "team", "blue");
    let cars = state["vehicles"].as_array()?;
    let current = &p["vehicleId"];
    if !current.is_null() {
        return cars.iter().find(|v| {
            v["id"] == *current && string(v, "team", "") == team && !flag(v, "destroyed", false)
        });
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
    let vehicle = friendly_vehicle(state, p);
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
            } else if gun_car && vehicle.is_some_and(|v| count(v,"shellCapacity",0)>0) {
                ("Боекомплект БТР","До 3 осколочных выстрелов и 28 патронов для пулемёта.")
            } else if gun_car {
                ("Пулемётная лента","До 28 патронов для бортового пулемёта.")
            } else {
                ("Патроны","Добавляет 24 патрона в запас бойца.")
            };
            let mut ammo=Recipe::new("ammo",name,"Рядом со своей мастерской.",detail,
                vec![("powder",2),("metal",1)],"Изготовить");
            ammo.require(workshop,"Подойди или подведи машину к своей мастерской.");
            let full = gun_car && vehicle.is_some_and(|v| (count(v,"shells",0)>=count(v,"shellCapacity",if tank {12} else {0})) && count(v,"mgAmmo",0)>=count(v,"mgCapacity",0));
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
fn product_id<'a>(r: &'a Recipe, state: &Value, p: &Value) -> &'a str {
    if r.id == "upgrade" {
        if string(p, "upgradeChoice", "mobility") == "mobility" {
            "mobility"
        } else {
            match friendly_vehicle(state, p).and_then(|v| v["kind"].as_str()) {
                Some("cargo6x6" | "apc") => "cargo_rack",
                Some("patrol4x4" | "scout_buggy") => "aux_fuel",
                Some("mbt") => "armor",
                _ => "service",
            }
        }
    } else if r.id == "ammo" && !p["vehicleId"].is_null() {
        match friendly_vehicle(state, p) {
            Some(v) if v["kind"] == "mbt" || count(v, "shellCapacity", 0) > 0 => "shells",
            Some(v) if count(v, "mgCapacity", 0) > 0 => "belt",
            _ => r.id,
        }
    } else {
        r.id
    }
}
struct Change {
    label: &'static str,
    from: String,
    to: String,
}
impl Change {
    fn new(label: &'static str, from: impl ToString, to: impl ToString) -> Self {
        Self {
            label,
            from: from.to_string(),
            to: to.to_string(),
        }
    }
}
fn changes(r: &Recipe, state: &Value, p: &Value) -> Vec<Change> {
    let mut result = Vec::with_capacity(3);
    let v = friendly_vehicle(state, p);
    match r.id {
        "wall" | "workshop" | "refinery" => {
            if r.id == "wall" {
                result.push(Change::new("Построить", "—", "1 секция"));
            } else {
                let current = state["factories"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|f| f["kind"] == r.id && f["team"] == p["team"])
                    .count();
                result.push(Change::new("У команды", current, current + 1));
            }
            let hp = match r.id {
                "wall" => 140,
                "refinery" => 260,
                _ => 300,
            };
            result.push(Change::new("Прочность", "—", hp));
        }
        "refine" => {
            let f = by_facility(p, state, "refinery", 2.6);
            let ready = f.map_or(0, |v| count(v, "ready", 0));
            if ready > 0 {
                let held = count(p, "fuel", 0);
                let moved = (32 - held).clamp(0, ready);
                result.push(Change::new("Топливо в рюкзаке", held, held + moved));
                result.push(Change::new("В приёмнике", ready, ready - moved));
            } else {
                result.push(Change::new("Топливо на выходе", 0, 6));
                result.push(Change::new("Цикл", "—", "12 с"));
            }
        }
        "grenade" => {
            let n = count(p, "grenades", 0);
            result.push(Change::new("Гранаты бойца", n, (n + 1).min(4)));
        }
        "ammo" => {
            if let Some(car) =
                v.filter(|v| !p["vehicleId"].is_null() && count(v, "mgCapacity", 0) > 0)
            {
                let cap = count(
                    car,
                    "shellCapacity",
                    if car["kind"] == "mbt" { 12 } else { 0 },
                );
                if cap > 0 {
                    let shells = count(car, "shells", 0);
                    result.push(Change::new("Боезапас пушки", shells, (shells + 3).min(cap)));
                }
                let n = count(car, "mgAmmo", 0);
                result.push(Change::new(
                    "Пулемётная лента",
                    n,
                    (n + 28).min(count(car, "mgCapacity", 0)),
                ));
            } else {
                let n = count(p, "reserve", 0);
                result.push(Change::new("Патроны в запасе", n, n + 24));
            }
        }
        "service" => {
            if let Some(car) = v {
                let cost = |id: &str| r.cost.iter().find(|(k, _)| *k == id).map_or(0, |(_, n)| *n);
                let fuel = number(car, "fuel", 0.);
                let hp = number(car, "hp", 0.);
                result.push(Change::new(
                    "Топливо в баке",
                    fuel.ceil(),
                    (fuel + cost("fuel") as f64)
                        .min(number(car, "maxFuel", 0.))
                        .ceil(),
                ));
                result.push(Change::new(
                    "Прочность корпуса",
                    hp.ceil(),
                    (hp + if cost("metal") > 0 { 80. } else { 0. })
                        .min(number(car, "maxHp", 0.))
                        .ceil(),
                ));
            }
        }
        "upgrade" => {
            if let Some(car) = v {
                let kind = string(car, "kind", "");
                let prev = string(car, "upgrade", "");
                let choice = string(p, "upgradeChoice", "mobility");
                let old = count(car, "upgradeLevel", 0);
                let tier = if prev == choice { (old + 1).min(2) } else { 1 };
                let get = |k: &str, expected: &str, i: i32| if k == expected { i } else { 0 };
                let old_u = get(prev, "utility", old);
                let new_u = get(choice, "utility", tier);
                let old_m = get(prev, "mobility", old);
                let new_m = get(choice, "mobility", tier);
                let mult = |m: i32, u: i32| {
                    (1. + 0.14 * m as f64)
                        * if kind == "mbt" {
                            1. - 0.075 * u as f64
                        } else {
                            1.
                        }
                };
                let speed = |m, u| format!("{}%", (100. * mult(m, u)).round() as i32);
                result.push(Change::new(
                    "Ход без груза",
                    speed(old_m, old_u),
                    speed(new_m, new_u),
                ));
                if old_u != new_u {
                    match kind {
                        "cargo6x6" | "apc" => {
                            let inc = if kind == "apc" { 12 } else { 32 };
                            let n = count(car, "cargoCapacity", 0);
                            result.push(Change::new("Мест в кузове", n, n + (new_u - old_u) * inc));
                        }
                        "mbt" => {
                            let n = number(car, "maxHp", 610.);
                            result.push(Change::new(
                                "Предел корпуса",
                                n.round(),
                                (n / (1. + 0.22 * old_u as f64) * (1. + 0.22 * new_u as f64))
                                    .round(),
                            ));
                        }
                        _ => {
                            let n = number(car, "maxFuel", 0.);
                            result.push(Change::new(
                                "Объём бака",
                                n.round(),
                                (n / (1. + 0.42 * old_u as f64) * (1. + 0.42 * new_u as f64))
                                    .round(),
                            ));
                        }
                    }
                }
                result.push(Change::new(
                    "Комплект",
                    if old == 0 {
                        "Штатный".to_owned()
                    } else {
                        format!("{} {old}", kit_name(prev))
                    },
                    format!("{} {tier}", kit_name(choice)),
                ));
            }
        }
        _ => {}
    }
    result
}
fn stock_mark(s: &Surface, key: &str, x: f32, y: f32, c: macroquad::prelude::Color) {
    if let Some(i) = MATERIALS.iter().position(|(id, _)| *id == key) {
        super::stash_ui::material_symbol(s, i, x, y, c);
    }
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
            pictures: fabrication_art::LoadoutPictures::new(),
            tab: 0,
            pick: 0,
            station: None,
            receipt: None,
        }
    }
    pub(super) fn browse(&mut self) {
        self.receipt = None;
        self.station = None;
    }
    pub(super) fn open_station(&mut self, kind: &str) {
        self.receipt = None;
        self.station = match kind {
            "refinery" => Some("refinery"),
            "workshop" => Some("workshop"),
            _ => None,
        };
        self.tab = 1;
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
            super::field_hud::weapon_picture(
                s,
                key,
                x + 33. + i as f32 * 158.,
                y + 228.,
                0.65,
                if weapon == *key { GOLD } else { MUTED },
            );
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
        use frontline_surge::arsenal::{FireMode, Grip, Kit, Optic};
        let optic = |id: &str| match id {
            "scope" => Optic::Scope,
            "reflex" => Optic::Reflex,
            _ => Optic::Irons,
        };
        let grip = |id: &str| match id {
            "stable" => Grip::Stable,
            "light" => Grip::Light,
            _ => Grip::Standard,
        };
        let current_kit = Kit {
            optic: optic(string(p, "optic", "irons")),
            grip: grip(string(p, "grip", "standard")),
            mode: FireMode::Semi,
        };
        let mut proposed = current_kit;
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
                if s.over(bx + i as f32 * 137., y + 308., 130., 37.) {
                    if *field == "optic" {
                        proposed.optic = optic(id);
                    } else {
                        proposed.grip = grip(id);
                    }
                }
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
        self.pictures.draw(s, weapon, proposed, x + 28., y + 354.);
        let view = |v: f32| format!("{:.0}°", v);
        let pct = |v: f32| format!("{:.0}%", 100. * v);
        for (i, (label, old, new)) in [
            (
                "Поле зрения",
                view(current_kit.ads_fov()),
                view(proposed.ads_fov()),
            ),
            (
                "Подвижность",
                pct(current_kit.mobility()),
                pct(proposed.mobility()),
            ),
            (
                "Разброс",
                pct(current_kit.spread_multiplier()),
                pct(proposed.spread_multiplier()),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let xx = x + 223. + i as f32 * 214.;
            s.text(label, xx, y + 378., 14., MUTED);
            let text = if old == new {
                new
            } else {
                format!("{old} → {new}")
            };
            s.bold().text(
                &text,
                xx,
                y + 407.,
                21.,
                if old == text { PAPER } else { GOLD },
            );
        }
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
        Action::None
    }
    pub(super) fn draw(&mut self, s: &Surface, state: &Value, p: &Value) -> (Action, bool) {
        let page = (self.station, self.tab, self.pick);
        let (action, close) = self.sheet(s, state, p);
        if page != (self.station, self.tab, self.pick) || close {
            self.receipt = None;
        }
        let now = macroquad::time::get_time();
        if !close && matches!(action, Action::Craft(_) | Action::Input(_)) {
            self.receipt = Some(Receipt::pending(p, now));
        }
        if let Some(msg) = self
            .receipt
            .as_mut()
            .and_then(|receipt| receipt.message(p, now))
        {
            let x = (s.w - 908.) * 0.5 + 28.;
            let y = (s.h - 572.) * 0.5 + 564.;
            s.bold().text("Результат", x, y, 13., GOLD);
            s.text(&s.shorten(msg, 14., 755.), x + 84., y, 14., PAPER);
        }
        (action, close)
    }
    fn sheet(&mut self, s: &Surface, state: &Value, p: &Value) -> (Action, bool) {
        let title = match self.station {
            Some("workshop") => "Мастерская",
            Some("refinery") => "Нефтеперегонка",
            _ => "Крафт",
        };
        let (x, y) = s.panel(908., 572., "ПОЛЕВОЕ СНАБЖЕНИЕ", title);
        if s.button(x + 762., y + 26., 120., 33., "Закрыть  K", false) {
            return (Action::None, true);
        }
        s.text("В рюкзаке", x + 28., y + 123., 15., MUTED);
        let mut xx = x + 129.;
        for (key, label) in MATERIALS {
            let amount = count(p, key, 0);
            let tint = if amount > 0 { PAPER } else { MUTED };
            stock_mark(s, key, xx, y + 118., tint);
            s.text(&format!("{label}  {amount}"), xx + 16., y + 123., 14., tint);
            xx += 143.;
        }
        let tabs: &[(usize, &str)] = match self.station {
            Some("workshop") => &[(1, "Боеприпасы"), (2, "Техника"), (3, "Снаряжение")],
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
            if chosen || hovered {
                s.rect(
                    left,
                    top,
                    289.,
                    66.,
                    alpha(GOLD, if chosen { 0.13 } else { 0.06 }),
                );
            }
            s.line(
                left,
                top + 65.,
                left + 289.,
                top + 65.,
                1.,
                if chosen { GOLD } else { EDGE },
            );
            product(s, product_id(r, state, p), left + 21., top + 43., 0.96);
            s.bold().text(r.name, left + 59., top + 26., 17., PAPER);
            s.text(
                if r.lock.is_none() {
                    "Готово к работе"
                } else {
                    "Недоступно"
                },
                left + 59.,
                top + 49.,
                13.,
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
        s.bold().text(r.name, dx, y + 228., 25., PAPER);
        product(s, product_id(r, state, p), dx + 32., y + 299., 2.9);
        let result = changes(r, state, p);
        let stats = dx + 158.;
        let stat_end = right;
        for (i, change) in result.iter().enumerate() {
            let by = y + 258. + i as f32 * 23.;
            s.text(change.label, stats, by, 14., MUTED);
            s.right(&change.to, stat_end, by, 15., GOLD);
            let after_x = stat_end - s.measure(&change.to, 15.) - 12.;
            s.right("→", after_x, by, 15., MUTED);
            s.right(&change.from, after_x - 23., by, 14., PAPER);
        }
        s.text(r.place, dx, y + 344., 14., MUTED);
        s.line(dx, y + 355., right, y + 355., 1., EDGE);
        let cost_label = if self.tab == 0 {
            "После установки"
        } else {
            "Расход из рюкзака"
        };
        s.bold().text(cost_label, dx, y + 380., 16., PAPER);
        let mut item_x = dx + 11.;
        for (key, n) in &r.cost {
            let have = count(p, key, 0);
            let tone = if have >= *n { PAPER } else { RED_TEAM };
            stock_mark(s, key, item_x, y + 402., tone);
            s.text(material(key), item_x + 16., y + 406., 14., tone);
            let result = if have >= *n {
                format!("{have} → {}", have - n)
            } else {
                format!("{have} / {n} нужно")
            };
            s.bold().text(&result, item_x + 16., y + 426., 15., tone);
            item_x += 160.;
        }
        if r.cost.is_empty() {
            s.text("Без расхода материалов", dx, y + 410., 14., MUTED);
        }
        if let Some(why) = &r.lock {
            s.wrap(why, dx, y + 452., right - dx, 14., RED_TEAM);
        }
        if r.lock.is_none() && s.over(dx, y + 235., 140., 92.) {
            s.wrap(&r.outcome, dx, y + 452., right - dx, 13., MUTED);
        }
        let by = y + 496.;
        if r.lock.is_none() {
            if s.button(dx, by, 198., 45., r.button, true) {
                return (Action::Craft(r.id.to_owned()), self.tab == 0);
            }
        } else {
            s.disabled(dx, by, 198., 45., r.button);
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
    fn pending_receipt_ignores_previous_notice_and_detects_repeat_ack() {
        let mut p = json!({"message":"Граната готова","messageT":1.2});
        let mut receipt = Receipt::pending(&p, 1.);
        assert_eq!(receipt.message(&p, 1.05), None);
        p["messageT"] = json!(1.1);
        assert_eq!(receipt.message(&p, 1.1), None);
        p["messageT"] = json!(2.5);
        assert_eq!(receipt.message(&p, 1.3), Some("Граната готова"));
        p["message"] = json!("Другое событие");
        assert_eq!(receipt.message(&p, 1.4), None);
    }
    #[test]
    fn result_preview_respects_partial_ammo_and_refinery_space() {
        let p = json!({"team":"blue","vehicleId":2,"fuel":30,"x":0,"z":0,"powder":5,"metal":4});
        let world = json!({"vehicles":[{"id":2,"kind":"apc","team":"blue","shellCapacity":12,
            "shells":11,"mgCapacity":100,"mgAmmo":99,"x":0,"z":0}],
            "factories":[{"kind":"refinery","team":"blue","x":0,"z":0,"ready":6},
            {"kind":"workshop","team":"blue","x":0,"z":0}]});
        let recipes = catalogue(1, &world, &p);
        let ammo = changes(&recipes[1], &world, &p);
        assert_eq!((&ammo[0].from[..], &ammo[0].to[..]), ("11", "12"));
        assert_eq!((&ammo[1].from[..], &ammo[1].to[..]), ("99", "100"));
        let fuel = changes(&recipes[0], &world, &p);
        assert_eq!((&fuel[0].from[..], &fuel[0].to[..]), ("30", "32"));
        assert_eq!((&fuel[1].from[..], &fuel[1].to[..]), ("6", "4"));
    }
    #[test]
    fn workshop_picture_follows_vehicle_and_equipment_choice() {
        let pilot = json!({"vehicleId":1,"team":"blue","upgradeChoice":"utility"});
        let mut state = json!({"vehicles":[{"id":1,"team":"blue","kind":"mbt","mgCapacity":90}]});
        let upgrade = &catalogue(2, &state, &pilot)[1];
        let ammo = &catalogue(1, &state, &pilot)[1];
        assert_eq!(product_id(upgrade, &state, &pilot), "armor");
        assert_eq!(product_id(ammo, &state, &pilot), "shells");
        state["vehicles"][0]["kind"] = json!("apc");
        assert_eq!(product_id(upgrade, &state, &pilot), "cargo_rack");
        assert_eq!(product_id(ammo, &state, &pilot), "belt");
        state["vehicles"][0]["kind"] = json!("scout_buggy");
        assert_eq!(product_id(upgrade, &state, &pilot), "aux_fuel");
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
