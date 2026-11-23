//! Big lettering for the message: a tiny pixel font, drawn two pixels to
//! a terminal row with half-block characters.

/// Each letter is five rows of pixels, `#` for lit. All capitals: at this
/// size they are the only letters that stay readable.
const FONT: &[(char, [&str; 5])] = &[
    ('a', [".#.", "#.#", "###", "#.#", "#.#"]),
    ('b', ["##.", "#.#", "##.", "#.#", "##."]),
    ('c', [".##", "#..", "#..", "#..", ".##"]),
    ('d', ["##.", "#.#", "#.#", "#.#", "##."]),
    ('e', ["###", "#..", "##.", "#..", "###"]),
    ('f', ["###", "#..", "##.", "#..", "#.."]),
    ('g', [".##", "#..", "#.#", "#.#", ".##"]),
    ('h', ["#.#", "#.#", "###", "#.#", "#.#"]),
    ('i', ["###", ".#.", ".#.", ".#.", "###"]),
    ('j', ["..#", "..#", "..#", "#.#", ".#."]),
    ('k', ["#.#", "#.#", "##.", "#.#", "#.#"]),
    ('l', ["#..", "#..", "#..", "#..", "###"]),
    ('m', ["#...#", "##.##", "#.#.#", "#...#", "#...#"]),
    ('n', ["#..#", "##.#", "#.##", "#..#", "#..#"]),
    ('o', [".#.", "#.#", "#.#", "#.#", ".#."]),
    ('p', ["##.", "#.#", "##.", "#..", "#.."]),
    ('q', [".#.", "#.#", "#.#", ".##", "..#"]),
    ('r', ["##.", "#.#", "##.", "#.#", "#.#"]),
    ('s', [".##", "#..", ".#.", "..#", "##."]),
    ('t', ["###", ".#.", ".#.", ".#.", ".#."]),
    ('u', ["#.#", "#.#", "#.#", "#.#", "###"]),
    ('v', ["#...#", "#...#", ".#.#.", ".#.#.", "..#.."]),
    ('w', ["#...#", "#...#", "#.#.#", "##.##", "#...#"]),
    ('x', ["#.#", "#.#", ".#.", "#.#", "#.#"]),
    ('y', ["#.#", "#.#", ".#.", ".#.", ".#."]),
    ('z', ["###", "..#", ".#.", "#..", "###"]),
    ('0', ["###", "#.#", "#.#", "#.#", "###"]),
    ('1', [".#.", "##.", ".#.", ".#.", "###"]),
    ('2', ["###", "..#", "###", "#..", "###"]),
    ('3', ["###", "..#", "###", "..#", "###"]),
    ('4', ["#.#", "#.#", "###", "..#", "..#"]),
    ('5', ["###", "#..", "###", "..#", "###"]),
    ('6', ["###", "#..", "###", "#.#", "###"]),
    ('7', ["###", "..#", "..#", "..#", "..#"]),
    ('8', ["###", "#.#", "###", "#.#", "###"]),
    ('9', ["###", "#.#", "###", "..#", "###"]),
    ('?', ["###", "..#", ".#.", "...", ".#."]),
    ('!', ["#", "#", "#", ".", "#"]),
    ('.', [".", ".", ".", ".", "#"]),
    (',', ["..", "..", "..", ".#", "#."]),
    ('\'', ["#", "#", ".", ".", "."]),
    ('-', ["...", "...", "###", "...", "..."]),
    (':', [".", "#", ".", "#", "."]),
    ('♥', [".#.#.", "#####", "#####", ".###.", "..#.."]),
    (' ', ["..", "..", "..", "..", ".."]),
];

/// Rows of terminal cells one line of big text takes.
pub const ROWS: usize = 3;

fn glyph(ch: char) -> Option<&'static [&'static str; 5]> {
    let ch = ch.to_ascii_lowercase();
    FONT.iter().find(|(c, _)| *c == ch).map(|(_, rows)| rows)
}

/// Whether every character of `text` has a big letter.
pub fn can_draw(text: &str) -> bool {
    text.chars().all(|c| glyph(c).is_some())
}

/// How many cells wide `text` is in big letters.
pub fn width(text: &str) -> usize {
    let cells: usize = text
        .chars()
        .filter_map(glyph)
        .map(|rows| rows[0].len() + 1)
        .sum();
    cells.saturating_sub(1)
}

/// The cells of `text` in big letters: for each lit cell, its column, its
/// row (0..[`ROWS`]), the character to draw, and which letter of the text
/// it belongs to.
pub fn cells(text: &str) -> Vec<(usize, usize, char, usize)> {
    let mut out = Vec::new();
    let mut x = 0;
    for (index, rows) in text.chars().filter_map(glyph).enumerate() {
        let lit = |col: usize, row: usize| row < 5 && rows[row].as_bytes()[col] == b'#';
        for col in 0..rows[0].len() {
            for cell_row in 0..ROWS {
                let ch = match (lit(col, cell_row * 2), lit(col, cell_row * 2 + 1)) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => continue,
                };
                out.push((x + col, cell_row, ch, index));
            }
        }
        x += rows[0].len() + 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_letter_is_five_even_rows() {
        for (ch, rows) in FONT {
            for row in rows {
                assert_eq!(row.len(), rows[0].len(), "{ch}");
                assert!(row.is_ascii(), "{ch}");
            }
        }
    }

    #[test]
    fn measures_what_it_draws() {
        let text = "be mine?";
        assert!(can_draw(text));
        let widest = cells(text).iter().map(|c| c.0).max().unwrap();
        assert_eq!(widest + 1, width(text));
    }
}
