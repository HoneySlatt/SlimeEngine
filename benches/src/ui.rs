//! The interface's text (`docs/adr/0136-the-interfaces-text.md`): labels and paragraphs laid out in
//! several writing systems, lines broken again in another width, and glyphs rasterized, with the
//! fonts of slime-ui's tests.

use std::hint::black_box;
use std::time::Duration;

use slime_ui::text::{Fonts, GlyphKey, GlyphRasterizer, TextBounds, TextStyle};

use crate::measure::{Context, Row, ms, percentile, timed};

/// The size of the text, in pixels, at scale 1.
const SIZE: f32 = 14.0;
/// The width that paragraphs are laid out at.
const WIDTH: f32 = 400.0;
/// How many characters a paragraph holds, at least.
const PARAGRAPH: usize = 2_000;
/// How many times each figure is taken.
const RUNS: usize = 15;

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

/// The fonts of slime-ui's tests.
fn fonts() -> Result<Fonts, String> {
    let directory = Context::root().join("crates/slime-ui/tests/fonts");
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

/// The sentences of slime-ui's test text `name`, repeated until they hold [`PARAGRAPH`]
/// characters: repeated words help any cache keyed by word, which parley has not.
fn paragraph(script: &Script) -> Result<String, String> {
    let path = Context::root().join(format!("crates/slime-ui/tests/texts/{}.txt", script.name));
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

/// The times of `RUNS` runs of `run`, after two that are not timed.
fn times(mut run: impl FnMut()) -> Vec<Duration> {
    run();
    run();
    (0..RUNS).map(|_| timed(&mut run).1).collect()
}

/// 1 000 labels laid out, each on a line of its own.
pub fn labels_1k(_: &mut Context) -> Result<Vec<Row>, String> {
    let mut fonts = fonts()?;
    let labels = labels(1_000);
    let style = style(&SCRIPTS[0]);
    let times = times(|| {
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
        let times = times(|| {
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
    let times = times(|| {
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
            let times = times(|| {
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

/// A game's menu drawn from a display list through `slime-ui-render`
/// (`docs/adr/0139-text-drawn-and-the-display-list.md`), at the tiers' sizes: the panels, slots, rows
/// and buttons of `render.canvas`'s menu, without its icons, laid out for 1920 × 1080 and scaled to
/// fit, and real texts, laid out with the fonts of slime-ui's tests at the screen's scale: a
/// description of some 1 500 characters, 34 rows of statistics, 30 buttons' labels and 100 counts,
/// some 2 400 glyphs. The first draw, which rasterizes its glyphs into the atlas, the renderer
/// warmed before it; then the CPU of `UiRenderer::draw` once they are there, and the canvas's GPU
/// time, which ADR-0137 bounds.
pub fn menu(context: &mut Context) -> Result<Vec<Row>, String> {
    use std::sync::Arc;

    use slime_color::LinearColor;
    use slime_render::Renderer;
    use slime_ui::Rect;
    use slime_ui::display::{BoxPrimitive, DisplayList, ShadowPrimitive, TextPrimitive};
    use slime_ui::text::{TEXT_CACHE_BUDGET, TextCache};
    use slime_ui_render::{GLYPH_ATLAS_BUDGET, UiRenderer};

    use crate::render::{FORMAT, TIERS, canvas_frames};

    let gpu = &context.gpu;
    let error = |error: slime_render::RenderError| error.to_string();
    let mut fonts = fonts()?;
    let path = Context::root().join("crates/slime-ui/tests/texts/english.txt");
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
            || ui.draw(gpu, &mut renderer, &fonts, &list, |_| None, &target),
            |stats| stats.canvas.gpu_time,
        )?;
        rows.push(Row::new(
            format!(
                "the first draw {at}, its {} glyphs rasterized into the atlas",
                first.rasterized
            ),
            ms(cold),
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
        rows.push(Row::new(
            format!("glyphs drawn, items, and the atlas's memory {at}"),
            format!(
                "{}, {}, {} MiB",
                last.glyphs,
                last.canvas.items,
                last.atlas_bytes >> 20
            ),
        ));
    }
    Ok(rows)
}
