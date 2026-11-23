//! `posy garden`: every kind of flower side by side, each seen from the
//! front, at an angle and from the side. Handy for tuning how they look.

use std::f32::consts::{FRAC_PI_2, TAU};
use std::path::Path;

use anyhow::Result;

use crate::bouquet::FlowerKind;
use crate::camera::{Camera, Framing, Viewport};
use crate::canvas::Canvas;
use crate::color::{Rgb, color_or};
use crate::export;
use crate::flowers::{BLOOM_TIME, Flower};
use crate::math::Vec3;
use crate::mesh::Mesh;
use crate::raster::Style;
use crate::render::Renderer;
use crate::rng::Rng;
use crate::scene::Scene;

const SPACING: f32 = 0.46;

pub fn run(out: &Path, cols: usize, rows: usize, open: f32, bg: Rgb, style: Style) -> Result<()> {
    let mut rng = Rng::new(7);
    let mut scene = Scene::empty();
    for (i, kind) in FlowerKind::ALL.into_iter().enumerate() {
        for (view, lean) in [0.0f32, 0.9, FRAC_PI_2].into_iter().enumerate() {
            let col = (i % 2) * 3 + view;
            let row = i / 2;
            let pos = Vec3::new(
                (col as f32 - 2.5) * SPACING,
                (2.0 - row as f32) * SPACING + 0.2,
                0.0,
            );
            let size = crate::scene::size_of(kind) * 0.75;
            scene.flowers.push(Flower {
                kind,
                pos: pos - Vec3::new(0.0, lean.sin() * size * 0.6, 0.0),
                axis: Vec3::new(0.25 * lean.sin(), lean.sin(), lean.cos()).normalized(),
                radius: size,
                spin: rng.range(0.0, TAU),
                petals: kind.petal_count(&mut rng),
                petal: color_or(None, kind.default_color()).to_col(),
                center: color_or(None, kind.default_center()).to_col(),
                bloom_at: 0.0,
                seed: rng.next_u64(),
            });
        }
    }
    scene.blooms = scene.flowers.iter().map(|_| Mesh::default()).collect();

    let mut canvas = Canvas::new(cols, rows);
    let view = Viewport {
        x: 0,
        y: 0,
        w: cols as i32,
        h: rows as i32,
    };
    let frame = Framing {
        width: 6.0 * SPACING,
        height: 5.0 * SPACING,
        center_y: 0.2,
    };
    let cam = Camera::new(0.0, 0.0, 1.0, view, frame);
    // `anim` below the bloom time makes the renderer build each flower
    // fresh, at whatever stage of opening was asked for.
    let anim = (open * BLOOM_TIME).min(BLOOM_TIME * 0.999);
    Renderer::new().draw(&mut canvas, &scene, &cam, anim, style, bg.to_col());
    export::write(out, &canvas, bg)?;
    println!("saved {}", out.display());
    Ok(())
}
