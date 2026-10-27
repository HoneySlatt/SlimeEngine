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
//! rules unchanged.
//!
//! A theme's value may name another, `"$name"`, and a color may be computed by a function, in
//! OKLCH, where equal steps look equal (`docs/adr/0149-the-looks-foundations.md`): `mix($a, $b,
//! 20%)`, the color a fifth of the way from `$a` to `$b`, its hue going the shorter way around, a
//! grey taking the other's hue; `lighten($a, 0.08)` and `darken($a, 0.08)`, the color lighter or
//! darker by that much of OKLCH's lightness, from 0 to 1; `alpha($a, 40%)`, the color of that
//! opacity. A color that leaves sRGB loses chroma until it fits, its lightness and hue kept. A
//! share is a number, `0.2`, a percentage, `20%`, or a value's name. Themes stack: of several
//! themes, the last that gives a name gives its value, and the values that name it follow it:
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
//! accent_hover = "lighten($accent, 0.06)"
//! accent_soft = "alpha($accent, 20%)"
//! radius = 8
//! ```
//!
//! ```toml
//! # A theme stacked after it: the accent changes, and its hover and soft fill with it.
//! accent = "#2BB673"
//! ```

use std::cell::RefCell;
use std::collections::HashMap;

use slime_color::{LinearColor, Oklch};
use slime_reflect::{Problem, Reflect, Registry, Value};
use slime_serialize::References;

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
    TextField,
    NumberField,
    CheckBox,
    Slider,
    Section,
    TabBar,
    Tab,
    Split,
    Menu,
    MenuBar,
    MenuItem,
    Choice,
    FloatingPanel,
}

/// The states of an element, as a rule names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "UiElementState")]
pub(crate) enum StateName {
    Hovered,
    Pressed,
    Disabled,
    Focused,
    FocusVisible,
    Checked,
    Expanded,
    Selected,
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
/// by the values of `themes`, the last that gives a name giving its value.
pub(crate) fn rules(
    registry: &Registry,
    sheets: &[StyleSheet],
    themes: &[Theme],
) -> Result<Vec<Rule>, DocumentError> {
    let values = Values::new(registry, themes)?;
    values.check()?;
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
                .read(&[&name, "select"], "UiSelector", Some(&values))
                .map_err(sheet_error)?
                .map(|value| Selector::from_value(&value).expect("read as a selector"))
                .unwrap_or_default();
            let style = document
                .read_part(&[&name, "style"], "UiStyle", Some(&values))
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

/// The values of stacked themes, worked out as the rules and the themes name them: `"$name"` for a
/// value, `"#rrggbb"` and `"#rrggbbaa"` for a color in sRGB, its alpha straight, and the color
/// functions.
struct Values<'a> {
    registry: &'a Registry,
    themes: &'a [Theme],
    /// The theme that gives each name: the last that holds it.
    owners: HashMap<String, usize>,
    /// The names being worked out, innermost last: one named again names itself.
    working: RefCell<Vec<String>>,
    /// The values worked out, by name and type, each worked out once.
    done: RefCell<HashMap<(String, String), Value>>,
}

impl<'a> Values<'a> {
    /// The values of `themes`, each parsed to learn the names it gives.
    fn new(registry: &'a Registry, themes: &'a [Theme]) -> Result<Self, DocumentError> {
        let mut owners = HashMap::new();
        for (index, theme) in themes.iter().enumerate() {
            let document = theme
                .source
                .parse(registry)
                .map_err(|error| DocumentError {
                    document: theme.name.clone(),
                    error,
                })?;
            for name in document.keys(&[]) {
                owners.insert(name, index);
            }
        }
        Ok(Self {
            registry,
            themes,
            owners,
            working: RefCell::new(Vec::new()),
            done: RefCell::new(HashMap::new()),
        })
    }

    /// Checks every value that a theme gives as a name or a function, whether a rule uses it or
    /// not: that each name it gives is given, that none names itself through others, and that its
    /// functions are given what they take.
    fn check(&self) -> Result<(), DocumentError> {
        let mut names: Vec<(&String, &usize)> = self.owners.iter().collect();
        names.sort();
        for (name, &index) in names {
            let Some(text) = self.text(name) else {
                continue;
            };
            let checked = if text.starts_with('$') || call(&text).is_some() {
                self.resolve(&text)
            } else {
                Ok(())
            };
            if let Err(problem) = checked {
                let theme = &self.themes[index];
                let document =
                    theme
                        .source
                        .parse(self.registry)
                        .map_err(|error| DocumentError {
                            document: theme.name.clone(),
                            error,
                        })?;
                return Err(DocumentError {
                    document: theme.name.clone(),
                    error: document.error_at(&[name], Problem::Invalid(problem)),
                });
            }
        }
        Ok(())
    }

    /// Follows `text`, a name or a function, as far as the theme's texts go: the names it gives
    /// must be given, none twice in one chain, and a function must take what it is given.
    fn resolve(&self, text: &str) -> Result<(), String> {
        let mut seen: Vec<String> = Vec::new();
        let mut text = text.to_owned();
        while let Some(name) = text.strip_prefix('$').map(str::to_owned) {
            if !self.owners.contains_key(&name) {
                return Err(format!("no theme gives a value named `{name}`"));
            }
            if seen.contains(&name) {
                return Err(format!("`{name}` names itself through other values"));
            }
            seen.push(name.clone());
            match self.text(&name) {
                Some(next) => text = next,
                None => return Ok(()),
            }
        }
        if call(&text).is_some() {
            self.color(&text)?;
        }
        Ok(())
    }

    /// The text that the theme that gives `name` holds for it; none for a value that is no text.
    fn text(&self, name: &str) -> Option<String> {
        let theme = &self.themes[*self.owners.get(name)?];
        let mut document = theme.source.parse(self.registry).ok()?;
        match document.read(&[name], "String", None) {
            Ok(Some(Value::Text(text))) => Some(text),
            _ => None,
        }
    }

    /// The value of `name`, of the type named `ty`, as the last theme that gives it holds it, its
    /// own names and functions worked out.
    fn named(&self, name: &str, ty: &str) -> Result<Value, String> {
        let key = (name.to_owned(), ty.to_owned());
        if let Some(value) = self.done.borrow().get(&key) {
            return Ok(value.clone());
        }
        let Some(&index) = self.owners.get(name) else {
            return Err(format!("no theme gives a value named `{name}`"));
        };
        if self.working.borrow().iter().any(|working| working == name) {
            return Err(format!("`{name}` names itself through other values"));
        }
        let theme = &self.themes[index];
        self.working.borrow_mut().push(name.to_owned());
        // A document of its own for each value, so that the values that it names, read while it is
        // read, read theirs apart.
        let read = theme
            .source
            .parse(self.registry)
            .map_err(|error| error.to_string())
            .and_then(|mut document| {
                document
                    .read(&[name], ty, Some(self))
                    .map_err(|error| error.to_string())
            });
        self.working.borrow_mut().pop();
        let value = match read {
            Ok(Some(value)) => value,
            Ok(None) => return Err(format!("no theme gives a value named `{name}`")),
            Err(error) => {
                return Err(format!(
                    "the value `{name}` of the theme `{}`: {error}",
                    theme.name
                ));
            }
        };
        self.done.borrow_mut().insert(key, value.clone());
        Ok(value)
    }

    /// The color that `text` gives: a name, sRGB's hexadecimal digits, or a function.
    fn color(&self, text: &str) -> Result<LinearColor, String> {
        let text = text.trim();
        if let Some(name) = text.strip_prefix('$') {
            let value = self.named(name, "LinearColor")?;
            return LinearColor::from_value(&value)
                .map_err(|_| format!("the value `{name}` is no color"));
        }
        if let Some(hex) = text.strip_prefix('#') {
            return srgb(hex)
                .ok_or_else(|| format!("`{text}` is not a color: `#rrggbb` or `#rrggbbaa`"));
        }
        let Some((function, arguments)) = call(text) else {
            return Err(format!(
                "`{text}` is no color: `#rrggbb`, a value's name, `$name`, or a function such as \
                 `mix($a, $b, 20%)`"
            ));
        };
        match (function, arguments.as_slice()) {
            ("mix", [a, b, share]) => Ok(mix(self.color(a)?, self.color(b)?, self.share(share)?)),
            ("lighten", [color, by]) => Ok(lighten(self.color(color)?, self.share(by)?)),
            ("darken", [color, by]) => Ok(lighten(self.color(color)?, -self.share(by)?)),
            ("alpha", [color, alpha]) => Ok(LinearColor {
                a: self.share(alpha)?.clamp(0.0, 1.0),
                ..self.color(color)?
            }),
            ("mix", _) => Err("`mix` takes two colors and a share: `mix($a, $b, 20%)`".to_owned()),
            ("lighten" | "darken", _) => Err(format!(
                "`{function}` takes a color and a share of lightness: `{function}($a, 0.08)`"
            )),
            ("alpha", _) => {
                Err("`alpha` takes a color and its opacity: `alpha($a, 40%)`".to_owned())
            }
            _ => Err(format!(
                "no color function `{function}`: `mix`, `lighten`, `darken` or `alpha`"
            )),
        }
    }

    /// The share that `text` gives: a number, a percentage, or a value's name.
    fn share(&self, text: &str) -> Result<f32, String> {
        let text = text.trim();
        if let Some(name) = text.strip_prefix('$') {
            // A share that a theme writes as text, `"20%"`, is read as a share; a number, as one.
            if let Some(text) = self.text(name) {
                return self.share(&text);
            }
            return match self.named(name, "f32")? {
                Value::F32(share) => Ok(share),
                _ => Err(format!("the value `{name}` is no number")),
            };
        }
        let (number, scale) = match text.strip_suffix('%') {
            Some(number) => (number, 0.01),
            None => (text, 1.0),
        };
        number
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|share| share.is_finite())
            .map(|share| share * scale)
            .ok_or_else(|| format!("`{text}` is no share: a number, `0.2`, or a percentage, `20%`"))
    }
}

impl References for Values<'_> {
    fn name(&self, _: u64) -> Option<String> {
        None
    }

    fn entity(&self, _: &str) -> Option<u64> {
        None
    }

    fn value(&self, ty: &str, text: &str) -> Option<Result<Value, String>> {
        // A name in place of an option, as a part writes one, stands for the option's value.
        let held = ty
            .strip_prefix("Option<")
            .and_then(|inner| inner.strip_suffix('>'));
        let inner = held.unwrap_or(ty);
        let wrap = |value: Value| {
            if held.is_some() {
                Value::Option(Some(Box::new(value)))
            } else {
                value
            }
        };
        if let Some(name) = text.strip_prefix('$') {
            return Some(self.named(name, inner).map(wrap));
        }
        if inner == "LinearColor" && (text.starts_with('#') || call(text).is_some()) {
            return Some(self.color(text).map(|color| wrap(color.to_value())));
        }
        None
    }
}

/// The function and the arguments of `text` if it calls one, `name(a, b)`: the arguments split at
/// the commas outside their own parentheses.
fn call(text: &str) -> Option<(&str, Vec<&str>)> {
    let text = text.trim();
    let open = text.find('(')?;
    let name = &text[..open];
    let inside = text[open + 1..].strip_suffix(')')?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return None;
    }
    let mut arguments = Vec::new();
    let (mut depth, mut start) = (0_i32, 0);
    for (at, byte) in inside.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                arguments.push(inside[start..at].trim());
                start = at + 1;
            }
            _ => {}
        }
    }
    arguments.push(inside[start..].trim());
    Some((name, arguments))
}

/// The color `share` of the way from `from` to `to` in OKLCH, its hue the shorter way around, a
/// grey taking the other's hue, its alpha straight between theirs.
fn mix(from: LinearColor, to: LinearColor, share: f32) -> LinearColor {
    // Below this chroma a color shows no hue.
    const GREY: f32 = 0.002;
    let (from, to) = (from.to_oklch(), to.to_oklch());
    let (from_hue, to_hue) = match (from.c < GREY, to.c < GREY) {
        (true, false) => (to.h, to.h),
        (false, true) => (from.h, from.h),
        _ => (from.h, to.h),
    };
    let turn = (to_hue - from_hue + 540.0).rem_euclid(360.0) - 180.0;
    let between = |a: f32, b: f32| a + (b - a) * share;
    Oklch {
        l: between(from.l, to.l),
        c: between(from.c, to.c),
        h: (from_hue + turn * share).rem_euclid(360.0),
        a: between(from.a, to.a),
    }
    .to_linear_in_gamut()
}

/// `color` lighter by `by` of OKLCH's lightness, darker for a negative `by`.
fn lighten(color: LinearColor, by: f32) -> LinearColor {
    let color = color.to_oklch();
    Oklch {
        l: (color.l + by).clamp(0.0, 1.0),
        ..color
    }
    .to_linear_in_gamut()
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
