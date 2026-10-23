# 0136. The interface's text

- Status: Accepted
- Date: 2026-10-23

## Context

[ADR-0132](0132-the-engines-ui-framework.md) decided how the engine's UI framework draws text:
parley shapes, orders, breaks and lays it out, with its `complex-scripts` feature, fontique finding
the fonts, fontconfig loaded at run time; swash rasterizes glyphs; text is a module of the headless
`slime-ui` until a second user asks for a crate; fonts are always named in a stack; the editor falls
back on the system's fonts, finding the system's font for a script once, then naming it; a glyph's
key holds its representation; shaped text is kept in a cache within a budget.
[ADR-0133](0133-font-files-under-the-open-font-license.md) allowed font files under the OFL, and
the tests to use small parts of Noto fonts held in the repository.

Stage 6b's second milestone, M6b.2, builds that text without a GPU. Building it raised what those
ADRs left open: where the tests' fonts come from; what the module's types are and in which units
they speak; how a text is cut with an ellipsis, which parley 0.12 does not do; where between two
pixels a glyph is drawn, and whether it is hinted; how the cache counts the memory of its texts; and
how the font that the system's fallback found comes to be named.

The owner decided on 2026-10-23 where the tests' fonts come from, among the options that
"Alternatives considered" lists, and validated the rest of the design below the same day. The
review of the milestone found that the fallback, as first built, remembered its fonts for a stack
and a language, so that the order of texts changed the font of a writing system; the owner then
decided, the same day, that the font found is remembered for a writing system, through
`icu_properties`.

## Decision

**The tests' fonts are parts of the Noto releases that the development machine holds.** Noto Sans,
Noto Sans Arabic, Hebrew, Devanagari and Thai of the release of 2026-10-01, Noto Sans CJK JP and SC
of Noto CJK 2.004, and Noto Color Emoji 2.051, as NixOS packages them, are cut down by HarfBuzz's
`hb-subset` to the characters of the tests' texts and what their layout tables reach, without
hinting, each at its regular weight but Noto Sans, which keeps its weight axis: some 700 KiB in all.
`scripts/subset-fonts.sh` makes them again from those releases, checked by their SHA-256; each has
its license beside it (`crates/slime-ui/tests/fonts/README.md`). Nothing was downloaded.

**The module's types.** `slime_ui::text` gives:

- `Fonts`: the fonts registered from their bytes, and the system's when a program asks for them
  (`Fonts::with_system_fonts`), as the editor will; a game's never do. On Linux, fontique opens
  fontconfig as the program runs, and a program without it has no system fonts; on Windows,
  DirectWrite. Without the system's fonts, a character that no font of the stack has is the first
  font's glyph 0.
- `TextStyle`: a stack of families, a size in logical pixels, a weight, italic, a line height
  relative to the fonts' own, an alignment by start and end, a direction, or that of the text's
  first letter that has one, and a language. One style per text: spans of several styles come with
  their first user.
- `TextBounds`: a width, past which lines break, and the most lines shown, past which the text is
  cut.
- `TextLayout`, which `Fonts::lay_out` gives at a scale: its lines, its glyph runs, each glyph at
  its place in pixels of the screen from the box's top left, y down; the cluster under a point, the
  position nearest it, a caret and the rectangles of a selection, in both directions. The runs are
  gathered into the engine's own types at each layout, so that parley's stay inside the crate and
  each run names its face by a `FontId` without a lookup when it is drawn.
- `GlyphRasterizer` and `GlyphKey`: a glyph's image, and what names it.
- `TextCache`: laid-out texts kept within a budget.

**A face is a `FontId`.** Laid-out text names each face it uses, one face of a font file at one
point of its variations, by an id that its `Fonts` gives the first time and keeps; a face that holds
color glyphs, bitmaps or layers (`CBDT`, `sbix`, `COLR`), is drawn in color.

**The engine cuts text with an ellipsis itself.** When a box with a width and a most lines holds
fewer lines than the text, or its last line shown is wider than the box, the text is cut at the
longest start of that line that fits with `…`: a search by halves over the line's clusters, each
candidate shaped alone in the paragraph's direction, then the text up to the cut and the ellipsis
laid out again. Shaping again matters: an Arabic letter before the cut takes its final form. The
spaces before the ellipsis are left out; the ellipsis ends the text logically, so at the left of a
text written from right to left.

**Glyphs are placed between pixels, and not hinted.** A glyph's origin is a whole pixel and one of
four positions between two, across the line; its baseline is rounded to a whole pixel, as parley
puts lines on whole pixels. Its key holds its face, its index, its size, that position and its
representation: coverage, or color. swash rasterizes without hinting: whether hinting makes small
text better on a screen of scale 1 is judged on images by the owner at M6b.3, when the renderer
draws text, with the blending. A color image's colors are not multiplied by their alpha.

**The font found for a writing system is named for it.** With the system's fonts, after each
layout, the family that a run was drawn with, which its stack did not name, is remembered for the
run's writing system and the text's language: the Unicode Script of its first letter that has one,
which `icu_properties` gives, already built for parley. From then on, the pieces of a text in that
writing system name that family after the stack's own, as spans of the style: the system is asked
once per writing system and language, whatever was laid out before, and Japanese, which fontique
asks again at each layout (172 ms in the study), costs what a font named costs. Spaces, punctuation
and marks belong to the piece of the letters before them.

**What the fonts lack.** A family that a stack names and the fonts do not have is left out, with a
warning in the log the first time; a stack of none of them draws nothing. A text cut in fonts that
lack the ellipsis and have the full stop ends in three full stops. A family without an italic face,
or without the weight asked for, is drawn in its closest face, its slant or boldness not made up.

**The cache counts an estimate of memory.** `TextCache` keeps texts by their text, style and scale:
one asked for in another box keeps its shaping, its lines broken again, and its start laid out
again with the ellipsis when that box cuts it. It lets go of the
texts used least recently past its budget, 32 MiB by default. A text's memory is an estimate,
`TextLayout::memory`, fitted to what a counting allocator measured of labels and paragraphs of seven
writing systems, not cut, within 12 % of it (`docs/measurements.md`): the allocator ran in a program
outside the repository, since the engine's code holds no `unsafe`.

**Benchmarks.** `ui.text.labels.1k`, `ui.text.paragraphs` and `ui.text.raster` measure what the
study measured, with the tests' fonts.

## Alternatives considered

Where the tests' fonts come from:

- **The same releases downloaded from the Noto projects' pages**, their SHA-256 checked: a source
  straight from upstream, but a download of some 50 MB for the same files.
- **Whole fonts**, not cut down: no tool, but some 45 MB in the repository, mostly Chinese and
  Japanese, and emoji.

The ellipsis:

- **Glyphs dropped past the cut, without shaping again**: cheaper, but the letter of a joining
  script before the cut keeps the form that joins it to the letter dropped.
- **The whole text laid out again for each candidate**: correct, but the cost of the paragraph at
  each step of the search.

Placing and rasterizing:

- **Hinting now**: sharper stems at small sizes on a screen of scale 1, but shapes that change with
  the size, and a choice made before anyone sees the text drawn; the owner deferred it to M6b.3.
- **Whole pixels only**: one image per glyph and size, but uneven spacing in a line. **More than
  four positions**: evener, for more images of each glyph.

The cache:

- **A budget in characters**: exact, but not memory, and a character of Arabic or Chinese costs
  other bytes than a Latin one.
- **Memory measured by the process's resident memory**: no estimate, but the allocator takes back
  memory let go before, and figures that differ threefold between Chinese and Japanese.

The fallback:

- **The fonts found added to the stack, for a stack and a language**, as first built: no Unicode
  data of the engine's own, but a font found for one writing system then draws every other that it
  covers, so that Greek came out in a Korean face after a Japanese text, and in another before it.
- **fontique's cache alone**: no code, but it keeps no lookup for the kana of Japanese, nor for
  Latin, Greek or Cyrillic in a language, which ask the system at each layout.
- **The memory left to the editor's milestone**, the first to use the system's fonts: less code
  now, the cost of fontique's cache meanwhile.

## Consequences

- `slime-ui` exists, holding text alone; parley, fontique and swash, and the crates that they bring,
  ICU4X's most of them, enter the build, under the licenses that ADR-0006 allows
  (`docs/dependencies.md`); `icu_properties`, one of them, is named by `slime-ui` too.
- The dev shell puts fontconfig on the loader's path, for the programs that ask for the system's
  fonts.
- The renderer's canvas and `slime-ui-render` (M6b.3) draw what `GlyphRasterizer` gives, from the
  keys of the glyphs that a layout places, into atlas pages of one channel and of four.
- Text is edited at M6b.6, on what parley's editing gives; the tree's elements lay text out through
  `TextCache` from M6b.4.
