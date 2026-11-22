//! The field map combines surveyed terrain with friendly radio reports.
use super::{alpha, flag, number, string, Surface, BLUE_TEAM, EDGE, GOLD, INK, MUTED, PAPER};
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
    Oil,
    Wood,
    Metal,
    Powder,
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
        Symbol::Oil => {
            draw_circle(
                p.x * s.scale,
                (p.y + r * 0.30) * s.scale,
                r * 0.69 * s.scale,
                c,
            );
            let a = vec2(p.x, p.y - r * 1.2) * s.scale;
            draw_triangle(
                a,
                vec2(p.x - r * 0.65, p.y + r * 0.30) * s.scale,
                vec2(p.x + r * 0.65, p.y + r * 0.30) * s.scale,
                c,
            );
        }
        Symbol::Wood => {
            line(vec2(0., -r), vec2(-r * 0.8, r * 0.5), 1.6);
            line(vec2(0., -r), vec2(r * 0.8, r * 0.5), 1.6);
            line(vec2(-r * 0.8, r * 0.5), vec2(r * 0.8, r * 0.5), 1.6);
            line(vec2(0., r * 0.4), vec2(0., r), 1.6);
        }
        Symbol::Metal => {
            line(vec2(-r, r * 0.65), vec2(-r * 0.5, -r * 0.45), 1.8);
            line(vec2(-r * 0.5, -r * 0.45), vec2(r * 0.38, -r), 1.8);
            line(vec2(r * 0.38, -r), vec2(r, r * 0.65), 1.8);
            line(vec2(r, r * 0.65), vec2(-r, r * 0.65), 1.8);
        }
        Symbol::Powder => {
            for (a, b) in [
                (vec2(-r, 0.), vec2(r, 0.)),
                (vec2(0., -r), vec2(0., r)),
                (vec2(-r * 0.65, -r * 0.65), vec2(r * 0.65, r * 0.65)),
            ] {
                line(a, b, 1.7);
            }
        }
        Symbol::General => {
            s.border(p.x - r * 0.65, p.y - r * 0.65, r * 1.3, r * 1.3, c);
        }
    }
}
fn resource_symbol(kind: &str) -> Symbol {
    match kind {
        "oil" | "oilwell" => Symbol::Oil,
        "wood" | "sawmill" => Symbol::Wood,
        "quarry" | "metal" => Symbol::Metal,
        "powder" | "gunpowder" => Symbol::Powder,
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
        let height = 604.;
        let x = (s.w - width) * 0.5;
        let y = (s.h - height) * 0.5;
        s.rect(0., 0., s.w, s.h, alpha(INK, 0.66));
        s.rect(x, y, width, height, Color::new(0.069, 0.115, 0.132, 0.98));
        s.border(x, y, width, height, EDGE);
        s.rect(x, y, width, 2.5, GOLD);
        s.text("КАРТА СЕКТОРА", x + 24., y + 31., 11., GOLD);
        s.text(
            "Сен-Лоран  /  Промышленная долина",
            x + 23.,
            y + 70.,
            32.,
            PAPER,
        );
        if s.button(x + width - 147., y + 20., 120., 31., "Закрыть   M", false) {
            return true;
        }
        let world = self.max - self.min;
        let available = Rect::new(x + 25., y + 99., 699., 447.);
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
        let terrain = Color::new(0.42, 0.48, 0.35, 1.);
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
                Color::new(0.25, 0.29, 0.23, 0.93)
            } else {
                match item.kind.as_str() {
                    "wheat" | "crop" | "grain" => Color::new(0.59, 0.57, 0.34, 0.8),
                    "gravel" | "yard" => Color::new(0.56, 0.55, 0.44, 0.7),
                    "forest" | "woods" => Color::new(0.28, 0.38, 0.27, 0.75),
                    _ => Color::new(0.38, 0.45, 0.33, 0.8),
                }
            };
            let corners = [vec2(-w, -h), vec2(w, -h), vec2(w, h), vec2(-w, h)]
                .map(|v| (center + turn(v, item.yaw)) * s.scale);
            draw_triangle(corners[0], corners[1], corners[2], color);
            draw_triangle(corners[0], corners[2], corners[3], color);
        }
        for (ink, offset) in [
            (Color::new(0.33, 0.33, 0.25, 1.), 1.6),
            (Color::new(0.76, 0.72, 0.53, 1.), 0.),
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
        let grid = Color::new(0.83, 0.87, 0.67, 0.32);
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
                Color::new(0.14, 0.20, 0.17, 1.)
            } else {
                Color::new(0.96, 0.80, 0.44, 1.)
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
                            if is_named { PAPER } else { GOLD },
                        );
                        occupied.push(box_);
                        break;
                    }
                }
            }
        }
        let metal_color = Color::new(0.77, 0.94, 1., 1.);
        for deposit in &self.deposits {
            let pt = proj.at(deposit.center);
            if !proj.inside(pt, 7.) {
                continue;
            }
            let metal = deposit.kind == "metal";
            if !metal && self.zoom < 1.5 {
                continue;
            }
            draw_circle(pt.x * s.scale, pt.y * s.scale, 6.6 * s.scale, INK);
            symbol(
                s,
                pt,
                resource_symbol(&deposit.kind),
                if metal { metal_color } else { GOLD },
                4.7,
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
            if string(prop, "kind", "") == "wall" {
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
        let dronetint = Color::new(0.42, 0.95, 0.84, 1.);
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
        s.border(rect.x, rect.y, rect.w, rect.h, EDGE);
        let sx = x + 753.;
        s.line(sx - 15., y + 91., sx - 15., y + 551., 1., EDGE);
        s.text("УСЛОВНЫЕ ЗНАКИ", sx, y + 113., 12., GOLD);
        for (i, (icon, title, color)) in [
            (Symbol::SelfUnit, "Ты", GOLD),
            (Symbol::Ally, "Союзник", BLUE_TEAM),
            (Symbol::Vehicle, "Доступная техника", PAPER),
            (Symbol::Drone, "Свой дрон", dronetint),
            (Symbol::Marker, "Командная метка", GOLD),
            (Symbol::Factory, "Своя постройка", BLUE_TEAM),
            (Symbol::Oil, "Нефть", GOLD),
            (Symbol::Wood, "Древесина", GOLD),
            (Symbol::Metal, "Металл · склад или карьер", metal_color),
            (Symbol::Powder, "Порох", GOLD),
        ]
        .iter()
        .enumerate()
        {
            let by = y + 141. + i as f32 * 22.;
            symbol(s, vec2(sx + 6., by - 4.), *icon, *color, 5., 0.);
            s.text(title, sx + 24., by, 14., PAPER);
        }
        s.text(
            if own_drone.is_some() {
                "СЕКТОР ДРОНА"
            } else {
                "ТВОЙ СЕКТОР"
            },
            sx,
            y + 378.,
            12.,
            GOLD,
        );
        let col = ((own.x - self.min.x) / world.x * 6.).floor().clamp(0., 5.) as u8;
        let row = ((own.y - self.min.y) / world.y * 6.).floor().clamp(0., 5.) as u8 + 1;
        s.text(
            &format!("{}{}", (b'A' + col) as char, row),
            sx,
            y + 418.,
            32.,
            PAPER,
        );
        s.text(
            &format!("X {:0.0}   Z {:0.0}", own.x, own.y),
            sx,
            y + 443.,
            13.,
            MUTED,
        );
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
        s.text("БЛИЖАЙШИЙ МЕТАЛЛ", sx, y + 477., 11., metal_color);
        for (i, site) in metals.iter().take(2).enumerate() {
            let col = ((site.center.x - self.min.x) / world.x * 6.)
                .floor()
                .clamp(0., 5.) as u8;
            let row = ((site.center.y - self.min.y) / world.y * 6.)
                .floor()
                .clamp(0., 5.) as u8
                + 1;
            s.text(
                &format!(
                    "{}{}   ·   {:.0} м",
                    (b'A' + col) as char,
                    row,
                    own.distance(site.center)
                ),
                sx,
                y + 496. + i as f32 * 33.,
                13.,
                PAPER,
            );
            s.text(
                &s.shorten(&site.name, 11., 228.),
                sx,
                y + 509. + i as f32 * 33.,
                11.,
                MUTED,
            );
        }
        let meters = if self.zoom > 2. { 25. } else { 50. };
        let bar = meters * scale;
        let bx = rect.x;
        let by = y + 569.;
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
        s.right(
            &format!(
                "Стрелки  сдвиг   ·   Home  к себе   ·   Колесо / + −  ×{:.1}",
                self.zoom
            ),
            x + width - 25.,
            by + 4.,
            13.,
            PAPER,
        );
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
