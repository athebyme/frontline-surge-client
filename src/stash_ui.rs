//! Backpack, ground piles and selected-resource transfers.
use super::crafting::footprint;
use super::{
    count, flag, number, string, Action, Surface, BLUE_TEAM, EDGE, GOLD, INK, MATERIALS, MUTED,
    PAPER,
};
use serde_json::{json, Value};

const STACK: i32 = 32;
pub(super) struct Stash {
    resource: usize,
    quantity: i32,
    pile: usize,
}

fn nearby_truck<'a>(p: &Value, state: &'a Value) -> Option<&'a Value> {
    let seated = &p["vehicleId"];
    state["vehicles"]
        .as_array()?
        .iter()
        .filter(|v| {
            !flag(v, "destroyed", false)
                && v["team"] == p["team"]
                && count(v, "cargoCapacity", 0) > 0
                && (seated.is_null() || v["id"] == *seated)
        })
        .filter_map(|v| {
            let (w, d) = match string(v, "kind", "") {
                "apc" => (2.8, 5.45),
                _ => (2.36, 6.),
            };
            let d2 = footprint(p, v, number(v, "w", w), number(v, "d", d));
            (v["id"] == *seated || d2 <= 2.65 * 2.65).then_some((d2, v))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|v| v.1)
}
fn amount_for(asked: i32, available: i32, space: i32) -> i32 {
    available.max(0).min(space.max(0)).min(asked)
}
fn nearby_piles<'a>(state: &'a Value, p: &Value, resource: &str) -> Vec<&'a Value> {
    let mut list = state["groundDrops"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let dx = number(p, "x", 0.) - number(v, "x", 0.);
            let dz = number(p, "z", 0.) - number(v, "z", 0.);
            let distance = dx * dx + dz * dz;
            (v["resource"] == resource && count(v, "amount", 0) > 0 && distance <= 2.6 * 2.6)
                .then_some((distance, v))
        })
        .collect::<Vec<_>>();
    list.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| a.1["id"].as_u64().cmp(&b.1["id"].as_u64()))
    });
    list.into_iter().map(|v| v.1).collect()
}

impl Stash {
    pub(super) fn new() -> Self {
        Self {
            resource: 0,
            quantity: 1,
            pile: 0,
        }
    }
    pub(super) fn draw(&mut self, s: &Surface, state: &Value, p: &Value) -> (Action, bool) {
        let (x, y) = s.panel(912., 566., "СНАРЯЖЕНИЕ", "Инвентарь");
        if s.button(x + 765., y + 24., 118., 34., "Закрыть  I", false) {
            return (Action::None, true);
        }
        s.text(
            "До 32 единиц каждого материала",
            x + 28.,
            y + 116.,
            14.,
            MUTED,
        );
        let truck = nearby_truck(p, state);
        let bank = truck.map(|v| &v["cargo"]).unwrap_or(&Value::Null);
        let cargo_total: i32 = MATERIALS.iter().map(|(key, _)| count(bank, key, 0)).sum();
        let cargo_capacity = truck.map_or(0, |v| count(v, "cargoCapacity", 0));
        s.text("МАТЕРИАЛ", x + 42., y + 160., 11., MUTED);
        s.right("РЮКЗАК", x + 272., y + 160., 11., MUTED);
        s.right("КУЗОВ", x + 355., y + 160., 11., MUTED);
        for (i, (key, name)) in MATERIALS.iter().enumerate() {
            let ry = y + 172. + i as f32 * 48.;
            let selected = self.resource == i;
            s.rect(
                x + 28.,
                ry,
                343.,
                42.,
                if selected {
                    super::alpha(GOLD, 0.16)
                } else {
                    INK
                },
            );
            s.border(x + 28., ry, 343., 42., if selected { GOLD } else { EDGE });
            s.text(
                name,
                x + 42.,
                ry + 27.,
                16.,
                if selected { GOLD } else { PAPER },
            );
            s.right(
                &count(p, key, 0).to_string(),
                x + 272.,
                ry + 27.,
                17.,
                PAPER,
            );
            s.right(
                &truck.map_or_else(|| "—".to_owned(), |_| count(bank, key, 0).to_string()),
                x + 355.,
                ry + 27.,
                17.,
                MUTED,
            );
            if s.over(x + 28., ry, 343., 42.) && s.click && !selected {
                self.resource = i;
                self.pile = 0;
            }
        }
        s.text(
            &truck.map_or_else(
                || "Кузов вне доступа".into(),
                |v| {
                    format!(
                        "{}   ·   {cargo_total} / {cargo_capacity}",
                        super::vehicle_name(string(v, "kind", ""))
                    )
                },
            ),
            x + 28.,
            y + 441.,
            15.,
            if truck.is_some() { BLUE_TEAM } else { MUTED },
        );
        s.wrap(
            "Подойди к своему грузовому транспорту или открой рюкзак за рулём.",
            x + 28.,
            y + 472.,
            340.,
            13.,
            MUTED,
        );
        let dx = x + 403.;
        let (key, name) = MATERIALS[self.resource];
        s.text(name, dx, y + 162., 24., PAPER);
        s.text("КОЛИЧЕСТВО", dx, y + 194., 11., MUTED);
        for (i, (q, label)) in [(1, "1"), (5, "5"), (STACK, "Весь стак")]
            .iter()
            .enumerate()
        {
            let bx = dx + i as f32 * 105.;
            if s.button(bx, y + 205., 96., 35., label, false) {
                self.quantity = *q;
            }
            if *q == self.quantity {
                s.rect(bx, y + 238., 96., 2., GOLD);
            }
        }
        let piles = nearby_piles(state, p, key);
        self.pile = self.pile.min(piles.len().saturating_sub(1));
        let pile = piles.get(self.pile).copied();
        let held = count(p, key, 0);
        let stored = count(bank, key, 0);
        let foot = p["vehicleId"].is_null();
        let busy = if !flag(p, "alive", true) {
            Some("Дождись возрождения.")
        } else if !p["droneId"].is_null() {
            Some("Сначала отзови дрон.")
        } else {
            None
        };
        let actions = [
            (
                "drop",
                "Выбросить",
                held,
                STACK,
                if !foot {
                    Some("Выйди из машины.")
                } else if held == 0 {
                    Some("Нет в рюкзаке.")
                } else {
                    None
                },
            ),
            (
                "load",
                "В кузов",
                held,
                cargo_capacity - cargo_total,
                if truck.is_none() {
                    Some("Нужен свой транспорт рядом.")
                } else if held == 0 {
                    Some("Нет в рюкзаке.")
                } else if cargo_total >= cargo_capacity {
                    Some("Кузов полон.")
                } else {
                    None
                },
            ),
            (
                "unload",
                "Из кузова",
                stored,
                STACK - held,
                if truck.is_none() {
                    Some("Нужен свой транспорт рядом.")
                } else if stored == 0 {
                    Some("Нет в кузове.")
                } else if held >= STACK {
                    Some("Стак в рюкзаке полон.")
                } else {
                    None
                },
            ),
            (
                "pickup",
                "Подобрать",
                pile.map_or(0, |v| count(v, "amount", 0)),
                STACK - held,
                if !foot {
                    Some("Выйди из машины.")
                } else if pile.is_none() {
                    Some("Такого материала рядом нет.")
                } else if held >= STACK {
                    Some("Стак в рюкзаке полон.")
                } else {
                    None
                },
            ),
        ];
        for (i, (op, label, available, space, lock)) in actions.iter().enumerate() {
            let ry = y + 260. + i as f32 * 48.;
            let why = busy.or(*lock);
            let qty = amount_for(self.quantity, *available, *space);
            let caption = if why.is_none() {
                format!("{label}   {qty}")
            } else {
                label.to_string()
            };
            if let Some(reason) = why {
                s.disabled(dx, ry, 157., 37., &caption);
                s.text(reason, dx + 169., ry + 24., 13., MUTED);
            } else {
                if s.button(dx, ry, 157., 37., &caption, false) {
                    let mut body = json!({"action":op,"resource":key,"amount":qty});
                    if *op == "pickup" {
                        if let Some(v) = pile {
                            body["dropId"] = v["id"].clone();
                        }
                    }
                    return (Action::Input(json!({"inventory":body})), false);
                }
                let direction = match *op {
                    "drop" => "Из рюкзака на землю",
                    "pickup" => "С земли в рюкзак",
                    "load" => "Из рюкзака в кузов",
                    _ => "Из кузова в рюкзак",
                };
                s.text(direction, dx + 169., ry + 24., 13., PAPER);
            }
        }
        if let Some(v) = pile {
            let by = y + 468.;
            let desc = format!(
                "На земле   {} / {}   ·   {} ед.",
                self.pile + 1,
                piles.len(),
                count(v, "amount", 0)
            );
            s.text(&desc, dx, by + 21., 14., BLUE_TEAM);
            if piles.len() > 1 {
                if s.button(dx + 265., by, 49., 28., "‹", false) {
                    self.pile = (self.pile + piles.len() - 1) % piles.len();
                }
                if s.button(dx + 321., by, 49., 28., "›", false) {
                    self.pile = (self.pile + 1) % piles.len();
                }
            }
        }
        if number(p, "messageT", 0.) > 0. {
            s.wrap(string(p, "message", ""), x + 28., y + 536., 844., 14., GOLD);
        }
        (Action::None, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_transfer_and_team_range() {
        assert_eq!(amount_for(5, 9, 2), 2);
        assert_eq!(amount_for(32, 7, 20), 7);
        let foot = json!({"team":"blue","x":0,"z":0});
        let world = json!({"vehicles":[{"id":1,"x":1,"z":0,"team":"red","cargoCapacity":96},{"id":2,"x":9,"z":2,"team":"blue","cargoCapacity":96},{"id":3,"x":1,"z":2,"team":"blue","cargoCapacity":96}]});
        assert_eq!(nearby_truck(&foot, &world).unwrap()["id"], 3);
        let mut seated = foot;
        seated["vehicleId"] = json!(1);
        assert!(nearby_truck(&seated, &world).is_none());
    }
}
