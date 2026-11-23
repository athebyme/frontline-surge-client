//! One bounded ground-debris mesh per damaged site. The reliable world carries
//! its final positions; only a client which witnessed the change plays the fall.
use super::*;
use frontline_surge::rubble::{self, Piece};
use std::collections::HashMap;

struct Falling {
    piece: Piece,
    since: Option<f64>,
}
struct Site {
    stamp: (u64, u64, bool),
    rocks: Vec<Falling>,
    page: Page,
    motion: bool,
}
#[derive(Default)]
pub(super) struct Rubble {
    sites: HashMap<(bool, usize), Site>,
}
impl Site {
    fn new(gpu: &mut dyn mq::RenderingBackend) -> Self {
        let page = Page {
            vertices: gpu.new_buffer(
                mq::BufferType::VertexBuffer,
                mq::BufferUsage::Dynamic,
                mq::BufferSource::empty::<Vertex>(rubble::LIMIT * 24),
            ),
            indices: gpu.new_buffer(
                mq::BufferType::IndexBuffer,
                mq::BufferUsage::Dynamic,
                mq::BufferSource::empty::<u16>(rubble::LIMIT * 36),
            ),
            count: 0,
            low: Vec3::ZERO,
            high: Vec3::ZERO,
            kind: PageKind::Template,
        };
        Self {
            stamp: (u64::MAX, 0, false),
            rocks: vec![],
            page,
            motion: false,
        }
    }
    fn update(&mut self, now: f64, gpu: &mut dyn mq::RenderingBackend) {
        self.motion = false;
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        let mut verts = Vec::with_capacity(self.rocks.len() * 24);
        let mut indices = Vec::with_capacity(self.rocks.len() * 36);
        for rock in &mut self.rocks {
            let p = &rock.piece;
            let [x, z] = rubble::center(p);
            let dims = Vec3::from_array(rubble::size(p));
            let end = vec3(x, dims.y * 0.5 + 0.018, z);
            let start = vec3(p[3] as f32, p[4] as f32, p[5] as f32) / 32.;
            let yaw = rubble::yaw(p);
            let (center, spin) = if let Some(since) = rock.since {
                let seconds = (now - since).max(0.) as f32;
                let drop_t = ((2. * (start.y - end.y).max(0.08) / 10.).sqrt()).max(0.25);
                let t = (seconds / drop_t).min(1.);
                let mut center = start.lerp(end, t);
                center.y = start.y + (end.y - start.y) * t * t;
                let bounce = ((seconds - drop_t) / 0.36).clamp(0., 1.);
                center.y += dims.y * 1.3 * (4. * bounce * (1. - bounce));
                if bounce >= 1. {
                    rock.since = None;
                } else {
                    self.motion = true;
                }
                (
                    center,
                    (1. - t) * 1.6 + 0.22 * (bounce * std::f32::consts::PI).sin(),
                )
            } else {
                (end, 0.)
            };
            let rot = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(spin);
            let k = p[6] as u32 >> 3;
            let material = rubble::material(p);
            let shading = mat_style(material);
            let grain = 0.82 + rubble::unit(k ^ 11) * 0.28;
            for face in 0..6 {
                let corners = faces_for(Vec3::ZERO, dims, face).map(|mut q| {
                    // Taper the top and offset it slightly to give the fractured edges a seam.
                    if q.y > 0. {
                        q.x = q.x * 0.77 + dims.x * 0.065;
                        q.z = q.z * 0.84 - dims.z * 0.058;
                    }
                    center + rot * q
                });
                let normal = (corners[1] - corners[0])
                    .cross(corners[2] - corners[0])
                    .normalize_or_zero();
                let base = verts.len() as u16;
                let style = tone(shading, grain * if face == 2 { 1.10 } else { 1. });
                for point in corners {
                    low = low.min(point);
                    high = high.max(point);
                    let uv = match face {
                        0 | 1 => [point.z, point.y],
                        2 | 3 => [point.x, point.z],
                        _ => [point.x, point.y],
                    };
                    verts.push(Vertex {
                        p: point.to_array(),
                        n: normal.to_array(),
                        uv,
                        style,
                    });
                }
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
        self.page.count = indices.len() as i32;
        self.page.low = low;
        self.page.high = high;
        if !indices.is_empty() {
            gpu.buffer_update(self.page.vertices, mq::BufferSource::slice(&verts));
            gpu.buffer_update(self.page.indices, mq::BufferSource::slice(&indices));
        }
    }
    fn release(self, gpu: &mut dyn mq::RenderingBackend) {
        gpu.delete_buffer(self.page.vertices);
        gpu.delete_buffer(self.page.indices);
    }
}
impl Rubble {
    pub(super) fn clear(&mut self, gpu: &mut dyn mq::RenderingBackend) {
        for (_, site) in self.sites.drain() {
            site.release(gpu);
        }
    }
    pub(super) fn sync(
        &mut self,
        state: &Value,
        witnessed: bool,
        gpu: &mut dyn mq::RenderingBackend,
        now: f64,
    ) {
        for (dynamic, key) in [(false, "propChanges"), (true, "builtProps")] {
            let list = state[key].as_array().map(Vec::as_slice).unwrap_or(&[]);
            if dynamic {
                let stale: Vec<_> = self
                    .sites
                    .keys()
                    .filter(|(d, i)| *d && *i >= list.len())
                    .copied()
                    .collect();
                for id in stale {
                    self.sites.remove(&id).unwrap().release(gpu);
                }
            }
            for (idx, v) in list.iter().enumerate() {
                let id = (
                    dynamic,
                    if dynamic {
                        idx
                    } else if let Some(i) = v["index"].as_u64() {
                        i as usize
                    } else {
                        continue;
                    },
                );
                let stamp = (
                    v["buildId"].as_u64().unwrap_or(0),
                    v["revision"].as_u64().unwrap_or(0),
                    dead(v),
                );
                if self.sites.get(&id).is_some_and(|s| s.stamp == stamp) {
                    continue;
                }
                let Some(rows) = v["rubble"].as_array().filter(|a| !a.is_empty()) else {
                    if let Some(site) = self.sites.remove(&id) {
                        site.release(gpu);
                    }
                    continue;
                };
                let site = self.sites.entry(id).or_insert_with(|| Site::new(gpu));
                let old = std::mem::take(&mut site.rocks);
                for row in rows.iter().take(rubble::LIMIT) {
                    let Ok(piece) = serde_json::from_value::<Piece>(row.clone()) else {
                        continue;
                    };
                    let since = if let Some(last) = old
                        .iter()
                        .find(|r| r.piece == piece && site.stamp.0 == stamp.0)
                    {
                        last.since
                    } else {
                        witnessed.then_some(now)
                    };
                    site.rocks.push(Falling { piece, since });
                }
                site.stamp = stamp;
                site.update(now, gpu);
            }
        }
        for site in self.sites.values_mut().filter(|s| s.motion) {
            site.update(now, gpu);
        }
    }
    pub(super) fn visit(
        &self,
        planes: &[Vec4; 6],
        eye: Vec3,
        draw: &mut impl FnMut(&Page, Mat4, f32),
    ) {
        for s in self.sites.values() {
            let p = &s.page;
            if p.count > 0
                && ((p.low + p.high) * 0.5).distance_squared(eye) < 125. * 125.
                && in_frustum(planes, p.low, p.high)
            {
                draw(p, Mat4::IDENTITY, 1.);
            }
        }
    }
}
