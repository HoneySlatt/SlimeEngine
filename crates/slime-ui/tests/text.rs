//! Text laid out and rasterized in several writing systems, with the test fonts of
//! `tests/fonts`: parts of Noto fonts that hold the characters of `tests/texts`
//! (`tests/fonts/README.md`).

use slime_ui::text::{
    Affinity, Alignment, Direction, Fonts, GlyphPixels, GlyphRasterizer, Representation,
    SUBPIXEL_POSITIONS, TextBounds, TextCache, TextLayout, TextPosition, TextStyle,
};

/// The test fonts' families, by script.
const LATIN: &str = "Noto Sans";
const ARABIC: &str = "Noto Sans Arabic";
const HEBREW: &str = "Noto Sans Hebrew";
const DEVANAGARI: &str = "Noto Sans Devanagari";
const THAI: &str = "Noto Sans Thai";
const JAPANESE: &str = "Noto Sans CJK JP";
const CHINESE: &str = "Noto Sans CJK SC";
const EMOJI: &str = "Noto Color Emoji";

/// The test fonts' files, and the family that each holds.
const FILES: [(&str, &str); 8] = [
    ("NotoSans.ttf", LATIN),
    ("NotoSansArabic.ttf", ARABIC),
    ("NotoSansHebrew.ttf", HEBREW),
    ("NotoSansDevanagari.ttf", DEVANAGARI),
    ("NotoSansThai.ttf", THAI),
    ("NotoSansCJKjp.otf", JAPANESE),
    ("NotoSansCJKsc.otf", CHINESE),
    ("NotoColorEmoji.ttf", EMOJI),
];

/// The bytes of the test font `file`.
fn font(file: &str) -> Vec<u8> {
    let path = format!("{}/tests/fonts/{file}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(path).expect("a test font")
}

/// Fonts that hold every test font, and none of the system's.
fn fonts() -> Fonts {
    let mut fonts = Fonts::new();
    for (file, _) in FILES {
        fonts.register(font(file)).expect("a font");
    }
    fonts
}

/// A style of the stack `families`, in `locale`.
fn style(families: &[&str], locale: Option<&str>) -> TextStyle {
    TextStyle {
        fonts: families.iter().map(|&family| family.to_owned()).collect(),
        locale: locale.map(str::to_owned),
        ..TextStyle::default()
    }
}

/// `text` laid out on lines that break only where it does, at scale 1.
fn line(fonts: &mut Fonts, text: &str, style: &TextStyle) -> TextLayout {
    fonts.lay_out(text, style, 1.0, TextBounds::default())
}

/// The x of the caret before the byte `index`.
fn caret_x(layout: &TextLayout, index: usize) -> f32 {
    layout
        .caret(TextPosition {
            index,
            affinity: Affinity::Downstream,
        })
        .x0
}

/// The glyphs' ids, in the order drawn.
fn glyphs(layout: &TextLayout) -> Vec<u32> {
    layout
        .runs()
        .iter()
        .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.id))
        .collect()
}

/// A test text, by its file's name.
fn sample(name: &str) -> String {
    let path = format!("{}/tests/texts/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).expect("a test text")
}

#[test]
fn the_test_fonts_are_registered_under_their_families() {
    let mut fonts = Fonts::new();
    for (file, family) in FILES {
        assert_eq!(
            fonts.register(font(file)).expect("a font"),
            [family],
            "{file}"
        );
    }
    assert!(fonts.register(b"not a font".to_vec()).is_err());
}

#[test]
fn arabic_is_joined_and_laid_out_right_to_left_with_latin_words_and_numbers_in_it() {
    let mut fonts = fonts();
    let arabic = style(&[ARABIC, LATIN], Some("ar"));
    // A letter alone, and the same letter joined to its neighbours: its initial, medial and
    // final forms are other glyphs. Its dot, a glyph of its own in this font, is the same.
    let alone = glyphs(&line(&mut fonts, "ب", &arabic));
    let joined = glyphs(&line(&mut fonts, "ببب", &arabic));
    let forms: std::collections::HashSet<_> = joined
        .iter()
        .filter(|glyph| !alone.contains(glyph))
        .collect();
    assert_eq!(forms.len(), 3, "{alone:?} {joined:?}");

    let text = "يدعم المحرك Vulkan و Direct3D 12 داخل الجملة";
    let layout = line(&mut fonts, text, &arabic);
    assert!(layout.is_right_to_left());
    // The text starts at the right: its first word is right of the Latin words that follow it.
    let vulkan = text.find("Vulkan").expect("a Latin word");
    assert!(caret_x(&layout, 0) > caret_x(&layout, vulkan));
    // A Latin word, and a number, read from left to right within it.
    assert!(caret_x(&layout, vulkan) < caret_x(&layout, vulkan + 3));
    let twelve = text.find("12").expect("a number");
    assert!(caret_x(&layout, twelve) < caret_x(&layout, twelve + 1));
    let runs = layout.runs();
    assert!(runs.iter().any(|run| run.right_to_left));
    assert!(runs.iter().any(|run| !run.right_to_left));
}

#[test]
fn hebrew_is_laid_out_right_to_left() {
    let mut fonts = fonts();
    let text = "קוראי מסך מקבלים עץ של צמתים";
    let layout = line(&mut fonts, text, &style(&[HEBREW, LATIN], Some("he")));
    assert!(layout.is_right_to_left());
    assert!(caret_x(&layout, 0) > caret_x(&layout, text.len() - "צמתים".len()));
    assert!(!glyphs(&layout).is_empty() && !glyphs(&layout).contains(&0));
}

#[test]
fn devanagari_letters_and_their_signs_make_one_cluster() {
    let mut fonts = fonts();
    // क, the virama ्, and ष: one conjunct, क्ष.
    let text = "क्ष";
    let layout = line(&mut fonts, text, &style(&[DEVANAGARI, LATIN], Some("hi")));
    let line = &layout.lines()[0];
    let middle = (line.top + line.bottom) / 2.0;
    assert_eq!(
        layout.cluster_at(layout.width() / 2.0, middle),
        Some(0..text.len())
    );
    // Fewer glyphs than characters: the conjunct is drawn as one.
    assert!(glyphs(&layout).len() < text.chars().count());
    // A caret is never put within it.
    for x in [
        0.0,
        layout.width() / 3.0,
        layout.width() * 2.0 / 3.0,
        layout.width(),
    ] {
        let index = layout.position_at(x, middle).index;
        assert!(index == 0 || index == text.len(), "{index}");
    }
}

#[test]
fn thai_breaks_between_its_words() {
    let mut fonts = fonts();
    let words = [
        "ภาษา",
        "ไทย",
        "เขียน",
        "โดย",
        "ไม่",
        "มี",
        "ช่อง",
        "ว่าง",
        "ระหว่าง",
        "คำ",
    ];
    let text = words.concat();
    let boundaries: Vec<usize> = words
        .iter()
        .scan(0, |end, word| {
            *end += word.len();
            Some(*end)
        })
        .collect();
    let layout = fonts.lay_out(
        &text,
        &style(&[THAI, LATIN], Some("th")),
        1.0,
        TextBounds::width(60.0),
    );
    assert!(layout.lines().len() > 2, "{} lines", layout.lines().len());
    for line in &layout.lines()[1..] {
        assert!(
            boundaries.contains(&line.text.start),
            "a line starts within a word, at byte {}",
            line.text.start
        );
    }
}

#[test]
fn japanese_and_chinese_break_between_characters() {
    let mut fonts = fonts();
    for (font, locale, name) in [(JAPANESE, "ja", "japanese"), (CHINESE, "zh-CN", "chinese")] {
        let text = sample(name).replace('\n', "");
        let layout = fonts.lay_out(
            &text,
            &style(&[font, LATIN], Some(locale)),
            1.0,
            TextBounds::width(120.0),
        );
        let lines = layout.lines();
        assert!(lines.len() > 10, "{name}: {} lines", lines.len());
        assert!(layout.width() <= 120.0 + 0.01, "{name}: {}", layout.width());
        let mut within = 0;
        for line in &lines[1..] {
            let first = text[line.text.start..]
                .chars()
                .next()
                .expect("a line holds text");
            // Not before the punctuation that ends a clause or a sentence.
            assert!(
                !"、。，".contains(first),
                "{name}: a line starts with {first}"
            );
            // A line that starts between two letters, not after punctuation: broken within a
            // sentence, which no space cuts into words.
            let before = text[..line.text.start]
                .chars()
                .next_back()
                .expect("a line before");
            if !"、。，".contains(before) {
                within += 1;
            }
        }
        assert!(
            within > lines.len() / 2,
            "{name}: {within} lines broken between letters"
        );
        assert!(
            !glyphs(&layout).is_empty() && !glyphs(&layout).contains(&0),
            "{name}"
        );
    }
}

#[test]
fn an_emoji_sequence_is_one_color_glyph() {
    let mut fonts = fonts();
    let layout = line(&mut fonts, "👨‍👩‍👧‍👦", &style(&[LATIN, EMOJI], None));
    let runs = layout.runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].glyphs.len(), 1);
    assert_ne!(runs[0].glyphs[0].id, 0);
    assert_eq!(runs[0].representation, Representation::Color);
    assert_eq!(fonts.family(runs[0].font), EMOJI);
}

#[test]
fn no_glyph_is_missing_from_the_test_texts() {
    let mut fonts = fonts();
    for (name, stack, locale) in [
        ("english", [LATIN, LATIN], "en"),
        ("arabic", [ARABIC, LATIN], "ar"),
        ("hebrew", [HEBREW, LATIN], "he"),
        ("hindi", [DEVANAGARI, LATIN], "hi"),
        ("thai", [THAI, LATIN], "th"),
        ("japanese", [JAPANESE, LATIN], "ja"),
        ("chinese", [CHINESE, LATIN], "zh-CN"),
        ("emoji", [LATIN, EMOJI], "en"),
    ] {
        for sentence in sample(name).lines() {
            let layout = fonts.lay_out(
                sentence,
                &style(&stack, Some(locale)),
                1.0,
                TextBounds::width(300.0),
            );
            let glyphs = glyphs(&layout);
            assert!(
                !glyphs.is_empty() && !glyphs.contains(&0),
                "{name}: {sentence}"
            );
        }
    }
}

#[test]
fn a_text_too_long_for_its_box_ends_with_an_ellipsis() {
    let mut fonts = fonts();
    let latin = style(&[LATIN], None);
    let text = "Every panel can be docked, undocked, resized, or grouped into tabs";
    let label = fonts.lay_out(text, &latin, 1.0, TextBounds::line(120.0));
    assert!(label.shown().ends_with('…'), "{}", label.shown());
    // Cut after a word, the space before the ellipsis left out.
    assert_eq!(label.cut(), Some(label.shown().len() - '…'.len_utf8()));
    assert!(text[label.cut().expect("cut")..].starts_with(' '));
    assert!(text.starts_with(label.shown().trim_end_matches('…')));
    assert_eq!(label.lines().len(), 1);
    assert!(label.width() <= 120.0 + 0.01, "{}", label.width());
    // At twice the scale, the same box in logical pixels.
    let sharp = fonts.lay_out(text, &latin, 2.0, TextBounds::line(120.0));
    assert!(sharp.width() <= 240.0 + 0.01);
    assert_eq!(sharp.shown(), label.shown());

    // Several lines: the last one shown is cut.
    let paragraph = sample("english").replace('\n', " ");
    let bounds = TextBounds {
        width: Some(200.0),
        max_lines: std::num::NonZeroUsize::new(3),
    };
    let three = fonts.lay_out(&paragraph, &latin, 1.0, bounds);
    assert_eq!(three.lines().len(), 3);
    assert!(three.shown().ends_with('…'));

    // A text that fits is not cut, nor one that ends in a line break, nor an empty one.
    let short = fonts.lay_out("Docked", &latin, 1.0, TextBounds::line(120.0));
    assert_eq!((short.cut(), short.shown()), (None, "Docked"));
    let ended = fonts.lay_out("Docked\n", &latin, 1.0, TextBounds::line(120.0));
    assert_eq!(ended.cut(), None);
    let empty = fonts.lay_out("", &latin, 1.0, TextBounds::line(120.0));
    assert_eq!((empty.cut(), empty.runs().len()), (None, 0));

    // A box narrower than a word: the ellipsis alone where the line starts.
    let narrow = fonts.lay_out(
        &paragraph,
        &latin,
        1.0,
        TextBounds {
            width: Some(2.0),
            max_lines: std::num::NonZeroUsize::new(2),
        },
    );
    assert!(
        narrow.shown().ends_with('…') && narrow.lines().len() <= 2,
        "{}",
        narrow.shown()
    );

    // A font without the ellipsis, Noto Sans Arabic: three full stops, which it has.
    let arabic = fonts.lay_out(
        sample("arabic").lines().next().expect("a sentence"),
        &style(&[ARABIC], Some("ar")),
        1.0,
        TextBounds::line(150.0),
    );
    assert!(arabic.shown().ends_with("..."), "{}", arabic.shown());
    assert!(!glyphs(&arabic).contains(&0));

    // Right to left, the ellipsis ends the text at its left.
    let arabic = sample("arabic");
    let first = arabic.lines().next().expect("a sentence");
    let cut = fonts.lay_out(
        first,
        &style(&[ARABIC, LATIN], Some("ar")),
        1.0,
        TextBounds::line(150.0),
    );
    let shown = cut.shown();
    assert!(shown.ends_with('…'));
    let end = TextPosition {
        index: shown.len(),
        affinity: Affinity::Upstream,
    };
    // Its start at the box's right, its start; the ellipsis left of every other character.
    assert!(
        (caret_x(&cut, 0) - 150.0).abs() < 1.0,
        "{}",
        caret_x(&cut, 0)
    );
    let left = cut.caret(end).x0;
    assert!(
        shown
            .char_indices()
            .all(|(index, _)| caret_x(&cut, index) > left),
        "{left}"
    );
}

#[test]
fn a_box_that_changes_breaks_the_lines_again() {
    let mut fonts = fonts();
    let latin = style(&[LATIN], None);
    let paragraph = sample("english").replace('\n', " ");
    let mut layout = fonts.lay_out(&paragraph, &latin, 1.0, TextBounds::width(400.0));
    let wide = layout.lines().len();
    layout.set_bounds(&mut fonts, TextBounds::width(200.0));
    let narrow = fonts.lay_out(&paragraph, &latin, 1.0, TextBounds::width(200.0));
    assert!(layout.lines().len() > wide);
    assert_eq!(layout.lines(), narrow.lines());
    assert_eq!(layout.runs(), narrow.runs());
    // Cut, then whole again.
    layout.set_bounds(&mut fonts, TextBounds::line(200.0));
    assert!(layout.cut().is_some());
    layout.set_bounds(&mut fonts, TextBounds::width(200.0));
    assert_eq!((layout.cut(), layout.shown()), (None, paragraph.as_str()));
}

#[test]
fn carets_and_the_cluster_under_a_point_follow_the_text_s_direction() {
    let mut fonts = fonts();
    let latin = line(&mut fonts, "Docked", &style(&[LATIN], None));
    let middle = (latin.lines()[0].top + latin.lines()[0].bottom) / 2.0;
    let xs: Vec<f32> = (0..=6).map(|index| caret_x(&latin, index)).collect();
    assert!(xs.windows(2).all(|pair| pair[0] < pair[1]), "{xs:?}");
    assert_eq!(latin.cluster_at((xs[0] + xs[1]) / 2.0, middle), Some(0..1));
    assert_eq!(latin.cluster_at(latin.width() + 20.0, middle), None);
    assert_eq!(latin.position_at(-5.0, middle).index, 0);
    assert_eq!(latin.position_at(latin.width() + 5.0, middle).index, 6);
    let selected = latin.selection(0..6);
    assert_eq!(selected.len(), 1);
    assert!((selected[0].x1 - selected[0].x0 - latin.width()).abs() < 0.5);

    let text = "מסך";
    let hebrew = line(&mut fonts, text, &style(&[HEBREW, LATIN], Some("he")));
    let indices: Vec<usize> = text
        .char_indices()
        .map(|(index, _)| index)
        .chain([text.len()])
        .collect();
    let xs: Vec<f32> = indices
        .iter()
        .map(|&index| caret_x(&hebrew, index))
        .collect();
    assert!(xs.windows(2).all(|pair| pair[0] > pair[1]), "{xs:?}");
    // Its start is at its right.
    assert_eq!(hebrew.position_at(hebrew.width() + 5.0, middle).index, 0);
    assert_eq!(hebrew.position_at(-5.0, middle).index, text.len());
    assert_eq!(
        hebrew.cluster_at((xs[0] + xs[1]) / 2.0, middle),
        Some(0..indices[1])
    );
}

#[test]
fn a_style_s_weight_alignment_and_direction_change_the_layout() {
    let mut fonts = fonts();
    let regular = line(&mut fonts, "Docked", &style(&[LATIN], None));
    let bold = line(
        &mut fonts,
        "Docked",
        &TextStyle {
            weight: 700.0,
            ..style(&[LATIN], None)
        },
    );
    // A variable font drawn bold is another face: its glyphs have other images.
    assert_ne!(regular.runs()[0].font, bold.runs()[0].font);
    assert!(bold.width() > regular.width());

    let end = fonts.lay_out(
        "Docked",
        &TextStyle {
            alignment: Alignment::End,
            ..style(&[LATIN], None)
        },
        1.0,
        TextBounds::width(300.0),
    );
    assert!(
        (caret_x(&end, 6) - 300.0).abs() < 0.5,
        "{}",
        caret_x(&end, 6)
    );
    let right_to_left = line(
        &mut fonts,
        "Docked",
        &TextStyle {
            direction: Direction::RightToLeft,
            ..style(&[LATIN], None)
        },
    );
    assert!(right_to_left.is_right_to_left());
    assert!(!regular.is_right_to_left());
}

#[test]
fn a_scale_multiplies_sizes_and_positions() {
    let mut fonts = fonts();
    let latin = style(&[LATIN], None);
    let one = fonts.lay_out("Docked", &latin, 1.0, TextBounds::default());
    let two = fonts.lay_out("Docked", &latin, 2.0, TextBounds::default());
    assert_eq!(one.runs()[0].size, 14.0);
    assert_eq!(two.runs()[0].size, 28.0);
    assert!((two.width() - one.width() * 2.0).abs() < 1.0);
}

#[test]
fn glyphs_are_rasterized_at_their_positions_between_pixels() {
    let mut fonts = fonts();
    let layout = line(&mut fonts, "Docked", &style(&[LATIN], None));
    let placed: Vec<_> = layout.runs()[0].placed().collect();
    assert_eq!(placed.len(), 6);
    let mut rasterizer = GlyphRasterizer::new();
    let image = rasterizer
        .rasterize(&fonts, placed[0].key)
        .expect("a letter draws something");
    let GlyphPixels::Coverage(pixels) = &image.pixels else {
        panic!("a letter is coverage");
    };
    assert_eq!(pixels.len(), (image.width * image.height) as usize);
    assert!(pixels.contains(&255));
    assert!(image.top > 0 && image.height > 0);
    // The glyphs' origins, a whole pixel and a position between two.
    for (glyph, run) in placed.iter().zip(&layout.runs()[0].glyphs) {
        let x = glyph.x as f32 + f32::from(glyph.key.subpixel()) / f32::from(SUBPIXEL_POSITIONS);
        assert!((x - run.x).abs() <= 0.5 / f32::from(SUBPIXEL_POSITIONS) + 1e-4);
    }
    // A space draws nothing.
    let space = line(&mut fonts, "a b", &style(&[LATIN], None));
    let keys: Vec<_> = space.runs()[0].placed().map(|glyph| glyph.key).collect();
    assert!(rasterizer.rasterize(&fonts, keys[1]).is_none());
}

#[test]
fn an_emoji_is_rasterized_in_color() {
    let mut fonts = fonts();
    let layout = line(&mut fonts, "🦊", &style(&[LATIN, EMOJI], None));
    let key = layout.runs()[0].placed().next().expect("a glyph").key;
    let image = GlyphRasterizer::new()
        .rasterize(&fonts, key)
        .expect("an emoji draws something");
    let GlyphPixels::Color(pixels) = &image.pixels else {
        panic!("an emoji is color");
    };
    assert_eq!(pixels.len(), (image.width * image.height * 4) as usize);
    // Colors that differ from one pixel to the next, not a single tint.
    let colors: std::collections::HashSet<_> = pixels
        .chunks(4)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect();
    assert!(colors.len() > 10);
    // Not multiplied by their alpha: an edge's colors stay brighter than its alpha.
    assert!(
        pixels
            .chunks(4)
            .any(|pixel| pixel[3] < 128 && pixel[..3].iter().any(|&channel| channel > pixel[3]))
    );
    // About the size of the em.
    assert!((10..=20).contains(&image.height), "{}", image.height);
}

#[test]
fn a_text_cache_keeps_its_texts_within_its_budget_least_recently_used_first() {
    let mut fonts = fonts();
    let latin = style(&[LATIN], None);
    let one = fonts
        .lay_out("one", &latin, 1.0, TextBounds::default())
        .memory();
    // Room for three such texts.
    let mut cache = TextCache::new(one * 3 + one / 2);
    for text in ["one", "two", "six"] {
        cache.get(&mut fonts, text, &latin, 1.0, TextBounds::default());
    }
    assert_eq!(cache.len(), 3);
    // "one" used again, then a fourth: "two", the least recently used, leaves.
    cache.get(&mut fonts, "one", &latin, 1.0, TextBounds::default());
    cache.get(&mut fonts, "ten", &latin, 1.0, TextBounds::default());
    assert_eq!(cache.len(), 3);
    assert!(cache.memory() <= one * 3 + one / 2);
    // Kept in another box, it is laid out in that box.
    let layout = cache.get(&mut fonts, "one", &latin, 1.0, TextBounds::line(10.0));
    assert!(layout.cut().is_some());
    assert_eq!(cache.len(), 3);
    // A text larger than the budget is kept alone.
    let mut small = TextCache::new(1);
    small.get(&mut fonts, "one", &latin, 1.0, TextBounds::default());
    small.get(&mut fonts, "two", &latin, 1.0, TextBounds::default());
    assert_eq!(small.len(), 1);
    assert_eq!(
        small
            .get(&mut fonts, "two", &latin, 1.0, TextBounds::default())
            .text(),
        "two"
    );
}
