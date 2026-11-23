//! The running app: holds all the state, reacts to keys and the mouse, and
//! draws a frame roughly 30 times a second.

use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveDate};
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use crossterm::{cursor, execute, terminal};

use crate::bouquet::{BouquetSpec, Effect, cycle};
use crate::camera::{Camera, Viewport};
use crate::canvas::Canvas;
use crate::cli::Cli;
use crate::color::{ColorMode, Rgb};
use crate::config::{self, Config};
use crate::effects::Effects;
use crate::export;
use crate::hud::{Bar, Button, Hud, Look};
use crate::library::{self, Library, Source};
use crate::menu::{self, Menu, Outcome};
use crate::raster::Style;
use crate::render::Renderer;
use crate::rng::{Rng, hash_str};
use crate::scene::{self, Scene};

const DEFAULT_TILT: f32 = 0.42;
/// How long the bottom bar stays lit after the last key press or mouse
/// move, in seconds, and how faint it gets after that.
const BAR_LINGER: f32 = 6.0;
const BAR_RESTING: f32 = 0.45;
/// Used for fading text when no background color is set.
pub const DEFAULT_BG: Rgb = Rgb(26, 27, 38);
/// "Far enough into the animation that everything has grown."
const GROWN: f32 = 1.0e4;
const KONAMI: [KeyCode; 8] = [
    KeyCode::Up,
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Down,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::Left,
    KeyCode::Right,
];

/// Where the bottom message comes from.
enum Messages {
    /// Set by the command line, today's special date, or the config file.
    Fixed(Vec<String>),
    /// Whatever the current bouquet says.
    FromBouquet,
}

pub struct App {
    config: Config,
    today: NaiveDate,
    library: Library,
    current: usize,
    /// The bouquet on screen. The builder edits this copy.
    spec: BouquetSpec,
    layout_seed: u64,
    scene: Scene,
    petal_colors: Vec<Rgb>,
    effects: Effects,
    effects_override: Option<Vec<Effect>>,
    occasion_effects: Vec<Effect>,
    messages: Messages,
    name: Option<String>,
    from: Option<String>,
    years: Option<i32>,
    hud: Hud,
    menu: Option<Menu>,
    /// The buttons along the bottom are hidden.
    bar_hidden: bool,
    /// What was answered, if the message is a question.
    answer: Option<bool>,
    /// How many columns the menu takes up.
    panel: usize,
    rng: Rng,

    spin: f32,
    spin_speed: f32,
    yaw_offset: f32,
    tilt: f32,
    zoom: f32,
    paused: bool,
    drag: Option<(u16, u16)>,
    /// The cell the mouse is over.
    mouse: Option<(usize, usize)>,
    /// Seconds since the last key press or mouse move.
    idle: f32,

    /// Seconds since start.
    time: f32,
    /// Seconds since the current bouquet started growing.
    anim: f32,
    grow: bool,

    recent_keys: Vec<KeyCode>,
    konami_armed: bool,

    canvas: Canvas,
    /// What is on the terminal right now, so only changes get sent.
    shown: Option<Canvas>,
    renderer: Renderer,
    mode: ColorMode,
    style: Style,
    background: Option<Rgb>,
    out: String,
    needs_clear: bool,
    /// Drawing a picture to keep: leave the buttons out of it.
    keepsake: bool,
    quit: bool,
}

/// The daily bouquet's seed: the date as a number, like 20261119.
pub fn daily_seed(day: NaiveDate) -> u64 {
    day.year() as u64 * 10_000 + day.month() as u64 * 100 + day.day() as u64
}

impl App {
    pub fn new(cli: &Cli, config: Config) -> Result<App> {
        let now = Local::now().date_naive();
        let today = match &cli.date {
            Some(s) => config::parse_date_arg(s, now)?,
            None => now,
        };
        let mut rng = Rng::from_time();
        let mut library = Library::load(
            daily_seed(today),
            today.month(),
            config::bouquets_dir().as_deref(),
        );
        let occasion = config.occasion(today).cloned();
        let years = occasion
            .as_ref()
            .and_then(|o| o.since)
            .map(|y| today.year() - y);

        // Which bouquet to start with: command line, then today's special
        // date, then the config file, then the daily bouquet.
        let wanted = cli
            .bouquet
            .clone()
            .or_else(|| occasion.as_ref().and_then(|o| o.bouquet.clone()))
            .or_else(|| config.bouquet.clone());
        let current = match wanted {
            Some(q) => library.resolve(&q, rng.next_u64())?,
            None => 0,
        };
        let mut spec = library.entries[current].spec.clone();
        if let Some(wrap) = cli.wrap {
            spec.wrap = wrap;
            spec.wrap_color = None;
        }
        let layout_seed = cli
            .seed
            .or(spec.seed)
            .unwrap_or_else(|| hash_str(&spec.name));

        let messages = if !cli.message.is_empty() {
            Messages::Fixed(cli.message.clone())
        } else if let Some(m) = occasion.as_ref().and_then(|o| o.message.clone()) {
            Messages::Fixed(vec![m])
        } else if !config.messages.is_empty() {
            Messages::Fixed(config.messages.clone())
        } else {
            Messages::FromBouquet
        };

        let name = cli.name.clone().or_else(|| config.name.clone());
        let from = cli.from.clone().or_else(|| config.from.clone());
        let mut hud = Hud::new(config.message_interval.max(1.0));
        if let Some(w) = library.warnings.first() {
            hud.toast(format!("skipped a bouquet file: {w}"));
        }

        let mode = cli.color.unwrap_or(config.color).resolve();
        let mut app = App {
            today,
            current,
            spec,
            layout_seed,
            scene: Scene::empty(),
            petal_colors: Vec::new(),
            effects: Effects::new(rng.next_u64()),
            effects_override: if cli.no_effects {
                Some(Vec::new())
            } else {
                cli.effects.clone()
            },
            occasion_effects: occasion.map(|o| o.effects).unwrap_or_default(),
            messages,
            name,
            from,
            years,
            hud,
            menu: cli.menu.then(Menu::new),
            bar_hidden: cli.clean || !config.buttons,
            answer: None,
            panel: 0,
            spin: 0.0,
            spin_speed: cli.speed.unwrap_or(config.spin_speed),
            yaw_offset: 0.0,
            tilt: DEFAULT_TILT,
            zoom: 1.0,
            paused: false,
            drag: None,
            mouse: None,
            idle: 0.0,
            time: 0.0,
            anim: 0.0,
            grow: config.grow && !cli.no_grow,
            recent_keys: Vec::new(),
            konami_armed: false,
            canvas: Canvas::new(0, 0),
            shown: None,
            renderer: Renderer::new(),
            mode,
            style: cli
                .style
                .or(config.style)
                .unwrap_or_else(|| Style::detect(mode)),
            background: config.background.as_deref().and_then(Rgb::parse),
            out: String::new(),
            needs_clear: false,
            keepsake: false,
            quit: false,
            library,
            config,
            rng,
        };
        app.apply(true);
        Ok(app)
    }

    // ----- bouquet management -----

    /// Rebuilds the scene from `self.spec`. With `replay`, it grows in again.
    fn apply(&mut self, replay: bool) {
        self.scene = scene::build(&self.spec, self.layout_seed);
        self.anim = if replay && self.grow { 0.0 } else { GROWN };
        if replay {
            self.answer = None;
        }
        let mut fx = self
            .effects_override
            .clone()
            .unwrap_or_else(|| self.spec.effects.clone());
        fx.extend(self.occasion_effects.iter().copied());
        self.effects.set(&fx);
        self.petal_colors = self.spec.petal_colors();
        self.refresh_messages();
    }

    /// Sets the words under the bouquet, and how they look, from the
    /// bouquet (or from the command line or config, when those say).
    fn refresh_messages(&mut self) {
        let spec = &self.spec;
        let or = |value: &Option<String>, fallback: &str| {
            value.clone().unwrap_or_else(|| fallback.to_string())
        };
        let raw = match (self.answer.filter(|_| spec.ask), &self.messages) {
            (Some(true), _) => vec![or(&spec.yes_reply, menu::DEFAULT_YES_REPLY)],
            (Some(false), _) => vec![or(&spec.no_reply, menu::DEFAULT_NO_REPLY)],
            (None, Messages::Fixed(list)) => list.clone(),
            (None, Messages::FromBouquet) => vec![or(&spec.message, "for you ♥")],
        };
        let filled: Vec<String> = raw.iter().map(|m| self.fill(m)).collect();
        let signature = spec
            .signature
            .clone()
            .or_else(|| self.from.as_ref().map(|f| format!("— {f}")));
        self.hud.look = Look {
            big: spec.message_big,
            color: spec.message_color.as_deref().and_then(Rgb::parse),
            effect: spec.message_effect,
        };
        self.hud.signature = signature.map(|s| self.fill(&s));
        self.hud.ask = (spec.ask && self.answer.is_none()).then(|| {
            (
                or(&spec.yes, menu::DEFAULT_YES),
                or(&spec.no, menu::DEFAULT_NO),
            )
        });
        self.hud.set_messages(filled);
    }

    /// An answer to the question was clicked.
    fn answered(&mut self, yes: bool) {
        self.answer = Some(yes);
        self.refresh_messages();
        if yes {
            let (cx, cy) = self.screen_center();
            self.effects.burst(cx, cy, 160);
            self.effects.enable(Effect::Hearts);
            self.effects.enable(Effect::Sparkles);
        }
    }

    fn fill(&self, template: &str) -> String {
        config::fill(
            template,
            self.name.as_deref(),
            self.from.as_deref(),
            self.years,
        )
    }

    fn switch_to(&mut self, index: usize) {
        self.current = index;
        self.spec = self.library.entries[index].spec.clone();
        self.layout_seed = self.spec.seed.unwrap_or_else(|| hash_str(&self.spec.name));
        self.apply(true);
        self.hud.toast(format!("♥ {}", self.spec.name));
    }

    fn step_bouquet(&mut self, step: i32) {
        let n = self.library.entries.len() as i32;
        self.switch_to((self.current as i32 + step).rem_euclid(n) as usize);
    }

    fn reshuffle(&mut self) {
        self.layout_seed = self.rng.next_u64();
        self.apply(false);
        self.hud.toast("reshuffled");
    }

    fn random_bouquet(&mut self) {
        let i = self.library.add_random(self.rng.next_u64());
        self.switch_to(i);
    }

    /// Saves the current bouquet (including its exact layout) to the bouquets folder.
    fn pin(&mut self) {
        let Some(dir) = config::bouquets_dir() else {
            self.hud.toast("no config folder to save into");
            return;
        };
        let mut spec = self.spec.clone();
        spec.seed = Some(self.layout_seed);
        if matches!(
            self.library.entries[self.current].source,
            Source::Daily | Source::Random
        ) {
            spec.name = format!("{}-{}", spec.name, self.today.format("%Y-%m-%d"));
        }
        match library::save(&dir, &spec) {
            Ok(path) => {
                self.current = self.library.upsert_saved(path.clone(), spec.clone());
                self.spec = spec;
                self.hud.toast(format!("pinned ♥ {}", path.display()));
            }
            Err(e) => self.hud.toast(format!("couldn't save: {e:#}")),
        }
    }

    fn snapshot_now(&mut self) {
        let dir = match &self.config.export_dir {
            Some(d) => config::expand_tilde(d),
            None => std::env::current_dir().unwrap_or_default(),
        };
        self.keepsake = true;
        self.render(self.canvas.width(), self.canvas.height());
        self.keepsake = false;
        match export::snapshot(&dir, &self.canvas, self.background.unwrap_or(DEFAULT_BG)) {
            Ok(path) => self.hud.toast(format!("saved ♥ {}", path.display())),
            Err(e) => self.hud.toast(format!("couldn't save: {e:#}")),
        }
    }

    // ----- input -----

    fn handle_event(&mut self, ev: Event) {
        match ev {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                self.idle = 0.0;
                self.on_key(key);
            }
            Event::Mouse(m) => {
                self.idle = 0.0;
                self.on_mouse(m);
            }
            Event::Resize(..) => self.needs_clear = true,
            _ => {}
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.hud.help {
            self.hud.help = false;
            return;
        }
        if self.hud.whisper.is_some() {
            self.on_whisper_key(key);
            return;
        }
        if let Some(menu) = self.menu.as_mut() {
            let outcome = menu.handle_key(key, &mut self.spec);
            self.on_menu(outcome);
            return;
        }
        if self.check_konami(key.code) {
            return;
        }
        if self.hud.ask.is_some() {
            match key.code {
                KeyCode::Char('y') => return self.answered(true),
                KeyCode::Char('n') => return self.answered(false),
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('q') => self.quit = true,
            // Escape is easy to press by accident on the way out of a menu.
            KeyCode::Esc => self.hud.toast("press q to leave"),
            KeyCode::Char('?') => self.press(Button::Help),
            KeyCode::Char('h') => self.toggle_bar(),
            KeyCode::Char(' ') => self.press(Button::Pause),
            KeyCode::Left => self.yaw_offset -= 0.15,
            KeyCode::Right => self.yaw_offset += 0.15,
            KeyCode::Up => self.tilt = (self.tilt + 0.06).clamp(-0.5, 1.3),
            KeyCode::Down => self.tilt = (self.tilt - 0.06).clamp(-0.5, 1.3),
            KeyCode::Char('+') | KeyCode::Char('=') => self.zoom_by(1.1),
            KeyCode::Char('-') | KeyCode::Char('_') => self.zoom_by(1.0 / 1.1),
            KeyCode::Char('0') => {
                self.zoom = 1.0;
                self.tilt = DEFAULT_TILT;
                self.yaw_offset = 0.0;
            }
            KeyCode::Char('[') => self.change_speed(-0.05),
            KeyCode::Char(']') => self.change_speed(0.05),
            KeyCode::Tab | KeyCode::Char('n') => self.press(Button::Next),
            KeyCode::BackTab | KeyCode::Char('p') => self.press(Button::Prev),
            KeyCode::Char('r') => self.reshuffle(),
            KeyCode::Char('s') | KeyCode::Char('R') => self.press(Button::Surprise),
            KeyCode::Char('g') => self.anim = 0.0,
            KeyCode::Char('m') | KeyCode::Char('b') | KeyCode::Enter => self.press(Button::Menu),
            KeyCode::Char('v') => self.change_look(1),
            KeyCode::Char('P') => self.pin(),
            KeyCode::Char('x') => self.snapshot_now(),
            KeyCode::Char('/') => self.hud.whisper = Some(String::new()),
            KeyCode::Char(c @ '1'..='6') => {
                let effect = Effect::ALL[c as usize - '1' as usize];
                let on = self.effects.toggle(effect);
                self.hud.toast(format!(
                    "{} {}",
                    effect.label(),
                    if on { "on" } else { "off" }
                ));
            }
            _ => {}
        }
    }

    /// Does what a button along the bottom says, whether it was clicked
    /// or its key was pressed.
    fn press(&mut self, button: Button) {
        match button {
            Button::Prev => self.step_bouquet(-1),
            Button::Next => self.step_bouquet(1),
            Button::Menu => self.menu = Some(Menu::new()),
            Button::Surprise => self.random_bouquet(),
            Button::Help => self.hud.help = !self.hud.help,
            Button::Yes => self.answered(true),
            Button::No => self.answered(false),
            Button::Pause => {
                self.paused = !self.paused;
                self.hud
                    .toast(if self.paused { "paused" } else { "spinning" });
            }
        }
    }

    fn toggle_bar(&mut self) {
        self.bar_hidden = !self.bar_hidden;
        if self.bar_hidden {
            self.hud
                .toast("buttons hidden · press h to bring them back");
        }
    }

    fn change_look(&mut self, step: i32) {
        self.style = cycle(&Style::ALL, self.style, step);
        self.hud.toast(format!("drawn with {}", self.style.label()));
    }

    fn zoom_by(&mut self, k: f32) {
        self.zoom = (self.zoom * k).clamp(0.4, 3.0);
    }

    fn change_speed(&mut self, delta: f32) {
        self.spin_speed = (self.spin_speed + delta).clamp(-3.0, 3.0);
        self.hud.toast(format!("spin {:.2}", self.spin_speed));
    }

    fn on_mouse(&mut self, m: MouseEvent) {
        let at = (m.column as usize, m.row as usize);
        self.mouse = Some(at);
        let click = m.kind == MouseEventKind::Down(MouseButton::Left);
        if self.hud.help {
            if click {
                self.hud.help = false;
            }
            return;
        }
        // The menu gets the mouse while it is over the menu, unless the
        // bouquet is being dragged.
        if self.drag.is_none()
            && at.0 < self.panel
            && let Some(menu) = self.menu.as_mut()
        {
            let outcome = menu.handle_mouse(m, &mut self.spec);
            self.on_menu(outcome);
            return;
        }
        match m.kind {
            MouseEventKind::Down(MouseButton::Left) => match self.hud.button_at(at.0, at.1) {
                Some(button) => self.press(button),
                None => self.drag = Some((m.column, m.row)),
            },
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some((x, y)) = self.drag {
                    self.yaw_offset += (m.column as f32 - x as f32) * 0.04;
                    self.tilt = (self.tilt + (m.row as f32 - y as f32) * 0.05).clamp(-0.5, 1.3);
                    self.drag = Some((m.column, m.row));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => self.drag = None,
            MouseEventKind::ScrollUp => self.zoom_by(1.1),
            MouseEventKind::ScrollDown => self.zoom_by(1.0 / 1.1),
            _ => {}
        }
    }

    fn on_menu(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Nothing => {}
            Outcome::Changed => {
                // Once you edit by hand, your edits win over --effects.
                self.effects_override = None;
                self.answer = None;
                self.apply(false);
            }
            Outcome::Reshuffle => self.reshuffle(),
            Outcome::Save => self.pin(),
            Outcome::Close => self.menu = None,
            Outcome::Look(step) => self.change_look(step),
            Outcome::Bar => self.toggle_bar(),
        }
    }

    /// Watches for ↑↑↓↓←→←→ B A. Returns true if the key was swallowed.
    fn check_konami(&mut self, code: KeyCode) -> bool {
        self.recent_keys.push(code);
        if self.recent_keys.len() > 10 {
            self.recent_keys.remove(0);
        }
        let n = self.recent_keys.len();
        if code == KeyCode::Char('b') && n >= 9 && self.recent_keys[n - 9..n - 1] == KONAMI {
            self.konami_armed = true; // don't open the builder: an 'a' may follow
            return true;
        }
        if self.konami_armed {
            self.konami_armed = false;
            if code == KeyCode::Char('a') {
                let (cx, cy) = self.screen_center();
                self.effects.burst(cx, cy, 160);
                self.effects.enable(Effect::Hearts);
                self.effects.enable(Effect::Sparkles);
                self.hud.rainbow(12.0);
                self.hud.toast("↑↑↓↓←→←→BA  ♥ you found it ♥");
                return true;
            }
        }
        false
    }

    fn on_whisper_key(&mut self, key: KeyEvent) {
        let Some(buf) = self.hud.whisper.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => self.hud.whisper = None,
            KeyCode::Enter => {
                let word = buf.trim().to_string();
                self.hud.whisper = None;
                if !word.is_empty() {
                    self.whisper(&word);
                }
            }
            KeyCode::Backspace => {
                buf.pop();
            }
            KeyCode::Char(c) if buf.chars().count() < 40 => buf.push(c),
            _ => {}
        }
    }

    /// Something was whispered: check the secret words, otherwise let it float away.
    fn whisper(&mut self, word: &str) {
        let squash = |s: &str| -> String {
            s.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect()
        };
        let key = squash(word);
        let (cx, cy) = self.screen_center();
        let float_y = self.canvas.height().saturating_sub(4) as f32;
        let float_x = |text: &str| cx - text.chars().count() as f32 / 2.0;

        if let Some(secret) = self
            .config
            .secrets
            .iter()
            .find(|s| squash(&s.word) == key)
            .cloned()
        {
            if let Some(m) = &secret.message {
                let text = self.fill(m);
                self.hud.float(text.clone(), float_x(&text), float_y);
            }
            if secret.burst {
                self.effects.burst(cx, cy, 90);
            }
            if let Some(q) = &secret.bouquet {
                match self.library.resolve(q, self.rng.next_u64()) {
                    Ok(i) => self.switch_to(i),
                    Err(e) => self.hud.toast(format!("{e}")),
                }
            }
            return;
        }

        if matches!(key.as_str(), "love" | "iloveyou" | "ily" | "iloveu") {
            self.effects.burst(cx, cy, 120);
            self.effects.enable(Effect::Hearts);
            self.hud.toast("♥ ♥ ♥");
            return;
        }

        if let Some(name) = self.name.clone()
            && squash(&name) == key
        {
            let text = format!("♥ {name} ♥");
            self.effects.burst(cx, cy, 100);
            self.hud.rainbow(8.0);
            self.hud.float(text.clone(), float_x(&text), float_y);
            return;
        }

        // Anything else floats away like a little note.
        let text = format!("♥ {word}");
        self.hud.float(text.clone(), float_x(&text), float_y);
    }

    fn screen_center(&self) -> (f32, f32) {
        (
            self.canvas.width() as f32 / 2.0,
            self.canvas.height() as f32 / 2.5,
        )
    }

    // ----- frame -----

    fn update(&mut self, dt: f32) {
        self.time += dt;
        self.anim += dt;
        self.idle += dt;
        if !self.paused {
            self.spin += dt * self.spin_speed;
        }
        self.effects.update(dt, &self.petal_colors);
        self.hud.update(dt);
    }

    fn render(&mut self, w: usize, h: usize) {
        self.canvas.resize(w, h);
        self.canvas.clear();
        if w < 24 || h < 12 {
            self.canvas
                .text(0, 0, "posy needs a bigger window ♥", Rgb(246, 122, 172));
            return;
        }
        let panel = if self.menu.is_some() {
            Menu::WIDTH.min(w / 2)
        } else {
            0
        };
        self.panel = panel;
        // Rows under the bouquet: the message, a signature, and the bar.
        let bar_shown = !self.keepsake && !self.bar_hidden;
        let hud_rows = self.hud.rows(w - panel, bar_shown).min(h / 2);
        let view = Viewport {
            x: panel as i32,
            y: 0,
            w: (w - panel) as i32,
            h: (h - hud_rows) as i32,
        };
        let cam = Camera::new(
            self.spin + self.yaw_offset,
            self.tilt,
            self.zoom,
            view,
            self.scene.framing,
        );

        self.renderer.draw(
            &mut self.canvas,
            &self.scene,
            &cam,
            self.anim,
            self.style,
            self.background.unwrap_or(DEFAULT_BG).to_col(),
        );
        self.effects.draw(&mut self.canvas, &cam, view);
        let fade = ((self.idle - BAR_LINGER) / 1.5).clamp(0.0, 1.0);
        let bar = Bar {
            glow: 1.0 - (1.0 - BAR_RESTING) * fade,
            hover: self.mouse,
            paused: self.paused,
        };
        let background = self.background.unwrap_or(DEFAULT_BG);
        let bar = bar_shown.then_some(&bar);
        self.hud
            .draw(&mut self.canvas, self.time, background, panel, bar);
        if let Some(menu) = &mut self.menu {
            menu.hover = self.mouse;
            menu.draw(&mut self.canvas, &self.spec, self.style, panel);
        }
        if self.paused {
            self.canvas.text(w as i32 - 4, 0, "▌▌", Rgb(120, 112, 136));
        }
    }

    fn present(&mut self, out: &mut impl Write) -> io::Result<()> {
        self.out.clear();
        self.out.push_str("\x1b[?2026h"); // begin synchronized update (no tearing)
        if self.needs_clear {
            self.out.push_str("\x1b[2J");
            self.needs_clear = false;
            self.shown = None;
        }
        self.canvas.write_ansi(
            &mut self.out,
            self.mode,
            self.background,
            self.shown.as_ref(),
        );
        self.shown
            .get_or_insert_with(|| Canvas::new(0, 0))
            .copy_from(&self.canvas);
        self.out.push_str("\x1b[?2026l");
        out.write_all(self.out.as_bytes())?;
        out.flush()
    }

    fn run_loop(&mut self, out: &mut impl Write, fps: u32) -> Result<()> {
        let frame = Duration::from_secs_f64(1.0 / fps as f64);
        let mut last = Instant::now();
        // Frames are due on an even beat, so they don't drift or bunch up.
        let mut due = last;
        while !self.quit {
            let now = Instant::now();
            // A frame that arrives late still only moves things on by a
            // little, so a hiccup shows as a pause, not a jump.
            let dt = (now - last).as_secs_f32().min(1.5 * frame.as_secs_f32());
            last = now;

            self.update(dt);
            let (w, h) = terminal::size()?;
            self.render(w as usize, h as usize);
            self.present(out)?;

            due += frame;
            let now = Instant::now();
            if due < now {
                due = now; // running late: carry on from here, don't catch up
            }
            // Handle input until it's time for the next frame.
            loop {
                let left = due.saturating_duration_since(Instant::now());
                if left.is_zero() || !event::poll(left)? {
                    break;
                }
                self.handle_event(event::read()?);
                if self.quit {
                    break;
                }
            }
        }
        Ok(())
    }
}

fn restore_terminal() {
    let _ = execute!(
        io::stdout(),
        event::DisableMouseCapture,
        cursor::Show,
        terminal::LeaveAlternateScreen
    );
    let _ = terminal::disable_raw_mode();
}

/// Runs the animation until the user quits.
pub fn run(cli: &Cli, config: Config) -> Result<()> {
    // Every frame has to travel down the ssh connection, and fewer,
    // evenly spaced frames look smoother than many that arrive in bursts.
    let remote = ["SSH_CONNECTION", "SSH_TTY"]
        .iter()
        .any(|name| std::env::var_os(name).is_some());
    let fps = cli
        .fps
        .or(config.fps)
        .unwrap_or(if remote { 30 } else { 60 })
        .clamp(5, 120);
    // Looked up in the background; mentioned only once posy has closed.
    let newer = config.update_check.then(crate::update::check_quietly);
    let mut app = App::new(cli, config)?;

    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::Hide,
        event::EnableMouseCapture
    )?;
    // If anything panics, put the terminal back before printing the error.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    let result = app.run_loop(&mut stdout, fps);
    restore_terminal();
    result?;
    println!("\x1b[38;2;246;122;172m♥\x1b[0m");
    if let Some(line) = newer.and_then(|found| found.try_recv().ok()) {
        println!("{line}");
    }
    Ok(())
}

/// Renders a single frame `at` seconds in and writes it to `out`.
pub fn snapshot(
    cli: &Cli,
    config: Config,
    out: &Path,
    size: &str,
    at: f32,
    keys: &[String],
    (frames, every): (usize, f32),
) -> Result<()> {
    let (w, h) = parse_size(size)?;
    let mut app = App::new(cli, config)?;
    for key in keys {
        let code = match key.as_str() {
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "enter" => KeyCode::Enter,
            "esc" => KeyCode::Esc,
            "tab" => KeyCode::Tab,
            "space" => KeyCode::Char(' '),
            other => KeyCode::Char(other.chars().next().context("empty key")?),
        };
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }
    app.keepsake = keys.is_empty();
    let step = 1.0 / 30.0;
    let mut t = 0.0;
    while t < at {
        app.update(step);
        t += step;
    }
    let background = app.background.unwrap_or(DEFAULT_BG);
    if frames <= 1 {
        app.render(w, h);
        export::write(out, &app.canvas, background)?;
        println!("saved {}", out.display());
        return Ok(());
    }
    // A run of frames, for making an animation: out-000.ans, out-001.ans ...
    let stem = out.file_stem().and_then(|s| s.to_str()).unwrap_or("frame");
    let ext = out.extension().and_then(|s| s.to_str()).unwrap_or("ans");
    for i in 0..frames {
        app.render(w, h);
        let path = out.with_file_name(format!("{stem}-{i:03}.{ext}"));
        export::write(&path, &app.canvas, background)?;
        let mut waited = 0.0;
        while waited < every {
            app.update(step);
            waited += step;
        }
    }
    println!("saved {frames} frames next to {}", out.display());
    Ok(())
}

/// Times the pieces of a frame, to find what makes it stutter.
pub fn bench(cli: &Cli, config: Config, size: &str) -> Result<()> {
    let (w, h) = parse_size(size)?;
    let mut app = App::new(cli, config)?;
    let mut out = String::new();
    for (label, from) in [("growing", 2.6f32), ("grown", 30.0)] {
        app.anim = from;
        let (mut draw, mut emit, mut worst, mut bytes) = (0.0f32, 0.0f32, 0.0f32, 0usize);
        let n = 90;
        for _ in 0..n {
            app.update(1.0 / 30.0);
            let t = Instant::now();
            app.render(w, h);
            let a = t.elapsed().as_secs_f32() * 1e3;
            let t = Instant::now();
            out.clear();
            app.canvas
                .write_ansi(&mut out, app.mode, app.background, None);
            let b = t.elapsed().as_secs_f32() * 1e3;
            draw += a;
            emit += b;
            worst = worst.max(a + b);
            bytes += out.len();
        }
        let n = n as f32;
        println!(
            "{label:8} draw {:.2}ms  emit {:.2}ms  worst {:.2}ms  {:.0} KB/frame",
            draw / n,
            emit / n,
            worst,
            bytes as f32 / n / 1024.0
        );
    }
    let tris: usize = app.scene.blooms.iter().map(|m| m.tris.len()).sum();
    println!(
        "triangles: base {} flowers {}",
        app.scene.base.tris.len(),
        tris
    );
    Ok(())
}

/// Parses a size like `100x48` into columns and rows.
pub fn parse_size(size: &str) -> Result<(usize, usize)> {
    size.split_once('x')
        .and_then(|(w, h)| Some((w.parse::<usize>().ok()?, h.parse::<usize>().ok()?)))
        .context("--size should look like 100x48")
}
