//! Text drawn over the bouquet: the message, signature, notifications,
//! the help box, the whisper prompt, and words floating away.

use crate::bigtext;
use crate::bouquet::TextEffect;
use crate::canvas::Canvas;
use crate::color::{Gradient, Rgb, hsv};

const SIGNATURE: Rgb = Rgb(120, 112, 136);
const BOX_BG: Rgb = Rgb(34, 28, 44);
const BOX_TEXT: Rgb = Rgb(225, 215, 232);
const BOX_KEY: Rgb = Rgb(246, 122, 172);
const BAR_TEXT: Rgb = Rgb(168, 158, 184);

/// The help box. A line with no key is a heading.
const HELP: &[(&str, &str)] = &[
    ("", "with the mouse"),
    ("drag", "turn the bouquet around"),
    ("scroll", "come closer or step back"),
    ("click", "the words along the bottom"),
    ("", ""),
    ("", "with the keyboard"),
    ("← → ↑ ↓", "turn and tilt"),
    ("+  -", "closer / further (0 puts it back)"),
    ("space", "stop or start the spinning"),
    ("n  p", "next / previous bouquet"),
    ("s", "surprise me with a new bouquet"),
    ("m", "make your own"),
    ("r", "rearrange the flowers"),
    ("g", "watch it grow again"),
    ("1 - 6", "hearts petals fireflies snow stars sparkles"),
    ("v", "change how it is drawn"),
    ("h", "hide or show the buttons along the bottom"),
    ("x", "save a picture of it"),
    ("/", "whisper something"),
    ("q", "leave"),
];

/// Something along the bottom you can click.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Prev,
    Menu,
    Surprise,
    Pause,
    Help,
    Next,
    /// The two answers to a question.
    Yes,
    No,
}

/// What the bottom bar needs to know to draw itself.
pub struct Bar {
    /// 0 (barely there) to 1 (fully lit).
    pub glow: f32,
    /// The cell the mouse is over.
    pub hover: Option<(usize, usize)>,
    pub paused: bool,
}

struct Floater {
    text: String,
    x: f32,
    y: f32,
    age: f32,
}

/// How the message is drawn.
#[derive(Clone, Copy, Default)]
pub struct Look {
    pub big: bool,
    pub color: Option<Rgb>,
    pub effect: TextEffect,
}

pub struct Hud {
    pub look: Look,
    /// A smaller line under the message.
    pub signature: Option<String>,
    /// The labels of the two answers, while a question is waiting for one.
    pub ask: Option<(String, String)>,
    messages: Vec<String>,
    index: usize,
    age: f32,
    interval: f32,
    toast: Option<(String, f32)>,
    pub help: bool,
    /// The text being typed after pressing `/`.
    pub whisper: Option<String>,
    floaters: Vec<Floater>,
    rainbow: f32,
    /// Where each button was last drawn: row, first and last column.
    buttons: Vec<(usize, usize, usize, Button)>,
}

impl Hud {
    pub fn new(interval: f32) -> Self {
        Hud {
            look: Look::default(),
            signature: None,
            ask: None,
            messages: Vec::new(),
            index: 0,
            age: 0.0,
            interval,
            toast: None,
            help: false,
            whisper: None,
            floaters: Vec::new(),
            rainbow: 0.0,
            buttons: Vec::new(),
        }
    }

    pub fn set_messages(&mut self, messages: Vec<String>) {
        if messages != self.messages {
            self.messages = messages;
            self.index = 0;
            self.age = 0.0;
        }
    }

    pub fn next_message(&mut self) {
        if !self.messages.is_empty() {
            self.index = (self.index + 1) % self.messages.len();
            self.age = 0.0;
        }
    }

    /// Shows a short notification at the top for a few seconds.
    pub fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), 0.0));
    }

    /// Sends a line of text floating up from (`x`, `y`) in cells.
    pub fn float(&mut self, text: String, x: f32, y: f32) {
        self.floaters.push(Floater {
            text,
            x,
            y,
            age: 0.0,
        });
    }

    /// Makes the message shimmer through the rainbow for `secs` seconds.
    pub fn rainbow(&mut self, secs: f32) {
        self.rainbow = secs;
    }

    pub fn update(&mut self, dt: f32) {
        self.age += dt;
        if self.messages.len() > 1 && self.age > self.interval {
            self.next_message();
        }
        let toast_done = match self.toast.as_mut() {
            Some((_, age)) => {
                *age += dt;
                *age > 3.0
            }
            None => false,
        };
        if toast_done {
            self.toast = None;
        }
        for f in &mut self.floaters {
            f.age += dt;
            f.y -= dt * 1.6;
        }
        self.floaters.retain(|f| f.age < 4.0);
        self.rainbow = (self.rainbow - dt).max(0.0);
    }

    /// The button at this cell, if there is one.
    pub fn button_at(&self, x: usize, y: usize) -> Option<Button> {
        self.buttons
            .iter()
            .find(|(row, x0, x1, _)| *row == y && (*x0..=*x1).contains(&x))
            .map(|b| b.3)
    }

    /// The message split into lines that fit `width` cells, and whether
    /// they can be drawn in big letters.
    fn lines(&self, width: usize) -> (Vec<String>, bool) {
        let Some(message) = self.messages.get(self.index) else {
            return (Vec::new(), false);
        };
        let room = width.saturating_sub(4).max(8);
        let wrap = |measure: &dyn Fn(&str) -> usize| -> Vec<String> {
            let mut lines = Vec::new();
            for part in message.split('\n') {
                let mut line = String::new();
                for word in part.split_whitespace() {
                    let longer = if line.is_empty() {
                        word.to_string()
                    } else {
                        format!("{line} {word}")
                    };
                    if !line.is_empty() && measure(&longer) > room {
                        lines.push(std::mem::replace(&mut line, word.to_string()));
                    } else {
                        line = longer;
                    }
                }
                lines.push(line);
            }
            lines
        };
        if self.look.big && bigtext::can_draw(message) {
            let lines = wrap(&bigtext::width);
            if lines.len() <= 3 && lines.iter().all(|l| bigtext::width(l) <= room) {
                return (lines, true);
            }
        }
        (wrap(&|s: &str| s.chars().count()), false)
    }

    /// How many rows at the bottom of a `width`-wide screen the message,
    /// signature, answers and bar take up.
    pub fn rows(&self, width: usize, bar: bool) -> usize {
        let (lines, big) = self.lines(width);
        let message = if big {
            lines.len() * (bigtext::ROWS + 1)
        } else {
            lines.len().max(1)
        };
        message
            + self.signature.is_some() as usize
            + if self.ask.is_some() { 2 } else { 0 }
            + if bar { 2 } else { 1 }
    }

    /// The colour of letter `i` of the message at time `t`.
    fn letter_color(&self, i: usize, t: f32) -> Rgb {
        let base = self.look.color.unwrap_or(Rgb(240, 110, 170));
        let light = base.lerp(Rgb::WHITE, 0.7);
        if self.rainbow > 0.0 {
            return hsv(i as f32 * 0.06 - t * 0.5, 0.55, 1.0);
        }
        match self.look.effect {
            TextEffect::Shimmer => {
                let dark = base.lerp(Rgb(40, 16, 44), 0.45);
                Gradient::new(dark, base, light).at(0.55 + 0.45 * (i as f32 * 0.45 - t * 3.0).sin())
            }
            TextEffect::Glow => base.lerp(light, 0.5 + 0.5 * (t * 1.8).sin()),
            TextEffect::Rainbow => hsv(i as f32 * 0.05 - t * 0.25, 0.5, 1.0),
            TextEffect::Plain => base.lerp(light, 0.25),
        }
    }

    /// Draws the HUD. Everything is centered in the columns from `x0` to the
    /// right edge. `fade` is the color text fades into.
    pub fn draw(&mut self, canvas: &mut Canvas, t: f32, fade: Rgb, x0: usize, bar: Option<&Bar>) {
        let (w, h) = (canvas.width(), canvas.height());
        let width = w.saturating_sub(x0);
        let center = |len: usize| (x0 + width.saturating_sub(len) / 2) as i32;
        self.buttons.clear();
        if let Some(bar) = bar {
            self.draw_bar(canvas, fade, x0, bar);
        }
        let mut row = h.saturating_sub(self.rows(width, bar.is_some())) as i32;

        if let Some(buf) = &self.whisper {
            let cursor = if (t * 2.0) as i32 % 2 == 0 { '_' } else { ' ' };
            let line = format!("whisper ♥ {buf}{cursor}");
            canvas.text(center(line.chars().count()), row, &line, BOX_KEY);
            row += 1;
        } else {
            // The message writes itself out, and fades between messages
            // when there is more than one.
            let alpha = if self.messages.len() > 1 {
                (self.age / 0.8)
                    .min((self.interval - self.age) / 0.8)
                    .clamp(0.0, 1.0)
            } else {
                1.0
            };
            let written = ((self.age - 0.4) * 24.0).max(0.0) as usize;
            let (lines, big) = self.lines(width);
            let mut before = 0;
            for line in &lines {
                if big {
                    let x = center(bigtext::width(line));
                    for (col, cell_row, ch, letter) in bigtext::cells(line) {
                        if before + letter < written {
                            let color = self.letter_color(before + letter, t);
                            let (x, y) = (x + col as i32, row + cell_row as i32);
                            canvas.put(x, y, ch, fade.lerp(color, alpha));
                        }
                    }
                    row += (bigtext::ROWS + 1) as i32;
                } else {
                    let x = center(line.chars().count());
                    for (i, ch) in line.chars().enumerate() {
                        if before + i < written {
                            let color = self.letter_color(before + i, t);
                            canvas.put(x + i as i32, row, ch, fade.lerp(color, alpha));
                        }
                    }
                    row += 1;
                }
                before += line.chars().count();
            }
            if lines.is_empty() {
                row += 1;
            }
        }

        if let Some(sig) = &self.signature {
            canvas.text(center(sig.chars().count()), row, sig, SIGNATURE);
            row += 1;
        }

        if let Some((yes, no)) = &self.ask {
            row += 1;
            let (yes, no) = (format!("  {yes}  "), format!("  {no}  "));
            let (yes_len, no_len) = (yes.chars().count(), no.chars().count());
            let x = center(yes_len + 6 + no_len).max(0) as usize;
            let hover = bar.and_then(|b| b.hover);
            let over = |x0: usize, len: usize| {
                hover.is_some_and(|(hx, hy)| hy as i32 == row && (x0..x0 + len).contains(&hx))
            };
            let pulse = 0.5 + 0.5 * (t * 2.4).sin();
            let yes_bg = if over(x, yes_len) {
                Rgb(255, 150, 195)
            } else {
                Rgb(214, 70, 130).lerp(Rgb(246, 122, 172), pulse)
            };
            canvas.fill_rect(x, row as usize, yes_len, 1, yes_bg);
            canvas.text(x as i32, row, &yes, Rgb(40, 14, 30));
            let no_x = x + yes_len + 6;
            let no_bg = if over(no_x, no_len) {
                Rgb(84, 74, 100)
            } else {
                Rgb(54, 46, 68)
            };
            canvas.fill_rect(no_x, row as usize, no_len, 1, no_bg);
            canvas.text(no_x as i32, row, &no, BAR_TEXT);
            let row = row as usize;
            self.buttons.push((row, x, x + yes_len - 1, Button::Yes));
            self.buttons
                .push((row, no_x, no_x + no_len - 1, Button::No));
        }

        for f in &self.floaters {
            let color = Rgb(249, 172, 206).lerp(fade, f.age / 4.0);
            canvas.text(f.x as i32, f.y as i32, &f.text, color);
        }

        if let Some((text, age)) = &self.toast {
            let line = format!(" {text} ");
            let len = line.chars().count();
            let x = (w.saturating_sub(len) / 2) as i32;
            canvas.fill_rect(x.max(0) as usize, 0, len, 1, BOX_BG);
            let fg = BOX_TEXT.lerp(BOX_BG, ((age - 2.4) / 0.6).clamp(0.0, 1.0));
            canvas.text(x, 0, &line, fg);
        }

        if self.help {
            self.draw_help(canvas);
        }
    }

    /// The row of things to click (or press) along the bottom.
    fn draw_bar(&mut self, canvas: &mut Canvas, fade: Rgb, x0: usize, bar: &Bar) {
        let (w, h) = (canvas.width(), canvas.height());
        let width = w.saturating_sub(x0);
        let pause = if bar.paused { "spin" } else { "pause" };
        // (key, label, button); "next" reads better with its arrow last.
        let wide = [
            ("‹", "previous", Button::Prev),
            ("m", "make your own", Button::Menu),
            ("s", "surprise me", Button::Surprise),
            ("space", pause, Button::Pause),
            ("?", "help", Button::Help),
            ("›", "next", Button::Next),
        ];
        let narrow = [
            ("‹", "", Button::Prev),
            ("m", "make", Button::Menu),
            ("s", "surprise", Button::Surprise),
            ("?", "help", Button::Help),
            ("›", "", Button::Next),
        ];
        const GAP: usize = 4;
        let size = |items: &[(&str, &str, Button)]| -> usize {
            let text: usize = items
                .iter()
                .map(|(k, l, _)| k.chars().count() + l.chars().count() + !l.is_empty() as usize)
                .sum();
            text + GAP * (items.len() - 1)
        };
        let items: &[(&str, &str, Button)] = if size(&wide) + 2 <= width {
            &wide
        } else if size(&narrow) + 2 <= width {
            &narrow
        } else {
            return;
        };

        let row = h - 1;
        let mut x = x0 + (width - size(items)) / 2;
        for (key, label, button) in items {
            let len = key.chars().count() + label.chars().count() + !label.is_empty() as usize;
            let hovered = bar
                .hover
                .is_some_and(|(hx, hy)| hy == row && (x.saturating_sub(1)..=x + len).contains(&hx));
            let glow = if hovered { 1.0 } else { bar.glow };
            let key_color = fade.lerp(BOX_KEY, glow);
            let label_color = fade.lerp(if hovered { BOX_TEXT } else { BAR_TEXT }, glow);
            if *button == Button::Next && !label.is_empty() {
                canvas.text(x as i32, row as i32, label, label_color);
                canvas.text((x + len - 1) as i32, row as i32, key, key_color);
            } else {
                canvas.text(x as i32, row as i32, key, key_color);
                canvas.text(
                    (x + key.chars().count() + 1) as i32,
                    row as i32,
                    label,
                    label_color,
                );
            }
            self.buttons
                .push((row, x.saturating_sub(1), x + len, *button));
            x += len + GAP;
        }
    }

    fn draw_help(&self, canvas: &mut Canvas) {
        let key_w = HELP
            .iter()
            .map(|(k, _)| k.chars().count())
            .max()
            .unwrap_or(0);
        let text_w = HELP
            .iter()
            .map(|(_, d)| d.chars().count())
            .max()
            .unwrap_or(0);
        let box_w = key_w + text_w + 9;
        let box_h = HELP.len() + 6;
        let x = canvas.width().saturating_sub(box_w) / 2;
        let y = canvas.height().saturating_sub(box_h) / 2;
        canvas.fill_rect(x, y, box_w, box_h, BOX_BG);
        canvas.text(x as i32 + 3, y as i32 + 1, "posy ♥ how to play", BOX_KEY);
        for (i, (key, desc)) in HELP.iter().enumerate() {
            let row = (y + 3 + i) as i32;
            if key.is_empty() {
                canvas.text(x as i32 + 3, row, desc, SIGNATURE);
            } else {
                canvas.text(x as i32 + 5, row, key, BOX_KEY);
                canvas.text((x + 7 + key_w) as i32, row, desc, BOX_TEXT);
            }
        }
        let close = "press any key to close";
        canvas.text(x as i32 + 3, (y + box_h - 2) as i32, close, SIGNATURE);
    }
}
