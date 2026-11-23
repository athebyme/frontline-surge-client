//! In-world sight markings, kit silhouettes and the match roster.
use super::*;
const WHITE_HUD: Color = PAPER;
const DIM: Color = Color::new(0.76, 0.78, 0.72, 1.);
const SKY_BLUE: Color = BLUE_TEAM;
const CORAL: Color = RED_TEAM;
const AMBER: Color = GOLD;
const PHOSPHOR: Color = Color::new(0.67, 0.81, 0.66, 1.);
const SHADOW: Color = Color::new(0.026, 0.035, 0.030, 0.93);

impl Surface<'_> {
    pub(super) fn ink(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.text(text, x + 0.7, y + 1.1, size, SHADOW);
        self.text(text, x, y, size, color);
    }
    pub(super) fn ink_right(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.ink(text, x - self.measure(text, size), y, size, color);
    }
    pub(super) fn ink_mid(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        self.ink(text, x - self.measure(text, size) * 0.5, y, size, color);
    }
    fn stroke(&self, x: f32, y: f32, bx: f32, by: f32, weight: f32, color: Color) {
        self.line(x, y, bx, by, weight + 2.1, SHADOW);
        self.line(x, y, bx, by, weight, color);
    }
    fn face(&self, pts: &[(f32, f32)], x: f32, y: f32, k: f32, c: Color) {
        let xy = |p: (f32, f32)| vec2((x + p.0 * k) * self.scale, (y + p.1 * k) * self.scale);
        for i in 1..pts.len() - 1 {
            draw_triangle(xy(pts[0]), xy(pts[i]), xy(pts[i + 1]), c);
        }
    }
}
#[derive(Clone, Copy)]
enum Mark {
    Cross,
    Bolt,
    Grenade,
    Shell,
    Hull,
    Armor,
    Carrier,
    Truck,
    Buggy,
    Patrol,
    Collision,
    MachineGun,
    Burst,
    Pulse,
    Unknown,
    Fuel,
    Battery,
    Bag,
    Rifle,
    Carbine,
    Smg,
}
fn icon(s: &Surface, mark: Mark, x: f32, y: f32, k: f32, c: Color) {
    for (d, color) in [(0.8, SHADOW), (0., c)] {
        let face = |points: &[(f32, f32)]| s.face(points, x + d, y + d, k, color);
        let bar = |px: f32, py: f32, w: f32, h: f32| {
            s.rect(x + d + px * k, y + d + py * k, w * k, h * k, color)
        };
        let cut = |ax: f32, ay: f32, bx: f32, by: f32, width: f32| {
            s.line(x + d + ax * k, y + d + ay * k, x + d + bx * k, y + d + by * k, width * k, INK);
        };
        match mark {
            Mark::Cross => {
                bar(5., 0., 5., 15.);
                bar(0., 5., 15., 5.);
            }
            Mark::Bolt => {
                face(&[(5., 0.), (12., 0.), (7., 6.), (1., 9.)]);
                face(&[(5., 6.), (12., 6.), (2., 16.)]);
            }
            Mark::Grenade => {
                face(&[
                    (3., 6.),
                    (9., 5.),
                    (13., 10.),
                    (11., 15.),
                    (5., 16.),
                    (1., 12.),
                ]);
                bar(5., 2., 4., 3.);
                bar(6., 0., 6., 2.);
                face(&[(11., 0.), (13., 1.), (16., 8.), (14., 8.)]);
            }
            Mark::Shell => {
                let w = 7.;
                face(&[(0., 6.), (w * 0.5, 0.), (w, 6.), (w, 10.), (0., 10.)]);
                bar(0., 11., w, 5.);
            }
            Mark::Hull => {
                // Low turret, long tube and a single track run survive at feed size.
                face(&[(0., 7.3), (14., 7.3), (20., 9.), (16.4, 11.4), (2.2, 11.4)]);
                face(&[(3., 6.8), (4.5, 3.8), (8.5, 2.8), (11.8, 3.), (14.3, 5.8), (12., 6.8)]);
                bar(12., 4., 12., 1.15);
                face(&[(0.7, 11.6), (18.8, 11.6), (15.7, 14.), (3., 14.)]);
                for axle in [4., 7.5, 11., 14.5] {
                    cut(axle, 12.2, axle + 0.75, 12.2, 1.);
                }
            }
            Mark::Armor => {
                face(&[(0., 2.5), (8., 0.), (16., 2.5), (14., 9.), (8., 14.), (2., 9.)]);
                cut(3., 4., 8., 2.6, 1.);
                cut(8., 2.6, 13., 4., 1.);
                cut(8., 3., 8., 10.7, 1.);
            }
            Mark::Carrier | Mark::Truck | Mark::Patrol | Mark::Buggy => {
                if matches!(mark, Mark::Carrier) {
                    face(&[
                        (0., 6.),
                        (4., 3.),
                        (16., 3.),
                        (22., 7.),
                        (21., 10.),
                        (0., 10.),
                    ]);
                    bar(8., 1., 5., 2.);
                    bar(12., 1., 6., 1.);
                } else if matches!(mark, Mark::Truck) {
                    bar(0., 2., 12., 7.);
                    bar(12., 8., 9., 2.);
                    face(&[(14., 2.), (18., 2.), (21., 5.), (21., 8.), (14., 8.)]);
                } else if matches!(mark, Mark::Buggy) {
                    face(&[
                        (0., 6.),
                        (5., 6.),
                        (9., 8.),
                        (15., 6.),
                        (20., 7.),
                        (20., 10.),
                        (0., 10.),
                    ]);
                    bar(7., 3., 1.5, 5.);
                    bar(7., 2., 6., 1.5);
                    face(&[(12., 2.), (14., 3.), (17., 7.), (15., 7.)]);
                } else {
                    face(&[
                        (0., 4.),
                        (4., 2.),
                        (13., 2.),
                        (16., 6.),
                        (20., 7.),
                        (20., 10.),
                        (0., 10.),
                    ]);
                }
                let wheels: &[f32] = if matches!(mark, Mark::Carrier | Mark::Truck) {
                    &[3., 9.5, 16.]
                } else {
                    &[3., 16.]
                };
                for &cx in wheels {
                    face(&[
                        (cx - 2.4, 10.),
                        (cx + 2.4, 10.),
                        (cx + 2.4, 12.),
                        (cx + 1., 14.),
                        (cx - 1., 14.),
                        (cx - 2.4, 12.),
                    ]);
                }
            }
            Mark::Collision => {
                face(&[
                    (1., 3.),
                    (5., 5.),
                    (8., 1.),
                    (8., 6.),
                    (14., 5.),
                    (10., 9.),
                    (13., 13.),
                    (8., 12.),
                    (6., 16.),
                    (5., 11.),
                    (0., 13.),
                    (3., 8.),
                ]);
            }
            Mark::MachineGun => {
                face(&[(0., 6.8), (7., 5.5), (8.5, 7.), (3.1, 8.7), (0.4, 11.)]);
                face(&[(6.7, 5.), (9., 3.8), (16., 3.8), (18., 5.), (17., 8.3), (7., 8.3)]);
                bar(16., 4.9, 7.8, 2.6);
                bar(22., 5.2, 8., 1.2);
                bar(27.6, 4.6, 2.4, 1.9);
                face(&[(7.6, 8.), (10., 8.), (8., 12.6), (6.6, 12.2)]);
                face(&[(11., 8.), (15., 8.), (15.6, 12.6), (11.7, 13.2)]);
                cut(11., 5.4, 15.8, 5.4, 0.9);
            }
            Mark::Burst => {
                for (x, y) in [(0., 0.), (4., 5.), (0., 10.)] {
                    face(&[
                        (x, y),
                        (x + 9., y),
                        (x + 13., y + 2.),
                        (x + 9., y + 3.5),
                        (x, y + 3.5),
                    ]);
                }
            }
            Mark::Pulse => {
                for x in [0., 13.] {
                    bar(x, 1., 8., 2.);
                    bar(x + 3., 3., 2., 3.);
                }
                face(&[(2., 6.), (19., 6.), (15., 10.), (7., 10.)]);
                face(&[
                    (9., 11.),
                    (14., 11.),
                    (11., 15.),
                    (13., 15.),
                    (7., 20.),
                    (9., 15.),
                    (7., 15.),
                ]);
            }
            Mark::Unknown => {
                bar(3., 1., 9., 2.5);
                bar(10., 3., 3., 4.);
                bar(6., 6., 5., 2.5);
                bar(6., 8., 2.5, 3.);
                bar(6., 13., 2.8, 2.8);
            }
            Mark::Fuel => {
                face(&[
                    (1., 4.),
                    (5., 1.),
                    (10., 1.),
                    (10., 4.),
                    (13., 5.),
                    (13., 15.),
                    (1., 15.),
                ]);
                bar(2., 0., 2., 2.);
            }
            Mark::Battery => {
                bar(0., 4., 2., 9.);
                bar(0., 3., 17., 2.);
                bar(0., 12., 17., 2.);
                bar(15., 3., 2., 10.);
                bar(18., 6., 2., 5.);
            }
            Mark::Bag => {
                bar(2., 5., 13., 10.);
                bar(5., 2., 7., 2.);
                bar(0., 8., 1., 6.);
                bar(16., 8., 1., 6.);
            }
            Mark::Rifle | Mark::Carbine | Mark::Smg => {
                let (muzzle, receiver, stock) = match mark {
                    Mark::Rifle => (50., 28., 9.),
                    Mark::Carbine => (39., 28., 9.),
                    _ => (31., 22., 7.),
                };
                face(&[(0., 6.), (stock, 7.), (stock + 1., 10.), (1., 14.)]);
                bar(stock, 5., receiver - stock, 6.);
                bar(receiver, 6., muzzle - receiver, 2.);
                face(&[(11., 10.), (15., 10.), (13., 16.), (10., 16.)]);
                if matches!(mark, Mark::Smg) {
                    bar(18., 9., 3., 8.);
                } else {
                    face(&[(18., 10.), (22., 10.), (24., 15.), (20., 17.)]);
                }
                if matches!(mark, Mark::Rifle) {
                    bar(11., 1., 9., 2.);
                    bar(13., 3., 2., 2.);
                    bar(18., 3., 2., 2.);
                } else {
                    bar(12., 3., 6., 2.);
                }
                bar(muzzle - 1., 5., 3., 3.);
            }
        }
    }
}
fn side(team: &str) -> Color {
    if team == "red" {
        CORAL
    } else {
        SKY_BLUE
    }
}
fn flag_mark(s: &Surface, x: f32, y: f32, k: f32, team: &str) {
    let c = side(team);
    if team == "red" {
        s.face(
            &[(0., 5.), (7., 0.), (14., 5.), (7., 10.)],
            x + 0.7,
            y + 1.,
            k,
            SHADOW,
        );
        s.face(&[(0., 5.), (7., 0.), (14., 5.), (7., 10.)], x, y, k, c);
        s.line(
            x + 4. * k,
            y + 5. * k,
            x + 10. * k,
            y + 5. * k,
            1.4 * k,
            INK,
        );
    } else {
        for n in [0., 5.] {
            s.face(
                &[(0., 0.), (6., 3.), (12., 0.), (12., 3.), (6., 6.), (0., 3.)],
                x,
                y + n * k,
                k,
                c,
            );
        }
    }
}
fn strip(s: &Surface, x: f32, y: f32, w: f32, ratio: f32, n: usize, c: Color) {
    let seg = w / n as f32;
    for i in 0..n {
        let dx = x + i as f32 * seg;
        s.rect(dx - 0.5, y - 0.5, seg - 1., 4., SHADOW);
        s.rect(dx, y, seg - 2., 3., alpha(DIM, 0.42));
        let fill = (ratio * n as f32 - i as f32).clamp(0., 1.);
        if fill > 0. {
            s.rect(dx, y, (seg - 2.) * fill, 3., c);
        }
    }
}
pub(super) fn shade(s: &Surface) {
    // Continuous falloff protects white type over the sky and light terrain without enclosing HUD groups.
    let mut mesh = Mesh {
        vertices: Vec::with_capacity(8),
        indices: vec![0, 1, 2, 1, 3, 2, 4, 5, 6, 5, 7, 6],
        texture: None,
    };
    for (y, a) in [(0., 0.28), (75., 0.), (s.h - 160., 0.), (s.h, 0.60)] {
        for x in [0., s.w] {
            mesh.vertices.push(Vertex::new(
                x * s.scale,
                y * s.scale,
                0.,
                0.,
                0.,
                alpha(SHADOW, a),
            ));
        }
    }
    draw_mesh(&mesh);
}
pub(super) fn match_score(s: &Surface, state: &Value, p: &Value) {
    let t = string(p, "team", "blue");
    let op = if t == "blue" { "red" } else { "blue" };
    let scores = &state["scores"];
    flag_mark(s, 24., 25., 1.05, t);
    let my = count(scores, t, 0).to_string();
    s.ink(&my, 45., 39., 30., side(t));
    let divider = 50. + s.measure(&my, 30.);
    s.stroke(divider + 3., 24., divider + 3., 37., 1., DIM);
    let their = count(scores, op, 0).to_string();
    s.ink(&their, divider + 13., 39., 30., side(op));
    let end = divider + 19. + s.measure(&their, 30.);
    flag_mark(s, end, 25., 0.85, op);
    s.ink(
        &clock(number(state, "remaining", 420.)),
        end + 28.,
        37.,
        19.,
        WHITE_HUD,
    );
    let n = state["players"].as_array().map_or(0, Vec::len);
    let cap = count(state, "fragLimit", 0);
    let brief = if n < 2 {
        "Ждём соперника".into()
    } else if cap > 0 {
        format!("До {cap} фрагов")
    } else {
        format!("В бою {n}")
    };
    s.ink(&brief, 25., 56., 12., DIM);
}
pub(super) fn compass(s: &Surface, yaw: f32) {
    let degrees = yaw.to_degrees();
    let center = s.w * 0.5;
    let bearing = (degrees.round() as i32).rem_euclid(360);
    s.bold().ink_mid(&format!("{bearing:03}°"), center, 25., 19., WHITE_HUD);
    let pos = |d: f32| center + (d - degrees) * 3.1;
    for n in ((degrees - 54.) / 5.).ceil() as i32..=((degrees + 54.) / 5.).floor() as i32 {
        let d = n * 5;
        let a = d.rem_euclid(360);
        let x = pos(d as f32);
        let card = a % 45 == 0;
        s.stroke(
            x,
            34.,
            x,
            if card { 41. } else { 37. },
            if card { 1.8 } else { 1. },
            if card { WHITE_HUD } else { DIM },
        );
        if card || a % 15 == 0 {
            let label = if card {
                ["С", "СВ", "В", "ЮВ", "Ю", "ЮЗ", "З", "СЗ"][(a / 45) as usize].into()
            } else {
                a.to_string()
            };
            s.ink_mid(
                &label,
                x,
                if card { 54. } else { 51. },
                if card { 14. } else { 10. },
                if card { WHITE_HUD } else { DIM },
            );
        }
    }
    s.face(&[(0., 0.), (8., 0.), (4., 4.)], center - 4., 27., 1., AMBER);
}
pub(super) fn soldier(s: &Surface, p: &Value, vehicle: Option<&Value>, gathering: bool) {
    let x = 25.;
    let base = s.h - 29.;
    let hp = count(p, "hp", 100).clamp(0, 100);
    let hp_color = if hp <= 30 { CORAL } else { WHITE_HUD };
    icon(s, Mark::Cross, x, base - 31., 1.15, hp_color);
    s.bold().ink(&hp.to_string(), x + 27., base - 12., 34., hp_color);
    let vit = x + 100.;
    strip(s, vit, base - 27., 81., hp as f32 / 100., 5, hp_color);
    icon(s, Mark::Bolt, vit, base - 17., 0.66, AMBER);
    strip(
        s,
        vit + 14.,
        base - 12.,
        67.,
        number(p, "stamina", 100.) as f32 / 100.,
        5,
        AMBER,
    );
    let name = s.shorten(string(p, "name", "Боец"), 14., 168.);
    flag_mark(s, x, base - 58., 0.7, string(p, "team", "blue"));
    s.ink(&name, x + 15., base - 49., 14., WHITE_HUD);
    if let Some(v) = vehicle {
        let cap = matches!(string(v, "kind", ""), "mbt" | "apc");
        let course = if cap {
            format!(
                "Курс {}   Башня {}",
                heading_label(v, "yaw"),
                heading_label(v, "turretYaw")
            )
        } else {
            format!("Курс {}", heading_label(v, "yaw"))
        };
        s.ink(&course, x, base + 5., 13., DIM);
    } else {
        let mut left = x;
        for (key, label) in [("I", "Рюкзак"), ("K", "Крафт"), ("B", "Стройка")] {
            left += s.keycap(key, left, base + 6.) + 5.;
            s.ink(label, left, base + 6., 12., DIM);
            left += s.measure(label, 12.) + 13.;
        }
        if gathering {
            let mut xx = x;
            for (i, (key, label)) in MATERIALS.into_iter().enumerate() {
                let num = count(p, key, 0);
                if num == 0 && !flag(p, "buildMode", false) {
                    continue;
                }
                let text = format!("{label} {num}");
                super::stash_ui::material_symbol(s, i, xx + 7., base - 81., DIM);
                s.ink(
                    &text,
                    xx + 21.,
                    base - 76.,
                    12.,
                    if num > 0 { WHITE_HUD } else { DIM },
                );
                xx += 21. + s.measure(&text, 12.) + 12.;
            }
        }
    }
}
pub(super) fn kit(s: &Surface, p: &Value) {
    let r = s.w - 25.;
    let b = s.h - 26.;
    let (name, gun) = match string(p, "weapon", "mr7") {
        "ar4" => ("AR-4", Mark::Carbine),
        "vx9" => ("VX-9", Mark::Smg),
        _ => ("MR-7", Mark::Rifle),
    };
    let mode = crafting::mode_name(string(p, "fireMode", "semi"));
    let head = format!("{name}   /   {mode}");
    s.ink_right(&head, r, b - 66., 16., WHITE_HUD);
    icon(s, gun, r - s.measure(&head, 16.) - 63., b - 79., 0.96, DIM);
    let ammo = count(p, "ammo", 0);
    let reserve = count(p, "reserve", 0);
    let reserve_text = reserve.to_string();
    let div = r - s.measure(&reserve_text, 21.) - 11.;
    let active = if ammo == 0 { CORAL } else { WHITE_HUD };
    s.ink_right(&reserve_text, r, b - 25., 21., DIM);
    s.stroke(div - 4., b - 22., div + 2., b - 42., 2., DIM);
    s.bold().ink_right(&ammo.to_string(), div - 12., b - 22., 42., active);
    let reload = number(p, "reload", 0.);
    if reload > 0. {
        let ratio = (1. - reload / number(p, "reloadTime", 2.).max(0.01)).clamp(0., 1.);
        s.ink_right("Перезарядка", r, b - 3., 14., AMBER);
        strip(s, r - 83., b + 2., 83., ratio as f32, 8, AMBER);
    } else if ammo == 0 && reserve > 0 {
        s.ink_right("R  Перезарядить", r, b, 14., AMBER);
    } else {
        let grenades = count(p, "grenades", 0);
        icon(s, Mark::Grenade, r - 58., b - 10., 0.76, DIM);
        s.ink_right(&format!("H  ×{grenades}"), r, b + 1., 13., DIM);
        let cd = count(p, "droneCooldown", 0);
        let tools = if !p["deployedDroneId"].is_null() {
            "J  Камера   Shift+J  Отозвать".into()
        } else if cd > 0 {
            format!("M  Карта   J  {cd} с")
        } else {
            "M  Карта   J  Дрон".into()
        };
        s.ink_right(&tools, r - 76., b + 1., 12., DIM);
    }
}
pub(super) fn vehicle(s: &Surface, v: &Value) {
    let r = s.w - 25.;
    let b = s.h - 26.;
    let kind = string(v, "kind", "");
    let speed = (number(v, "speed", 0.).abs() * 3.6).round() as i32;
    let hull = number(v, "hp", 0.);
    let fuel = number(v, "fuel", 0.);
    let max_h = number(v, "maxHp", 100.).max(1.);
    let max_f = number(v, "maxFuel", 100.).max(1.);
    let hx = r - 256.;
    let fx = r - 118.;
    icon(s, Mark::Armor, hx, b - 43., 0.9, WHITE_HUD);
    icon(s, Mark::Fuel, fx, b - 46., 0.9, AMBER);
    for (xx, val, max, color, space) in [
        (hx + 25., hull, max_h, WHITE_HUD, 99.),
        (fx + 20., fuel, max_f, AMBER, 98.),
    ] {
        let ratio = (val / max).clamp(0., 1.);
        s.ink(
            &format!("{} / {}", val.ceil() as i32, max.ceil() as i32),
            xx,
            b - 32.,
            15.,
            if ratio < 0.21 { CORAL } else { color },
        );
        strip(s, xx, b - 24., space, ratio as f32, 6, color);
    }
    s.ink(vehicle_name(kind), hx, b - 103., 18., WHITE_HUD);
    s.bold().ink_right(&format!("{speed} км/ч"), r, b - 102., 21., WHITE_HUD);
    let mut row = b - 65.;
    if matches!(kind, "mbt" | "apc") {
        let main = kind == "mbt" || count(v, "shellCapacity", 0) > 0;
        if main {
            let shell = if kind == "apc" {
                Mark::Burst
            } else {
                Mark::Shell
            };
            let cycle = if kind == "apc" { frontline_surge::vehicle_gun::APC_RELOAD }
                else { frontline_surge::vehicle_gun::TANK_RELOAD };
            let remaining = number(v, "gunRemaining", 0.);
            if remaining > 0. {
                strip(s, hx + 24., row + 5., 54., (1. - remaining / cycle).clamp(0., 1.), 6, AMBER);
            }
            icon(s, shell, hx, row - 15., 0.94, WHITE_HUD);
            s.ink(
                "ЛКМ",
                hx + if kind == "apc" { 24. } else { 16. },
                row,
                14.,
                DIM,
            );
            s.ink_right(
                &count(v, "shells", 0).to_string(),
                fx - 16.,
                row,
                19.,
                WHITE_HUD,
            );
        }
        let xx = if main { fx } else { hx };
        icon(s, Mark::MachineGun, xx, row - 13., 0.84, WHITE_HUD);
        s.ink(if main { "ПКМ" } else { "ЛКМ" }, xx + 31., row, 14., DIM);
        s.ink_right(
            &count(v, "mgAmmo", 0).to_string(),
            if main { r } else { hx + 121. },
            row,
            19.,
            WHITE_HUD,
        );
        row -= 20.;
    }
    let capacity = count(v, "cargoCapacity", 0);
    if capacity > 0 {
        let cargo = &v["cargo"];
        let units = MATERIALS
            .iter()
            .map(|(key, _)| count(cargo, key, 0))
            .sum::<i32>();
        let goods = MATERIALS
            .iter()
            .filter(|(key, _)| count(cargo, key, 0) > 0)
            .map(|(key, n)| format!("{n} ×{}", count(cargo, key, 0)))
            .collect::<Vec<_>>()
            .join("  ");
        icon(s, Mark::Bag, hx, row - 12., 0.68, DIM);
        s.ink(
            &format!("T  Груз {units}/{capacity}"),
            hx + 17.,
            row,
            14.,
            DIM,
        );
        s.ink_right(&s.shorten(&goods, 12., 102.), r, row, 12., DIM);
        row -= 19.;
    }
    let rank = match count(v, "upgradeLevel", 0) {
        1 => " I",
        2 => " II",
        _ => "",
    };
    // Upgrades sit above the vehicle title when there are two cargo/weapon lines.
    let kit_y = if row >= b - 85. { b - 84. } else { b - 124. };
    s.ink(
        &format!("Комплект · {}{rank}", kit_name(string(v, "upgrade", ""))),
        hx,
        kit_y,
        12.,
        DIM,
    );
    s.ink_right(
        "Y  Вид    Колесо  Зум    M  Карта    F  Выйти",
        r,
        b + 4.,
        12.,
        DIM,
    );
}
pub(super) fn drone(
    s: &Surface,
    state: &Value,
    p: &Value,
    d: &Value,
    zoom: bool,
    camera: &Camera3D,
) {
    let cx = s.w * 0.5;
    let cy = s.h * 0.5;
    let l = 28.;
    let r = s.w - 28.;
    let b = s.h - 28.;
    let (yaw, pitch) = crate::motion::view_angles(camera.target - camera.position);
    compass(s, yaw);
    s.ink("FPV", l, 42., 26., WHITE_HUD);
    s.ink(if zoom { "×2" } else { "×1" }, l + 53., 42., 17., PHOSPHOR);
    s.ink("РАЗВЕДКА", l, 59., 12., PHOSPHOR);
    let launch = number(d, "launchRemaining", 0.);
    if launch > 0. {
        s.ink(&format!("Запуск   {:.1} с", launch), l, 80., 15., PHOSPHOR);
    }
    let signal = (number(d, "signal", 1.).clamp(0., 1.) * 100.).round() as i32;
    for i in 0..5 {
        let x = r - 95. + i as f32 * 6.;
        let h = 4. + i as f32 * 3.;
        s.stroke(
            x,
            40.,
            x,
            40. - h,
            3.,
            if i * 20 < signal {
                PHOSPHOR
            } else {
                alpha(DIM, 0.38)
            },
        );
    }
    s.ink_right(
        &format!("{signal}%"),
        r,
        41.,
        21.,
        if signal < 25 { CORAL } else { PHOSPHOR },
    );
    s.ink_right("СИГНАЛ", r, 58., 12., DIM);
    let forward = (camera.target - camera.position).normalize_or_zero();
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    let roll = camera
        .up
        .dot(right)
        .atan2(camera.up.dot(right.cross(forward)));
    let horizon = cy + pitch * 54.;
    let (sin, cos) = (-roll).sin_cos();
    for sign in [-1., 1.] {
        for off in [38., 57.] {
            let end = off + if off == 38. { 13. } else { 7. };
            s.stroke(
                cx + sign * off * cos,
                horizon + sign * off * sin,
                cx + sign * end * cos,
                horizon + sign * end * sin,
                1.2,
                PHOSPHOR,
            );
        }
        let dx = cx + sign * 13.;
        s.stroke(dx, cy - 5., dx, cy + 5., 1.4, PHOSPHOR);
        s.stroke(dx, cy - 5., dx - sign * 4., cy - 5., 1.4, PHOSPHOR);
    }
    s.rect(cx - 1., cy - 1., 2., 2., PHOSPHOR);
    let alt = number(d, "y", 0.).max(0.);
    let speed = number(d, "speed", 0.).max(0.);
    let climb = number(d, "vSpeed", 0.);
    let range =
        (number(d, "x", 0.) - number(p, "x", 0.)).hypot(number(d, "z", 0.) - number(p, "z", 0.));
    let battery = number(d, "battery", 0.).clamp(0., 60.);
    let bat = if battery < 12. { CORAL } else { PHOSPHOR };
    icon(s, Mark::Battery, l, b - 41., 1.22, bat);
    s.ink(&clock(battery), l + 34., b - 24., 27., bat);
    strip(s, l, b - 13., 105., (battery / 60.) as f32, 8, bat);
    s.ink(
        &format!("Высота {alt:0.0} м     Удаление {range:0.0} м"),
        l,
        b - 68.,
        15.,
        WHITE_HUD,
    );
    s.ink(
        &format!("Ход {speed:0.0} м/с     Набор {climb:+0.0} м/с"),
        l,
        b - 89.,
        14.,
        DIM,
    );
    s.ink("Shift  Вверх     ПКМ  Вниз", l, b + 7., 12., DIM);
    let newest = state["markers"].as_array().into_iter().flatten().find(|m| {
        m["team"] == p["team"] && m["owner"] == p["id"] && number(m, "remaining", 0.) > 0.
    });
    let marker = newest.map_or_else(
        || "ЛКМ  Передать метку".into(),
        |v| format!("Метка активна   {} с", count(v, "remaining", 0)),
    );
    s.ink_right(&marker, r, b - 46., 19., WHITE_HUD);
    let charges = count(d, "charges", 0);
    let pulse = if charges == 0 {
        "Импульс израсходован"
    } else if battery < 14. {
        "Мало энергии на импульс"
    } else {
        "X  Импульс   −14 с"
    };
    s.ink_right(
        pulse,
        r,
        b - 24.,
        15.,
        if charges > 0 && battery >= 14. {
            PHOSPHOR
        } else {
            DIM
        },
    );
    s.ink_right(
        "Z  Зум   M  Карта   J  Зависание   Shift+J  Отозвать",
        r,
        b + 7.,
        12.,
        DIM,
    );
}
fn rifle_mark(id: &str) -> Mark {
    match id {
        "vx9" => Mark::Smg,
        "ar4" => Mark::Carbine,
        _ => Mark::Rifle,
    }
}
pub(super) fn weapon_picture(s: &Surface, id: &str, x: f32, y: f32, scale: f32, c: Color) {
    icon(s, rifle_mark(id), x, y, scale, c);
}
fn vehicle_mark(id: &str) -> Mark {
    match id {
        "apc" => Mark::Carrier,
        "cargo6x6" => Mark::Truck,
        "patrol4x4" => Mark::Patrol,
        "scout_buggy" => Mark::Buggy,
        _ => Mark::Hull,
    }
}
#[derive(Clone, Copy)]
struct EventMark {
    platform: Option<Mark>,
    tool: Mark,
    width: f32,
}
impl EventMark {
    fn new(w: &str, v: &str) -> Self {
        let (platform, tool, width) = match w {
            "mr7" => (None, Mark::Rifle, 52.),
            "vx9" => (None, Mark::Smg, 33.),
            "ar4" => (None, Mark::Carbine, 41.),
            "grenade" => (None, Mark::Grenade, 17.),
            "drone_pulse" => (None, Mark::Pulse, 21.),
            "tank" => (Some(Mark::Hull), Mark::Shell, 8.),
            "apc_he" => (Some(Mark::Carrier), Mark::Burst, 17.),
            "vehicle_mg" => (
                if v.is_empty() {
                    None
                } else {
                    Some(vehicle_mark(v))
                },
                Mark::MachineGun,
                30.,
            ),
            "ram" => (
                if v.is_empty() {
                    None
                } else {
                    Some(vehicle_mark(v))
                },
                Mark::Collision,
                15.,
            ),
            _ => (None, Mark::Unknown, 16.),
        };
        Self {
            platform,
            tool,
            width: width + if platform.is_some() { 29. } else { 0. },
        }
    }
    fn draw(self, s: &Surface, x: f32, y: f32, k: f32) {
        if let Some(platform) = self.platform {
            icon(s, platform, x, y + 1.5 * k, k, DIM);
            s.stroke(x + 24. * k, y + 13. * k, x + 26. * k, y + 3. * k, k, AMBER);
            icon(s, self.tool, x + 29. * k, y, k, WHITE_HUD);
        } else {
            icon(s, self.tool, x, y, k, WHITE_HUD);
        }
    }
}
pub(super) fn feed(s: &Surface, state: &Value, top: f32) {
    let t = number(state, "elapsed", 0.);
    let max = (s.w * 0.5 - 200.).min(390.);
    for (i, e) in state["events"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .filter(|e| e["type"] == "kill" && t - number(e, "at", t) < 5.5)
        .take(3)
        .enumerate()
    {
        let glyph = EventMark::new(string(e, "weapon", ""), string(e, "vehicleKind", ""));
        let k = if s.scale < 0.9 { 1.02 } else { 0.94 };
        let iw = glyph.width * k;
        let name_w = ((max - iw - 25.) * 0.5).clamp(42., 132.);
        let a = s.shorten(string(e, "actorName", "Боец"), 15., name_w);
        let b = s.shorten(string(e, "targetName", "Боец"), 15., name_w);
        let right = s.w - 24.;
        let y = top + i as f32 * 25.;
        let team = string(e, "team", "blue");
        s.ink_right(
            &b,
            right,
            y,
            15.,
            side(if team == "blue" { "red" } else { "blue" }),
        );
        let x = right - s.measure(&b, 15.) - 9. - iw;
        glyph.draw(s, x, y - 13., k);
        s.ink_right(&a, x - 10., y, 15., side(team));
    }
}
pub(super) fn legend(s: &Surface, x: f32, y: f32) {
    let samples = [
        ("mr7", ""),
        ("vx9", ""),
        ("ar4", ""),
        ("grenade", ""),
        ("drone_pulse", ""),
        ("tank", "mbt"),
        ("apc_he", "apc"),
        ("vehicle_mg", "mbt"),
        ("vehicle_mg", "apc"),
        ("ram", "mbt"),
        ("ram", "apc"),
        ("ram", "cargo6x6"),
        ("ram", "patrol4x4"),
        ("ram", "scout_buggy"),
    ];
    for (i, (w, v)) in samples.into_iter().enumerate() {
        let xx = x + (i / 7) as f32 * 313.;
        let yy = y + (i % 7) as f32 * 31.;
        EventMark::new(w, v).draw(s, xx, yy - 13., 0.9);
        let e = serde_json::json!({"weapon":w,"vehicleKind":v});
        s.text(kill_source(&e), xx + 73., yy, 15., PAPER);
    }
}
pub(super) fn prompt(s: &Surface, text: &str, y: f32, color: Color, seated: bool) {
    let limit = if seated { s.w * 0.44 } else { s.w * 0.65 };
    let (key, label) = text
        .strip_prefix('[')
        .and_then(|t| t.split_once(']'))
        .filter(|(k, _)| k.chars().count() <= 4)
        .map_or((None, text), |(k, t)| (Some(k), t.trim_start()));
    let key_w = key.map_or(0., |k| s.keycap_width(k) + 6.);
    let label = s.shorten(label, 15., limit - key_w);
    let x = if seated {
        25.
    } else {
        (s.w - key_w - s.measure(&label, 15.)) * 0.5
    };
    if let Some(k) = key {
        s.keycap(k, x, y - 1.);
    }
    s.ink(&label, x + key_w, y, 15., color);
}
pub(super) fn scoreboard(s: &Surface, state: &Value, local: &Value) {
    let players = state["players"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let w = (s.w - 100.).min(1000.);
    let x = (s.w - w) * 0.5;
    let y = (s.h - 495.) * 0.5;
    let col = (w - 62.) * 0.5;
    s.rect(0., 0., s.w, s.h, alpha(INK, 0.88));
    s.ink("Командный бой", x, y + 35., 32., WHITE_HUD);
    s.ink_right(
        &clock(number(state, "remaining", 420.)),
        x + w,
        y + 35.,
        27.,
        WHITE_HUD,
    );
    let cap = count(state, "fragLimit", 0);
    let result = if cap > 0 {
        format!("До {cap} фрагов или до конца времени")
    } else {
        "Победа по числу фрагов к концу времени".into()
    };
    s.ink(&result, x, y + 59., 15., DIM);
    for (i, (team, title)) in [("blue", "Синие"), ("red", "Красные")]
        .into_iter()
        .enumerate()
    {
        let xx = x + i as f32 * (col + 62.);
        let tint = side(team);
        flag_mark(s, xx, y + 98., 1.6, team);
        s.ink(title, xx + 29., y + 114., 24., tint);
        s.ink_right(
            &count(&state["scores"], team, 0).to_string(),
            xx + col,
            y + 117.,
            37.,
            tint,
        );
        s.rect(xx, y + 127., col, 2., tint);
        let heads = [
            ("Очки", "points"),
            ("Фраги", "kills"),
            ("Пом.", "assists"),
            ("Смерти", "deaths"),
        ];
        let tab = |j: usize| xx + col - ((3 - j) as f32) * 49.;
        s.ink("Позывной", xx, y + 150., 13., DIM);
        for (j, (header, _)) in heads.iter().enumerate() {
            s.ink_right(header, tab(j), y + 150., 12., DIM);
        }
        let mut rows = players
            .iter()
            .filter(|p| p["team"] == team)
            .collect::<Vec<_>>();
        rows.sort_by_key(|v| {
            (
                std::cmp::Reverse(count(v, "points", 0)),
                std::cmp::Reverse(count(v, "kills", 0)),
                count(v, "deaths", 0),
            )
        });
        for (j, p) in rows.iter().take(12).enumerate() {
            let by = y + 176. + j as f32 * 23.;
            let me = p["id"] == local["id"];
            if me {
                s.face(
                    &[(0., 0.), (5., 4.), (0., 8.)],
                    xx - 12.,
                    by - 9.,
                    1.,
                    AMBER,
                );
            }
            s.ink(
                &s.shorten(string(p, "name", "Боец"), 15., col - 202.),
                xx,
                by,
                15.,
                if me { AMBER } else { WHITE_HUD },
            );
            for (n, (_, key)) in heads.iter().enumerate() {
                s.ink_right(
                    &count(p, key, 0).to_string(),
                    tab(n),
                    by,
                    15.,
                    if me { AMBER } else { WHITE_HUD },
                );
            }
        }
    }
    s.ink("Фраг   +100      Помощь   +50", x, y + 480., 14., DIM);
    s.ink_right("Tab   Вернуться в бой", x + w, y + 480., 14., DIM);
}
