//! Builds the 3D bouquet from a [`BouquetSpec`].
//!
//! A scene is one mesh for everything that doesn't change shape (the wrap,
//! stems, leaves, ribbon) plus a list of [`Flower`]s. Every point of the
//! mesh has a "born" time, which is how the bouquet grows in at the start;
//! flowers are rebuilt each frame while they open, then kept.

mod wrap;

use std::f32::consts::{PI, TAU};

use crate::bouquet::{BouquetSpec, FlowerKind};
use crate::camera::Framing;
use crate::color::{Col, Rgb, color_or, mix};
use crate::flowers::{self, BLOOM_TIME, Flower, LEAF_INK};
use crate::math::{Vec3, basis, bezier};
use crate::mesh::{Mesh, Patch};
use crate::rng::Rng;

/// More stems than this stops looking like a bouquet.
const MAX_STEMS: usize = 30;

// Growth timeline, in seconds after the bouquet appears.
pub const STEMS_START: f32 = 0.5;
pub const STEM_TIME: f32 = 1.3;
pub const LEAVES_START: f32 = 1.5;
pub const BLOOM_START: f32 = 2.6;
pub const BLOOM_STAGGER: f32 = 0.13;

const STEM_GREEN: Col = [0.16, 0.46, 0.22];

pub struct Scene {
    /// Wrap, stems, leaves and ribbon.
    pub base: Mesh,
    pub flowers: Vec<Flower>,
    /// Each flower in full bloom, built once.
    pub blooms: Vec<Mesh>,
    /// How much room the camera should leave for it.
    pub framing: Framing,
}

impl Scene {
    pub fn empty() -> Scene {
        Scene {
            base: Mesh::default(),
            flowers: Vec::new(),
            blooms: Vec::new(),
            framing: Framing::default(),
        }
    }
}

/// Holds the scene being built and the random numbers used to build it.
struct Builder {
    scene: Scene,
    rng: Rng,
}

/// Builds the bouquet. The same spec and seed always give the same scene.
pub fn build(spec: &BouquetSpec, seed: u64) -> Scene {
    let mut b = Builder {
        scene: Scene::empty(),
        rng: Rng::new(seed),
    };
    let wrap_color = color_or(spec.wrap_color.as_deref(), spec.wrap.default_color());
    let ribbon = color_or(spec.ribbon_color.as_deref(), "pink");

    let items = pick(&mut b, spec);
    let layout = arrange(&mut b, &items);
    let dome_radius = layout.radius;

    // The wrap is built at one size, then scaled to suit the bouquet.
    let mut info = wrap::build(&mut b, spec.wrap, wrap_color.to_col(), ribbon.to_col());
    let scale = (dome_radius / 0.50).clamp(0.8, 1.9);
    for v in &mut b.scene.base.verts {
        v.pos = v.pos * scale;
    }
    info.scale(scale);

    if let Some(tail) = &info.tail {
        stem_tail(&mut b, tail);
    }
    collar(&mut b, &info, scale);
    flowers(&mut b, items, &info, layout);

    // Make room for anything that reaches past the flowers, like the
    // handle of a basket.
    let highest = b
        .scene
        .base
        .verts
        .iter()
        .map(|v| v.pos.y)
        .fold(0.0, f32::max);
    let framing = &mut b.scene.framing;
    let (top, bottom) = (
        (framing.center_y + framing.height / 2.0).max(highest + 0.08),
        framing.center_y - framing.height / 2.0,
    );
    framing.height = top - bottom;
    framing.center_y = (top + bottom) / 2.0;

    b.scene.blooms = b
        .scene
        .flowers
        .iter()
        .map(|f| {
            let mut mesh = Mesh::default();
            f.build(&mut mesh, BLOOM_TIME);
            mesh
        })
        .collect();
    b.scene
}

struct Item {
    kind: FlowerKind,
    petal: Rgb,
    center: Rgb,
}

/// Every stem in the bouquet, biggest blooms first.
fn pick(b: &mut Builder, spec: &BouquetSpec) -> Vec<Item> {
    let mut items = Vec::new();
    for entry in &spec.flowers {
        let petal = color_or(entry.color.as_deref(), entry.kind.default_color());
        let center = color_or(entry.center.as_deref(), entry.kind.default_center());
        for _ in 0..entry.count {
            items.push(Item {
                kind: entry.kind,
                petal,
                center,
            });
        }
    }
    items.truncate(MAX_STEMS);
    b.rng.shuffle(&mut items);

    // Big blooms go near the top of the dome, small ones and fillers
    // around the edge, with a little randomness so it isn't too neat.
    let mut keyed: Vec<(f32, Item)> = items
        .into_iter()
        .map(|it| (it.kind.radius() + b.rng.range(0.0, 0.05), it))
        .collect();
    keyed.sort_by(|x, y| y.0.total_cmp(&x.0));
    keyed.into_iter().map(|(_, it)| it).collect()
}

/// Where every head goes: a direction out from the middle of the dome
/// for each, how big each is, and how big the dome has to be.
struct Layout {
    radius: f32,
    spots: Vec<Vec3>,
    sizes: Vec<f32>,
}

/// Spreads the heads over a dome so that none pushes through another.
///
/// They start on a golden-angle spiral from the top down, then neighbours
/// that are too close nudge each other apart. If they still don't fit, the
/// dome grows and they try again.
fn arrange(b: &mut Builder, items: &[Item]) -> Layout {
    /// The lowest a head may sit, as the height of its direction (the
    /// sine of 30 degrees): any lower and it dips into the wrap.
    const LOWEST: f32 = 0.5;
    /// Heads may brush against each other this much; a little overlap
    /// looks fuller, and one simply sits in front of the other.
    const SNUG: f32 = 0.70;
    let n = items.len();
    let sizes: Vec<f32> = items
        .iter()
        .map(|it| size_of(it.kind) * b.rng.range(0.94, 1.08))
        .collect();
    // How much elbow room each needs.
    let room: Vec<f32> = items
        .iter()
        .zip(&sizes)
        .map(|(it, size)| match it.kind {
            FlowerKind::Lavender | FlowerKind::BabysBreath => 0.07,
            // Cupped flowers are rounder than they are wide.
            FlowerKind::Rose | FlowerKind::Tulip | FlowerKind::Peony | FlowerKind::Carnation => {
                size * 0.88
            }
            _ => *size,
        })
        .collect();

    let golden_angle = PI * (3.0 - 5f32.sqrt());
    let start_angle = b.rng.range(0.0, TAU);
    let mut spots: Vec<Vec3> = (0..n)
        .map(|i| {
            let frac = if n > 1 {
                i as f32 / (n - 1) as f32
            } else {
                0.0
            };
            let elevation = (86.0 - 54.0 * frac.sqrt()).to_radians();
            let azimuth = start_angle + i as f32 * golden_angle;
            Vec3::new(
                elevation.cos() * azimuth.cos(),
                elevation.sin(),
                elevation.cos() * azimuth.sin(),
            )
        })
        .collect();

    let area: f32 = room.iter().map(|r| r * r).sum();
    let mut radius = (0.7 * area.sqrt()).clamp(0.26, 1.4);
    for _ in 0..40 {
        let mut worst = 0.0f32;
        for _ in 0..80 {
            worst = 0.0;
            for i in 0..n {
                for j in i + 1..n {
                    let apart = spots[i] - spots[j];
                    let (gap, want) = (apart.length() * radius, (room[i] + room[j]) * SNUG);
                    if gap >= want || gap < 1e-5 {
                        continue;
                    }
                    worst = worst.max((want - gap) / want);
                    let push = apart.normalized() * ((want - gap) / radius * 0.5);
                    spots[i] += push;
                    spots[j] = spots[j] - push;
                }
            }
            for spot in &mut spots {
                let mut v = spot.normalized();
                if v.y < LOWEST {
                    let flat = Vec3::new(v.x, 0.0, v.z);
                    let flat = if flat.length() > 1e-5 {
                        flat.normalized()
                    } else {
                        Vec3::new(1.0, 0.0, 0.0)
                    };
                    v = flat * (1.0 - LOWEST * LOWEST).sqrt() + Vec3::UP * LOWEST;
                }
                *spot = v;
            }
            if worst < 0.04 {
                break;
            }
        }
        if worst < 0.04 || radius >= 1.4 {
            break;
        }
        radius *= 1.04;
    }
    Layout {
        radius,
        spots,
        sizes,
    }
}

fn flowers(b: &mut Builder, items: Vec<Item>, info: &wrap::WrapInfo, layout: Layout) {
    let n = items.len();
    let dome_radius = layout.radius;
    let dome_center = Vec3::new(0.0, info.dome_y, 0.0);
    let (top, bottom) = (info.dome_y + dome_radius + 0.30, info.bottom - 0.04);
    b.scene.framing = Framing {
        width: 2.0 * dome_radius + 0.55,
        height: top - bottom,
        center_y: (top + bottom) / 2.0,
    };

    for (i, item) in items.into_iter().enumerate() {
        let axis = layout.spots[i];
        // Fillers reach a little past the blooms so they peek out between.
        let filler = item.kind.radius() == 0.0;
        let reach = if filler {
            b.rng.range(1.0, 1.08)
        } else {
            b.rng.range(0.96, 1.0)
        };
        let pos = dome_center + axis * (dome_radius * reach);

        let order = i as f32 / n as f32;
        let start = STEMS_START + order * 0.35;
        stem(b, info.neck, pos, axis, start);

        let flower = Flower {
            kind: item.kind,
            pos,
            axis,
            radius: layout.sizes[i],
            spin: b.rng.range(0.0, TAU),
            petals: item.kind.petal_count(&mut b.rng),
            petal: item.petal.to_col(),
            center: item.center.to_col(),
            bloom_at: BLOOM_START + i as f32 * BLOOM_STAGGER,
            seed: b.rng.next_u64(),
        };
        b.scene.flowers.push(flower);
    }
}

/// How big a flower is drawn. Spikes and sprays have no head to measure,
/// so they get a size of their own.
pub fn size_of(kind: FlowerKind) -> f32 {
    match kind {
        FlowerKind::Lavender => 0.12,
        FlowerKind::BabysBreath => 0.19,
        kind => kind.radius(),
    }
}

/// A gently curved stem from the neck of the wrap up to a flower, with a
/// leaf or two near the top to fill the gaps between blooms.
fn stem(b: &mut Builder, from: Vec3, to: Vec3, axis: Vec3, start: f32) {
    let flat = Vec3::new(to.x, 0.0, to.z);
    let outward = if flat.length() < 1e-4 {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        flat.normalized()
    };
    // Arrive at the flower along the way it faces.
    let ctrl = from.lerp(to, 0.55) + outward * 0.04 - axis * 0.06;
    let path: Vec<Vec3> = (0..=10)
        .map(|j| bezier(from, ctrl, to, j as f32 / 10.0))
        .collect();
    b.scene.base.tube(
        &path,
        5,
        Patch::new(10, 5).born(start, STEM_TIME).ink(LEAF_INK),
        |_| 0.011,
        |t| mix(STEM_GREEN, flowers::GREEN, t * 0.6),
    );

    if b.rng.chance(0.75) {
        let t = b.rng.range(0.70, 0.88);
        let at = bezier(from, ctrl, to, t);
        let along = (bezier(from, ctrl, to, t + 0.05) - at).normalized();
        let born = start + t * STEM_TIME;
        let (az, len) = (b.rng.range(0.0, TAU), b.rng.range(0.11, 0.16));
        let (p, q) = basis(along);
        let out = p * az.cos() + q * az.sin();
        let lean = (b.rng.range(40.0, 60.0), b.rng.range(80.0, 110.0));
        flowers::leaf(
            &mut b.scene.base,
            at,
            along,
            out,
            len,
            lean,
            born,
            &mut b.rng,
        );
    }
}

/// The cut stem ends poking out below the wrap.
fn stem_tail(b: &mut Builder, tail: &wrap::Tail) {
    for i in 0..tail.count {
        let a = i as f32 / tail.count as f32 * TAU + 0.3;
        let r1 = tail.r0 * (1.0 + tail.spread);
        let path = [
            Vec3::new(tail.r0 * a.cos(), tail.top, tail.r0 * a.sin()),
            Vec3::new(
                r1 * a.cos(),
                tail.bottom + 0.02 * (i % 3) as f32,
                r1 * a.sin(),
            ),
        ];
        b.scene.base.tube(
            &path,
            5,
            Patch::new(1, 5).born(0.15, 0.4).ink(LEAF_INK),
            |_| 0.011,
            |t| mix(STEM_GREEN, [0.30, 0.50, 0.24], t),
        );
    }
}

/// A ring of big leaves fanning out from the top of the wrap.
fn collar(b: &mut Builder, info: &wrap::WrapInfo, scale: f32) {
    let count = 8 + b.rng.below(3);
    let offset = b.rng.range(0.0, TAU);
    for i in 0..count {
        let a = offset + i as f32 / count as f32 * TAU + b.rng.range(-0.15, 0.15);
        let base = Vec3::new(info.leaf_r * a.cos(), info.leaf_y, info.leaf_r * a.sin());
        let len = b.rng.range(0.26, 0.34) * scale;
        let lean = (b.rng.range(30.0, 45.0), b.rng.range(80.0, 105.0));
        let born = LEAVES_START + i as f32 * 0.07;
        flowers::leaf(
            &mut b.scene.base,
            base,
            Vec3::UP,
            Vec3::new(a.cos(), 0.0, a.sin()),
            len,
            lean,
            born,
            &mut b.rng,
        );
    }
}
