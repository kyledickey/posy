//! Turns 3D bouquet coordinates into terminal cells.
//!
//! World space: y is up, and the bouquet sits around the origin, about
//! 1.5 units tall. The camera spins the world around the y axis (yaw), tips it
//! toward you (tilt), and applies a gentle perspective.

use crate::math::Vec3;

const CAMERA_DISTANCE: f32 = 4.0;

/// The rectangle of terminal cells the bouquet is drawn into.
#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// How much room the bouquet needs, in world units.
#[derive(Clone, Copy, Debug)]
pub struct Framing {
    pub width: f32,
    pub height: f32,
    /// Height of the middle of the bouquet, which it spins and tilts around.
    pub center_y: f32,
}

impl Default for Framing {
    fn default() -> Self {
        Framing {
            width: 1.4,
            height: 1.5,
            center_y: 0.2,
        }
    }
}

/// A point after projection: `x`/`y` in cells, `z` toward the viewer
/// (bigger = closer).
#[derive(Clone, Copy, Debug)]
pub struct Projected {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub struct Camera {
    cos_yaw: f32,
    sin_yaw: f32,
    cos_tilt: f32,
    sin_tilt: f32,
    /// Cells per world unit, vertically.
    pub scale: f32,
    /// Cells per world unit, horizontally. Terminal cells are about twice as
    /// tall as they are wide, so this is double `scale`.
    pub sx: f32,
    cx: f32,
    cy: f32,
    pivot_y: f32,
}

impl Camera {
    pub fn new(yaw: f32, tilt: f32, zoom: f32, view: Viewport, frame: Framing) -> Camera {
        let fit = ((view.h as f32 - 1.0) / frame.height)
            .min((view.w as f32 - 2.0) / (2.0 * frame.width))
            .max(1.0);
        let scale = fit * zoom;
        Camera {
            cos_yaw: yaw.cos(),
            sin_yaw: yaw.sin(),
            cos_tilt: tilt.cos(),
            sin_tilt: tilt.sin(),
            scale,
            sx: scale * 2.0,
            cx: view.x as f32 + view.w as f32 / 2.0,
            cy: view.y as f32 + view.h as f32 / 2.0,
            pivot_y: frame.center_y,
        }
    }

    /// Rotates a direction (like a surface normal) into camera space.
    pub fn rotate(&self, v: Vec3) -> Vec3 {
        let xr = v.x * self.cos_yaw - v.z * self.sin_yaw;
        let zr = v.x * self.sin_yaw + v.z * self.cos_yaw;
        let yt = v.y * self.cos_tilt - zr * self.sin_tilt;
        let zt = v.y * self.sin_tilt + zr * self.cos_tilt;
        Vec3::new(xr, yt, zt)
    }

    /// Projects a world-space point to the screen.
    pub fn project(&self, p: Vec3) -> Projected {
        let c = self.rotate(Vec3::new(p.x, p.y - self.pivot_y, p.z));
        let f = CAMERA_DISTANCE / (CAMERA_DISTANCE - c.z);
        Projected {
            x: self.cx + c.x * f * self.sx,
            y: self.cy - c.y * f * self.scale,
            z: c.z,
        }
    }
}
