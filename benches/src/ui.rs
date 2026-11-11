//! The interface's text (`docs/adr/0136-the-interfaces-text.md`): labels and paragraphs laid out in
//! several writing systems, lines broken again in another width, and glyphs rasterized, with the
//! fonts of uwui's tests.

use std::hint::black_box;
use std::time::Duration;

use slime_color::LinearColor;
use uwui::display::DisplayList;
use uwui::display::ImageKey;
use uwui::look::{self, Colors, Density};
use uwui::preferences::{Preferences, TextSize};
use uwui::style::{Dimension, Display, FlexDirection, Justify, StyleBuilder, Track};
use uwui::text::{Fonts, GlyphKey, GlyphRasterizer, TextBounds, TextStyle};
use uwui::theme::{self, Fallback, Length, StyleSheet, Theme, Token, Tokens};
use uwui::tree::{Element, ElementId, FrameStats, LayoutDirection, RootSettings, Ui};

use crate::allocations::counted;
use crate::measure::{Context, Row, ms, percentile, timed};

/// The size of the text, in pixels, at scale 1.
const SIZE: f32 = 14.0;
/// The width that paragraphs are laid out at.
const WIDTH: f32 = 400.0;
/// How many characters a paragraph holds, at least.
const PARAGRAPH: usize = 2_000;
/// How many times each figure is taken.
pub(crate) const RUNS: usize = 15;

/// A paragraph of the benchmarks: its name, its stack, its language, and the separator of its
/// sentences.
struct Script {
    name: &'static str,
    fonts: [&'static str; 2],
    locale: &'static str,
    separator: &'static str,
}

const SCRIPTS: &[Script] = &[
    Script {
        name: "english",
        fonts: ["Noto Sans", "Noto Sans"],
        locale: "en",
        separator: " ",
    },
    Script {
        name: "arabic",
        fonts: ["Noto Sans Arabic", "Noto Sans"],
        locale: "ar",
        separator: " ",
    },
    Script {
        name: "hindi",
        fonts: ["Noto Sans Devanagari", "Noto Sans"],
        locale: "hi",
        separator: " ",
    },
    Script {
        name: "thai",
        fonts: ["Noto Sans Thai", "Noto Sans"],
        locale: "th",
        separator: " ",
    },
    Script {
        name: "japanese",
        fonts: ["Noto Sans CJK JP", "Noto Sans"],
        locale: "ja",
        separator: "",
    },
    Script {
        name: "chinese",
        fonts: ["Noto Sans CJK SC", "Noto Sans"],
        locale: "zh-CN",
        separator: "",
    },
    Script {
        name: "emoji",
        fonts: ["Noto Sans", "Noto Color Emoji"],
        locale: "en",
        separator: " ",
    },
];

/// The fonts of uwui's tests.
fn fonts() -> Result<Fonts, String> {
    let directory = Context::root().join("crates/uwui/tests/fonts");
    let mut fonts = Fonts::new();
    for file in [
        "NotoSans.ttf",
        "NotoSansArabic.ttf",
        "NotoSansDevanagari.ttf",
        "NotoSansThai.ttf",
        "NotoSansCJKjp.otf",
        "NotoSansCJKsc.otf",
        "NotoColorEmoji.ttf",
    ] {
        let path = directory.join(file);
        let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        fonts
            .register(bytes)
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(fonts)
}

fn style(script: &Script) -> TextStyle {
    TextStyle {
        fonts: script.fonts.iter().map(|&font| font.to_owned()).collect(),
        size: SIZE,
        locale: Some(script.locale.to_owned()),
        ..TextStyle::default()
    }
}

/// The sentences of uwui's test text `name`, repeated until they hold [`PARAGRAPH`]
/// characters: repeated words help any cache keyed by word, which parley has not.
fn paragraph(script: &Script) -> Result<String, String> {
    let path = Context::root().join(format!("crates/uwui/tests/texts/{}.txt", script.name));
    let text =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let sentences: Vec<&str> = text.lines().collect();
    let mut paragraph = String::new();
    for sentence in sentences.iter().cycle() {
        if paragraph.chars().count() >= PARAGRAPH {
            break;
        }
        if !paragraph.is_empty() {
            paragraph.push_str(script.separator);
        }
        paragraph.push_str(sentence);
    }
    Ok(paragraph)
}

/// `count` labels of 12 to 30 letters, names of entities and components, the same at each run.
fn labels(count: usize) -> Vec<String> {
    const WORDS: &[&str] = &[
        "Transform",
        "Mesh",
        "Renderer",
        "Point",
        "Light",
        "Camera",
        "Collider",
        "Rigid",
        "Body",
        "Audio",
        "Source",
        "Spawn",
        "Marker",
        "Door",
        "Lantern",
        "Tree",
        "Rock",
        "Path",
        "Water",
        "Sky",
        "Shadow",
        "Cascade",
        "Trigger",
        "Volume",
        "Player",
        "Enemy",
        "Weapon",
        "Shield",
        "Torch",
        "Bridge",
        "Gate",
        "Tower",
        "Wall",
        "Floor",
        "Roof",
        "Window",
        "Banner",
        "Crate",
    ];
    // A xorshift generator: the same labels at each run.
    let mut state: u64 = 7;
    let mut next = |below: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % below as u64) as usize
    };
    (0..count)
        .map(|_| {
            let target = 12 + next(19);
            let mut label = String::new();
            while label.len() < target {
                if !label.is_empty() {
                    label.push(' ');
                }
                label.push_str(WORDS[next(WORDS.len())]);
            }
            label.truncate(30);
            label.trim_end().to_owned()
        })
        .collect()
}

/// The times of `runs` runs of `run`, after two that are not timed.
pub(crate) fn times<T>(runs: usize, mut run: impl FnMut() -> T) -> Vec<Duration> {
    black_box(run());
    black_box(run());
    (0..runs).map(|_| timed(|| black_box(run())).1).collect()
}

/// The counters of the frame of an interface standing still that `frame` runs, after frames that
/// left nothing to do: its elements, those that it visited, the items of its display list, and the
/// allocations of the calling thread and the bytes that they took
/// (`docs/adr/0186-counters-and-ratios-in-ci.md`).
pub(crate) fn still_counters(frame: impl FnOnce() -> (DisplayList, FrameStats)) -> Vec<Row> {
    let ((list, stats), allocations) = counted(frame);
    vec![
        Row::count("elements", stats.elements),
        Row::count("elements visited by a frame standing still", stats.visited),
        Row::count("items of a frame standing still", list.len()),
        Row::count("allocations of a frame standing still", allocations.calls),
        Row::count(
            "bytes allocated by a frame standing still",
            allocations.taken,
        ),
    ]
}

/// 1 000 labels laid out, each on a line of its own.
pub fn labels_1k(_: &mut Context) -> Result<Vec<Row>, String> {
    let mut fonts = fonts()?;
    let labels = labels(1_000);
    let style = style(&SCRIPTS[0]);
    let times = times(RUNS, || {
        for label in &labels {
            black_box(fonts.lay_out(label, &style, 1.0, TextBounds::default()));
        }
    });
    Ok(Row::times("1 000 labels laid out", &times).into())
}

/// A paragraph of each writing system laid out at [`WIDTH`]; then the English one broken again in
/// another width.
pub fn paragraphs(_: &mut Context) -> Result<Vec<Row>, String> {
    let mut fonts = fonts()?;
    let mut rows = Vec::new();
    for script in SCRIPTS {
        let text = paragraph(script)?;
        let style = style(script);
        let times = times(RUNS, || {
            black_box(fonts.lay_out(&text, &style, 1.0, TextBounds::width(WIDTH)));
        });
        rows.push(Row::new(
            format!(
                "a paragraph of {} characters of {} laid out, median of {}",
                text.chars().count(),
                script.name,
                times.len()
            ),
            ms(percentile(&times, 0.5)),
        ));
    }
    let text = paragraph(&SCRIPTS[0])?;
    let mut layout = fonts.lay_out(&text, &style(&SCRIPTS[0]), 1.0, TextBounds::width(WIDTH));
    let mut narrow = false;
    let times = times(RUNS, || {
        narrow = !narrow;
        let width = if narrow { WIDTH - 20.0 } else { WIDTH };
        layout.set_bounds(&mut fonts, TextBounds::width(width));
    });
    rows.push(Row::new(
        format!(
            "the English paragraph's lines broken again in another width, median of {}",
            times.len()
        ),
        ms(percentile(&times, 0.5)),
    ));
    Ok(rows)
}

/// Glyphs rasterized at 14, 32 and 64 pixels: Latin letters, Chinese characters and emoji.
pub fn raster(_: &mut Context) -> Result<Vec<Row>, String> {
    let mut fonts = fonts()?;
    let sets = [
        (
            "a Latin letter",
            &SCRIPTS[0],
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
        ),
        (
            "a Chinese character",
            &SCRIPTS[5],
            "编辑器在左侧显示实体的层级结构右检查中间世界视图文书写系统使用汉字每个代表一音节句子",
        ),
        ("an emoji", &SCRIPTS[6], "💻💙💚🍣🍕🌮🐱🐶🦊💬✅🟢"),
    ];
    let mut rows = Vec::new();
    for (what, script, text) in sets {
        for size in [14.0, 32.0, 64.0] {
            let style = TextStyle {
                size,
                ..style(script)
            };
            let layout = fonts.lay_out(text, &style, 1.0, TextBounds::default());
            let keys: Vec<GlyphKey> = layout
                .runs()
                .iter()
                .flat_map(|run| run.placed().map(|glyph| glyph.key))
                .collect();
            let mut rasterizer = GlyphRasterizer::new();
            let times = times(RUNS, || {
                for &key in &keys {
                    black_box(rasterizer.rasterize(&fonts, key));
                }
            });
            let each = percentile(&times, 0.5) / u32::try_from(keys.len()).unwrap_or(1);
            rows.push(Row::new(
                format!(
                    "{what} rasterized at {size} pixels, median of {}",
                    times.len()
                ),
                ms(each),
            ));
        }
    }
    Ok(rows)
}

/// A game's menu drawn from a display list through `uwui-render`
/// (`docs/adr/0139-text-drawn-and-the-display-list.md`), at the tiers' sizes: the panels, slots, rows
/// and buttons of `render.canvas`'s menu, without its icons, laid out for 1920 × 1080 and scaled to
/// fit, and real texts, laid out with the fonts of uwui's tests at the screen's scale: a
/// description of some 1 500 characters, 34 rows of statistics, 30 buttons' labels and 100 counts,
/// some 2 400 glyphs. The first draw, which rasterizes its glyphs into the atlas, the renderer
/// warmed before it; then the CPU of `UiRenderer::draw` once they are there, and the canvas's GPU
/// time, which ADR-0137 bounds.
pub fn menu(context: &mut Context) -> Result<Vec<Row>, String> {
    use std::sync::Arc;

    use slime_render::Renderer;
    use uwui::Rect;
    use uwui::display::{BoxPrimitive, DisplayList, ShadowPrimitive, TextPrimitive};
    use uwui::text::{TEXT_CACHE_BUDGET, TextCache};
    use uwui_render::{GLYPH_ATLAS_BUDGET, UiRenderer};

    use crate::render::{CANVAS_FRAMES, FORMAT, TIERS, canvas_frames};

    let runs = context.runs(CANVAS_FRAMES);
    let gpu = &context.gpu;
    let error = |error: slime_render::RenderError| error.to_string();
    let mut fonts = fonts()?;
    let path = Context::root().join("crates/uwui/tests/texts/english.txt");
    let english =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let words: Vec<&str> = english.split_whitespace().collect();
    let description = english
        .lines()
        .cycle()
        .take(14)
        .collect::<Vec<_>>()
        .join(" ");
    let mut renderer = Renderer::new(gpu, FORMAT).map_err(error)?;
    let mut rows = Vec::new();
    for (width, height) in TIERS {
        let target = gpu
            .create_render_target("benchmark frame", width, height, FORMAT)
            .map_err(|error| error.to_string())?;
        let (w, h) = (width as f32, height as f32);
        let scale = (w / 1920.0).min(h / 1080.0);
        let (left, top) = ((w - 1920.0 * scale) / 2.0, (h - 1080.0 * scale) / 2.0);
        let rect = |x: f32, y: f32, width: f32, height: f32| {
            Rect::new(
                left + x * scale,
                top + y * scale,
                width * scale,
                height * scale,
            )
        };
        let mut texts = TextCache::new(TEXT_CACHE_BUDGET);
        let mut lay_out = |text: &str, size: f32, bounds: TextBounds| {
            let style = TextStyle {
                fonts: vec!["Noto Sans".to_owned()],
                size,
                ..TextStyle::default()
            };
            Arc::clone(texts.get(&mut fonts, text, &style, scale, bounds))
        };
        let mut list = DisplayList::new();
        let text = |list: &mut DisplayList, layout, (x, y): (f32, f32), clip| {
            list.push(
                TextPrimitive {
                    origin: [left + x * scale, top + y * scale],
                    layout,
                    color: LinearColor::rgb(0.9, 0.88, 0.8),
                },
                clip,
            );
        };
        let plain = |rect: Rect, fill: LinearColor| BoxPrimitive {
            rect,
            radii: [6.0 * scale; 4],
            fill,
            border_width: 0.0,
            border: LinearColor::BLACK,
        };
        list.push(
            plain(
                Rect::new(0.0, 0.0, w, h),
                LinearColor::rgba(0.0, 0.0, 0.0, 0.6),
            ),
            None,
        );
        let panels = [
            (60.0, 80.0, 820.0, 860.0),
            (920.0, 80.0, 500.0, 520.0),
            (920.0, 640.0, 500.0, 300.0),
            (1460.0, 80.0, 400.0, 860.0),
        ];
        let cards = (0..16).map(|index| (60.0 + index as f32 * 112.0, 960.0, 100.0, 100.0));
        for (x, y, width, height) in panels.into_iter().chain(cards) {
            list.push(
                ShadowPrimitive {
                    rect: rect(x, y, width, height),
                    radii: [12.0 * scale; 4],
                    color: LinearColor::rgba(0.0, 0.0, 0.0, 0.5),
                    offset: [0.0, 8.0 * scale],
                    blur: 24.0 * scale,
                    spread: 0.0,
                },
                None,
            );
            list.push(
                BoxPrimitive {
                    border_width: 2.0 * scale,
                    border: LinearColor::rgb(0.5, 0.42, 0.3),
                    ..plain(
                        rect(x, y, width, height),
                        LinearColor::rgba(0.08, 0.08, 0.1, 0.92),
                    )
                },
                None,
            );
        }
        // The inventory's slots, and their counts.
        for index in 0..100 {
            let (x, y) = (
                80.0 + (index % 10) as f32 * 76.0,
                120.0 + (index / 10) as f32 * 76.0,
            );
            list.push(
                plain(rect(x, y, 68.0, 68.0), LinearColor::rgb(0.05, 0.05, 0.06)),
                None,
            );
            let count = lay_out(&format!("{}", 10 + index % 90), 12.0, TextBounds::default());
            text(&mut list, count, (x + 46.0, y + 50.0), None);
        }
        // The statistics, a list cut at its panel's bottom.
        let stats = Some(rect(1460.0, 120.0, 400.0, 800.0));
        for row in 0..34 {
            let y = 130.0 + row as f32 * 23.0;
            list.push(
                plain(
                    rect(1480.0, y, 360.0, 21.0),
                    LinearColor::rgb(0.1, 0.1, 0.1),
                ),
                stats,
            );
            let line = format!(
                "{} {} {}: 3.14159, 1,024",
                words[row % words.len()],
                words[(row * 7 + 3) % words.len()],
                words[(row * 13 + 5) % words.len()]
            );
            text(
                &mut list,
                lay_out(&line, 14.0, TextBounds::line(352.0)),
                (1484.0, y + 2.0),
                stats,
            );
        }
        // The buttons, and the description, longer than its panel.
        for index in 0..30 {
            let (x, y) = (
                936.0 + (index % 3) as f32 * 160.0,
                656.0 + (index / 3) as f32 * 28.0,
            );
            list.push(
                plain(rect(x, y, 156.0, 24.0), LinearColor::rgb(0.18, 0.15, 0.12)),
                None,
            );
            let name = lay_out(
                words[(index * 5) % words.len()],
                13.0,
                TextBounds::line(140.0),
            );
            text(&mut list, name, (x + 8.0, y + 4.0), None);
        }
        let paragraph = lay_out(&description, 14.0, TextBounds::width(460.0));
        text(
            &mut list,
            paragraph,
            (940.0, 96.0),
            Some(rect(920.0, 80.0, 500.0, 520.0)),
        );

        let at = format!("at {width} × {height}");
        // The renderer warmed by another bridge, so that the first draw measures the atlas.
        UiRenderer::new(GLYPH_ATLAS_BUDGET)
            .draw(gpu, &mut renderer, &fonts, &list, |_| None, &target)
            .map_err(error)?;
        gpu.wait_idle().map_err(|error| error.to_string())?;
        let mut ui = UiRenderer::new(GLYPH_ATLAS_BUDGET);
        let (first, cold) = timed(|| ui.draw(gpu, &mut renderer, &fonts, &list, |_| None, &target));
        let first = first.map_err(error)?;
        gpu.wait_idle().map_err(|error| error.to_string())?;
        let (cpu, gpu_times, last) = canvas_frames(
            gpu,
            runs,
            || ui.draw(gpu, &mut renderer, &fonts, &list, |_| None, &target),
            |stats| stats.canvas.gpu_time,
        )?;
        rows.push(Row::new(
            format!("the first draw {at}, its glyphs rasterized into the atlas"),
            ms(cold),
        ));
        rows.push(Row::count(
            format!("glyphs rasterized by the first draw {at}"),
            first.rasterized,
        ));
        rows.extend(Row::times(&format!("CPU of `UiRenderer::draw` {at}"), &cpu));
        if !gpu_times.is_empty() {
            rows.push(Row::new(
                format!(
                    "canvas on the GPU {at}, median of what {} frames reported",
                    gpu_times.len()
                ),
                ms(percentile(&gpu_times, 0.5)),
            ));
        }
        rows.push(Row::count(format!("glyphs drawn {at}"), last.glyphs));
        rows.push(Row::count(format!("items {at}"), last.canvas.items));
        rows.push(Row::count(format!("draws {at}"), last.canvas.draws));
        rows.push(Row::count(
            format!("bytes of the atlas {at}"),
            last.atlas_bytes,
        ));
    }
    Ok(rows)
}

/// The Latin font of uwui's tests alone.
fn latin() -> Result<Fonts, String> {
    let path = Context::root().join("crates/uwui/tests/fonts/NotoSans.ttf");
    let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut fonts = Fonts::new();
    fonts
        .register(bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(fonts)
}

/// Writes the style of a text of the tree's benchmarks: its color left to its kind's, so that the
/// rules that give one win (`docs/adr/0230-the-cores-roles-and-each-kinds-defaults.md`).
fn look(s: StyleBuilder) -> StyleBuilder {
    let text = style(&SCRIPTS[0]);
    s.fonts(text.fonts).locale(text.locale)
}

/// Writes that an element lays its children out in a column.
fn column(s: StyleBuilder) -> StyleBuilder {
    s.direction(FlexDirection::Column)
}

/// Writes a size of `width` by `height` logical pixels.
fn sized(s: StyleBuilder, width: f32, height: f32) -> StyleBuilder {
    s.width(Dimension::Pixels(width))
        .height(Dimension::Pixels(height))
}

/// A space of the look's tree, which its own styles name in place of a number.
const TREE_SPACE: Token<Length> = Token::new("tree_space", Fallback::Value(4.0));
/// The look's tree's own tokens.
const TREE_TOKENS: Tokens = Tokens::new(&[&TREE_SPACE]);

/// A root of 1920 × 1080 logical pixels at scale 1.
const SCREEN: RootSettings = RootSettings {
    size: [1920.0, 1080.0],
    scale: 1.0,
    direction: LayoutDirection::LeftToRight,
    origin: [0.0; 2],
    modal: false,
};

/// The editor's style sheet: its labels, buttons and panels, the inspector's names apart.
const EDITOR_STYLE: &str = r#"
[label]
select = { kind = "Label" }
style.text.color = "$text"

[button]
select = { kind = "Button" }
style.paint = { background = "$button", border_width = 1, border = "$border", radii = [4, 4, 4, 4] }

[panel]
select = { kind = "Scroll" }
style.paint = { background = "$panel", border_width = 1, border = "$border" }

[name]
select = { kind = "Label", inside = { kind = "Container", class = "fields" } }
style.text.color = "$muted"
"#;

/// A dark or a light theme of the editor's style sheet, which gives the core's roles too
/// (`docs/adr/0230-the-cores-roles-and-each-kinds-defaults.md`).
fn theme(light: bool) -> Theme {
    let (text, muted, button, border, panel) = if light {
        ("#212529", "#495057", "#f8f9fa", "#ced4da", "#ffffff")
    } else {
        ("#e3e5e8", "#9aa0a6", "#3a3d43", "#4e525a", "#26282c")
    };
    Theme::from_text(
        "editor.theme.kdl",
        format!(
            "color {{\n    text \"{text}\"\n    muted \"{muted}\"\n    button \"{button}\"\n\
             \x20   border \"{border}\"\n    panel \"{panel}\"\n    canvas_text $text\n\
             \x20   gray_text $muted\n    button_face $button\n    accent \"#4c6ef5\"\n\
             \x20   highlight $accent\n    highlight_text $text\n    focus $accent\n}}\n"
        ),
    )
}

/// An editor's window: a toolbar, a hierarchy, a viewport, an inspector and a browser of assets,
/// each panel a slot built by its own revision: for each of its labels, a row of the hierarchy
/// and of the browser, and a field of the inspector, a name and a value, in a component of ten.
struct Editor {
    ui: Ui,
    labels: Vec<String>,
    /// Writes the own style of its labels and buttons: the tree's text, or none under the look.
    text: fn(StyleBuilder) -> StyleBuilder,
    panels: [ElementId; 4],
    /// The value that the inspector shows in its first field, which a change moves.
    value: u64,
}

impl Editor {
    /// The tree, its labels and buttons in the tree's text, styled by the editor's four rules.
    fn new(fonts: Fonts, labels: Vec<String>) -> Self {
        let sheets = [StyleSheet::from_text("editor.style", EDITOR_STYLE)];
        Self::styled(fonts, labels, &sheets, &[theme(false)], look)
    }

    /// The tree styled by `sheets` and `themes`, its labels and buttons given `text` as their own
    /// style.
    fn styled(
        fonts: Fonts,
        labels: Vec<String>,
        sheets: &[StyleSheet],
        themes: &[Theme],
        text: fn(StyleBuilder) -> StyleBuilder,
    ) -> Self {
        Self::styled_with(fonts, labels, &[], sheets, themes, text)
    }

    /// The same, its interface declaring the lists of `tokens` first.
    fn styled_with(
        fonts: Fonts,
        labels: Vec<String>,
        tokens: &[Tokens],
        sheets: &[StyleSheet],
        themes: &[Theme],
        text: fn(StyleBuilder) -> StyleBuilder,
    ) -> Self {
        let mut ui = Ui::new(fonts);
        for list in tokens {
            ui.declare_tokens(list).expect("names of the tree's own");
        }
        let root = ui.add_root("main", SCREEN, column);
        ui.set_style(sheets, themes).expect("the tree's style");
        let mut panels = Vec::new();
        ui.build(root, 0, |children| {
            let toolbar = Element::container().style(|s| s.column_gap(4.0));
            panels.push(children.add_slot("toolbar", toolbar));
            let main =
                Element::container().style(|s| s.grow(1.0).min_height(Dimension::Pixels(0.0)));
            children.add("main", main, |children| {
                let panel = |width: f32| {
                    Element::scroll()
                        .style(|s| column(s).width(Dimension::Pixels(width)).padding(4.0))
                };
                panels.push(children.add_slot("hierarchy", panel(300.0)));
                let viewport = Element::container().style(|s| s.grow(1.0));
                children.add("viewport", viewport, |_| {});
                panels.push(children.add_slot("inspector", panel(400.0)));
                panels.push(children.add_slot("assets", panel(300.0)));
            });
        });
        let mut editor = Self {
            ui,
            labels,
            text,
            panels: [panels[0], panels[1], panels[2], panels[3]],
            value: 0,
        };
        editor.build(0);
        editor
    }

    /// Builds the panels whose revision moved: all of them at first, then the inspector alone,
    /// which reads `revision`.
    fn build(&mut self, revision: u64) {
        let [toolbar, hierarchy, inspector, assets] = self.panels;
        let labels = &self.labels;
        let text = self.text;
        let button = |label: String| Element::button(label).style(text);
        self.ui.build(toolbar, 0, |children| {
            for index in 0..30_usize {
                children.add(index, button(format!("Tool {index}")), |_| {});
            }
        });
        self.ui.build(hierarchy, 0, |children| {
            for (index, label) in labels.iter().enumerate() {
                let row = Element::container()
                    .style(|s| s.padding_start((index % 4) as f32 * 12.0).column_gap(4.0));
                children.add(index, row, |children| {
                    children.add("name", Element::label(label.as_str()).style(text), |_| {});
                    children.add("shown", button("o".to_owned()), |_| {});
                });
            }
        });
        let value = self.value;
        // A component's fields in a grid of names and values, ten to a component.
        self.ui.build(inspector, revision, |children| {
            let grid = |s: StyleBuilder| {
                s.display(Display::Grid)
                    .columns(vec![Track::Pixels(160.0), Track::Fraction(1.0)])
                    .row_gap(2.0)
                    .column_gap(8.0)
            };
            for (component, fields) in labels.chunks(10).enumerate() {
                children.add(component, Element::container().style(column), |children| {
                    let header = Element::label(fields[0].as_str()).style(text);
                    children.add("header", header, |_| {});
                    children.add(
                        "fields",
                        Element::container().style(grid).class("fields"),
                        |children| {
                            for (index, label) in fields.iter().enumerate() {
                                let name = Element::label(label.as_str()).style(text);
                                children.add(format!("{index} name"), name, |_| {});
                                let first = component == 0 && index == 0;
                                let shown = if first { value } else { index as u64 };
                                let value = Element::label(format!("{shown}.25")).style(text);
                                children.add(format!("{index} value"), value, |_| {});
                            }
                        },
                    );
                });
            }
        });
        self.ui.build(assets, 0, |children| {
            for (index, label) in labels.iter().enumerate() {
                let row = Element::container().style(|s| s.column_gap(4.0));
                children.add(index, row, |children| {
                    let icon =
                        Element::image(ImageKey(index as u64 % 16)).style(|s| sized(s, 16.0, 16.0));
                    children.add("icon", icon, |_| {});
                    children.add("name", Element::label(label.as_str()).style(text), |_| {});
                });
            }
        });
    }
}

/// A tree of the editor's size, some 12 500 elements: its first frame, which lays out its texts;
/// a frame after one value of the inspector changed; a frame with nothing changed.
pub fn tree(context: &mut Context) -> Result<Vec<Row>, String> {
    let runs = context.runs(RUNS);
    let labels = labels(1_500);
    let mut firsts = Vec::new();
    for _ in 0..runs {
        let (fonts, labels) = (latin()?, labels.clone());
        let (mut editor, built) = timed(|| Editor::new(fonts, labels));
        let (_, framed) = timed(|| black_box(editor.ui.frame()));
        firsts.push(built + framed);
    }
    let mut editor = Editor::new(latin()?, labels);
    let (list, _) = editor.ui.frame();
    // Counted before the series, whose length a run of `--counters` cuts.
    let counters = still_counters(|| {
        editor.build(0);
        editor.ui.frame()
    });
    // The frame after one value of the inspector changed, which builds the inspector again.
    let (_, rebuilt) = counted(|| {
        editor.value = 1;
        editor.build(1);
        editor.ui.frame()
    });
    let mut revision = 1;
    let changed = times(runs, || {
        revision += 1;
        editor.value = revision;
        editor.build(revision);
        editor.ui.frame()
    });
    let still = times(runs, || {
        editor.build(revision);
        editor.ui.frame()
    });
    let mut light = false;
    let restyled = times(runs, || {
        light = !light;
        editor
            .ui
            .set_style(
                &[StyleSheet::from_text("editor.style", EDITOR_STYLE)],
                &[theme(light)],
            )
            .expect("the editor's style");
        editor.ui.frame()
    });
    let mut rows = counters;
    rows.push(Row::count("items of the first frame", list.len()));
    rows.push(Row::count(
        "allocations of a frame after one value of the inspector changed",
        rebuilt.calls,
    ));
    rows.push(Row::count(
        "bytes allocated by a frame after one value of the inspector changed",
        rebuilt.taken,
    ));
    rows.extend(Row::times(
        "the first frame: built, its texts and its tree laid out, drawn",
        &firsts,
    ));
    rows.extend(Row::times(
        "a frame after one value of the inspector changed",
        &changed,
    ));
    rows.extend(Row::times("a frame with nothing changed", &still));
    rows.extend(Row::times(
        "a frame after the theme changed, every element styled again",
        &restyled,
    ));
    Ok(rows)
}

/// The time that a frame of 240 Hz gives the interface.
const FRAME_240: Duration = Duration::from_nanos(4_166_667);

/// The most allocations that the frame after a change of the look's colours makes for each element
/// that it styles again, in a debug build
/// (`docs/adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md`).
const ALLOCATIONS_PER_ELEMENT_STYLED: f64 = 20.0;

/// The most allocations that the first frame of the transition makes for each element that it draws
/// again, in a debug build: the frames after it make within a few allocations of it
/// (`docs/adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md`).
const ALLOCATIONS_PER_ELEMENT_DRAWN: f64 = 19.0;

/// The size of text that the frame after a change of it lays every text out at: twice the look's,
/// WCAG's 200 % (`docs/adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md`).
const LARGER_TEXT: f32 = 2.0;

/// The most frames that a transition is given to end, past which the benchmark fails.
const TRANSITION_FRAMES: usize = 1_000;

/// The tree of [`tree`] styled by the editor's look, its labels and buttons with no style of their
/// own: a frame after its theme of colours changed between dark and light, which styles every
/// element again under the look's rules and starts their transitions, then the frames of 240 Hz
/// that the transitions take to end. Counted: the allocations of reading the look's documents
/// apart, the elements that the frame of the change styled, its allocations and their bytes, the
/// frames of the transition, and the elements that its first frame draws again with its
/// allocations and their bytes; and each frame's allocations for each element, which a target
/// bounds. Then the frame after the user's size of text changed to twice the look's, which lays
/// every text out again, as the tree's first frame does: it allocates no more for each element
/// than the same tree's first frame at that size, its target
/// (`docs/adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md`). And the
/// same tree whose labels' and buttons' own styles name a role and a token of the tree's own: the
/// frame after the change of its colours, which reads their values again, within the same target
/// for each element styled as the frame of the change
/// (`docs/adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md`).
pub fn look_tree(context: &mut Context) -> Result<Vec<Row>, String> {
    let runs = context.runs(RUNS);
    let fonts = || {
        let mut fonts = Fonts::new();
        look::register_fonts(&mut fonts);
        fonts
    };
    let sheets = [look::style_sheet()];
    let themes = |colors| look::themes(colors, Density::Comfortable);
    // The same tree's first frame at twice the look's size of text, which lays every text out at
    // it, by which the frame of a change of the size of text is bounded.
    let mut larger_first = Editor::styled(
        fonts(),
        labels(1_500),
        &sheets,
        &themes(Colors::Dark),
        |s| s,
    );
    larger_first.ui.set_preferences(Preferences {
        text_size: TextSize::factor(LARGER_TEXT).expect("within its range"),
        ..Preferences::default()
    });
    let ((_, built), first) = counted(|| larger_first.ui.frame());
    drop(larger_first);
    // The same tree, its labels' and buttons' own styles naming a role and a token of the tree's
    // own, which its theme gives by the look's name: the frame of a change of the look's colours
    // reads their values again (`docs/adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md`).
    let mut named_themes = themes(Colors::Dark);
    named_themes.push(Theme::from_text(
        "tree.theme.kdl",
        "length {\n    tree_space $space_s\n}\n",
    ));
    let mut named = Editor::styled_with(
        fonts(),
        labels(1_500),
        &[TREE_TOKENS],
        &sheets,
        &named_themes,
        |s| s.text_color(theme::CANVAS_TEXT).padding_start(TREE_SPACE),
    );
    named.ui.frame();
    let mut light_themes = themes(Colors::Light);
    light_themes.push(Theme::from_text(
        "tree.theme.kdl",
        "length {\n    tree_space $space_s\n}\n",
    ));
    named
        .ui
        .set_style(&sheets, &light_themes)
        .expect("the look's documents are valid");
    named.ui.advance(FRAME_240);
    let ((_, named_stats), named_change) = counted(|| named.ui.frame());
    drop(named);
    let mut editor = Editor::styled(
        fonts(),
        labels(1_500),
        &sheets,
        &themes(Colors::Dark),
        |s| s,
    );
    editor.ui.frame();
    let mut light = false;
    // Gives the interface its theme of the other colours, which reads the look's documents.
    let mut restyle = |editor: &mut Editor| {
        light = !light;
        let colors = if light { Colors::Light } else { Colors::Dark };
        editor
            .ui
            .set_style(&sheets, &themes(colors))
            .expect("the look's documents are valid");
    };
    // The frame of the change, which styles every element again.
    let change = |editor: &mut Editor| {
        editor.ui.advance(FRAME_240);
        editor.ui.frame()
    };
    // The frames of 240 Hz until no transition goes on, and their time together.
    let settle = |editor: &mut Editor| -> Result<(usize, Duration), String> {
        let mut spent = Duration::ZERO;
        for frames in 0..TRANSITION_FRAMES {
            if !editor.ui.animating() {
                return Ok((frames, spent));
            }
            editor.ui.advance(FRAME_240);
            let (_, took) = timed(|| black_box(editor.ui.frame()));
            spent += took;
        }
        Err(format!(
            "the look's transitions go on past {TRANSITION_FRAMES} frames"
        ))
    };
    // Gives the interface the user's size of text, twice the look's or the look's own.
    let mut larger = false;
    let mut resize = |editor: &mut Editor| {
        larger = !larger;
        let factor = if larger { LARGER_TEXT } else { 1.0 };
        editor.ui.set_preferences(Preferences {
            text_size: TextSize::factor(factor).expect("within its range"),
            ..Preferences::default()
        });
    };
    settle(&mut editor)?;
    // Counted before the series, whose length a run of `--counters` cuts.
    resize(&mut editor);
    let ((_, resized), text) = counted(|| editor.ui.frame());
    resize(&mut editor);
    editor.ui.frame();
    let ((), read) = counted(|| restyle(&mut editor));
    let ((_, stats), allocations) = counted(|| change(&mut editor));
    // The first frame of the transition, which draws again every element that moves.
    editor.ui.advance(FRAME_240);
    let ((_, moved), moving) = counted(|| editor.ui.frame());
    let (frames, _) = settle(&mut editor)?;
    let frames = frames + 1;
    let mut changed = Vec::new();
    let mut transitions = Vec::new();
    let mut resizes = Vec::new();
    let mut readings = Vec::new();
    for _ in 0..runs {
        let ((), read) = timed(|| restyle(&mut editor));
        readings.push(read);
        let (_, took) = timed(|| black_box(change(&mut editor)));
        changed.push(took);
        transitions.push(settle(&mut editor)?.1);
    }
    // To twice the look's size and back, each a change of the size of text.
    for _ in 0..runs {
        resize(&mut editor);
        let (_, took) = timed(|| black_box(editor.ui.frame()));
        resizes.push(took);
    }
    let first_per_element = first.calls as f64 / built.styled.max(1) as f64;
    let mut rows = vec![
        Row::count(
            "elements styled by the frame of the change, own styles naming tokens",
            named_stats.styled,
        ),
        Row::count(
            "allocations of the frame of the change, own styles naming tokens",
            named_change.calls,
        ),
        Row::per(
            "allocations of the frame of the change for each element styled, own styles naming \
             tokens",
            named_change.calls,
            named_stats.styled as u64,
            ALLOCATIONS_PER_ELEMENT_STYLED,
        ),
        Row::count("allocations of reading the look's documents", read.calls),
        Row::count("elements styled by the frame of the change", stats.styled),
        Row::count("allocations of the frame of the change", allocations.calls),
        Row::count(
            "bytes allocated by the frame of the change",
            allocations.taken,
        ),
        Row::count("frames of 240 Hz of the transition", frames),
        Row::count("elements drawn by a frame of the transition", moved.painted),
        Row::count("allocations of a frame of the transition", moving.calls),
        Row::count("bytes allocated by a frame of the transition", moving.taken),
        Row::count(
            "elements styled by the first frame at twice the size of text",
            built.styled,
        ),
        Row::count(
            "allocations of the first frame at twice the size of text",
            first.calls,
        ),
        Row::count(
            "elements styled by the frame after the size of text changed",
            resized.styled,
        ),
        Row::count(
            "allocations of the frame after the size of text changed",
            text.calls,
        ),
        Row::count(
            "bytes allocated by the frame after the size of text changed",
            text.taken,
        ),
        // The targets of ADR-0232, for each element that the frame styled or drew again.
        Row::per(
            "allocations of the frame of the change for each element styled",
            allocations.calls,
            stats.styled as u64,
            ALLOCATIONS_PER_ELEMENT_STYLED,
        ),
        Row::per(
            "allocations of a frame of the transition for each element drawn",
            moving.calls,
            moved.painted as u64,
            ALLOCATIONS_PER_ELEMENT_DRAWN,
        ),
        // ADR-0237's target: no more than the first frame at that size, for each element.
        Row::per(
            "allocations of the frame after the size of text changed for each element styled",
            text.calls,
            resized.styled as u64,
            (first_per_element * 100.0).ceil() / 100.0,
        ),
    ];
    rows.extend(Row::times(
        "the look's documents read, its sheet and its themes, by `Ui::set_style`",
        &readings,
    ));
    rows.extend(Row::times(
        "a frame after the look's colours changed, every element styled again",
        &changed,
    ));
    rows.extend(Row::times(
        "the frames of the transition that it starts, together",
        &transitions,
    ));
    rows.extend(Row::times(
        "a frame after the size of text changed, every text laid out again",
        &resizes,
    ));
    Ok(rows)
}

/// A list of 100 000 rows, a name and a value in each, on a screen of 1080 logical pixels: frames
/// while it scrolls by 7 pixels each, which shows other rows every third, then while it stands
/// still.
pub fn list(context: &mut Context) -> Result<Vec<Row>, String> {
    let runs = context.runs(RUNS);
    const ROWS: usize = 100_000;
    let labels = labels(1_000);
    let mut ui = Ui::new(latin()?);
    let root = ui.add_root("main", SCREEN, column);
    let mut list = None;
    let mut build = |ui: &mut Ui| {
        ui.build(root, 0, |children| {
            let element = Element::list(ROWS, 20.0).style(|s| s.grow(1.0));
            list = Some(children.add("list", element, |rows| {
                for row in rows.rows() {
                    let item = Element::container().style(|s| s.justify(Justify::SpaceBetween));
                    rows.add(row, item, |children| {
                        let name = labels[row % labels.len()].as_str();
                        children.add("name", Element::label(name).style(look), |_| {});
                        let value = Element::label(format!("{row}")).style(look);
                        children.add("value", value, |_| {});
                    });
                }
            }));
        });
        list.unwrap()
    };
    let list = build(&mut ui);
    ui.frame();
    build(&mut ui);
    ui.frame();
    // Counted before the series, whose length a run of `--counters` cuts.
    let counters = still_counters(|| {
        build(&mut ui);
        ui.frame()
    });
    let mut offset = 0.0;
    let scrolled = times(runs, || {
        offset += 7.0;
        ui.scroll_to(list, [0.0, offset]);
        build(&mut ui);
        ui.frame()
    });
    let still = times(runs, || {
        build(&mut ui);
        ui.frame()
    });
    let mut rows = counters;
    rows.extend(Row::times("a frame scrolled by 7 pixels", &scrolled));
    rows.extend(Row::times("a frame standing still", &still));
    Ok(rows)
}
