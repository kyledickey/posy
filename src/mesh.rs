//! Triangle meshes, and the few ways we build them: curved patches (petals,
//! leaves, paper), tubes (stems, ribbon) and little blobs (buds, florets).

use std::f32::consts::{PI, TAU};

use crate::color::Col;
use crate::math::{Vec3, basis};

/// How a surface is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mat {
    Solid,
    /// See-through. The number is how opaque it is where it faces you; the
    /// edges, where you look along the glass, are always stronger.
    Glass(f32),
}

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: Vec3,
    pub normal: Vec3,
    pub color: Col,
    /// Multiplies the lighting: below 1 for creases and shaded hollows.
    pub shade: f32,
    /// When this point appears while the bouquet grows in.
    pub born: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub v: [u32; 3],
    /// Which petal/leaf/stem this belongs to. Where two different parts
    /// overlap on screen, the one behind gets a soft shadow line.
    pub part: u16,
    pub mat: Mat,
    /// Scales how dense a character this surface is drawn with. Petals use
    /// the full range; leaves and paper are kept lighter so blooms stand out.
    pub ink: f32,
}

/// One point of a patch, as returned by the closure given to [`Mesh::patch`].
pub struct P {
    pub pos: Vec3,
    pub color: Col,
    pub shade: f32,
}

impl P {
    pub fn new(pos: Vec3, color: Col) -> P {
        P {
            pos,
            color,
            shade: 1.0,
        }
    }

    pub fn shade(self, shade: f32) -> P {
        P { shade, ..self }
    }
}

/// Options for [`Mesh::patch`].
#[derive(Clone, Copy)]
pub struct Patch {
    /// Grid resolution along `u` and `v`.
    pub nu: usize,
    pub nv: usize,
    /// The `v` direction wraps around (tubes, vases).
    pub closed: bool,
    pub mat: Mat,
    /// Points appear at `born + u * grow`.
    pub born: f32,
    pub grow: f32,
    pub ink: f32,
}

impl Patch {
    pub fn new(nu: usize, nv: usize) -> Patch {
        Patch {
            nu,
            nv,
            closed: false,
            mat: Mat::Solid,
            born: 0.0,
            grow: 0.0,
            ink: 1.0,
        }
    }

    pub fn ink(self, ink: f32) -> Patch {
        Patch { ink, ..self }
    }

    pub fn closed(self) -> Patch {
        Patch {
            closed: true,
            ..self
        }
    }

    pub fn mat(self, mat: Mat) -> Patch {
        Patch { mat, ..self }
    }

    pub fn born(self, born: f32, grow: f32) -> Patch {
        Patch { born, grow, ..self }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub verts: Vec<Vertex>,
    pub tris: Vec<Tri>,
    parts: u16,
}

impl Mesh {
    pub fn clear(&mut self) {
        self.verts.clear();
        self.tris.clear();
        self.parts = 0;
    }

    /// How many parts have been handed out, so several meshes drawn together
    /// can keep their part numbers apart.
    pub fn parts(&self) -> u16 {
        self.parts
    }

    /// A curved sheet. `f(u, v)` is called on a grid with `u` in 0..=1 and
    /// `v` in -1..=1 (or 0..1 around, when closed). Normals come from the
    /// shape itself.
    pub fn patch(&mut self, o: Patch, mut f: impl FnMut(f32, f32) -> P) -> u16 {
        self.parts += 1;
        let part = self.parts;
        let (nu, nv) = (o.nu.max(1), o.nv.max(if o.closed { 3 } else { 1 }));
        let cols = if o.closed { nv } else { nv + 1 };
        let first = self.verts.len();

        for i in 0..=nu {
            let u = i as f32 / nu as f32;
            for j in 0..cols {
                let v = if o.closed {
                    j as f32 / nv as f32
                } else {
                    j as f32 / nv as f32 * 2.0 - 1.0
                };
                let p = f(u, v);
                self.verts.push(Vertex {
                    pos: p.pos,
                    normal: Vec3::UP,
                    color: p.color,
                    shade: p.shade,
                    born: o.born + u * o.grow,
                });
            }
        }

        let at = |i: usize, j: usize| first + i * cols + j;
        // Normals: the cross product of the two grid directions.
        for i in 0..=nu {
            for j in 0..cols {
                let (i0, i1) = (i.saturating_sub(1), (i + 1).min(nu));
                let (j0, j1) = if o.closed {
                    ((j + cols - 1) % cols, (j + 1) % cols)
                } else {
                    (j.saturating_sub(1), (j + 1).min(cols - 1))
                };
                let du = self.verts[at(i1, j)].pos - self.verts[at(i0, j)].pos;
                let dv = self.verts[at(i, j1)].pos - self.verts[at(i, j0)].pos;
                let n = du.cross(dv);
                self.verts[at(i, j)].normal = if n.length() > 1e-9 {
                    n.normalized()
                } else {
                    Vec3::default()
                };
            }
        }
        // Where the sheet pinches to a point there is no normal; borrow the
        // neighbouring row's.
        for i in 0..=nu {
            for j in 0..cols {
                if self.verts[at(i, j)].normal == Vec3::default() {
                    let other = if i == 0 { 1 } else { i - 1 };
                    let n = self.verts[at(other, j)].normal;
                    self.verts[at(i, j)].normal = if n == Vec3::default() { Vec3::UP } else { n };
                }
            }
        }

        let quads = if o.closed { cols } else { cols - 1 };
        for i in 0..nu {
            for j in 0..quads {
                let j1 = (j + 1) % cols;
                let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i + 1, j1), at(i, j1));
                for v in [[a, b, c], [a, c, d]] {
                    self.tris.push(Tri {
                        v: v.map(|x| x as u32),
                        part,
                        mat: o.mat,
                        ink: o.ink,
                    });
                }
            }
        }
        part
    }

    /// A round tube along `path`. `radius(t)` and `color(t)` run 0..1 along it.
    pub fn tube(
        &mut self,
        path: &[Vec3],
        sides: usize,
        o: Patch,
        radius: impl Fn(f32) -> f32,
        color: impl Fn(f32) -> Col,
    ) -> u16 {
        let n = path.len() - 1;
        self.patch(
            Patch {
                nu: n,
                nv: sides,
                closed: true,
                ..o
            },
            |u, v| {
                let i = ((u * n as f32).round() as usize).min(n);
                let dir = (path[(i + 1).min(n)] - path[i.saturating_sub(1)]).normalized();
                let (a, b) = basis(dir);
                let ang = -v * TAU; // this way round, normals point outward
                let out = a * ang.cos() + b * ang.sin();
                P::new(path[i] + out * radius(u), color(u))
            },
        )
    }

    /// A small ball, stretched by `stretch` along `axis`.
    pub fn blob(&mut self, center: Vec3, r: f32, axis: Vec3, stretch: f32, color: Col, o: Patch) {
        let (a, b) = basis(axis);
        self.patch(
            Patch {
                nu: 4,
                nv: 6,
                closed: true,
                ..o
            },
            |u, v| {
                let (lat, lon) = (u * PI, v * TAU);
                let ring = a * lon.cos() + b * lon.sin();
                let p = center + (ring * lat.sin() + axis * (lat.cos() * stretch)) * r;
                P::new(p, color)
            },
        );
    }
}
