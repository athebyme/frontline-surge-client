//! Small isometric shop illustrations. Each face is drawn in catalogue coordinates.
use super::super::Surface;
use frontline_surge::arsenal::{Grip, Kit, Optic};
use macroquad::prelude::*;

pub(super) struct LoadoutPictures(Texture2D);

impl LoadoutPictures {
    pub(super) fn new() -> Self {
        let texture = Texture2D::from_file_with_format(
            include_bytes!("../assets/loadout-previews.png"), Some(ImageFormat::Png),
        );
        texture.set_filter(FilterMode::Linear);
        Self(texture)
    }

    pub(super) fn draw(&self, s: &Surface, weapon: &str, kit: Kit, x: f32, y: f32) {
        let platform = match weapon { "vx9" => 1, "ar4" => 2, _ => 0 };
        let optic = match kit.optic { Optic::Irons => 0, Optic::Reflex => 1, Optic::Scope => 2 };
        let grip = match kit.grip { Grip::Standard => 0, Grip::Light => 1, Grip::Stable => 2 };
        let width = self.0.width() / 3.;
        let height = self.0.height() / 9.;
        draw_texture_ex(&self.0, x * s.scale, y * s.scale, WHITE, DrawTextureParams {
            source: Some(Rect::new(grip as f32 * width, (3 * platform + optic) as f32 * height, width, height)),
            dest_size: Some(vec2(182., 182. * height / width) * s.scale),
            ..Default::default()
        });
    }
}

struct View<'a, 'f> {
    s: &'a Surface<'f>,
    x: f32,
    y: f32,
    k: f32,
}
impl View<'_, '_> {
    fn at(&self, x: f32, z: f32, h: f32) -> Vec2 {
        vec2(
            (self.x + (x - z * 0.65) * self.k) * self.s.scale,
            (self.y + (x * 0.22 + z * 0.38 - h) * self.k) * self.s.scale,
        )
    }
    fn quad(&self, p: [Vec2; 4], c: Color) {
        draw_triangle(p[0], p[1], p[2], c);
        draw_triangle(p[0], p[2], p[3], c);
    }
    fn block(&self, xyz: (f32, f32, f32), dims: (f32, f32, f32), c: Color) {
        let (x, z, h) = xyz;
        let (w, d, t) = dims;
        let px = |a, b, e| self.at(a, b, e);
        self.quad(
            [
                px(x, z + d, h),
                px(x + w, z + d, h),
                px(x + w, z + d, h + t),
                px(x, z + d, h + t),
            ],
            shade(c, 0.78),
        );
        self.quad(
            [
                px(x + w, z, h),
                px(x + w, z + d, h),
                px(x + w, z + d, h + t),
                px(x + w, z, h + t),
            ],
            shade(c, 0.51),
        );
        self.quad(
            [
                px(x, z, h + t),
                px(x + w, z, h + t),
                px(x + w, z + d, h + t),
                px(x, z + d, h + t),
            ],
            c,
        );
    }
    fn barrel(&self, x: f32, z: f32, h: f32, r: f32, t: f32, c: Color) {
        let pt = |i: usize, top: f32| {
            let a = i as f32 * std::f32::consts::TAU / 8.;
            self.at(x + r * a.cos(), z + r * a.sin(), top)
        };
        for i in [4, 5, 6, 7, 0, 1, 2, 3] {
            self.quad(
                [pt(i, h), pt(i + 1, h), pt(i + 1, h + t), pt(i, h + t)],
                shade(c, 0.55 + 0.07 * (i % 4) as f32),
            );
        }
        let center = self.at(x, z, h + t);
        for i in 0..8 {
            draw_triangle(center, pt(i, h + t), pt(i + 1, h + t), c);
        }
    }
}
fn shade(c: Color, k: f32) -> Color {
    Color::new(c.r * k, c.g * k, c.b * k, c.a)
}
const OLIVE: Color = Color::new(0.53, 0.56, 0.43, 1.);
const STEEL: Color = Color::new(0.61, 0.65, 0.62, 1.);
const DARK: Color = Color::new(0.32, 0.36, 0.33, 1.);
const SAND: Color = Color::new(0.73, 0.68, 0.55, 1.);
const BRASS: Color = Color::new(0.75, 0.65, 0.44, 1.);
pub(super) fn product(s: &Surface, id: &str, x: f32, y: f32, k: f32) {
    let v = View { s, x, y, k };
    match id {
        "wall" => {
            for layer in 0..3 {
                for (start, len) in if layer % 2 == 0 {
                    [(0., 10.), (11., 10.), (22., 10.)]
                } else {
                    [(0., 5.), (6., 15.), (22., 10.)]
                } {
                    v.block(
                        (start, 0., layer as f32 * 5.),
                        (len, 6., 4.8),
                        if layer == 1 { SAND } else { shade(SAND, 0.91) },
                    );
                }
            }
        }
        "workshop" => {
            v.block((-2., -1., 0.), (33., 22., 1.6), DARK);
            v.block((0., 0., 1.6), (28., 18., 12.), SAND);
            v.block((4., 18., 1.6), (14., 0.6, 8.), DARK);
            v.block((20., 18., 5.), (5., 0.5, 4.), STEEL);
            v.block((-1., -1., 13.6), (30., 20., 2.), OLIVE);
            for i in 0..7 {
                v.block(
                    (i as f32 * 4.2, -0.9, 15.4),
                    (0.8, 19.8, 0.6),
                    shade(OLIVE, 0.88),
                );
            }
            v.block((21., 3., 15.5), (3., 3., 5.), DARK);
            v.block((0., 20., 0.), (6., 5., 4.), SAND);
        }
        "refinery" => {
            v.block((-3., -2., 0.), (32., 23., 2.), DARK);
            v.barrel(6., 6., 2., 5.7, 20., STEEL);
            v.barrel(6., 6., 16., 5.95, 1.2, BRASS);
            v.barrel(19., 9., 2., 4.8, 12., OLIVE);
            v.barrel(19., 9., 12., 5.05, 1.1, BRASS);
            v.block((6., 5., 20.), (15., 1.9, 1.9), BRASS);
            v.block((19., 5., 13.), (1.9, 1.9, 8.5), BRASS);
            v.block((1., 16., 2.), (8., 4., 4.), DARK);
        }
        "refine" => {
            v.block((0., 0., 0.), (16., 8., 17.), OLIVE);
            v.block((2., 1., 17.), (2., 5., 4.), OLIVE);
            v.block((11., 1., 17.), (2., 5., 4.), OLIVE);
            v.block((2., 1., 20.), (11., 5., 2.), OLIVE);
            v.barrel(14., 5., 17., 2.2, 2., BRASS);
            v.block((6., 8., 4.), (3., 0.55, 9.), BRASS);
        }
        "ammo" | "shells" => {
            v.block((0., 0., 0.), (25., 13., 7.), OLIVE);
            v.block((0., 0., 7.), (25., 1.8, 10.), shade(OLIVE, 0.8));
            for i in 0..4 {
                v.barrel(
                    3.5 + i as f32 * 5.5,
                    7.,
                    7.,
                    1.6,
                    if id == "shells" { 11. } else { 6. },
                    BRASS,
                );
                v.barrel(
                    3.5 + i as f32 * 5.5,
                    7.,
                    if id == "shells" { 18. } else { 13. },
                    1.2,
                    2.,
                    STEEL,
                );
            }
            v.block((4., 13., 1.), (2., 0.6, 4.), STEEL);
            v.block((19., 13., 1.), (2., 0.6, 4.), STEEL);
        }
        "grenade" => {
            v.barrel(8., 7., 0., 4.5, 2.5, OLIVE);
            v.barrel(8., 7., 2.4, 6., 9., OLIVE);
            v.barrel(8., 7., 11.4, 4.7, 3., OLIVE);
            v.barrel(8., 7., 14.4, 2.8, 3., DARK);
            v.block((5., 5., 17.), (10., 3., 1.8), STEEL);
            v.block((13.5, 5., 9.), (1.6, 3., 9.), STEEL);
            v.barrel(8., 7., 6., 6.1, 0.8, shade(OLIVE, 0.67));
        }
        "service" => {
            v.block((0., 0., 0.), (21., 11., 8.), shade(STEEL, 0.72));
            v.block((-1., -1., 8.), (23., 13., 2.), STEEL);
            v.block((5., 4., 10.), (2., 3., 3.), DARK);
            v.block((15., 4., 10.), (2., 3., 3.), DARK);
            v.block((5., 4., 12.5), (12., 3., 1.5), DARK);
            v.block((9., 11., 3.), (3., 1., 5.), BRASS);
        }
        "cargo_rack" => {
            v.block((0., 0., 0.), (25., 15., 2.), DARK);
            for z in [0., 13.5] {
                for x in [0., 12., 23.5] {
                    v.block((x, z, 2.), (1.5, 1.5, 9.), STEEL);
                }
                for h in [5., 10.] {
                    v.block((0., z, h), (25., 1.5, 1.), OLIVE);
                }
            }
        }
        "aux_fuel" => {
            v.block((0., 0., 0.), (24., 12., 2.), DARK);
            v.block((1., 1., 2.), (22., 10., 10.), OLIVE);
            for x in [5., 17.] {
                v.block((x, 0.8, 2.), (1.8, 10.4, 10.4), STEEL);
            }
            v.barrel(11., 4., 12., 2., 1., DARK);
        }
        "belt" => {
            for i in 0..7 {
                let x = i as f32 * 2.8;
                let z = if i > 3 { (i - 3) as f32 * 1.3 } else { 0. };
                v.block((x, z, 0.), (2., 9., 2.), BRASS);
                v.block((x - 0.45, z + 3., 0.7), (3., 3., 1.7), DARK);
            }
        }
        "mobility" => {
            for i in 0..2 {
                v.barrel(7. + i as f32 * 10., 7., 0., 6.5, 6., DARK);
                v.barrel(7. + i as f32 * 10., 7., 6., 3.8, 0.9, STEEL);
                v.barrel(7. + i as f32 * 10., 7., 6.9, 1.6, 0.5, BRASS);
            }
            v.block((3., 5., 7.4), (17., 4., 2.), STEEL);
        }
        _ => {
            for i in 0..3 {
                v.block(
                    (i as f32 * 1.3, i as f32 * 1.4, i as f32 * 3.),
                    (22., 12., 2.7),
                    if i == 2 { OLIVE } else { STEEL },
                );
            }
            v.barrel(5., 5., 8.7, 1., 0.8, BRASS);
            v.barrel(19., 10., 8.7, 1., 0.8, BRASS);
        }
    }
}
