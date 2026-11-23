//! The field map combines surveyed terrain with friendly radio reports.
use super::{alpha, flag, number, string, Surface, BLUE_TEAM, EDGE, GOLD, INK, MATERIALS, MUTED, PAPER};
use super::stash_ui::material_symbol_scaled;
use macroquad::prelude::*;
use serde_json::Value;

struct SurveyLine {
    center: Vec2,
    along: Vec2,
    width: f32,
}
struct Mark {
    center: Vec2,
    kind: String,
    name: String,
}
struct Footprint {
    center: Vec2,
    size: Vec2,
    yaw: f32,
    kind: String,
}
pub(super) struct TacticalMap {
    min: Vec2,
    max: Vec2,
    roads: Vec<SurveyLine>,
    ground: Vec<Footprint>,
    marks: Vec<Mark>,
    deposits: Vec<Mark>,
    zoom: f32,
    offset: Vec2,
}
fn point(v: &Value) -> Vec2 {
    vec2(number(v, "x", 0.) as f32, number(v, "z", 0.) as f32)
}
fn feature(v: &Value) -> Footprint {
    Footprint {
        center: point(v),
        size: vec2(number(v, "w", 2.) as f32, number(v, "d", 2.) as f32),
        yaw: number(v, "yaw", 0.) as f32,
        kind: string(v, "kind", "").into(),
    }
}
fn turn(v: Vec2, a: f32) -> Vec2 {
    let (si, co) = a.sin_cos();
    vec2(v.x * co - v.y * si, v.x * si + v.y * co)
}

struct Projection {
    rect: Rect,
    center: Vec2,
    scale: f32,
}
impl Projection {
    fn at(&self, p: Vec2) -> Vec2 {
        self.rect.center() + (p - self.center) * self.scale
    }
    fn inside(&self, p: Vec2, pad: f32) -> bool {
        p.x >= self.rect.x + pad
            && p.x <= self.rect.right() - pad
            && p.y >= self.rect.y + pad
            && p.y <= self.rect.bottom() - pad
    }
    // Clip long roads crossing the viewport even if neither endpoint is visible.
    fn line(&self, s: &Surface, a: Vec2, b: Vec2, width: f32, c: Color) {
        let a = self.at(a);
        let b = self.at(b);
        let delta = b - a;
        let mut enter = 0_f32;
        let mut leave = 1_f32;
        for (p, q) in [
            (-delta.x, a.x - self.rect.x),
            (delta.x, self.rect.right() - a.x),
            (-delta.y, a.y - self.rect.y),
            (delta.y, self.rect.bottom() - a.y),
        ] {
            if p.abs() < 0.0001 {
                if q < 0. {
                    return;
                }
            } else {
                let r = q / p;
                if p < 0. {
                    enter = enter.max(r);
                } else {
                    leave = leave.min(r);
                }
            }
        }
        if enter <= leave {
            let aa = a + delta * enter;
            let bb = a + delta * leave;
            s.line(aa.x, aa.y, bb.x, bb.y, width, c);
        }
    }
}

#[derive(Clone, Copy)]
enum Symbol {
    SelfUnit,
    Ally,
    Factory,
    Vehicle,
    Drone,
    Marker,
    Material(usize),
    General,
}
fn symbol(s: &Surface, p: Vec2, kind: Symbol, c: Color, r: f32, direction: f32) {
    let line =
        |a: Vec2, b: Vec2, th: f32| s.line(p.x + a.x, p.y + a.y, p.x + b.x, p.y + b.y, th, c);
    match kind {
        Symbol::SelfUnit => {
            let tip = turn(vec2(0., -r), direction);
            let a = turn(vec2(-r * 0.68, r * 0.8), direction);
            let b = turn(vec2(r * 0.68, r * 0.8), direction);
            let screen = |q: Vec2| (q + p) * s.scale;
            draw_triangle(screen(tip), screen(a), screen(b), c);
        }
        Symbol::Ally => {
            draw_circle(p.x * s.scale, p.y * s.scale, r * s.scale, c);
            draw_circle(p.x * s.scale, p.y * s.scale, r * s.scale * 0.39, INK);
        }
        Symbol::Factory => {
            s.rect(p.x - r, p.y - r, r * 2., 2. * r, c);
            s.rect(p.x - r * 0.43, p.y - r * 0.43, r * 0.86, r * 0.86, INK);
        }
        Symbol::Vehicle => {
            s.border(p.x - r * 1.2, p.y - r * 0.65, r * 2.4, r * 1.3, c);
            line(vec2(-r * 0.25, 0.), vec2(r * 1.4, -r * 0.8), 1.6);
        }
        Symbol::Drone => {
            let d = r * 0.72;
            line(vec2(-d, -d), vec2(d, d), 1.4);
            line(vec2(-d, d), vec2(d, -d), 1.4);
            for (xx, yy) in [(-d, -d), (d, -d), (-d, d), (d, d)] {
                draw_circle_lines(
                    (p.x + xx) * s.scale,
                    (p.y + yy) * s.scale,
                    r * 0.34 * s.scale,
                    s.scale,
                    c,
                );
            }
        }
        Symbol::Marker => {
            let shape = [
                vec2(0., -r),
                vec2(r, 0.),
                vec2(0., r),
                vec2(-r, 0.),
                vec2(0., -r),
            ];
            for pair in shape.windows(2) {
                line(pair[0], pair[1], 1.5);
            }
            draw_circle(p.x * s.scale, p.y * s.scale, 1.2 * s.scale, c);
        }
        Symbol::Material(index) => {
            material_symbol_scaled(s, index, p.x, p.y, r / 9., c);
        }
        Symbol::General => {
            s.border(p.x - r * 0.65, p.y - r * 0.65, r * 1.3, r * 1.3, c);
        }
    }
}
fn resource_symbol(kind: &str) -> Symbol {
    match kind {
        "wood" | "sawmill" => Symbol::Material(0),
        "metal" | "quarry" => Symbol::Material(1),
        "oil" | "oilwell" => Symbol::Material(2),
        "powder" | "gunpowder" => Symbol::Material(3),
        "fuel" => Symbol::Material(4),
        "base" | "workyard" => Symbol::Factory,
        _ => Symbol::General,
    }
}
fn crew_is_visible(v: &Value, friendly: &str, players: &[Value]) -> bool {
    if flag(v, "destroyed", false) {
        return false;
    }
    let owner = string(v, "team", "");
    if !owner.is_empty() && owner != "neutral" && owner != friendly {
        return false;
    }
    let driver = &v["driver"];
    driver.is_null()
        || players
            .iter()
            .any(|p| p["id"] == *driver && string(p, "team", "") == friendly)
}

impl TacticalMap {
    pub(super) fn new() -> Self {
        let level: Value = serde_json::from_str(include_str!("../assets/level.json"))
            .expect("embedded tactical chart");
        let bounds = &level["bounds"];
        let min = vec2(
            number(bounds, "minX", -280.) as f32,
            number(bounds, "minZ", -230.) as f32,
        );
        let max = vec2(
            number(bounds, "maxX", 280.) as f32,
            number(bounds, "maxZ", 230.) as f32,
        );
        let mut roads = Vec::new();
        let mut ground = Vec::new();
        let mut marks = Vec::new();
        for v in level["roads"].as_array().into_iter().flatten() {
            let f = feature(v);
            let axis = if f.size.y >= f.size.x {
                vec2(0., f.size.y * 0.5)
            } else {
                vec2(f.size.x * 0.5, 0.)
            };
            roads.push(SurveyLine {
                center: f.center,
                along: turn(axis, f.yaw),
                width: f.size.x.min(f.size.y),
            });
        }
        for v in level["fields"].as_array().into_iter().flatten() {
            ground.push(feature(v));
        }
        for v in level["props"].as_array().into_iter().flatten() {
            if matches!(
                string(v, "kind", ""),
                "house" | "barn" | "ruin" | "tower" | "church" | "warehouse"
            ) {
                ground.push(feature(v));
            }
            if let Some(k) = v["resource"].as_str() {
                if matches!(k, "oil" | "metal" | "wood" | "powder" | "gunpowder")
                    && !matches!(string(v, "kind", ""), "tree" | "pine")
                {
                    let mapped = match k {
                        "oil" => "Нефть",
                        "metal" => "Металл",
                        "wood" => "Древесина",
                        _ => "Порох",
                    };
                    marks.push(Mark {
                        center: point(v),
                        kind: k.into(),
                        name: mapped.into(),
                    });
                }
            }
        }
        for v in level["landmarks"].as_array().into_iter().flatten() {
            marks.push(Mark {
                center: point(v),
                kind: string(v, "kind", "village").into(),
                name: string(v, "name", "").into(),
            });
        }
        let deposits = level["resourcePoints"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| Mark {
                center: point(v),
                kind: string(v, "resource", "").into(),
                name: string(v, "name", "").into(),
            })
            .collect();
        Self {
            deposits,
            min,
            max,
            roads,
            ground,
            marks,
            zoom: 1.,
            offset: Vec2::ZERO,
        }
    }
    pub(super) fn recenter(&mut self) {
        self.offset = Vec2::ZERO;
    }
    pub(super) fn draw(&mut self, s: &Surface, state: &Value, local: &Value) -> bool {
        let wheel = mouse_wheel().1;
        if wheel > 0. || is_key_pressed(KeyCode::Equal) {
            self.zoom = (self.zoom * 1.5).min(4.5);
        }
        if wheel < 0. || is_key_pressed(KeyCode::Minus) {
            self.zoom = (self.zoom / 1.5).max(1.);
        }
        let width = 1010.;
        let (x, y) = s.panel(width, 620., "Сен-Лоран · Промышленная долина", "Тактическая карта");
        if s.button(x + width - 147., y + 25., 120., 34., "Закрыть   M", false) {
            return true;
        }
        let world = self.max - self.min;
        let available = Rect::new(x + 37., y + 128., 578., 419.);
        let basis = (available.w / world.x).min(available.h / world.y);
        let rect = Rect::new(
            available.x + (available.w - world.x * basis) / 2.,
            available.y + (available.h - world.y * basis) / 2.,
            world.x * basis,
            world.y * basis,
        );
        let scale = basis * self.zoom;
        let half = vec2(rect.w, rect.h) / (2. * scale);
        let operator = point(local);
        let own_drone = super::own_drone(state, local);
        let own = own_drone.map_or(operator, point);
        let center = if self.zoom <= 1.01 {
            self.offset = Vec2::ZERO;
            (self.max + self.min) * 0.5
        } else {
            if is_key_pressed(KeyCode::Home) {
                self.recenter();
            }
            let keys = vec2(
                (is_key_down(KeyCode::Right) as i32 - is_key_down(KeyCode::Left) as i32) as f32,
                (is_key_down(KeyCode::Down) as i32 - is_key_down(KeyCode::Up) as i32) as f32,
            );
            self.offset += keys.normalize_or_zero() * 250. * get_frame_time().min(0.12) / scale;
            let target = own + self.offset;
            let c = vec2(
                target
                    .x
                    .clamp(self.min.x + half.x - 0.001, self.max.x - half.x + 0.001),
                target
                    .y
                    .clamp(self.min.y + half.y - 0.001, self.max.y - half.y + 0.001),
            );
            if keys != Vec2::ZERO {
                self.offset = c - own;
            }
            c
        };
        let proj = Projection {
            rect,
            scale,
            center,
        };
        let terrain = Color::from_rgba(82, 92, 77, 255);
        s.rect(rect.x, rect.y, rect.w, rect.h, terrain);
        for item in &self.ground {
            let center = proj.at(item.center);
            let w = (item.size.x * scale * 0.5).max(0.8);
            let h = (item.size.y * scale * 0.5).max(0.8);
            if !proj.inside(center, w.max(h) + 2.) {
                continue;
            }
            let building = matches!(
                item.kind.as_str(),
                "house" | "barn" | "ruin" | "tower" | "church" | "warehouse"
            );
            let color = if building {
                Color::from_rgba(45, 52, 49, 255)
            } else {
                match item.kind.as_str() {
                    "wheat" | "crop" | "grain" => Color::from_rgba(115, 113, 85, 255),
                    "gravel" | "yard" => Color::from_rgba(114, 113, 97, 255),
                    "forest" | "woods" => Color::from_rgba(61, 76, 61, 255),
                    _ => Color::from_rgba(76, 87, 72, 255),
                }
            };
            let corners = [vec2(-w, -h), vec2(w, -h), vec2(w, h), vec2(-w, h)]
                .map(|v| (center + turn(v, item.yaw)) * s.scale);
            draw_triangle(corners[0], corners[1], corners[2], color);
            draw_triangle(corners[0], corners[2], corners[3], color);
        }
        for (ink, offset) in [
            (Color::from_rgba(62, 67, 58, 255), 1.6),
            (Color::from_rgba(177, 175, 151, 255), 0.),
        ] {
            for road in &self.roads {
                proj.line(
                    s,
                    road.center - road.along,
                    road.center + road.along,
                    (road.width * scale * 0.74).clamp(1.65, 14.) + offset,
                    ink,
                );
            }
        }
        let grid = alpha(PAPER, 0.25);
        for i in 0..=6 {
            let fx = self.min.x + world.x * i as f32 / 6.;
            let fy = self.min.y + world.y * i as f32 / 6.;
            proj.line(s, vec2(fx, self.min.y), vec2(fx, self.max.y), 0.6, grid);
            proj.line(s, vec2(self.min.x, fy), vec2(self.max.x, fy), 0.6, grid);
            if i < 6 {
                let mx = proj.at(vec2(fx + world.x / 12., self.min.y)).x;
                let my = proj.at(vec2(self.min.x, fy + world.y / 12.)).y;
                if mx > rect.x && mx < rect.right() {
                    s.mid(
                        &((b'A' + i as u8) as char).to_string(),
                        mx,
                        rect.y - 5.,
                        10.,
                        MUTED,
                    );
                }
                if my > rect.y && my < rect.bottom() {
                    s.right(&(i + 1).to_string(), rect.x - 6., my + 3., 10., MUTED);
                }
            }
        }
        let mut occupied: Vec<Rect> = Vec::new();
        for mark in self.marks.iter().rev() {
            let pt = proj.at(mark.center);
            if !proj.inside(pt, 8.) {
                continue;
            }
            let deposit = matches!(
                mark.kind.as_str(),
                "oil" | "wood" | "metal" | "powder" | "gunpowder"
            );
            // The well landmark already labels the field; individual deposits only need icons at overview scale.
            let is_named = !(deposit
                && matches!(
                    mark.name.as_str(),
                    "Нефть" | "Древесина" | "Металл" | "Порох"
                ));
            let tint = if is_named {
                INK
            } else {
                PAPER
            };
            if !is_named {
                draw_circle(pt.x * s.scale, pt.y * s.scale, 4.4 * s.scale, INK);
            }
            symbol(
                s,
                pt,
                resource_symbol(&mark.kind),
                tint,
                if is_named { 4.4 } else { 3.1 },
                0.,
            );
            if is_named || self.zoom > 1.05 {
                let fs = if is_named { 12. } else { 10. };
                let tw = s.measure(&mark.name, fs) + 5.;
                for (ox, oy) in [
                    (7., -6.),
                    (-tw - 7., -6.),
                    (7., 9.),
                    (-tw - 7., 9.),
                    (-tw * 0.5, -22.),
                    (-tw * 0.5, 14.),
                ] {
                    let box_ = Rect::new(pt.x + ox, pt.y + oy, tw, 15.);
                    if proj.inside(vec2(box_.x, box_.y), 1.)
                        && proj.inside(vec2(box_.right(), box_.bottom()), 1.)
                        && !occupied.iter().any(|p| p.overlaps(&box_))
                    {
                        s.rect(box_.x, box_.y, tw, 15., alpha(INK, 0.84));
                        s.text(
                            &mark.name,
                            box_.x + 2.,
                            box_.y + 11.5,
                            fs,
                            PAPER,
                        );
                        occupied.push(box_);
                        break;
                    }
                }
            }
        }
        let metal_color = BLUE_TEAM;
        let mut hovered: Option<(&Mark, Vec2, f32)> = None;
        for deposit in &self.deposits {
            let pt = proj.at(deposit.center);
            if !proj.inside(pt, 7.) {
                continue;
            }
            let metal = deposit.kind == "metal";
            let distance = s.pointer.distance_squared(pt);
            if distance < 10. * 10. && hovered.as_ref().map_or(true, |h| distance < h.2) {
                hovered = Some((deposit, pt, distance));
            }
            draw_circle(pt.x * s.scale, pt.y * s.scale, 7. * s.scale, INK);
            symbol(
                s,
                pt,
                resource_symbol(&deposit.kind),
                if metal { metal_color } else { PAPER },
                5.7,
                0.,
            );
        }
        let team = string(local, "team", "blue");
        let players = state["players"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for prop in state["builtProps"].as_array().into_iter().flatten() {
            if string(prop, "team", "") != team
                || flag(prop, "dead", false)
                || number(prop, "hp", 1.) <= 0.
            {
                continue;
            }
            let pt = proj.at(point(prop));
            if !proj.inside(pt, 4.) {
                continue;
            }
            if matches!(string(prop, "kind", ""), "wall" | "barricade") {
                s.rect(pt.x - 2., pt.y - 1.4, 4., 2.8, BLUE_TEAM);
            }
        }
        for f in state["factories"].as_array().into_iter().flatten() {
            let pt = proj.at(point(f));
            if string(f, "team", "") != team || !proj.inside(pt, 6.) {
                continue;
            }
            symbol(s, pt, Symbol::Factory, BLUE_TEAM, 4.5, 0.);
        }
        for v in state["vehicles"].as_array().into_iter().flatten() {
            let pt = proj.at(point(v));
            if !crew_is_visible(v, team, players) || !proj.inside(pt, 7.) {
                continue;
            }
            symbol(
                s,
                pt,
                Symbol::Vehicle,
                if string(v, "team", "") == team {
                    BLUE_TEAM
                } else {
                    PAPER
                },
                5.7,
                0.,
            );
        }
        for p in players {
            let pt = proj.at(point(p));
            if p["id"] == local["id"]
                || string(p, "team", "") != team
                || !flag(p, "alive", true)
                || !proj.inside(pt, 6.)
            {
                continue;
            }
            symbol(s, pt, Symbol::Ally, BLUE_TEAM, 4.4, 0.);
            if self.zoom > 1.45 && proj.inside(pt, 55.) {
                let name = s.shorten(string(p, "name", "Боец"), 10., 83.);
                let w = s.measure(&name, 10.) + 5.;
                s.rect(pt.x + 7., pt.y - 8., w, 14., INK);
                s.text(&name, pt.x + 9., pt.y + 3., 10., BLUE_TEAM);
            }
        }
        let dronetint = BLUE_TEAM;
        for mark in state["markers"].as_array().into_iter().flatten() {
            let pt = proj.at(point(mark));
            if string(mark, "team", "") != team
                || !proj.inside(pt, 8.)
                || number(mark, "remaining", 0.) <= 0.
            {
                continue;
            }
            draw_circle(pt.x * s.scale, pt.y * s.scale, 7. * s.scale, INK);
            symbol(s, pt, Symbol::Marker, GOLD, 6., 0.);
            if self.zoom > 1. && proj.inside(pt, 36.) {
                let label = format!("{} с", number(mark, "remaining", 0.).ceil() as i32);
                s.text(&label, pt.x + 10., pt.y + 3., 10., PAPER);
            }
        }
        for d in state["drones"].as_array().into_iter().flatten() {
            let pt = proj.at(point(d));
            if string(d, "team", "") != team || !proj.inside(pt, 8.) {
                continue;
            }
            draw_circle(pt.x * s.scale, pt.y * s.scale, 7. * s.scale, INK);
            symbol(s, pt, Symbol::Drone, dronetint, 5.3, 0.);
        }
        let pt = proj.at(operator);
        if !local.is_null() && proj.inside(pt, 8.) {
            draw_circle(pt.x * s.scale, pt.y * s.scale, 8.8 * s.scale, INK);
            symbol(
                s,
                pt,
                Symbol::SelfUnit,
                GOLD,
                7.,
                number(local, "yaw", 0.) as f32,
            );
        }
        if let Some((site, pt, _)) = hovered {
            let material = MATERIALS.iter().find(|(key, _)| *key == site.kind.as_str())
                .map_or("Ресурс", |(_, name)| *name);
            let detail = format!("{material}  ·  {:.0} м", own.distance(site.center));
            let name = s.shorten(&site.name, 14., 248.);
            let width = s.measure(&name, 14.).max(s.measure(&detail, 12.)) + 22.;
            let left = (pt.x + 11.).clamp(rect.x + 2., rect.right() - width - 2.);
            let top = (pt.y - 52.).clamp(rect.y + 2., rect.bottom() - 49.);
            s.rect(left, top, width, 47., INK);
            s.border(left, top, width, 47., EDGE);
            s.text(&name, left + 10., top + 19., 14., PAPER);
            s.text(&detail, left + 10., top + 37., 12., MUTED);
        }
        s.border(rect.x, rect.y, rect.w, rect.h, EDGE);
        let sx = x + 643.;
        let right = x + width - 28.;
        let sector = |pos: Vec2| {
            let col = ((pos.x - self.min.x) / world.x * 6.).floor().clamp(0., 5.) as u8;
            let row = ((pos.y - self.min.y) / world.y * 6.).floor().clamp(0., 5.) as u8 + 1;
            format!("{}{}", (b'A' + col) as char, row)
        };
        s.line(sx - 20., y + 118., sx - 20., y + 548., 1., EDGE);
        let sector_label = if own_drone.is_some() { "СЕКТОР ДРОНА" } else { "ТВОЙ СЕКТОР" };
        s.text(sector_label, sx, y + 132., 12., MUTED);
        s.bold().text(&sector(own), sx, y + 174., 35., PAPER);
        s.right(&format!("X {:.0}   Z {:.0}", own.x, own.y), right, y + 169., 15., MUTED);
        s.line(sx, y + 190., right, y + 190., 1., EDGE);
        s.text("СОЮЗНИКИ И МЕТКИ", sx, y + 217., 12., MUTED);
        for (i, (kind, label, tint)) in [
            (Symbol::SelfUnit, "Ты", GOLD), (Symbol::Drone, "Дрон", dronetint),
            (Symbol::Ally, "Союзник", BLUE_TEAM), (Symbol::Marker, "Метка", GOLD),
            (Symbol::Vehicle, "Техника", PAPER), (Symbol::Factory, "Постройка", BLUE_TEAM),
        ].into_iter().enumerate() {
            let bx = sx + (i % 2) as f32 * 161.;
            let by = y + 245. + (i / 2) as f32 * 27.;
            symbol(s, vec2(bx + 8., by - 5.), kind, tint, 6., 0.);
            s.text(label, bx + 25., by, 15., PAPER);
        }
        s.line(sx, y + 316., right, y + 316., 1., EDGE);
        s.text("СНАБЖЕНИЕ", sx, y + 342., 12., MUTED);
        for (i, (_, label)) in MATERIALS.iter().enumerate() {
            let bx = sx + (i % 2) as f32 * 161.;
            let by = y + 368. + (i / 2) as f32 * 24.;
            let color = if i == 1 { metal_color } else { PAPER };
            symbol(s, vec2(bx + 8., by - 5.), Symbol::Material(i), color, 7., 0.);
            s.text(label, bx + 25., by, 15., PAPER);
        }
        s.line(sx, y + 435., right, y + 435., 1., EDGE);
        let mut metals = self
            .deposits
            .iter()
            .filter(|m| m.kind == "metal")
            .collect::<Vec<_>>();
        metals.sort_by(|a, b| {
            a.center
                .distance_squared(own)
                .total_cmp(&b.center.distance_squared(own))
        });
        s.text("БЛИЖАЙШИЙ МЕТАЛЛ", sx, y + 460., 12., MUTED);
        for (i, site) in metals.iter().take(2).enumerate() {
            let by = y + 488. + i as f32 * 34.;
            s.text(&sector(site.center), sx, by, 16., metal_color);
            s.text(&s.shorten(&site.name, 14., 222.), sx + 39., by, 14., PAPER);
            s.right(&format!("{:.0} м", own.distance(site.center)), right, by, 15., PAPER);
        }
        let meters = if self.zoom > 2. { 25. } else { 50. };
        let bar = meters * scale;
        let bx = rect.x;
        let by = y + 583.;
        s.line(bx, by, bx + bar, by, 1.5, MUTED);
        s.line(bx, by - 4., bx, by + 1., 1.5, MUTED);
        s.line(bx + bar, by - 4., bx + bar, by + 1., 1.5, MUTED);
        s.text(
            &format!("{} м", meters as u16),
            bx + bar + 9.,
            by + 3.,
            11.,
            MUTED,
        );
        s.text("Стрелки  ·  сдвиг", x + 250., by + 5., 13., MUTED);
        if s.button(x + 383., by - 16., 130., 33.,"К себе   Home", false) {
            self.recenter();
        }
        s.text("Колесо  ·  масштаб", sx, by + 5., 13., MUTED);
        if s.button(right - 165., by - 16., 42., 33., "−", false) {
            self.zoom = (self.zoom / 1.5).max(1.);
        }
        s.mid(&format!("×{:.1}", self.zoom), right - 84., by + 5., 16., PAPER);
        if s.button(right - 42., by - 16., 42., 33.,"+", false) {
            self.zoom = (self.zoom * 1.5).min(4.5);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn chart_hides_hostile_vehicles_and_unknown_occupants() {
        let players = vec![json!({"id":7,"team":"red"}), json!({"id":4,"team":"blue"})];
        assert!(!crew_is_visible(&json!({"team":"red"}), "blue", &players));
        assert!(!crew_is_visible(&json!({"driver":7}), "blue", &players));
        assert!(!crew_is_visible(&json!({"driver":17}), "blue", &players));
        assert!(crew_is_visible(
            &json!({"team":"blue","driver":4}),
            "blue",
            &players
        ));
        assert!(crew_is_visible(
            &json!({"team":"neutral","driver":null}),
            "blue",
            &players
        ));
    }
}
