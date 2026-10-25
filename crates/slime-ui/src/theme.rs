//! Style sheets and themes: documents that style an interface's elements
//! (`docs/adr/0141-style-sheets-and-themes.md`).
//!
//! A *style sheet* holds rules, each a table named as its author likes, applied in the order of the
//! file: what elements it selects, `select`, and the part of their style that it sets, `style`, a
//! part of a [`Style`] as `slime-serialize` writes one, which leaves out what it does not set and
//! writes an option as a list of none or one value, `shadow = [{ ... }]`, so that a rule can empty
//! one. A *theme* holds named values, which a
//! rule's fields give as `"$name"` in place of a value; a color may also be given as sRGB,
//! `"#rrggbb"` or `"#rrggbbaa"`. Changing the theme changes the values that the rules give, the
//! rules unchanged:
//!
//! ```toml
//! # A style sheet.
//! [panel]
//! select = { classes = ["panel"] }
//! style.paint = { background = "$surface", radii = ["$radius", "$radius", "$radius", "$radius"] }
//!
//! [toolbar-button-pressed]
//! select = { kind = "Button", states = ["Pressed"], inside = { class = "toolbar" } }
//! style.paint.background = "$accent"
//! ```
//!
//! ```toml
//! # A theme.
//! surface = "#26282C"
//! accent = "#4C6EF5"
//! radius = 8
//! ```

use std::cell::RefCell;

use slime_color::LinearColor;
use slime_reflect::{Problem, Reflect, Registry, Value};
use slime_serialize::{Document, References};

use crate::DocumentError;
use crate::document::Source;
use crate::style::Style;

/// A style sheet: rules that select elements and set a part of their style, in the order they are
/// applied, read when an interface takes it ([`Ui::set_style`](crate::tree::Ui::set_style)).
#[derive(Clone, Debug)]
pub struct StyleSheet {
    name: String,
    source: Source,
}

/// A theme: the values that the rules of style sheets name.
#[derive(Clone, Debug)]
pub struct Theme {
    name: String,
    source: Source,
}

impl StyleSheet {
    /// The style sheet that `text` holds, named `name` in its errors: its file, usually.
    pub fn from_text(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: Source::Text(text.into()),
        }
    }

    /// The style sheet that `bytes`, the binary encoding of its document, hold, as the cooker
    /// writes them.
    pub fn from_binary(name: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            source: Source::Binary(bytes),
        }
    }

    /// Its name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Theme {
    /// The theme that `text` holds, named `name` in its errors: its file, usually.
    pub fn from_text(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: Source::Text(text.into()),
        }
    }

    /// The theme that `bytes`, the binary encoding of its document, hold, as the cooker writes
    /// them.
    pub fn from_binary(name: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            source: Source::Binary(bytes),
        }
    }

    /// Its name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// What a rule selects: elements of a kind, holding classes, of a key, in states, inside an
/// ancestor of a kind or a class. A rule selects an element that every condition it gives holds
/// for.
#[derive(Clone, Debug, Default, PartialEq, Reflect)]
#[reflect(name = "UiSelector", default)]
pub(crate) struct Selector {
    /// The kind of element.
    pub(crate) kind: Option<KindName>,
    /// Classes that the element holds, all of them.
    pub(crate) classes: Vec<String>,
    /// The element's key among its siblings, a name, or a number written `#3`.
    pub(crate) key: Option<String>,
    /// States that the element is in, all of them.
    pub(crate) states: Vec<StateName>,
    /// An ancestor that the element lies inside, at any depth.
    pub(crate) inside: Option<Ancestor>,
}

/// The kinds of elements, as a rule names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "UiElementKind")]
pub(crate) enum KindName {
    Container,
    Label,
    Image,
    Button,
    Scroll,
    List,
}

/// The states of an element, as a rule names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "UiElementState")]
pub(crate) enum StateName {
    Hovered,
    Pressed,
    Disabled,
    Focused,
}

/// An ancestor that a rule asks for: of a kind, holding a class, or both.
#[derive(Clone, Debug, Default, PartialEq, Reflect)]
#[reflect(name = "UiAncestor", default)]
pub(crate) struct Ancestor {
    /// Its kind.
    pub(crate) kind: Option<KindName>,
    /// A class that it holds.
    pub(crate) class: Option<String>,
}

/// A rule of a style sheet, its names replaced by the theme's values.
#[derive(Clone, Debug)]
pub(crate) struct Rule {
    pub(crate) selector: Selector,
    /// A part of a [`Style`], as reflection holds it.
    pub(crate) style: Value,
}

/// The registry of the types that style sheets and themes hold.
pub(crate) fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register::<Style>();
    registry.register::<Selector>();
    registry
}

/// Reads the rules of `sheets`, in their order and the order of their files, their names replaced
/// by the values of `theme`.
pub(crate) fn rules(
    registry: &Registry,
    sheets: &[StyleSheet],
    theme: &Theme,
) -> Result<Vec<Rule>, DocumentError> {
    let theme_error = |error| DocumentError {
        document: theme.name.clone(),
        error,
    };
    let theme_document = theme.source.parse(registry).map_err(theme_error)?;
    let names = Names {
        theme: &theme.name,
        document: RefCell::new(theme_document),
    };
    // What every rule's part is laid over, to check that it lays over a style whole.
    let default = Style::default().to_value();
    let mut rules = Vec::new();
    for sheet in sheets {
        let sheet_error = |error| DocumentError {
            document: sheet.name.clone(),
            error,
        };
        let mut document = sheet.source.parse(registry).map_err(sheet_error)?;
        for name in document.keys(&[]) {
            if !document.is_table(&[&name]) {
                return Err(sheet_error(document.error_at(
                    &[&name],
                    Problem::Invalid(format!(
                        "`{name}` is not a rule: a rule is a table of `select` and `style`"
                    )),
                )));
            }
            let selector = document
                .read(&[&name, "select"], "UiSelector", Some(&names))
                .map_err(sheet_error)?
                .map(|value| Selector::from_value(&value).expect("read as a selector"))
                .unwrap_or_default();
            let style = document
                .read_part(&[&name, "style"], "UiStyle", Some(&names))
                .map_err(sheet_error)?
                .unwrap_or(Value::Struct(Vec::new()));
            // A part that leaves out a field of what it sets whole, an option's value, without
            // a default to give it, lays over no style: refused here, not when a frame styles.
            registry
                .overlay("UiStyle", &mut default.clone(), &style)
                .map_err(|error| {
                    let problem = Problem::Invalid(error.to_string());
                    sheet_error(document.error_at(&[&name, "style"], problem))
                })?;
            for key in document.keys(&[&name]) {
                if key != "select" && key != "style" {
                    return Err(sheet_error(document.error_at(
                        &[&name, &key],
                        Problem::Invalid(format!("a rule holds `select` and `style`, not `{key}`")),
                    )));
                }
            }
            rules.push(Rule { selector, style });
        }
    }
    Ok(rules)
}

/// What the texts of a style sheet stand for: `"$name"` for the theme's value of that name,
/// `"#rrggbb"` and `"#rrggbbaa"` for a color in sRGB, its alpha straight.
struct Names<'a> {
    theme: &'a str,
    document: RefCell<Document<'a>>,
}

impl References for Names<'_> {
    fn name(&self, _: u64) -> Option<String> {
        None
    }

    fn entity(&self, _: &str) -> Option<u64> {
        None
    }

    fn value(&self, ty: &str, text: &str) -> Option<Result<Value, String>> {
        if let Some(name) = text.strip_prefix('$') {
            let theme = self.theme;
            // A name in place of an option, as a part writes one, stands for the option's value.
            let held = ty
                .strip_prefix("Option<")
                .and_then(|inner| inner.strip_suffix('>'));
            // A theme's values give colors in sRGB too, but name no other value.
            let read = self
                .document
                .borrow_mut()
                .read(&[name], held.unwrap_or(ty), Some(&Colors));
            return Some(match read {
                Ok(Some(value)) if held.is_some() => Ok(Value::Option(Some(Box::new(value)))),
                Ok(Some(value)) => Ok(value),
                Ok(None) => Err(format!("the theme `{theme}` has no value named `{name}`")),
                Err(error) => Err(format!(
                    "the value `{name}` of the theme `{theme}`: {error}"
                )),
            });
        }
        Colors.value(ty, text)
    }
}

/// What the texts of a theme stand for: colors in sRGB.
struct Colors;

impl References for Colors {
    fn name(&self, _: u64) -> Option<String> {
        None
    }

    fn entity(&self, _: &str) -> Option<u64> {
        None
    }

    fn value(&self, ty: &str, text: &str) -> Option<Result<Value, String>> {
        let hex = text.strip_prefix('#')?;
        if ty != "LinearColor" {
            return None;
        }
        Some(
            srgb(hex)
                .map(|color| color.to_value())
                .ok_or_else(|| format!("`{text}` is not a color: `#rrggbb` or `#rrggbbaa`")),
        )
    }
}

/// The color that `hex`, six or eight hexadecimal digits, gives in sRGB, its alpha straight.
fn srgb(hex: &str) -> Option<LinearColor> {
    if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    let mut color = LinearColor::from_srgb8(byte(0)?, byte(2)?, byte(4)?);
    if hex.len() == 8 {
        color.a = f32::from(byte(6)?) / 255.0;
    }
    Some(color)
}
