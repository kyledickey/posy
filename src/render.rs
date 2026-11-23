//! Draws a [`Scene`] onto a [`Canvas`] through a [`Camera`]: move every
//! point into view, light it, fill the triangles, then let [`Frame`] turn
//! the picture into characters.

use crate::camera::Camera;
use crate::canvas::Canvas;
use crate::color::Col;
use crate::color::lit;
use crate::flowers::BLOOM_TIME;
use crate::math::{Vec3, clamp01};
use crate::mesh::{Mat, Mesh, Vertex};
use crate::raster::{Frame, Pv, SX, SY, Shape, Style};
use crate::scene::Scene;

/// Direction the light comes from, in camera space: upper left, toward you.
/// It stays put while the bouquet turns, so the bouquet is always well lit.
const LIGHT: Vec3 = Vec3::new(-0.350_5, 0.500_7, 0.791_6);

/// Part numbers reserved for each flower, so petals of neighbouring
/// flowers never share one.
const PARTS_PER_FLOWER: u16 = 200;

pub struct Renderer {
    frame: Frame,
    points: Vec<Pv>,
    solid: Vec<Shape>,
    /// Glass, with how far away each piece is.
    panes: Vec<(f32, Shape)>,
    glass: Vec<Shape>,
    scratch: Mesh,
    /// Drawing a picture rather than characters: colour carries the shading.
    picture: bool,
}

impl Renderer {
    pub fn new() -> Renderer {
        Renderer {
            frame: Frame::new(),
            points: Vec::new(),
            solid: Vec::new(),
            panes: Vec::new(),
            glass: Vec::new(),
            scratch: Mesh::default(),
            picture: false,
        }
    }

    /// Draws everything that has been "born" by `anim` seconds.
    pub fn draw(
        &mut self,
        canvas: &mut Canvas,
        scene: &Scene,
        cam: &Camera,
        anim: f32,
        style: Style,
        back: Col,
    ) {
        self.frame.begin(canvas.width(), canvas.height());
        self.frame.style = style;
        self.frame.back = back;
        self.picture = style.is_picture();
        self.frame.flower_parts = scene.base.parts() + 1;
        self.frame.per_flower = PARTS_PER_FLOWER;
        self.solid.clear();
        self.panes.clear();
        self.mesh(&scene.base, cam, anim, 0);

        let mut scratch = std::mem::take(&mut self.scratch);
        for (i, (flower, bloom)) in scene.flowers.iter().zip(&scene.blooms).enumerate() {
            let since = anim - flower.bloom_at;
            let parts = scene.base.parts() + 1 + i as u16 * PARTS_PER_FLOWER;
            if since >= BLOOM_TIME {
                self.mesh(bloom, cam, f32::INFINITY, parts);
            } else {
                scratch.clear();
                if flower.build(&mut scratch, since) {
                    self.mesh(&scratch, cam, f32::INFINITY, parts);
                }
            }
        }
        self.scratch = scratch;

        // Glass goes on last, farthest first, so it tints what's inside.
        self.panes.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.glass.clear();
        self.glass.extend(self.panes.iter().map(|p| p.1));
        self.frame.fill(&self.solid, &self.glass);
        self.frame.shadow_edges();
        self.frame.resolve(canvas);
    }

    fn mesh(&mut self, mesh: &Mesh, cam: &Camera, anim: f32, parts: u16) {
        self.points.clear();
        let picture = self.picture;
        self.points
            .extend(mesh.verts.iter().map(|v| light(cam, v, picture)));
        for tri in &mesh.tris {
            let [i, j, k] = tri.v.map(|x| x as usize);
            let born = mesh.verts[i]
                .born
                .max(mesh.verts[j].born)
                .max(mesh.verts[k].born);
            if born > anim {
                continue;
            }
            let (mut a, mut b, mut c) = (self.points[i], self.points[j], self.points[k]);
            let part = tri.part + parts;
            for p in [&mut a, &mut b, &mut c] {
                p.lum *= tri.ink;
            }
            match tri.mat {
                Mat::Solid => self.solid.push(Shape {
                    corners: [a, b, c],
                    part,
                }),
                Mat::Glass(opacity) => {
                    // Glass: `alpha` arrives holding how edge-on the
                    // surface is; turn that into how much it shows.
                    for p in [&mut a, &mut b, &mut c] {
                        let edge = p.alpha;
                        p.alpha = opacity + (1.0 - opacity) * 0.85 * edge.powi(3);
                        p.lum = 0.12 + 0.75 * edge * edge;
                    }
                    let depth = (a.z + b.z + c.z) / 3.0;
                    self.panes.push((
                        depth,
                        Shape {
                            corners: [a, b, c],
                            part,
                        },
                    ));
                }
            }
        }
    }
}

/// Projects and lights one point. For glass, `alpha` comes back as how
/// edge-on the surface is (0 facing you, 1 seen along its edge).
fn light(cam: &Camera, v: &Vertex, picture: bool) -> Pv {
    let p = cam.project(v.pos);
    let mut n = cam.rotate(v.normal);
    // Surfaces are lit on both sides; the underside is a little dimmer.
    let back = n.z < 0.0;
    if back {
        n = -n;
    }
    let half = n.dot(LIGHT) * 0.5 + 0.5;
    let mut b = (0.22 + 0.78 * half * half) * v.shade;
    if back {
        b *= 0.85;
    }
    // Things on the far side of the bouquet fall away into shadow.
    b *= 0.84 + 0.16 * clamp01((p.z + 0.7) / 1.2);
    let b = clamp01(b);
    Pv {
        x: p.x * SX as f32,
        y: p.y * SY as f32,
        z: p.z,
        // Characters carry the shading, so colors can stay vivid. In a
        // picture the colour has to do it all.
        color: if picture {
            lit(v.color, 0.26 + 0.74 * b)
        } else {
            lit(v.color, 0.36 + 0.64 * b)
        },
        lum: b,
        alpha: 1.0 - n.z,
    }
}
