//! uwui's reader of KDL (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`):
//! a document of the size of the editor's look, and a large one, read.

use std::hint::black_box;
use std::time::Instant;

use crate::allocations::counted;
use crate::measure::{Context, Row};

/// How many times each document is read.
const RUNS: usize = 30;

/// Some rules of a style sheet and a theme's values, in the notation that ADR-0241 maps uwui's
/// documents onto, which a document repeats: every form of string, names, annotations, numbers,
/// comments and a slashdash.
const PIECE: &str = r##"// A rule of a style sheet.
menu {
    select kind=Menu
    paint background=$raised border_width=1 border=$raised_border {
        radii $radius_l $radius_l $radius_l $radius_l
        shadow color=$shadow blur=$shadow_blur spread=0 { offset 0 $shadow_y }
    }
    layout direction=Column {
        padding start=$space_xs end=$space_xs top=$space_xs bottom=$space_xs
        min_size (px)$menu_width Auto
    }
}
menu-separator {
    select { classes separator; inside kind=Menu }
    paint background=$raised_border
    layout { size Auto (px)1; margin start=$space_s end=$space_s /-top=0 }
}
/* A theme's values, by their kinds. */
color { accent "#4C8DFF"; accent_hover "lighten($accent, 0.05)"; scrim "#00000099" }
length { space_m 8; radius_l 8; menu_width 180 }
duration { fast (ms)120; slow (s)0.25 }
text { tick "\u{E06C}"; note #"a raw \ text"# }
"##;

/// A document of `PIECE` repeated until it holds at least `bytes`.
fn document(bytes: usize) -> String {
    PIECE.repeat(bytes.div_ceil(PIECE.len()))
}

/// The documents read: one of the editor's look's size, its sheet and five themes, 28 kB; one of
/// 4 MB; and the most nodes that [`uwui_kdl::Limits::DEFAULT`] allows, 2^20 at the top, `a;` each.
pub fn read(context: &mut Context) -> Result<Vec<Row>, String> {
    let runs = context.runs(RUNS);
    let most = "a;".repeat(uwui_kdl::Limits::DEFAULT.items);
    let mut rows = Vec::new();
    for (name, text) in [
        ("look", document(28 << 10)),
        ("large", document(4 << 20)),
        ("most nodes", most),
    ] {
        // The document is kept while the allocations are counted, so that what it holds is what
        // it took and did not give back.
        let (read, allocations) = counted(|| uwui_kdl::read(&text));
        let document =
            read.map_err(|error| format!("the {name} document does not read: {error}"))?;
        let held = allocations.taken - allocations.freed;
        drop(document);
        let mut times = Vec::with_capacity(runs);
        for _ in 0..runs {
            let start = Instant::now();
            let _ = black_box(
                uwui_kdl::read(black_box(&text)).map(|document| document.nodes().count()),
            );
            times.push(start.elapsed());
        }
        let what = format!("the {name} document, {} bytes, read", text.len());
        rows.extend(Row::times(&what, &times));
        rows.push(Row::count(
            format!("allocations of reading the {name} document"),
            allocations.calls,
        ));
        rows.push(Row::count(
            format!("bytes taken reading the {name} document"),
            allocations.taken,
        ));
        rows.push(Row::count(
            format!("bytes that the {name} document holds"),
            held,
        ));
        rows.push(Row::new(
            format!("bytes that the {name} document holds for each byte of its text"),
            format!("{:.2}", held as f64 / text.len() as f64),
        ));
    }
    Ok(rows)
}
