# posy

A little 3D bouquet for your terminal. It grows in from bare stems, blooms,
and slowly spins, with hearts, petals, fireflies, snow or stars around it.
It can be painted with tiny coloured blocks, or drawn in ascii.

Inspired by [alishsharma17/Ascii-Flower-Bouquet-](https://github.com/alishsharma17/Ascii-Flower-Bouquet-)
(@alishsharma on TikTok).

## Getting it

```sh
curl -fsSL https://raw.githubusercontent.com/kyledickey/posy/main/install.sh | sh
```

That downloads the ready-made program for your computer (Mac or Linux),
checks it arrived intact, and puts it in `~/.local/bin`. Later,
`posy update` fetches the newest version, and `posy update --check` only
looks. posy also looks once a day by itself and mentions a new version
when you close it; `update_check = false` in the config turns that off.

Or build it yourself, with Rust installed: `cargo install --path .`

## Running it

```sh
posy                 # today's bouquet (new every day)
posy -b roses        # a preset
posy -b random -w vase -e snow,stars --name Sam --from me
posy list            # every bouquet you can show
posy init            # write ~/.config/posy/config.toml
```

## Using it

Everything can be done with the mouse: drag to turn the bouquet, scroll to
zoom, and click the words along the bottom. `m` (or clicking "make your
own") opens a menu for changing the flowers, colours, wrap and message
while you watch; "add a flower" there lets you pick which one.

| key                  | does                                                    |
| -------------------- | ------------------------------------------------------- |
| arrows / mouse drag  | turn and tilt                                           |
| `+` `-` `0` / scroll | zoom, reset view                                        |
| `space`              | pause the spin                                          |
| `n` / `p`            | next / previous bouquet                                 |
| `s`                  | surprise me: a brand-new random bouquet                 |
| `m`                  | make your own                                           |
| `r`                  | rearrange the flowers                                   |
| `g`                  | grow it again                                           |
| `1`–`6`              | toggle hearts, petals, fireflies, snow, stars, sparkles |
| `v`                  | change how it is drawn                                  |
| `h`                  | hide or show the buttons along the bottom               |
| `[` `]`              | spin slower / faster                                    |
| `P`                  | save this exact bouquet to `~/.config/posy/bouquets/`   |
| `x`                  | save a picture: `.html` (to send) and `.ans` (to `cat`) |
| `/`                  | whisper a word…                                         |
| `?` / `q`            | help / quit                                             |

There may also be a secret or two.

## How it is drawn

`--style` (or `style` in the config, or `v` while it runs) picks one of:

- `fine`: each character cell is painted as 2x4 coloured squares. The
  sharpest, but it needs a terminal that draws the newest block characters
  (Ghostty and Kitty do).
- `blocks`: 2x2 squares per cell. Works in any terminal with full colour.
- `ascii`: letters and symbols, coloured.

Left alone, posy picks the best one it thinks your terminal can show. If
you see little boxes with numbers in them instead of flowers, press `v`.

## Bouquet files

A bouquet is a small TOML file. Run `posy show classic` to see one, then save
your own in `~/.config/posy/bouquets/` (or use "save this bouquet" in the menu).

```toml
name = "ours"
message = "for you, {name} ♥"
wrap = "cone"               # cone, vase, jar, basket, bare
wrap_color = "cream"
ribbon_color = "red"
effects = ["hearts", "sparkles"]

[[flowers]]
kind = "rose"               # rose, tulip, daisy, sunflower, lily, cosmos,
count = 6                   # carnation, peony, lavender, babys-breath
color = "red"               # a name (see src/color.rs) or "#rrggbb"

[[flowers]]
kind = "babys-breath"
count = 3
```

### The words

The message under the bouquet can be dressed up, in the menu under "words"
or in the file:

```toml
message = "will you be my girlfriend?"
message_big = true          # big pixel letters
message_color = "pink"
message_effect = "shimmer"  # shimmer, glow, rainbow, plain
signature = "— me"
```

It can also be a question with two answers to click (or `y` / `n`):

```toml
ask = true
yes = "yes ♥"               # the two answers...
no = "no"
yes_reply = "best day ever ♥"   # ...and what the message turns into
no_reply = "that's okay ♥"
```

Saying yes sets off a burst of hearts. For a clean screen with nothing but
the bouquet and the words, hide the buttons along the bottom with `h` (or
start with `--clean`).

## Config

`posy init` writes a commented example (see `examples/config.toml`). It
covers her name, rotating messages, special dates (anniversaries,
birthdays, with `{years}` counting up), secret words, colors and speed.
Test a special date with `--date 02-14`.

## How it works

- `bouquet.rs`: the data, i.e. what you write in a TOML file.
- `flowers.rs`: every flower is a real 3D model built petal by petal. A
  petal is a curved sheet described by a few numbers (length, width, how it
  tilts from base to tip, how far it wraps around the middle), and each kind
  of flower is a recipe for arranging them. Blooming is every petal leaning
  out from upright.
- `scene/`: arranges the flowers on a dome, grows stems and leaves to them,
  and builds the wrap. Every point has a "born" time, which is how the
  bouquet grows in.
- `render.rs` and `raster.rs`: light the triangles and fill them into a
  picture 4x8 times finer than the terminal (spread over your CPU cores),
  then turn each cell of that picture into a character.
  - Blocks: the cell is cut into squares, the squares are sorted into two
    colours, and the cell gets the block character with that pattern.
    `octants.rs` lists those characters (`tools/octants.py` regenerates it).
  - Ascii: filled cells pick from a short brightness ramp; cells on an
    outline pick the character whose *shape* fits best (so an edge running
    along the bottom of a cell becomes `_`, a diagonal becomes `/`).
    `glyphs.rs` holds the measured shape of every character
    (`tools/glyphs.py` regenerates it).
- `menu.rs` and `hud.rs`: the "make your own" menu, and the message, help
  and buttons drawn over the bouquet.
- `app.rs`: the loop. Read input, update, draw, then send the terminal only
  the cells that changed.

`posy garden out.html` renders every kind of flower from three angles, which
is handy when tuning a flower. Setting `POSY_DUMP=out.ppm` also saves the
picture before it becomes characters.

## Releasing

Set the new version in `Cargo.toml`, commit, then tag it and push the tag:

```sh
git tag v0.2.0 && git push origin v0.2.0
```

The release workflow checks the tag matches `Cargo.toml`, builds for Linux
and Mac (Intel and ARM each), tries the install script against the result,
and publishes the files with their checksums. Running the workflow by hand
does everything except publish.
