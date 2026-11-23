//! The flowers themselves, modelled petal by petal.
//!
//! Every flower is built in its own little space: the stem tip is the
//! origin, `y` points along the way the flower faces, and the flower is
//! about one unit across. A [`Petal`] is a curved sheet described by a
//! handful of numbers (how long, how wide, how it tilts from base to tip,
//! how much it wraps around the middle), and each kind of flower is mostly
//! a recipe for arranging petals.
//!
//! `open` runs from 0 (a closed bud) to 1 (full bloom): opening is just
//! every petal leaning out from upright toward its final tilt.

use std::f32::consts::{PI, TAU};

use crate::bouquet::FlowerKind;
use crate::color::{Col, dim, mix};
use crate::math::{Vec3, basis, clamp01, ease_out, lerp, smoothstep};
use crate::mesh::{Mesh, P, Patch};
use crate::rng::Rng;

/// How long a flower takes to open, and how long it shows as a bud first.
pub const BLOOM_TIME: f32 = 1.6;
pub const BUD_TIME: f32 = 0.8;

/// The angle that spreads petals evenly in a spiral (about 137.5 degrees).
const GOLDEN: f32 = 2.399_963;

const WHITE: Col = [1.0, 0.98, 0.95];
const PLUM: Col = [0.25, 0.06, 0.22];
/// Leaves and stems are drawn with lighter characters than petals.
pub const LEAF_INK: f32 = 0.5;

pub const GREEN: Col = [0.24, 0.62, 0.28];
const DARK_GREEN: Col = [0.13, 0.42, 0.20];
const LIME: Col = [0.62, 0.80, 0.32];
const GOLD: Col = [0.96, 0.74, 0.16];

/// One flower in the bouquet.
#[derive(Clone, Debug)]
pub struct Flower {
    pub kind: FlowerKind,
    /// The stem tip, in world space.
    pub pos: Vec3,
    /// Which way the flower faces.
    pub axis: Vec3,
    /// Size in world units.
    pub radius: f32,
    pub spin: f32,
    pub petals: u32,
    pub petal: Col,
    pub center: Col,
    /// When the bud opens. It appears as a bud a little before this.
    pub bloom_at: f32,
    pub seed: u64,
}

impl Flower {
    /// Builds the flower as it looks `since` seconds after it started to
    /// open (negative while it is still a bud). Returns false if it hasn't
    /// appeared yet.
    pub fn build(&self, mesh: &mut Mesh, since: f32) -> bool {
        if since < -BUD_TIME {
            return false;
        }
        let (size, open) = if since < 0.0 {
            (0.30 + 0.25 * (since + BUD_TIME) / BUD_TIME, 0.0)
        } else {
            let open = ease_out(since / BLOOM_TIME);
            (0.55 + 0.45 * open, open)
        };
        let mut cx = Ctx::new(mesh, self.pos, self.axis, self.spin, self.radius * size);
        cx.open = open;
        cx.rng = Rng::new(self.seed);
        match self.kind {
            FlowerKind::Rose => rose(&mut cx, self),
            FlowerKind::Tulip => tulip(&mut cx, self),
            FlowerKind::Daisy => daisy(&mut cx, self),
            FlowerKind::Sunflower => sunflower(&mut cx, self),
            FlowerKind::Lily => lily(&mut cx, self),
            FlowerKind::Cosmos => cosmos(&mut cx, self),
            FlowerKind::Carnation => carnation(&mut cx, self),
            FlowerKind::Peony => peony(&mut cx, self),
            FlowerKind::Lavender => lavender(&mut cx, self),
            FlowerKind::BabysBreath => babys_breath(&mut cx, self),
        }
        true
    }
}

/// Where we are building: a mesh, and the little space the flower lives in.
struct Ctx<'a> {
    mesh: &'a mut Mesh,
    origin: Vec3,
    x: Vec3,
    y: Vec3,
    z: Vec3,
    scale: f32,
    open: f32,
    born: f32,
    grow: f32,
    ink: f32,
    rng: Rng,
}

impl<'a> Ctx<'a> {
    fn new(mesh: &'a mut Mesh, origin: Vec3, axis: Vec3, spin: f32, scale: f32) -> Ctx<'a> {
        let (a, _) = basis(axis);
        let c = a.cross(axis);
        let x = a * spin.cos() + c * spin.sin();
        Ctx {
            mesh,
            origin,
            x,
            y: axis,
            z: x.cross(axis),
            scale,
            open: 1.0,
            born: 0.0,
            grow: 0.0,
            ink: 1.0,
            rng: Rng::new(0),
        }
    }

    /// Flower space to world space.
    fn at(&self, x: f32, y: f32, z: f32) -> Vec3 {
        self.origin + (self.x * x + self.y * y + self.z * z) * self.scale
    }

    fn patch(&self, nu: usize, nv: usize) -> Patch {
        Patch::new(nu, nv).born(self.born, self.grow).ink(self.ink)
    }
}

/// The shape of one petal (or leaf). Lengths are in flower units; angles
/// are degrees above flat, so 0 lies flat and 90 stands straight up.
#[derive(Clone, Copy)]
struct Petal {
    /// Direction around the flower, in radians.
    az: f32,
    /// Where the base sits: distance from the middle, and height.
    r0: f32,
    y0: f32,
    len: f32,
    /// Half the width at the widest point.
    width: f32,
    tilt0: f32,
    tilt1: f32,
    /// 1 wraps the petal around the middle of the flower like a cup;
    /// 0 leaves it flat.
    wrap: f32,
    /// Curls the edges up (or down, if negative).
    cup: f32,
    /// How far along its length the petal is widest, 0..1.
    widest: f32,
    /// Tip shape: below 1 is rounded, above 1 is pointed.
    point: f32,
    /// Wavy edges.
    ruffle: f32,
    /// Little notches along the tip.
    teeth: f32,
    nu: usize,
    nv: usize,
}

const PETAL: Petal = Petal {
    az: 0.0,
    r0: 0.1,
    y0: 0.1,
    len: 1.0,
    width: 0.3,
    tilt0: 30.0,
    tilt1: 10.0,
    wrap: 0.0,
    cup: 0.0,
    widest: 0.55,
    point: 0.8,
    ruffle: 0.0,
    teeth: 0.0,
    nu: 7,
    nv: 4,
};

/// Adds one petal. `paint(u, v)` gives the color and shade at a point,
/// with `u` running base to tip and `v` side to side.
fn petal(cx: &mut Ctx, p: &Petal, paint: impl Fn(f32, f32) -> (Col, f32)) {
    let open = cx.open;
    let t0 = lerp(87.0, p.tilt0, open).to_radians();
    let t1 = lerp(97.0, p.tilt1, open).to_radians();
    let len = p.len * (0.6 + 0.4 * open);
    let phase = cx.rng.range(0.0, TAU);

    // Walk the midline from base to tip, bending as we go.
    let n = p.nu;
    let mut line = Vec::with_capacity(n + 1);
    let (mut r, mut y) = (p.r0, p.y0);
    for i in 0..=n {
        let u = i as f32 / n as f32;
        let th = t0 + (t1 - t0) * u.powf(1.4);
        line.push((r, y, th));
        r = (r + th.cos() * len / n as f32).max(0.004);
        y += th.sin() * len / n as f32;
    }

    let fat = 0.5f32.ln() / p.widest.clamp(0.05, 0.95).ln();
    let (ca, sa) = (p.az.cos(), p.az.sin());
    let opts = cx.patch(p.nu, p.nv);
    let (origin, ex, ey, ez, scale) = (cx.origin, cx.x, cx.y, cx.z, cx.scale);
    cx.mesh.patch(opts, |u, v| {
        let v = -v; // so the normal points out of the petal's upper face
        let (r, y, th) = line[((u * n as f32).round() as usize).min(n)];
        let w = p.width * (PI * u.powf(fat)).sin().max(0.0).powf(p.point);

        // Across the petal: an arc around the flower's middle.
        let (mut rad, tan) = if p.wrap > 0.01 {
            let rho = (r / p.wrap).max(0.02);
            let psi = (v * w / rho).clamp(-2.9, 2.9);
            (r - rho + rho * psi.cos(), rho * psi.sin())
        } else {
            (r, v * w)
        };
        let lift =
            p.cup * w * v * v + p.ruffle * p.width * (v * 6.0 + u * 5.0 + phase).sin() * u * u;
        let notch = p.teeth * len * (v * 3.5 * PI).cos().abs() * u.powi(4);
        rad += -th.sin() * lift - th.cos() * notch;
        let y = y + th.cos() * lift - th.sin() * notch;

        let (x, z) = (rad * ca - tan * sa, rad * sa + tan * ca);
        let (color, shade) = paint(u, v);
        P::new(origin + (ex * x + ey * y + ez * z) * scale, color).shade(shade)
    });
}

/// A darker, richer version of a petal color, for the heart of a flower.
fn deepen(c: Col) -> Col {
    mix(dim(c, 0.62), PLUM, 0.18)
}

/// Green sepals under the head, and the little swelling they grow from.
fn sepals(cx: &mut Ctx, count: usize, len: f32, tilt0: f32, tilt1: f32) {
    let open = cx.open;
    for i in 0..count {
        let p = Petal {
            az: i as f32 / count as f32 * TAU + 0.3,
            r0: 0.05,
            y0: 0.0,
            len,
            width: len * 0.3,
            // Sepals hug the bud, then fold back as it opens.
            tilt0: lerp(80.0, tilt0, open),
            tilt1: lerp(92.0, tilt1, open),
            widest: 0.35,
            point: 1.2,
            cup: 0.3,
            nu: 4,
            nv: 2,
            ..PETAL
        };
        // The bud lerp inside `petal` would stand them up again; they are
        // already placed, so build them as if fully open.
        let saved = (cx.open, cx.ink);
        (cx.open, cx.ink) = (1.0, LEAF_INK);
        petal(cx, &p, |u, _| (mix(DARK_GREEN, GREEN, u), 0.75 + 0.25 * u));
        (cx.open, cx.ink) = saved;
    }
    let c = cx.at(0.0, 0.05, 0.0);
    cx.mesh.blob(
        c,
        0.13 * cx.scale,
        cx.y,
        1.0,
        DARK_GREEN,
        cx.patch(0, 0).ink(LEAF_INK),
    );
}

/// A low dome in the middle of a flower. `paint(r, a)` colors it, with `r`
/// from the middle (0) to the rim (1) and `a` around.
fn dome(cx: &mut Ctx, radius: f32, height: f32, y: f32, paint: impl Fn(f32, f32) -> (Col, f32)) {
    let (origin, ex, ey, ez, scale) = (cx.origin, cx.x, cx.y, cx.z, cx.scale);
    let opts = cx.patch(6, 18).closed();
    cx.mesh.patch(opts, |u, v| {
        let a = -v * TAU;
        let (x, z) = (radius * u * a.cos(), radius * u * a.sin());
        let h = y + height * (1.0 - u * u);
        let (color, shade) = paint(u, v);
        P::new(origin + (ex * x + ey * h + ez * z) * scale, color).shade(shade)
    });
}

fn rose(cx: &mut Ctx, f: &Flower) {
    sepals(cx, 5, 0.5, 30.0, -25.0);
    let (deep, pale) = (deepen(f.petal), mix(f.petal, WHITE, 0.35));
    let n = 17;
    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let tone = cx.rng.range(-0.05, 0.05);
        let p = Petal {
            az: i as f32 * GOLDEN + cx.rng.range(-0.15, 0.15),
            r0: 0.03 + 0.17 * t,
            y0: 0.10,
            len: 0.60 + 0.46 * t,
            width: 0.30 + 0.42 * t,
            // The heart stays a tight upright spiral; outer petals lean
            // out and roll their tips over.
            tilt0: lerp(87.0, 58.0, t),
            tilt1: lerp(84.0, 2.0, t * t),
            wrap: 1.0 - 0.2 * t,
            widest: 0.62,
            point: 0.55,
            ruffle: 0.04 * t,
            nu: 8,
            nv: 6,
            ..PETAL
        };
        let body = mix(deep, f.petal, 0.3 + 0.7 * t);
        petal(cx, &p, |u, _| {
            let rim = smoothstep(0.80, 0.97, u);
            let shade = (0.50 + 0.42 * smoothstep(0.0, 0.6, u)) * (0.88 + 0.12 * t);
            (mix(body, pale, 0.5 * rim), shade + 0.30 * rim + tone)
        });
    }
}

fn peony(cx: &mut Ctx, f: &Flower) {
    sepals(cx, 5, 0.45, 20.0, 5.0);
    let (deep, pale) = (deepen(f.petal), mix(f.petal, WHITE, 0.4));
    let n = 24;
    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let tone = cx.rng.range(-0.06, 0.06);
        let p = Petal {
            az: i as f32 * GOLDEN + cx.rng.range(-0.2, 0.2),
            r0: 0.03 + 0.14 * t,
            y0: 0.10,
            len: 0.50 + 0.52 * t,
            width: 0.26 + 0.34 * t,
            // A deep bowl of crowded, crinkled petals.
            tilt0: lerp(86.0, 48.0, t),
            tilt1: lerp(78.0, 28.0, t),
            wrap: 0.9 - 0.25 * t,
            widest: 0.66,
            point: 0.5,
            ruffle: 0.07 + 0.05 * t,
            teeth: 0.04,
            nu: 7,
            nv: 6,
            ..PETAL
        };
        let body = mix(deep, f.petal, 0.25 + 0.75 * t);
        petal(cx, &p, |u, _| {
            let rim = smoothstep(0.7, 1.0, u);
            let shade = (0.40 + 0.60 * smoothstep(0.0, 0.65, u)) * (0.78 + 0.22 * t);
            (mix(body, pale, 0.5 * rim), shade + 0.12 * rim + tone)
        });
    }
}

fn carnation(cx: &mut Ctx, f: &Flower) {
    // Carnations sit on a long green cup.
    let c = cx.at(0.0, 0.0, 0.0);
    cx.mesh.blob(
        c,
        0.15 * cx.scale,
        cx.y,
        2.2,
        DARK_GREEN,
        cx.patch(0, 0).ink(LEAF_INK),
    );
    let (deep, pale) = (deepen(f.petal), mix(f.petal, WHITE, 0.45));
    let n = 30;
    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let tone = cx.rng.range(-0.07, 0.07);
        let p = Petal {
            az: i as f32 * GOLDEN + cx.rng.range(-0.2, 0.2),
            r0: 0.03 + 0.10 * t,
            y0: 0.22,
            len: 0.45 + 0.50 * t,
            width: 0.20 + 0.20 * t,
            // A ruffled pom-pom: every petal fans out a little further.
            tilt0: lerp(88.0, 40.0, t),
            tilt1: lerp(80.0, 8.0, t),
            wrap: 0.5,
            widest: 0.78,
            point: 0.35,
            ruffle: 0.16,
            teeth: 0.10,
            nu: 6,
            nv: 6,
            ..PETAL
        };
        let body = mix(deep, f.petal, 0.3 + 0.7 * t);
        petal(cx, &p, |u, _| {
            let rim = smoothstep(0.78, 1.0, u);
            let shade = (0.40 + 0.60 * smoothstep(0.0, 0.7, u)) * (0.8 + 0.2 * t);
            (mix(body, pale, 0.6 * rim), shade + 0.15 * rim + tone)
        });
    }
}

fn tulip(cx: &mut Ctx, f: &Flower) {
    let c = cx.at(0.0, 0.03, 0.0);
    cx.mesh.blob(
        c,
        0.14 * cx.scale,
        cx.y,
        1.0,
        GREEN,
        cx.patch(0, 0).ink(LEAF_INK),
    );
    let (deep, pale) = (deepen(f.petal), mix(f.petal, WHITE, 0.3));
    // Three inner petals, then three outer ones overlapping the gaps.
    for i in 0..6 {
        let outer = i >= 3;
        let tone = cx.rng.range(-0.04, 0.04);
        let p = Petal {
            az: i as f32 * TAU / 3.0 + if outer { PI / 3.0 } else { 0.0 },
            r0: if outer { 0.12 } else { 0.09 },
            y0: 0.05,
            len: 1.75,
            width: if outer { 0.64 } else { 0.58 },
            // Belly out, then curve back in to a point: the tulip egg.
            tilt0: 38.0,
            tilt1: if outer { 100.0 } else { 104.0 },
            wrap: 1.0,
            widest: 0.5,
            point: 0.85,
            nu: 9,
            nv: 6,
            ..PETAL
        };
        petal(cx, &p, |u, v| {
            let rim = smoothstep(0.8, 1.0, u);
            // A soft pale flame up the middle of each petal.
            let flame = (1.0 - v.abs()).powi(3) * smoothstep(0.1, 0.5, u) * 0.3;
            let body = mix(
                mix(deep, f.petal, smoothstep(0.0, 0.5, u)),
                pale,
                flame + 0.3 * rim,
            );
            let shade = if outer { 1.0 } else { 0.8 } + tone - 0.12 * v.abs();
            (body, shade)
        });
    }
}

fn daisy(cx: &mut Ctx, f: &Flower) {
    sepals(cx, 8, 0.4, 14.0, 8.0);
    let n = f.petals.max(8) as usize;
    let shadow = mix(f.petal, [0.55, 0.52, 0.72], 0.4);
    // Two overlapping rings of slim petals.
    for i in 0..n * 2 {
        let back = i >= n;
        let tone = cx.rng.range(-0.06, 0.06);
        let p = Petal {
            az: (i as f32 + if back { 0.5 } else { 0.0 }) / n as f32 * TAU
                + cx.rng.range(-0.04, 0.04),
            r0: 0.17,
            y0: if back { 0.10 } else { 0.13 },
            len: cx.rng.range(0.80, 0.90) * if back { 0.95 } else { 1.0 },
            width: 2.7 / n as f32,
            tilt0: if back { 6.0 } else { 16.0 },
            tilt1: if back { -6.0 } else { -2.0 },
            cup: -0.25,
            widest: 0.62,
            point: 0.55,
            nu: 5,
            nv: 2,
            ..PETAL
        };
        petal(cx, &p, |u, v| {
            let body = mix(shadow, f.petal, smoothstep(0.0, 0.35, u));
            let shade = if back { 0.72 } else { 1.0 } + tone - 0.10 * (1.0 - v.abs());
            (body, shade)
        });
    }
    let rim = mix(f.center, [0.75, 0.38, 0.05], 0.55);
    dome(cx, 0.25, 0.13, 0.14, |r, a| {
        let seeds = 0.5 + 0.5 * (a * TAU * 9.0 + r * 14.0).sin();
        (mix(f.center, rim, r * r), 0.85 + 0.2 * seeds - 0.2 * r)
    });
}

fn sunflower(cx: &mut Ctx, f: &Flower) {
    sepals(cx, 10, 0.55, 10.0, 0.0);
    let n = f.petals.max(12) as usize;
    let amber = mix(f.petal, [0.95, 0.45, 0.05], 0.5);
    for i in 0..n * 2 {
        let back = i >= n;
        let tone = cx.rng.range(-0.06, 0.06);
        let p = Petal {
            az: (i as f32 + if back { 0.5 } else { 0.0 }) / n as f32 * TAU
                + cx.rng.range(-0.05, 0.05),
            r0: 0.40,
            y0: if back { 0.08 } else { 0.11 },
            len: cx.rng.range(0.56, 0.66),
            width: 3.4 / n as f32,
            tilt0: if back { 4.0 } else { 20.0 },
            tilt1: if back { -10.0 } else { 0.0 },
            cup: 0.25,
            widest: 0.4,
            point: 1.1,
            nu: 5,
            nv: 2,
            ..PETAL
        };
        petal(cx, &p, |u, _| {
            let body = mix(amber, f.petal, smoothstep(0.0, 0.5, u));
            (body, if back { 0.7 } else { 1.0 } + tone)
        });
    }
    // The seed head: dark, with a fuzzy golden ring.
    let dark = dim(f.center, 0.75);
    dome(cx, 0.46, 0.09, 0.11, |r, a| {
        let seeds = (a * TAU * 13.0 + r * 30.0).sin() * (a * TAU * 21.0 - r * 30.0).sin();
        let ring = smoothstep(0.55, 0.8, r) * (1.0 - smoothstep(0.85, 1.0, r));
        let color = mix(mix(dark, f.center, r), GOLD, 0.45 * ring);
        (color, 0.75 + 0.3 * seeds + 0.2 * ring)
    });
}

fn cosmos(cx: &mut Ctx, f: &Flower) {
    sepals(cx, 8, 0.35, 12.0, 6.0);
    let n = f.petals.max(5) as usize;
    let (deep, pale) = (deepen(f.petal), mix(f.petal, WHITE, 0.35));
    for i in 0..n {
        let tone = cx.rng.range(-0.05, 0.05);
        let p = Petal {
            az: i as f32 / n as f32 * TAU + cx.rng.range(-0.04, 0.04),
            r0: 0.12,
            y0: 0.10 + 0.012 * (i % 2) as f32,
            len: cx.rng.range(0.86, 0.94),
            width: 2.75 / n as f32,
            tilt0: 22.0,
            tilt1: 2.0,
            cup: -0.12,
            widest: 0.72,
            point: 0.42,
            teeth: 0.07,
            nu: 6,
            nv: 6,
            ..PETAL
        };
        petal(cx, &p, |u, v| {
            // Fine pleats running the length of the petal.
            let pleat = 0.5 + 0.5 * (v * 3.0 * PI).cos();
            let body = mix(mix(deep, f.petal, smoothstep(0.0, 0.3, u)), pale, 0.3 * u);
            (body, 0.86 + 0.14 * pleat + tone)
        });
    }
    let rim = mix(f.center, [0.7, 0.35, 0.05], 0.5);
    dome(cx, 0.16, 0.12, 0.11, |r, a| {
        let seeds = 0.5 + 0.5 * (a * TAU * 7.0).sin();
        (mix(f.center, rim, r), 0.9 + 0.2 * seeds - 0.2 * r)
    });
}

fn lily(cx: &mut Ctx, f: &Flower) {
    let c = cx.at(0.0, 0.03, 0.0);
    cx.mesh.blob(
        c,
        0.10 * cx.scale,
        cx.y,
        1.6,
        GREEN,
        cx.patch(0, 0).ink(LEAF_INK),
    );
    let throat = mix(f.petal, LIME, 0.6);
    let stripe = mix(deepen(f.petal), [0.86, 0.20, 0.42], 0.55);
    let seed = cx.rng.next_u64() as u32;
    for i in 0..6 {
        let inner = i % 2 == 0;
        let p = Petal {
            az: i as f32 / 6.0 * TAU,
            r0: 0.05,
            y0: if inner { 0.08 } else { 0.05 },
            len: 1.30,
            width: if inner { 0.27 } else { 0.21 },
            // A trumpet whose tips curl right back.
            tilt0: 68.0,
            tilt1: -38.0,
            cup: 0.45,
            widest: 0.5,
            point: 1.0,
            ruffle: 0.05,
            nu: 10,
            nv: 6,
            ..PETAL
        };
        petal(cx, &p, |u, v| {
            let mid = (1.0 - v.abs() * 2.5).max(0.0) * (1.0 - smoothstep(0.55, 0.9, u));
            let freckle = crate::rng::hash01((u * 23.0) as i32 + i * 31, (v * 9.0) as i32, seed);
            let spot = if freckle > 0.8 && u > 0.15 && u < 0.6 {
                0.6
            } else {
                0.0
            };
            let body = mix(throat, f.petal, smoothstep(0.05, 0.35, u));
            let color = mix(body, stripe, (0.55 * mid + spot).min(0.8));
            (color, 0.62 + 0.38 * smoothstep(0.0, 0.4, u))
        });
    }
    // Stamens: fine stalks ending in pollen.
    let open = cx.open;
    for i in 0..6 {
        let az = (i as f32 + 0.5) / 6.0 * TAU;
        let lean = 0.22 * open;
        let dir = Vec3::new(az.cos() * lean, 1.0, az.sin() * lean).normalized();
        let reach = 0.80 * (0.6 + 0.4 * open);
        let path: Vec<Vec3> = (0..=4)
            .map(|k| {
                let d = dir * (reach * k as f32 / 4.0);
                cx.at(d.x, 0.1 + d.y, d.z)
            })
            .collect();
        let opts = cx.patch(4, 4);
        cx.mesh.tube(
            &path,
            4,
            opts,
            |_| 0.022 * cx.scale,
            |t| mix(LIME, WHITE, t),
        );
        let tip_dir = (cx.x * dir.x + cx.y * dir.y + cx.z * dir.z).normalized();
        cx.mesh.blob(
            path[4],
            0.06 * cx.scale,
            tip_dir,
            1.7,
            f.center,
            cx.patch(0, 0),
        );
    }
}

fn lavender(cx: &mut Ctx, f: &Flower) {
    let height = 2.4;
    let path: Vec<Vec3> = (0..=6)
        .map(|k| cx.at(0.0, height * k as f32 / 6.0, 0.0))
        .collect();
    let opts = cx.patch(6, 4).ink(LEAF_INK);
    cx.mesh
        .tube(&path, 4, opts, |_| 0.035 * cx.scale, |_| DARK_GREEN);
    let (deep, pale) = (deepen(f.petal), mix(f.petal, WHITE, 0.35));
    // Rings of little buds, opening from the bottom of the spike upward.
    let rings = 12;
    for j in 0..rings {
        let t = j as f32 / (rings - 1) as f32;
        let swell = clamp01(cx.open * (rings as f32 + 3.0) - j as f32).max(0.25);
        let y = 0.45 + (height - 0.5) * t;
        let r = 0.20 * (1.0 - 0.6 * t);
        for k in 0..5 {
            let a = k as f32 / 5.0 * TAU + j as f32 * 0.9;
            let out = Vec3::new(a.cos(), 0.45, a.sin()).normalized();
            let wobble = cx.rng.range(-0.03, 0.03);
            let c = cx.at(r * a.cos(), y + wobble, r * a.sin());
            let dir = (cx.x * out.x + cx.y * out.y + cx.z * out.z).normalized();
            let color = mix(
                mix(deep, f.petal, cx.rng.f32()),
                pale,
                0.5 * t * cx.rng.f32(),
            );
            let size = (0.15 - 0.06 * t) * swell * cx.scale;
            cx.mesh.blob(c, size, dir, 1.4, color, cx.patch(0, 0));
        }
    }
}

fn babys_breath(cx: &mut Ctx, f: &Flower) {
    let twig = mix(GREEN, [0.6, 0.7, 0.5], 0.5);
    let open = cx.open;
    for _ in 0..6 {
        let a = cx.rng.range(0.0, TAU);
        let lean = cx.rng.range(0.25, 0.75);
        let dir = Vec3::new(a.cos() * lean, 1.0, a.sin() * lean).normalized();
        let reach = cx.rng.range(0.55, 1.0);
        let fork = dir * (reach * 0.55);
        twig_line(cx, Vec3::default(), fork, twig);
        // Each branch splits into a few fine twigs with a floret on the end.
        for _ in 0..3 {
            let jitter = Vec3::new(
                cx.rng.range(-0.3, 0.3),
                cx.rng.range(0.1, 0.45),
                cx.rng.range(-0.3, 0.3),
            );
            let tip = fork + (dir * 0.2 + jitter) * reach;
            twig_line(cx, fork, tip, twig);
            let c = cx.at(tip.x, tip.y, tip.z);
            let color = mix(f.petal, WHITE, cx.rng.range(0.0, 0.5));
            let size = cx.rng.range(0.085, 0.115) * (0.4 + 0.6 * open) * cx.scale;
            cx.mesh.blob(c, size, cx.y, 0.8, color, cx.patch(0, 0));
        }
    }
}

fn twig_line(cx: &mut Ctx, from: Vec3, to: Vec3, color: Col) {
    let path = [cx.at(from.x, from.y, from.z), cx.at(to.x, to.y, to.z)];
    let opts = cx.patch(1, 3).ink(LEAF_INK);
    cx.mesh.tube(&path, 3, opts, |_| 0.03 * cx.scale, |_| color);
}

/// A leaf growing from `base`. It starts out along `up` and falls away
/// toward `out`; `lean` is how far, in degrees, at its base and its tip.
#[allow(clippy::too_many_arguments)]
pub fn leaf(
    mesh: &mut Mesh,
    base: Vec3,
    up: Vec3,
    out: Vec3,
    len: f32,
    lean: (f32, f32),
    born: f32,
    rng: &mut Rng,
) {
    let mut cx = Ctx::new(mesh, base, up, 0.0, len);
    cx.x = (out - up * out.dot(up)).normalized();
    cx.z = cx.x.cross(up);
    cx.born = born;
    cx.grow = 0.5;
    cx.ink = LEAF_INK;
    // No two leaves quite the same green.
    let hue = rng.f32();
    let body = mix(
        mix(DARK_GREEN, GREEN, hue),
        [0.20, 0.48, 0.40],
        0.35 * rng.f32(),
    );
    let tip = mix(body, LIME, 0.35);
    let p = Petal {
        r0: 0.0,
        y0: 0.0,
        len: 1.0,
        width: rng.range(0.17, 0.23),
        tilt0: 90.0 - lean.0,
        tilt1: 90.0 - lean.1,
        cup: 0.55,
        widest: 0.42,
        point: 0.95,
        ruffle: 0.03,
        nu: 8,
        nv: 4,
        ..PETAL
    };
    petal(&mut cx, &p, |u, v| {
        // A darker rib down the middle, lighter toward the tip.
        let rib = if v.abs() < 0.1 { 0.72 } else { 1.0 };
        (mix(body, tip, u * u), rib * (0.8 + 0.2 * u))
    });
}
