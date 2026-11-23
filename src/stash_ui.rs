//! Backpack, ground piles and selected-resource transfers.
use super::crafting::footprint;
use super::{
    alpha, count, flag, number, string, Action, Surface, BLUE_TEAM, EDGE, GOLD, INK, MATERIALS,
    MUTED, PAPER,
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
fn nearby_piles<'a>(state: &'a Value, p: &Value) -> Vec<&'a Value> {
    let mut list = state["groundDrops"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let dx = number(p, "x", 0.) - number(v, "x", 0.);
            let dz = number(p, "z", 0.) - number(v, "z", 0.);
            let distance = dx * dx + dz * dz;
            (count(v, "amount", 0) > 0 && distance <= 2.6 * 2.6).then_some((distance, v))
        })
        .collect::<Vec<_>>();
    list.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| a.1["id"].as_u64().cmp(&b.1["id"].as_u64()))
    });
    list.into_iter().map(|v| v.1).collect()
}

// Pictograms follow the same material order in both ledgers.
pub(super) fn material_symbol(
    s: &Surface,
    kind: usize,
    x: f32,
    y: f32,
    color: macroquad::prelude::Color,
) {
    let stroke = |ax, ay, bx, by| s.line(x + ax, y + ay, x + bx, y + by, 1.7, color);
    let edges: &[(f32, f32, f32, f32)] = match kind {
        0 => &[
            (-8., 2., 2., -7.),
            (2., -7., 9., -4.),
            (9., -4., -1., 6.),
            (-1., 6., -8., 2.),
            (-8., 2., -8., 6.),
            (-8., 6., -1., 9.),
            (-1., 9., 9., -1.),
            (9., -1., 9., -4.),
        ],
        1 => &[
            (-9., -3., 3., -7.),
            (3., -7., 9., -4.),
            (9., -4., -3., 1.),
            (-3., 1., -9., -3.),
            (-9., -3., -9., 3.),
            (-9., 3., -3., 7.),
            (-3., 7., 9., 2.),
            (9., 2., 9., -4.),
        ],
        2 => &[
            (0., -9., -6., 1.),
            (-6., 1., -6., 5.),
            (-6., 5., -3., 8.),
            (-3., 8., 3., 8.),
            (3., 8., 6., 5.),
            (6., 5., 6., 1.),
            (6., 1., 0., -9.),
        ],
        3 => &[
            (-9., 7., -3., -1.),
            (-3., -1., 1., 2.),
            (1., 2., 4., -3.),
            (4., -3., 9., 7.),
            (9., 7., -9., 7.),
            (-4., -5., -3., -5.),
            (0., -8., 1., -8.),
        ],
        _ => &[
            (-6., -5., -2., -8.),
            (-2., -8., 4., -8.),
            (4., -8., 4., -4.),
            (4., -4., 7., -2.),
            (7., -2., 7., 8.),
            (7., 8., -6., 8.),
            (-6., 8., -6., -5.),
            (-2., -2., 3., 4.),
            (3., -2., -2., 4.),
        ],
    };
    for &(ax, ay, bx, by) in edges {
        stroke(ax, ay, bx, by);
    }
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
        let (x, y) = s.panel(990., 542., "СНАРЯЖЕНИЕ", "Инвентарь");
        if s.button(x + 842., y + 28., 120., 36., "Закрыть   I", false) {
            return (Action::None, true);
        }
        let truck = nearby_truck(p, state);
        let bank = truck.map(|v| &v["cargo"]).unwrap_or(&Value::Null);
        let cargo_total = MATERIALS
            .iter()
            .map(|(key, _)| count(bank, key, 0))
            .sum::<i32>();
        let cargo_capacity = truck.map_or(0, |v| count(v, "cargoCapacity", 0));

        let bag_x = x + 26.;
        let cargo_x = x + 319.;
        let action_x = x + 627.;
        let col = 281.;
        for (left, title, desc, accent) in [
            (
                bag_x,
                "Рюкзак",
                "До 32 ед. в каждой ячейке".to_owned(),
                GOLD,
            ),
            (
                cargo_x,
                "Кузов",
                truck.map_or_else(
                    || "Транспорт вне доступа".into(),
                    |v| super::vehicle_name(string(v, "kind", "")).to_owned(),
                ),
                BLUE_TEAM,
            ),
        ] {
            s.rect(left, y + 166., col, 1., alpha(accent, 0.65));
            s.bold().text(title, left + 12., y + 139., 22., accent);
            s.text(&desc, left + 12., y + 159., 13., MUTED);
        }
        if truck.is_some() {
            s.right(
                &format!("{cargo_total} / {cargo_capacity}"),
                cargo_x + col - 11.,
                y + 139.,
                18.,
                BLUE_TEAM,
            );
        }
        // Row selection is shared by the two containers, including their empty slots.
        for (i, _) in MATERIALS.iter().enumerate() {
            let row_y = y + 179. + i as f32 * 48.;
            if s.click
                && (s.over(bag_x, row_y, col, 44.)
                    || (truck.is_some() && s.over(cargo_x, row_y, col, 44.)))
                && self.resource != i
            {
                self.resource = i;
                self.pile = 0;
            }
        }
        let nearby = nearby_piles(state, p);
        s.text("НА ЗЕМЛЕ", bag_x, y + 450., 12., MUTED);
        let mut chip_x = bag_x + 78.;
        let mut has_drops = false;
        for (i, (key, name)) in MATERIALS.iter().enumerate() {
            let units: i32 = nearby
                .iter()
                .filter(|v| v["resource"] == *key)
                .map(|v| count(v, "amount", 0))
                .sum();
            if units > 0 {
                has_drops = true;
                let label = format!("{name} {units}");
                let width = s.measure(&label, 12.) + 16.;
                let hover = s.over(chip_x, y + 428., width, 32.);
                if hover && s.click && self.resource != i {
                    self.resource = i;
                    self.pile = 0;
                }
                let color = if i == self.resource || hover {
                    GOLD
                } else {
                    MUTED
                };
                s.border(chip_x, y + 428., width, 32., color);
                s.mid(&label, chip_x + width * 0.5, y + 449., 12., color);
                chip_x += width + 4.;
            }
        }
        if !has_drops {
            s.text("В радиусе подбора пусто", chip_x, y + 450., 15., MUTED);
        }
        for (left, contents, is_bag, accent) in
            [(bag_x, p, true, GOLD), (cargo_x, bank, false, BLUE_TEAM)]
        {
            if !is_bag && truck.is_none() {
                s.rect(left, y + 179., col, 236., alpha(INK, 0.45));
                s.text("Нет кузова рядом", left + 15., y + 266., 19., PAPER);
                s.wrap(
                    "Подойди к союзной машине с грузовым отсеком.",
                    left + 15.,
                    y + 293.,
                    col - 30.,
                    15.,
                    MUTED,
                );
                continue;
            }
            for (i, (key, label)) in MATERIALS.iter().enumerate() {
                let row_y = y + 179. + i as f32 * 48.;
                let selected = self.resource == i;
                let stock = count(contents, key, 0);
                let color = if selected {
                    accent
                } else if stock > 0 {
                    PAPER
                } else {
                    MUTED
                };
                if selected || s.over(left, row_y, col, 44.) {
                    s.rect(
                        left,
                        row_y,
                        col,
                        44.,
                        alpha(accent, if selected { 0.15 } else { 0.07 }),
                    );
                }
                s.rect(
                    left,
                    row_y + 43.,
                    col,
                    1.,
                    alpha(accent, if selected { 0.8 } else { 0.16 }),
                );
                if selected {
                    s.rect(left, row_y + 8., 3., 28., accent);
                }
                material_symbol(s, i, left + 24., row_y + 21., color);
                s.text(label, left + 46., row_y + 27., 17., color);
                s.right(
                    &stock.to_string(),
                    left + col - 12.,
                    row_y + 28.,
                    21.,
                    color,
                );
                if is_bag {
                    s.rect(left + 46., row_y + 35., 77., 2., EDGE);
                    s.rect(
                        left + 46.,
                        row_y + 35.,
                        77. * (stock as f32 / STACK as f32).clamp(0., 1.),
                        2.,
                        accent,
                    );
                }
            }
        }
        let (key, name) = MATERIALS[self.resource];
        let held = count(p, key, 0);
        let stored = count(bank, key, 0);
        let piles: Vec<_> = nearby
            .into_iter()
            .filter(|v| v["resource"] == key)
            .collect();
        self.pile = self.pile.min(piles.len().saturating_sub(1));
        if !piles.is_empty() {
            if piles.len() > 1 {
                if s.button(action_x + 252., y + 445., 36., 30., "‹", false) {
                    self.pile = (self.pile + piles.len() - 1) % piles.len();
                }
                if s.button(action_x + 297., y + 445., 36., 30., "›", false) {
                    self.pile = (self.pile + 1) % piles.len();
                }
            }
            s.text(
                &format!(
                    "Стопка {} / {}   ·   {} ед.",
                    self.pile + 1,
                    piles.len(),
                    count(piles[self.pile], "amount", 0)
                ),
                action_x,
                y + 466.,
                14.,
                MUTED,
            );
        }
        let pile = piles.get(self.pile).copied();
        let ground = pile.map_or(0, |v| count(v, "amount", 0));
        let foot = p["vehicleId"].is_null();
        s.line(action_x - 15., y + 115., action_x - 15., y + 473., 1., EDGE);
        s.bold().text(name, action_x, y + 140., 25., PAPER);
        s.text("КОЛИЧЕСТВО", action_x, y + 169., 12., MUTED);
        if s.button(action_x, y + 180., 38., 35., "−", false) {
            self.quantity = (self.quantity - 1).max(1);
        }
        if s.button(action_x + 94., y + 180., 38., 35., "+", false) {
            self.quantity = (self.quantity + 1).min(STACK);
        }
        for (i, (q, label)) in [(1, "1"), (5, "5"), (STACK, "Все")].iter().enumerate() {
            if s.button(
                action_x + 150. + i as f32 * 62.,
                y + 180.,
                57.,
                35.,
                label,
                *q == self.quantity,
            ) {
                self.quantity = *q;
            }
        }
        s.mid(
            &self.quantity.to_string(),
            action_x + 66.,
            y + 205.,
            21.,
            PAPER,
        );

        let busy = if !flag(p, "alive", true) {
            Some("После возрождения")
        } else if !p["droneId"].is_null() {
            Some("Отзови дрон")
        } else {
            None
        };
        let (foot_lock, bag_full, bag_empty) = (
            (!foot).then_some("Выйди из машины"),
            (held >= STACK).then_some("Ячейка полна"),
            (held == 0).then_some("Нет в рюкзаке"),
        );
        let mut row = 0;
        for (op, label, available, space, blocked, balance) in [
            (
                "load",
                "В кузов",
                held,
                cargo_capacity - cargo_total,
                bag_empty.or((cargo_total >= cargo_capacity).then_some("Кузов полон")),
                stored,
            ),
            (
                "unload",
                "Из кузова",
                stored,
                STACK - held,
                (stored == 0).then_some("Нет в кузове").or(bag_full),
                held,
            ),
            (
                "pickup",
                "Подобрать",
                ground,
                STACK - held,
                foot_lock
                    .or((ground == 0).then_some("Рядом пусто"))
                    .or(bag_full),
                held,
            ),
            (
                "drop",
                "Выбросить",
                held,
                STACK,
                foot_lock.or(bag_empty),
                held,
            ),
        ] {
            if matches!(op, "load" | "unload") && truck.is_none() {
                continue;
            }
            let by = y + 233. + row as f32 * 55.;
            row += 1;
            let qty = amount_for(self.quantity, available, space);
            if let Some(reason) = busy.or(blocked) {
                s.disabled(action_x, by, 147., 39., label);
                s.text(reason, action_x + 159., by + 25., 13., MUTED);
            } else {
                if s.button(action_x, by, 147., 39., &format!("{label}   {qty}"), false) {
                    let mut body = json!({"action":op,"resource":key,"amount":qty});
                    if let Some(v) = pile.filter(|_| op == "pickup") {
                        body["dropId"] = v["id"].clone();
                    }
                    return (Action::Input(json!({"inventory":body})), false);
                }
                let dest = if op == "load" {
                    "Кузов"
                } else {
                    "Рюкзак"
                };
                let after = if op == "drop" {
                    balance - qty
                } else {
                    balance + qty
                };
                s.text(
                    &format!("{dest}   {balance} → {after}"),
                    action_x + 159.,
                    by + 25.,
                    14.,
                    PAPER,
                );
            }
        }
        if number(p, "messageT", 0.) > 0. {
            s.rect(x + 26., y + 483., 938., 39., alpha(GOLD, 0.12));
            s.rect(x + 26., y + 483., 3., 39., GOLD);
            s.text(
                &s.shorten(string(p, "message", ""), 16., 900.),
                x + 39.,
                y + 508.,
                16.,
                PAPER,
            );
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
