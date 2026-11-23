//! A grid of colored characters with a depth buffer, drawn once per frame
//! and then written to the terminal in one go.

use std::fmt::Write;

use crate::color::{ColorMode, Rgb};

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Option<Rgb>,
    /// Depth of whatever drew here; nerer things (bigger z) win.
    pub z: f32,
}

impl Cell {
    /// Whether the two would look the same on screen.
    fn looks_like(&self, other: &Cell) -> bool {
        self.ch == other.ch && self.bg == other.bg && (self.ch == ' ' || self.fg == other.fg)
    }
}

const EMPTY: Cell = Cell {
    ch: ' ',
    fg: Rgb::WHITE,
    bg: None,
    z: f32::NEG_INFINITY,
};

pub struct Canvas {
    w: usize,
    h: usize,
    cells: Vec<Cell>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Self {
        Canvas {
            w,
            h,
            cells: vec![EMPTY; w * h],
        }
    }

    pub fn width(&self) -> usize {
        self.w
    }

    pub fn height(&self) -> usize {
        self.h
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        if (w, h) != (self.w, self.h) {
            *self = Canvas::new(w, h);
        }
    }

    pub fn clear(&mut self) {
        self.cells.fill(EMPTY);
    }

    pub fn get(&self, x: usize, y: usize) -> &Cell {
        &self.cells[y * self.w + x]
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            None
        } else {
            Some(y as usize * self.w + x as usize)
        }
    }

    /// Draws `ch` if it is nearer than what is already there.
    pub fn plot(&mut self, x: i32, y: i32, z: f32, ch: char, fg: Rgb) {
        if let Some(i) = self.index(x, y) {
            let cell = &mut self.cells[i];
            if z > cell.z {
                cell.z = z;
                cell.ch = ch;
                cell.fg = fg;
            }
        }
    }

    /// Sets a cell outright, background included.
    pub fn paint(&mut self, x: usize, y: usize, z: f32, ch: char, fg: Rgb, bg: Rgb) {
        self.cells[y * self.w + x] = Cell {
            ch,
            fg,
            bg: Some(bg),
            z,
        };
    }

    /// Draws on top of everything (for text and UI). Keeps the cell's background.
    pub fn put(&mut self, x: i32, y: i32, ch: char, fg: Rgb) {
        if let Some(i) = self.index(x, y) {
            let cell = &mut self.cells[i];
            cell.z = f32::INFINITY;
            cell.ch = ch;
            cell.fg = fg;
        }
    }

    /// Writes a line of text on top of everything.
    pub fn text(&mut self, x: i32, y: i32, s: &str, fg: Rgb) {
        for (i, ch) in s.chars().enumerate() {
            self.put(x + i as i32, y, ch, fg);
        }
    }

    /// Blanks a rectangle and paints its background (for panels and boxes).
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, bg: Rgb) {
        for row in y..(y + h).min(self.h) {
            for col in x..(x + w).min(self.w) {
                self.cells[row * self.w + col] = Cell {
                    ch: ' ',
                    fg: Rgb::WHITE,
                    bg: Some(bg),
                    z: f32::INFINITY,
                };
            }
        }
    }

    /// Appends the escape codes that paint this canvas to `out`. Only the
    /// cells that differ from `shown` (what is on screen now) are sent, and
    /// colors are only re-sent when they change.
    pub fn write_ansi(
        &self,
        out: &mut String,
        mode: ColorMode,
        base_bg: Option<Rgb>,
        shown: Option<&Canvas>,
    ) {
        let shown = shown.filter(|s| (s.w, s.h) == (self.w, self.h));
        let mut fg: Option<Rgb> = None;
        let mut bg: Option<Option<Rgb>> = None;
        for y in 0..self.h {
            // Where the terminal's cursor is on this row, if we put it there.
            let mut at: Option<usize> = None;
            for x in 0..self.w {
                let cell = &self.cells[y * self.w + x];
                if let Some(shown) = shown
                    && cell.looks_like(&shown.cells[y * self.w + x])
                {
                    continue;
                }
                if at != Some(x) {
                    let _ = write!(out, "\x1b[{};{}H", y + 1, x + 1);
                }
                let want_bg = cell.bg.or(base_bg);
                if bg != Some(want_bg) {
                    match want_bg {
                        Some(c) => mode.write_bg(out, c),
                        None => out.push_str("\x1b[49m"),
                    }
                    bg = Some(want_bg);
                }
                if cell.ch != ' ' && fg != Some(cell.fg) {
                    mode.write_fg(out, cell.fg);
                    fg = Some(cell.fg);
                }
                out.push(cell.ch);
                at = Some(x + 1);
            }
        }
        out.push_str("\x1b[0m");
    }

    /// Makes this canvas a copy of `other`, reusing its memory.
    pub fn copy_from(&mut self, other: &Canvas) {
        (self.w, self.h) = (other.w, other.h);
        self.cells.clone_from(&other.cells);
    }
}
