//! The "make your own" menu: a side panel for changing the bouquet while it
//! spins next to it. Everything works with the arrow keys and enter, or by
//! clicking.

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use crate::bouquet::{BouquetSpec, Effect, FlowerEntry, FlowerKind, TextEffect, WrapStyle, cycle};
use crate::canvas::Canvas;
use crate::color::{NAMED, Rgb, color_or};
use crate::raster::Style;

const PANEL_BG: Rgb = Rgb(30, 26, 40);
const SELECTED_BG: Rgb = Rgb(78, 46, 86);
const HOVER_BG: Rgb = Rgb(46, 38, 58);
const TEXT: Rgb = Rgb(226, 218, 234);
const DIM: Rgb = Rgb(134, 124, 150);
const ACCENT: Rgb = Rgb(246, 122, 172);

/// The most flowers of one kind, and the fewest.
const MOST: u32 = 12;

/// Which screen of the menu is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Main,
    /// One line of the bouquet: its kind, how many, its colours.
    Flower(usize),
    /// Choosing the kind of flower number `usize`.
    Kind(usize),
    Color(Paint),
}

/// Something that has a colour you can pick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Paint {
    Petal(usize),
    Center(usize),
    Wrap,
    Ribbon,
    Message,
}

/// One line of a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    Header(&'static str),
    Gap,
    Flower(usize),
    AddFlower,
    Wrap,
    Color(Paint),
    Effect(Effect),
    Look,
    /// A line of words you can type.
    Text(Field),
    /// Big or small letters for the message.
    Size,
    /// How the message moves.
    Motion,
    /// Whether the message is a question with answers to click.
    Ask,
    /// Hide or show the buttons along the bottom.
    Bar,
    Shuffle,
    Save,
    Done,
    Kind(usize),
    Count(usize),
    Remove(usize),
    PickKind(FlowerKind),
    /// A colour, by its place in [`NAMED`].
    PickColor(usize),
}

impl Row {
    fn selectable(self) -> bool {
        !matches!(self, Row::Header(_) | Row::Gap)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Name,
    Message,
    Signature,
    Yes,
    No,
    YesReply,
    NoReply,
}

impl Field {
    fn label(self) -> &'static str {
        match self {
            Field::Name => "its name",
            Field::Message => "message",
            Field::Signature => "signed",
            Field::Yes => "one answer",
            Field::No => "the other",
            Field::YesReply => "then say",
            Field::NoReply => "then say",
        }
    }

    /// What it says now (or would say, left alone).
    fn get(self, spec: &BouquetSpec) -> String {
        let or = |value: &Option<String>, fallback: &str| {
            value.clone().unwrap_or_else(|| fallback.to_string())
        };
        match self {
            Field::Name => spec.name.clone(),
            Field::Message => or(&spec.message, "for you ♥"),
            Field::Signature => or(&spec.signature, ""),
            Field::Yes => or(&spec.yes, DEFAULT_YES),
            Field::No => or(&spec.no, DEFAULT_NO),
            Field::YesReply => or(&spec.yes_reply, DEFAULT_YES_REPLY),
            Field::NoReply => or(&spec.no_reply, DEFAULT_NO_REPLY),
        }
    }

    fn set(self, spec: &mut BouquetSpec, text: String) {
        let value = (!text.is_empty()).then_some(text);
        match self {
            Field::Name => {
                if let Some(name) = value {
                    spec.name = name;
                }
            }
            Field::Message => spec.message = value,
            Field::Signature => spec.signature = value,
            Field::Yes => spec.yes = value,
            Field::No => spec.no = value,
            Field::YesReply => spec.yes_reply = value,
            Field::NoReply => spec.no_reply = value,
        }
    }
}

pub const DEFAULT_YES: &str = "yes ♥";
pub const DEFAULT_NO: &str = "no";
pub const DEFAULT_YES_REPLY: &str = "♥ ♥ ♥";
pub const DEFAULT_NO_REPLY: &str = "that's okay ♥";

/// What the app should do after a key press or click in the menu.
pub enum Outcome {
    Nothing,
    /// The bouquet changed; rebuild the scene.
    Changed,
    Reshuffle,
    Save,
    Close,
    /// Switch to the next (or previous) way of drawing.
    Look(i32),
    /// Hide or show the buttons along the bottom.
    Bar,
}

/// A place you can click, remembered from the last time the menu was drawn.
#[derive(Clone, Copy)]
struct Hit {
    y: usize,
    x0: usize,
    x1: usize,
    target: Target,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Row(usize),
    Less(usize),
    More(usize),
    Back,
}

/// How one row is drawn.
#[derive(Default)]
struct Line {
    /// A colour dot before the label.
    dot: Option<Rgb>,
    label: String,
    value: Option<String>,
    /// A colour dot before the value.
    value_dot: Option<Rgb>,
    /// Show ‹ › around the value: left and right change it.
    arrows: bool,
    /// A check mark at the right: this is the one currently chosen.
    tick: bool,
    faint: bool,
}

pub struct Menu {
    /// The pages you have gone through, each with where its cursor is.
    stack: Vec<(Page, usize)>,
    edit: Option<(Field, String)>,
    /// The bouquet as it was before a picker opened, to put back on escape.
    backup: Option<BouquetSpec>,
    scroll: usize,
    hits: Vec<Hit>,
    /// The cell the mouse is over.
    pub hover: Option<(usize, usize)>,
}

impl Menu {
    pub const WIDTH: usize = 40;

    pub fn new() -> Self {
        Menu {
            stack: vec![(Page::Main, 1)],
            edit: None,
            backup: None,
            scroll: 0,
            hits: Vec::new(),
            hover: None,
        }
    }

    fn page(&self) -> Page {
        self.stack.last().map_or(Page::Main, |p| p.0)
    }

    fn cursor(&self) -> usize {
        self.stack.last().map_or(0, |p| p.1)
    }

    fn set_cursor(&mut self, cursor: usize) {
        if let Some(top) = self.stack.last_mut() {
            top.1 = cursor;
        }
    }

    fn rows(&self, spec: &BouquetSpec) -> Vec<Row> {
        match self.page() {
            Page::Main => {
                let mut rows = vec![Row::Header("flowers")];
                rows.extend((0..spec.flowers.len()).map(Row::Flower));
                rows.extend([
                    Row::AddFlower,
                    Row::Gap,
                    Row::Header("held in"),
                    Row::Wrap,
                    Row::Color(Paint::Wrap),
                    Row::Color(Paint::Ribbon),
                    Row::Gap,
                    Row::Header("floating around"),
                ]);
                rows.extend(Effect::ALL.iter().map(|&e| Row::Effect(e)));
                rows.extend([
                    Row::Gap,
                    Row::Header("words"),
                    Row::Text(Field::Message),
                    Row::Size,
                    Row::Color(Paint::Message),
                    Row::Motion,
                    Row::Text(Field::Signature),
                    Row::Gap,
                    Row::Header("a question"),
                    Row::Ask,
                ]);
                if spec.ask {
                    rows.extend([
                        Row::Text(Field::Yes),
                        Row::Text(Field::YesReply),
                        Row::Text(Field::No),
                        Row::Text(Field::NoReply),
                    ]);
                }
                rows.extend([
                    Row::Gap,
                    Row::Header("picture"),
                    Row::Look,
                    Row::Bar,
                    Row::Text(Field::Name),
                    Row::Gap,
                    Row::Shuffle,
                    Row::Save,
                    Row::Done,
                ]);
                rows
            }
            Page::Flower(i) => {
                let mut rows = vec![Row::Kind(i), Row::Count(i), Row::Color(Paint::Petal(i))];
                if spec.flowers.get(i).is_some_and(|f| f.kind.radius() > 0.0) {
                    rows.push(Row::Color(Paint::Center(i)));
                }
                rows.extend([Row::Gap, Row::Remove(i), Row::Done]);
                rows
            }
            Page::Kind(_) => FlowerKind::ALL.into_iter().map(Row::PickKind).collect(),
            Page::Color(_) => (0..NAMED.len()).map(Row::PickColor).collect(),
        }
    }

    /// The row the cursor is on, with the cursor nudged onto something
    /// selectable if the page changed under it.
    fn current(&mut self, rows: &[Row]) -> Row {
        let mut cursor = self.cursor().min(rows.len() - 1);
        if !rows[cursor].selectable() {
            cursor = rows.iter().position(|r| r.selectable()).unwrap_or(0);
        }
        self.set_cursor(cursor);
        rows[cursor]
    }

    // ----- keys and clicks -----

    pub fn handle_key(&mut self, key: KeyEvent, spec: &mut BouquetSpec) -> Outcome {
        if self.edit.is_some() {
            return self.edit_key(key, spec);
        }
        let rows = self.rows(spec);
        let row = self.current(&rows);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.step(-1, spec),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.step(1, spec),
            KeyCode::Left => adjust(row, -1, spec),
            KeyCode::Right => adjust(row, 1, spec),
            KeyCode::Enter | KeyCode::Char(' ') => self.activate(row, spec),
            KeyCode::Esc | KeyCode::Backspace => self.back(spec),
            KeyCode::Char('m') | KeyCode::Char('q') => self.close(spec),
            KeyCode::Delete | KeyCode::Char('d') => match row {
                Row::Flower(i) => {
                    spec.flowers.remove(i);
                    Outcome::Changed
                }
                _ => Outcome::Nothing,
            },
            _ => Outcome::Nothing,
        }
    }

    pub fn handle_mouse(&mut self, m: MouseEvent, spec: &mut BouquetSpec) -> Outcome {
        let at = (m.column as usize, m.row as usize);
        match m.kind {
            MouseEventKind::ScrollUp if self.edit.is_none() => return self.step(-1, spec),
            MouseEventKind::ScrollDown if self.edit.is_none() => return self.step(1, spec),
            MouseEventKind::Down(MouseButton::Left) => {
                // A click anywhere keeps whatever was being typed.
                let typed = if self.edit.is_some() {
                    self.finish_edit(spec)
                } else {
                    Outcome::Nothing
                };
                // The ‹ and › of a row sit on top of the row itself.
                let hit = self
                    .hits
                    .iter()
                    .filter(|h| h.y == at.1 && (h.x0..=h.x1).contains(&at.0))
                    .min_by_key(|h| matches!(h.target, Target::Row(_)))
                    .copied();
                let Some(hit) = hit else {
                    return typed;
                };
                let rows = self.rows(spec);
                let clicked = match hit.target {
                    Target::Back => self.back(spec),
                    Target::Row(i) | Target::Less(i) | Target::More(i) if i >= rows.len() => {
                        Outcome::Nothing
                    }
                    Target::Row(i) => {
                        self.set_cursor(i);
                        self.preview(spec);
                        self.activate(rows[i], spec)
                    }
                    Target::Less(i) => {
                        self.set_cursor(i);
                        adjust(rows[i], -1, spec)
                    }
                    Target::More(i) => {
                        self.set_cursor(i);
                        adjust(rows[i], 1, spec)
                    }
                };
                return match (typed, clicked) {
                    (Outcome::Changed, Outcome::Nothing) => Outcome::Changed,
                    (_, clicked) => clicked,
                };
            }
            _ => {}
        }
        Outcome::Nothing
    }

    /// Moves the cursor to the next row you can select. In a picker the
    /// bouquet follows along, so you see each choice before you make it.
    fn step(&mut self, by: i32, spec: &mut BouquetSpec) -> Outcome {
        let rows = self.rows(spec);
        self.current(&rows);
        let n = rows.len() as i32;
        let mut cursor = self.cursor() as i32;
        for _ in 0..n {
            cursor = (cursor + by).rem_euclid(n);
            if rows[cursor as usize].selectable() {
                break;
            }
        }
        self.set_cursor(cursor as usize);
        self.preview(spec)
    }

    /// In a picker, tries the choice under the cursor on the bouquet.
    fn preview(&mut self, spec: &mut BouquetSpec) -> Outcome {
        let rows = self.rows(spec);
        match (self.page(), rows.get(self.cursor())) {
            (Page::Kind(i), Some(&Row::PickKind(kind))) => {
                if let Some(f) = spec.flowers.get_mut(i) {
                    f.kind = kind;
                }
                Outcome::Changed
            }
            (Page::Color(paint), Some(&Row::PickColor(c))) => {
                set_paint(spec, paint, NAMED[c].0.to_string());
                Outcome::Changed
            }
            _ => Outcome::Nothing,
        }
    }

    fn activate(&mut self, row: Row, spec: &mut BouquetSpec) -> Outcome {
        match row {
            Row::Header(_) | Row::Gap => {}
            Row::Flower(i) => self.open(Page::Flower(i), 0),
            Row::AddFlower => {
                // Put a new flower in right away so you can watch it change
                // as you move through the list. Escape takes it out again.
                self.backup = Some(spec.clone());
                let kind = FlowerKind::ALL
                    .into_iter()
                    .find(|k| !spec.flowers.iter().any(|f| f.kind == *k))
                    .unwrap_or(FlowerKind::Rose);
                spec.flowers.push(FlowerEntry {
                    kind,
                    count: 3,
                    color: None,
                    center: None,
                });
                let at = FlowerKind::ALL.iter().position(|k| *k == kind).unwrap_or(0);
                self.open(Page::Kind(spec.flowers.len() - 1), at);
                return Outcome::Changed;
            }
            Row::Kind(i) => {
                self.backup = Some(spec.clone());
                let kind = spec.flowers[i].kind;
                let at = FlowerKind::ALL.iter().position(|k| *k == kind).unwrap_or(0);
                self.open(Page::Kind(i), at);
            }
            Row::Color(paint) => {
                self.backup = Some(spec.clone());
                let name = paint_name(spec, paint);
                let at = NAMED
                    .iter()
                    .position(|(n, _)| n.eq_ignore_ascii_case(&name))
                    .unwrap_or(0);
                self.open(Page::Color(paint), at);
            }
            Row::Wrap
            | Row::Effect(_)
            | Row::Count(_)
            | Row::Look
            | Row::Size
            | Row::Motion
            | Row::Ask => return adjust(row, 1, spec),
            Row::Bar => return Outcome::Bar,
            Row::Text(field) => self.edit = Some((field, field.get(spec))),
            Row::Shuffle => return Outcome::Reshuffle,
            Row::Save => return Outcome::Save,
            Row::Done => return self.back(spec),
            Row::Remove(i) => {
                spec.flowers.remove(i);
                self.stack.truncate(1);
                self.scroll = 0;
                return Outcome::Changed;
            }
            Row::PickKind(_) | Row::PickColor(_) => {
                // The bouquet already shows this choice; keep it.
                self.backup = None;
                let page = self.page();
                self.stack.pop();
                self.scroll = 0;
                // A flower just added: leave the cursor on it.
                if let (Page::Kind(i), Page::Main) = (page, self.page()) {
                    let rows = self.rows(spec);
                    if let Some(at) = rows.iter().position(|r| *r == Row::Flower(i)) {
                        self.set_cursor(at);
                    }
                }
                return Outcome::Changed;
            }
        }
        Outcome::Nothing
    }

    fn open(&mut self, page: Page, cursor: usize) {
        self.stack.push((page, cursor));
        self.scroll = 0;
    }

    /// Goes back a page, undoing a picker that was only being looked at.
    fn back(&mut self, spec: &mut BouquetSpec) -> Outcome {
        if self.stack.len() <= 1 {
            return Outcome::Close;
        }
        self.stack.pop();
        self.scroll = 0;
        match self.backup.take() {
            Some(old) => {
                *spec = old;
                Outcome::Changed
            }
            None => Outcome::Nothing,
        }
    }

    fn close(&mut self, spec: &mut BouquetSpec) -> Outcome {
        if let Some(old) = self.backup.take() {
            *spec = old;
        }
        Outcome::Close
    }

    fn edit_key(&mut self, key: KeyEvent, spec: &mut BouquetSpec) -> Outcome {
        let Some((_, buf)) = &mut self.edit else {
            return Outcome::Nothing;
        };
        match key.code {
            KeyCode::Enter => return self.finish_edit(spec),
            KeyCode::Esc => self.edit = None,
            KeyCode::Backspace => {
                buf.pop();
            }
            KeyCode::Char(c) if buf.chars().count() < 60 => buf.push(c),
            _ => {}
        }
        Outcome::Nothing
    }

    fn finish_edit(&mut self, spec: &mut BouquetSpec) -> Outcome {
        let Some((field, buf)) = self.edit.take() else {
            return Outcome::Nothing;
        };
        let text = buf.trim().to_string();
        field.set(spec, text);
        Outcome::Changed
    }

    // ----- drawing -----

    pub fn draw(&mut self, canvas: &mut Canvas, spec: &BouquetSpec, style: Style, width: usize) {
        let h = canvas.height();
        canvas.fill_rect(0, 0, width, h, PANEL_BG);
        self.hits.clear();
        let rows = self.rows(spec);
        self.current(&rows);
        let (page, cursor) = (self.page(), self.cursor());

        let title = match page {
            Page::Main => "✿ make your own".to_string(),
            Page::Flower(i) => match spec.flowers.get(i) {
                Some(f) => format!("‹ {}", f.kind.label()),
                None => "‹ back".into(),
            },
            Page::Kind(_) => "‹ pick a flower".into(),
            Page::Color(_) => "‹ pick a colour".into(),
        };
        canvas.text(2, 1, &title, ACCENT);
        if page != Page::Main {
            self.hits.push(Hit {
                y: 1,
                x0: 0,
                x1: width,
                target: Target::Back,
            });
        }

        // Scroll just enough to keep the cursor in view.
        let top = 3;
        let room = h.saturating_sub(top + 4).max(1);
        if cursor < self.scroll + 1 {
            self.scroll = cursor.saturating_sub(1);
        } else if cursor + 2 > self.scroll + room {
            self.scroll = cursor + 2 - room;
        }
        self.scroll = self.scroll.min(rows.len().saturating_sub(room));

        // Where the value of a row starts, and where its › goes.
        let value_x = width / 2;
        let more_x = width.saturating_sub(3);
        for (k, (i, &row)) in rows
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(room)
            .enumerate()
        {
            let y = top + k;
            let line = self.line(row, spec, style);
            let selected = i == cursor;
            if row.selectable() {
                let hovered = self.hover.is_some_and(|(x, hy)| hy == y && x < width);
                if selected {
                    canvas.fill_rect(1, y, width.saturating_sub(2), 1, SELECTED_BG);
                    canvas.text(2, y as i32, "▸", ACCENT);
                } else if hovered {
                    canvas.fill_rect(1, y, width.saturating_sub(2), 1, HOVER_BG);
                }
                self.hits.push(Hit {
                    y,
                    x0: 0,
                    x1: width.saturating_sub(1),
                    target: Target::Row(i),
                });
            }
            let y = y as i32;
            let mut x = if row.selectable() { 4 } else { 2 };
            if let Some(dot) = line.dot {
                canvas.text(x, y, "●", dot);
                x += 2;
            }
            let room_for_label = if line.value.is_some() {
                value_x.saturating_sub(x as usize + 1)
            } else {
                width.saturating_sub(x as usize + 2)
            };
            let color = if line.faint { DIM } else { TEXT };
            canvas.text(x, y, &clip(&line.label, room_for_label), color);

            if let Some(value) = &line.value {
                let mut x = value_x;
                if line.arrows {
                    let arrow = if selected { ACCENT } else { DIM };
                    canvas.text(x as i32, y, "‹", arrow);
                    canvas.text(more_x as i32, y, "›", arrow);
                    self.hits.push(Hit {
                        y: y as usize,
                        x0: x.saturating_sub(1),
                        x1: x + 1,
                        target: Target::Less(i),
                    });
                    self.hits.push(Hit {
                        y: y as usize,
                        x0: more_x.saturating_sub(1),
                        x1: more_x + 1,
                        target: Target::More(i),
                    });
                    x += 2;
                }
                if let Some(dot) = line.value_dot {
                    canvas.text(x as i32, y, "●", dot);
                    x += 2;
                }
                let room = more_x.saturating_sub(x + 1);
                canvas.text(x as i32, y, &clip(value, room), TEXT);
            }
            if line.tick {
                canvas.text(more_x as i32, y, "✓", ACCENT);
            }
        }
        if self.scroll + room < rows.len() {
            canvas.text(2, (top + room) as i32, "⌄ more below", DIM);
        }

        let hints = if self.edit.is_some() {
            ["type your words", "enter to keep them · esc to cancel"]
        } else if page == Page::Main {
            [
                "↑ ↓ move   ← → change   enter open",
                "or just click · esc closes",
            ]
        } else {
            ["↑ ↓ move   enter choose", "or just click · esc goes back"]
        };
        canvas.text(
            2,
            h as i32 - 3,
            &clip(hints[0], width.saturating_sub(3)),
            DIM,
        );
        canvas.text(
            2,
            h as i32 - 2,
            &clip(hints[1], width.saturating_sub(3)),
            DIM,
        );
    }

    fn line(&self, row: Row, spec: &BouquetSpec, style: Style) -> Line {
        let chooser = |label: &str, value: String| Line {
            label: label.into(),
            value: Some(value),
            arrows: true,
            ..Line::default()
        };
        let button = |label: &str| Line {
            label: label.into(),
            ..Line::default()
        };
        let typed = |label: &str, field: Field, current: String| {
            let value = match &self.edit {
                Some((f, buf)) if *f == field => format!("{buf}▏"),
                _ => current,
            };
            Line {
                label: label.into(),
                value: Some(value),
                ..Line::default()
            }
        };
        match row {
            Row::Header(text) => Line {
                label: text.into(),
                faint: true,
                ..Line::default()
            },
            Row::Gap => Line::default(),
            Row::Flower(i) => {
                let f = &spec.flowers[i];
                Line {
                    dot: Some(paint_color(spec, Paint::Petal(i))),
                    ..chooser(f.kind.label(), f.count.to_string())
                }
            }
            Row::AddFlower => button("+ add a flower"),
            Row::Wrap => chooser("what", wrap_label(spec.wrap).into()),
            Row::Color(paint) => {
                let label = match paint {
                    Paint::Petal(_) => "colour",
                    Paint::Center(_) => "middle",
                    Paint::Wrap => "its colour",
                    Paint::Ribbon => "ribbon",
                    Paint::Message => "colour",
                };
                Line {
                    value_dot: Some(paint_color(spec, paint)),
                    ..chooser(label, paint_name(spec, paint))
                }
            }
            Row::Effect(e) => {
                let on = spec.effects.contains(&e);
                Line {
                    label: e.label().into(),
                    value: Some(if on { "● on" } else { "○ off" }.into()),
                    faint: !on,
                    ..Line::default()
                }
            }
            Row::Look => chooser("drawn with", style.label().into()),
            Row::Text(field) => typed(field.label(), field, field.get(spec)),
            Row::Size => chooser(
                "letters",
                if spec.message_big { "big" } else { "small" }.into(),
            ),
            Row::Motion => chooser("moves", spec.message_effect.label().into()),
            Row::Ask => chooser("ask it", if spec.ask { "yes" } else { "no" }.into()),
            Row::Bar => button("hide or show the bottom buttons"),
            Row::Shuffle => button("✿ rearrange the flowers"),
            Row::Save => button("♥ save this bouquet"),
            Row::Done => button(if self.page() == Page::Main {
                "✓ done"
            } else {
                "‹ back"
            }),
            Row::Kind(i) => chooser("flower", spec.flowers[i].kind.label().into()),
            Row::Count(i) => chooser("how many", spec.flowers[i].count.to_string()),
            Row::Remove(_) => button("✕ take it out"),
            Row::PickKind(kind) => Line {
                dot: Some(color_or(None, kind.default_color())),
                tick: matches!(self.page(), Page::Kind(i) if spec.flowers.get(i).is_some_and(|f| f.kind == kind)),
                ..button(kind.label())
            },
            Row::PickColor(c) => Line {
                dot: Some(NAMED[c].1),
                tick: matches!(self.page(), Page::Color(p) if paint_name(spec, p).eq_ignore_ascii_case(NAMED[c].0)),
                ..button(NAMED[c].0)
            },
        }
    }
}

fn wrap_label(wrap: WrapStyle) -> &'static str {
    match wrap {
        WrapStyle::Cone => "paper cone",
        WrapStyle::Vase => "glass vase",
        WrapStyle::Jar => "jam jar",
        WrapStyle::Basket => "basket",
        WrapStyle::Bare => "just a ribbon",
    }
}

/// The name of the colour something has now.
fn paint_name(spec: &BouquetSpec, paint: Paint) -> String {
    let flower = |i: usize| spec.flowers.get(i);
    match paint {
        Paint::Petal(i) => flower(i).map_or("pink".to_string(), |f| {
            f.color.clone().unwrap_or(f.kind.default_color().into())
        }),
        Paint::Center(i) => flower(i).map_or("gold".to_string(), |f| {
            f.center.clone().unwrap_or(f.kind.default_center().into())
        }),
        Paint::Wrap => spec
            .wrap_color
            .clone()
            .unwrap_or(spec.wrap.default_color().into()),
        Paint::Ribbon => spec.ribbon_color.clone().unwrap_or("pink".into()),
        Paint::Message => spec.message_color.clone().unwrap_or("pink".into()),
    }
}

fn paint_color(spec: &BouquetSpec, paint: Paint) -> Rgb {
    color_or(Some(&paint_name(spec, paint)), "pink")
}

fn set_paint(spec: &mut BouquetSpec, paint: Paint, name: String) {
    match paint {
        Paint::Petal(i) => {
            if let Some(f) = spec.flowers.get_mut(i) {
                f.color = Some(name);
            }
        }
        Paint::Center(i) => {
            if let Some(f) = spec.flowers.get_mut(i) {
                f.center = Some(name);
            }
        }
        Paint::Wrap => spec.wrap_color = Some(name),
        Paint::Ribbon => spec.ribbon_color = Some(name),
        Paint::Message => spec.message_color = Some(name),
    }
}

/// ← / → on a row.
fn adjust(row: Row, step: i32, spec: &mut BouquetSpec) -> Outcome {
    match row {
        Row::Flower(i) | Row::Count(i) => {
            let f = &mut spec.flowers[i];
            f.count = (f.count as i32 + step).clamp(1, MOST as i32) as u32;
        }
        Row::Kind(i) => {
            let f = &mut spec.flowers[i];
            f.kind = cycle(&FlowerKind::ALL, f.kind, step);
        }
        Row::Wrap => {
            spec.wrap = cycle(&WrapStyle::ALL, spec.wrap, step);
            spec.wrap_color = None; // each one has its own natural colour
        }
        Row::Color(paint) => {
            let name = paint_name(spec, paint);
            let n = NAMED.len() as i32;
            let next = match NAMED
                .iter()
                .position(|(known, _)| known.eq_ignore_ascii_case(&name))
            {
                Some(i) => (i as i32 + step).rem_euclid(n),
                None => 0,
            };
            set_paint(spec, paint, NAMED[next as usize].0.to_string());
        }
        Row::Effect(e) => {
            if let Some(i) = spec.effects.iter().position(|&x| x == e) {
                spec.effects.remove(i);
            } else {
                spec.effects.push(e);
            }
        }
        Row::Look => return Outcome::Look(step),
        Row::Size => spec.message_big = !spec.message_big,
        Row::Motion => spec.message_effect = cycle(&TextEffect::ALL, spec.message_effect, step),
        Row::Ask => spec.ask = !spec.ask,
        _ => return Outcome::Nothing,
    }
    Outcome::Changed
}

/// Cuts `s` to at most `max` characters, ending in "…" if it was longer.
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn press(menu: &mut Menu, spec: &mut BouquetSpec, code: KeyCode) {
        menu.handle_key(KeyEvent::new(code, KeyModifiers::NONE), spec);
    }

    #[test]
    fn adding_a_flower_lets_you_pick_which() {
        let mut spec = BouquetSpec::default();
        let mut menu = Menu::new();
        // Down from the first flower to "add a flower", then open it.
        press(&mut menu, &mut spec, KeyCode::Down);
        press(&mut menu, &mut spec, KeyCode::Enter);
        assert_eq!(menu.page(), Page::Kind(1));
        // Move to a different flower: the bouquet follows.
        let before = spec.flowers[1].kind;
        press(&mut menu, &mut spec, KeyCode::Down);
        assert_ne!(spec.flowers[1].kind, before);
        let picked = spec.flowers[1].kind;
        press(&mut menu, &mut spec, KeyCode::Enter);
        assert_eq!(menu.page(), Page::Main);
        assert_eq!(spec.flowers[1].kind, picked);
    }

    #[test]
    fn escape_undoes_a_picker() {
        let mut spec = BouquetSpec::default();
        let before = spec.clone();
        let mut menu = Menu::new();
        press(&mut menu, &mut spec, KeyCode::Down);
        press(&mut menu, &mut spec, KeyCode::Enter);
        press(&mut menu, &mut spec, KeyCode::Down);
        press(&mut menu, &mut spec, KeyCode::Esc);
        assert_eq!(spec, before);
        assert_eq!(menu.page(), Page::Main);
    }
}
