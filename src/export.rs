//! Saving a frame: as an HTML page you can send, as ANSI text you can `cat`,
//! or as plain text.

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::octants::OCTANTS;

/// A page showing the frame. Letters are saved as text you can select;
/// a block picture is drawn by the page itself, since most browser fonts
/// are missing the block characters.
pub fn to_html(canvas: &Canvas, page_bg: Rgb) -> String {
    let painted = (0..canvas.height())
        .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
        .any(|(x, y)| canvas.get(x, y).ch != ' ' && block_pattern(canvas.get(x, y).ch).is_some());
    if painted {
        return to_html_picture(canvas, page_bg);
    }
    let mut s = String::new();
    let _ = write!(
        s,
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>for you ♥</title>\n<style>\
         body{{margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;background:{}}}\
         pre{{font:var(--fs,14px)/1.2 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;color:#ddd}}\
         </style></head><body><pre>",
        page_bg.to_hex()
    );
    for y in 0..canvas.height() {
        let mut open: Option<(Rgb, Option<Rgb>)> = None;
        for x in 0..canvas.width() {
            let c = canvas.get(x, y);
            let blank = c.ch == ' ' && c.bg.is_none();
            if blank {
                // A plain space can sit inside any span unless that span paints a background.
                if matches!(open, Some((_, Some(_)))) {
                    s.push_str("</span>");
                    open = None;
                }
            } else if open != Some((c.fg, c.bg)) {
                if open.is_some() {
                    s.push_str("</span>");
                }
                let _ = write!(s, "<span style=\"color:{}", c.fg.to_hex());
                if let Some(bg) = c.bg {
                    let _ = write!(s, ";background:{}", bg.to_hex());
                }
                s.push_str("\">");
                open = Some((c.fg, c.bg));
            }
            match c.ch {
                '<' => s.push_str("&lt;"),
                '>' => s.push_str("&gt;"),
                '&' => s.push_str("&amp;"),
                ch => s.push(ch),
            }
        }
        if open.is_some() {
            s.push_str("</span>");
        }
        s.push('\n');
    }
    s.push_str("</pre></body></html>\n");
    s
}

/// Which of a cell's eight squares (2 across, 4 down) a block character
/// fills, or `None` for ordinary text.
fn block_pattern(ch: char) -> Option<usize> {
    if ch == ' ' {
        return Some(0);
    }
    // Block characters all live far above ordinary text.
    if (ch as u32) < 0x2580 {
        return None;
    }
    OCTANTS.iter().position(|c| *c == ch)
}

/// The frame as a page that paints it square by square.
fn to_html_picture(canvas: &Canvas, page_bg: Rgb) -> String {
    let (w, h) = (canvas.width(), canvas.height());
    let number = |c: Rgb| (c.0 as u32) << 16 | (c.1 as u32) << 8 | c.2 as u32;
    // Each cell is three numbers: its pattern (or 256 + a character),
    // its colour, and its background.
    let mut cells = String::new();
    for y in 0..h {
        for x in 0..w {
            let c = canvas.get(x, y);
            let what = block_pattern(c.ch).unwrap_or(256 + c.ch as usize);
            let bg = c.bg.unwrap_or(page_bg);
            let _ = write!(cells, "{what},{},{},", number(c.fg), number(bg));
        }
    }
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>for you ♥</title>\n<style>\
         body{{margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;background:{bg}}}\
         canvas{{max-width:100vw;max-height:100vh}}\
         </style></head><body><canvas id=\"c\"></canvas><script>\n\
         const W={w},H={h},D=[{cells}];\n\
         const cw=12,ch=24,c=document.getElementById('c'),g=c.getContext('2d');\n\
         c.width=W*cw;c.height=H*ch;\n\
         const hex=n=>'#'+n.toString(16).padStart(6,'0');\n\
         g.font=(ch*0.8)+'px ui-monospace,Menlo,Consolas,monospace';g.textAlign='center';g.textBaseline='middle';\n\
         for(let i=0;i<W*H;i++){{const x=(i%W)*cw,y=Math.floor(i/W)*ch,m=D[i*3];\n\
         g.fillStyle=hex(D[i*3+2]);g.fillRect(x,y,cw,ch);g.fillStyle=hex(D[i*3+1]);\n\
         if(m<256){{for(let k=0;k<8;k++)if(m>>k&1)g.fillRect(x+(k%2)*cw/2,y+(k>>1)*ch/4,cw/2,ch/4);}}\n\
         else g.fillText(String.fromCodePoint(m-256),x+cw/2,y+ch/2);}}\n\
         </script></body></html>\n",
        bg = page_bg.to_hex(),
    )
}

/// Truecolor ANSI text: `cat` it in a terminal to see the bouquet.
pub fn to_ansi(canvas: &Canvas) -> String {
    let mut s = String::new();
    for y in 0..canvas.height() {
        let mut line = String::new();
        let mut last: Option<Rgb> = None;
        let mut back: Option<Rgb> = None;
        for x in 0..canvas.width() {
            let c = canvas.get(x, y);
            if c.bg != back {
                match c.bg {
                    Some(b) => {
                        let _ = write!(line, "\x1b[48;2;{};{};{}m", b.0, b.1, b.2);
                    }
                    None => line.push_str("\x1b[49m"),
                }
                back = c.bg;
            }
            if c.ch != ' ' && last != Some(c.fg) {
                let _ = write!(line, "\x1b[38;2;{};{};{}m", c.fg.0, c.fg.1, c.fg.2);
                last = Some(c.fg);
            }
            line.push(c.ch);
        }
        s.push_str(if back.is_some() {
            &line
        } else {
            line.trim_end()
        });
        s.push_str("\x1b[0m\n");
    }
    s
}

pub fn to_plain(canvas: &Canvas) -> String {
    let mut s = String::new();
    for y in 0..canvas.height() {
        let line: String = (0..canvas.width()).map(|x| canvas.get(x, y).ch).collect();
        s.push_str(line.trim_end());
        s.push('\n');
    }
    s
}

/// Writes one file, picking the format from the extension
/// (`.html`, `.txt`, anything else gets ANSI).
pub fn write(path: &Path, canvas: &Canvas, bg: Rgb) -> Result<()> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let text = match ext.as_str() {
        "html" | "htm" => to_html(canvas, bg),
        "txt" => to_plain(canvas),
        _ => to_ansi(canvas),
    };
    fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// Saves `posy-<timestamp>.html` and `.ans` into `dir`. Returns the HTML path.
pub fn snapshot(dir: &Path, canvas: &Canvas, bg: Rgb) -> Result<PathBuf> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let html = dir.join(format!("posy-{stamp}.html"));
    write(&html, canvas, bg)?;
    write(&dir.join(format!("posy-{stamp}.ans")), canvas, bg)?;
    Ok(html)
}
