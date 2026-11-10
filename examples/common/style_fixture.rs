//! The fixtures of styles that the image tests keep beside their reference images
//! (`docs/adr/0229-styles-typed-as-declarations.md`): a line for each element of a tree, its path
//! and a fingerprint of its style, its numbers rounded to four places, in `tests/reference/`.
//! They were recorded through the reflected cascade, which the typed one replaced, and outlive it.
//! `SLIME_UPDATE_REFERENCE_IMAGES` writes them again, as it renders the references: a change of a
//! look or of `uwui::style::Style`'s fields changes them on purpose; a change of the cascade must
//! not.

use std::path::Path;

use uwui::tree::{ElementId, Ui};

/// An element's line of a fixture, and its style as it was fingerprinted, which a failure shows.
pub struct Styled {
    pub line: String,
    pub style: String,
}

/// The lines of every element of `ui` below `root`, `root` first, depth first, each `prefix`, its
/// path and its style's fingerprint.
pub fn styles(ui: &Ui, root: ElementId, prefix: &str) -> Vec<Styled> {
    let mut styled = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let path = ui.path_text(id).expect("an element of the tree");
        let style = rounded(&format!("{:?}", ui.style(id).expect("a styled element")));
        styled.push(Styled {
            line: format!("{prefix} {path} {:016x}", fingerprint(&style)),
            style,
        });
        stack.extend(ui.children(id).iter().rev());
    }
    styled
}

/// Checks `styled` against the fixture `name` of `tests/reference/`, or writes it when the
/// references are asked to be written again.
pub fn check(name: &str, styled: &[Styled]) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/reference")
        .join(name);
    let text: String = styled
        .iter()
        .map(|styled| format!("{}\n", styled.line))
        .collect();
    if std::env::var_os("SLIME_UPDATE_REFERENCE_IMAGES").is_some() {
        std::fs::write(&path, text).expect("the fixture is written");
        return;
    }
    let recorded = std::fs::read_to_string(&path).expect("the fixture is read");
    for (number, (styled, want)) in styled.iter().zip(recorded.lines()).enumerate() {
        assert_eq!(
            styled.line,
            want,
            "line {} of {}: its element's style is now {}",
            number + 1,
            path.display(),
            styled.style
        );
    }
    assert_eq!(
        styled.len(),
        recorded.lines().count(),
        "the elements of {}",
        path.display()
    );
}

/// `text` with each number that holds a point or an exponent written to four places: the look's
/// colours are computed through OKLCH's cube roots and arctangents, whose last bits differ between
/// the platforms' mathematical libraries. A number that falls on a boundary of the fourth place may
/// still round apart on two platforms.
fn rounded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(|c: char| c.is_ascii_digit() || c == '-') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest
            .char_indices()
            .skip(1)
            .find(|&(_, c)| !(c.is_ascii_digit() || matches!(c, '.' | 'e' | '-')))
            .map_or(rest.len(), |(at, _)| at);
        let number = &rest[..end];
        match number.parse::<f64>() {
            Ok(value) if number.contains(['.', 'e']) => out.push_str(&format!("{value:.4}")),
            _ => out.push_str(number),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// FNV-1a's 64 bits of `text`: a fingerprint that every platform computes alike.
fn fingerprint(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}
