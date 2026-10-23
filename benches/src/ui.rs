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
