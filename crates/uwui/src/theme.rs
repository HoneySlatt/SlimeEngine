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
//! A rule's part becomes a list of typed *declarations*, one for each field it sets, which the
//! cascade applies in order (`docs/adr/0229-styles-typed-as-declarations.md`). A rule sets a color
//! whole: a part that gives some of a color's channels alone is refused. A rule that names a kind
//! sets only what that kind reads of a style: its layout, its paint and its transition, which every
//! kind reads but for a layer's backdrop, which a container alone reads, as a layer's root; and the
//! parts of a style that it draws, a check box its control's colors and its tick, a label its text.
//! A rule that sets more is refused.
//!
//! Code names a theme's value by a *token* of its kind, a color, a length, a duration or a number,
//! in place of a literal ([`Token`]); a crate lists its tokens and a program declares each list to
//! its interface, which reads each token once, in its kind, when it takes its themes: a value of
//! another kind refuses them, and a token that they lack takes its fallback
//! (`docs/adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md`). The core declares seven
//! tokens, its *roles*, the colors that its components' defaults read, which every look gives
//! under their names (`docs/adr/0230-the-cores-roles-and-each-kinds-defaults.md`):
//! [`CANVAS_TEXT`] for text and the marks beside it, [`GRAY_TEXT`] for a placeholder and a
//! shortcut, [`BUTTON_FACE`] for what a control is drawn on, [`ACCENT`] for what it is filled with,
//! [`HIGHLIGHT`] for a field's selection, [`HIGHLIGHT_TEXT`] for a tick over the accent, [`FOCUS`]
//! for a ring that a component draws itself. Each kind's defaults take them, under every rule.
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
use crate::style::{Style, StyleDeclaration};

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
    Splitter,
    Menu,
    MenuBar,
    MenuItem,
    Choice,
    FloatingPanel,
    SectionHeader,
    ToggleButton,
    Segments,
    Segment,
    Progress,
    ColorWell,
    ColorPart,
    Surface,
}

impl KindName {
    /// Every kind, in the order of the enum, by which [`KindName::index`] numbers them.
    pub(crate) const ALL: [Self; 28] = [
        Self::Container,
        Self::Label,
        Self::Image,
        Self::Button,
        Self::Scroll,
        Self::List,
        Self::TextField,
        Self::NumberField,
        Self::CheckBox,
        Self::Slider,
        Self::Section,
        Self::TabBar,
        Self::Tab,
        Self::Split,
        Self::Splitter,
        Self::Menu,
        Self::MenuBar,
        Self::MenuItem,
        Self::Choice,
        Self::FloatingPanel,
        Self::SectionHeader,
        Self::ToggleButton,
        Self::Segments,
        Self::Segment,
        Self::Progress,
        Self::ColorWell,
        Self::ColorPart,
        Self::Surface,
    ];

    /// Its place in [`KindName::ALL`].
    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

#[cfg(test)]
mod documents_tests;
mod token;

pub use token::{
    AnyToken, Color, Duration, Fallback, Length, Number, Token, TokenError, TokenKind, TokenOr,
    Tokens,
};
pub(crate) use token::{Erased, KindId, Resolved};

/// Text, and the marks drawn beside it: a thumb, a chevron, a tick in a menu. A role, a color that
/// the core declares and every look gives, which its components' defaults read
/// (`docs/adr/0230-the-cores-roles-and-each-kinds-defaults.md`), white when the themes lack it.
pub const CANVAS_TEXT: Token<Color> = Token::new(
    "canvas_text",
    Fallback::Value(LinearColor::rgba(1.0, 1.0, 1.0, 1.0)),
);

/// Text of less weight: a field's placeholder, a number's unit, a menu item's shortcut. A role,
/// grey when the themes lack it, 0.3 in linear light.
pub const GRAY_TEXT: Token<Color> = Token::new(
    "gray_text",
    Fallback::Value(LinearColor::rgba(0.3, 0.3, 0.3, 1.0)),
);

/// What a control is drawn on: a box unchecked, a track, a handle, a floating panel's corner that
/// sizes it. A role, grey when the themes lack it, 0.3 in linear light.
pub const BUTTON_FACE: Token<Color> = Token::new(
    "button_face",
    Fallback::Value(LinearColor::rgba(0.3, 0.3, 0.3, 1.0)),
);

/// What a control is filled with once checked, or up to its value. A role, blue when the themes
/// lack it, (0.2, 0.4, 0.9) in linear light.
pub const ACCENT: Token<Color> = Token::new(
    "accent",
    Fallback::Value(LinearColor::rgba(0.2, 0.4, 0.9, 1.0)),
);

/// The fill behind the text that a field holds selected, drawn at a share of its opacity so that
/// the text, drawn in its own color, still shows through it. A role, [`ACCENT`]'s value when the
/// themes lack it.
pub const HIGHLIGHT: Token<Color> = Token::new("highlight", Fallback::Token(&ACCENT));

/// A mark drawn over [`ACCENT`]: a check box's tick. A role, [`CANVAS_TEXT`]'s value when the
/// themes lack it.
pub const HIGHLIGHT_TEXT: Token<Color> =
    Token::new("highlight_text", Fallback::Token(&CANVAS_TEXT));

/// The ring of what holds the focus, where a component draws one itself. A role, [`ACCENT`]'s
/// value when the themes lack it.
pub const FOCUS: Token<Color> = Token::new("focus", Fallback::Token(&ACCENT));

/// The core's seven roles, which every interface declares, each after the roles that its fallback
/// names.
pub(crate) const ROLES: Tokens = Tokens::new(&[
    &CANVAS_TEXT,
    &GRAY_TEXT,
    &BUTTON_FACE,
    &ACCENT,
    &HIGHLIGHT,
    &HIGHLIGHT_TEXT,
    &FOCUS,
]);

/// How opaque a field's selection is drawn over its text: a share of [`HIGHLIGHT`]'s alpha, so
/// that the text, drawn in its own color, still shows through it.
pub(crate) const SELECTION_OPACITY: f32 = 0.4;

/// The value of each token that an interface declared, read from the themes or taken from its
/// fallback, by its name.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TokenValues(HashMap<&'static str, Resolved>);

impl TokenValues {
    /// Every token of `declared` at its fallback: what an interface given no theme draws in.
    pub(crate) fn fallbacks(declared: &[Erased]) -> Self {
        let mut values = Self::default();
        for token in declared {
            let value = token.fallback_value(&|name, kind| values.get(name, kind));
            values.0.insert(token.name, value);
        }
        values
    }

    /// The value of the token `name` of `kind`; none where no token of that name and kind was
    /// declared.
    pub(crate) fn get(&self, name: &str, kind: KindId) -> Option<Resolved> {
        self.0
            .get(name)
            .copied()
            .filter(|value| value.kind() == kind)
    }

    /// The value of the color `token`, its fallback's where it was not declared.
    pub(crate) fn color(&self, token: &Token<Color>) -> LinearColor {
        let value = self.value(&token.erased());
        match value {
            Resolved::Color(color) => color,
            _ => unreachable!("a color token's value is a color"),
        }
    }

    /// The value of `token`, its fallback's where it was not declared.
    pub(crate) fn value(&self, token: &Erased) -> Resolved {
        self.get(token.name, token.kind)
            .unwrap_or_else(|| token.fallback_value(&|name, kind| self.get(name, kind)))
    }
}

/// Whether an element of `kind` reads the field that `declaration` sets, as [`read_by`] lists it
/// (`docs/adr/0229-styles-typed-as-declarations.md`).
pub(crate) fn reads(kind: KindName, declaration: &StyleDeclaration) -> bool {
    let path = declaration.path();
    if path == "paint.backdrop" {
        return kind == KindName::Container;
    }
    if ["layout.", "paint.", "transition."]
        .iter()
        .any(|part| path.starts_with(part))
    {
        return true;
    }
    // `text` stands for the text's style and its color, which every text is drawn in.
    let leaf = if path.starts_with("text.style.") || path == "text.color" {
        "text"
    } else {
        path.as_str()
    };
    read_by(kind).iter().any(|(read, _)| *read == leaf)
}

/// The style that an element of `kind` starts from, under every rule: its colors from the roles'
/// values in `tokens`, as [`read_by`] gives them, and [`Style::default`]'s for everything else
/// (`docs/adr/0230-the-cores-roles-and-each-kinds-defaults.md`).
pub(crate) fn base(kind: KindName, tokens: &TokenValues) -> Style {
    let mut style = Style::default();
    for &(leaf, role) in read_by(kind) {
        let Some(role) = role else {
            continue;
        };
        let color = tokens.color(&role);
        match leaf {
            "text" => style.text.color = color,
            "text.selection" => {
                style.text.selection = LinearColor {
                    a: color.a * SELECTION_OPACITY,
                    ..color
                };
            }
            "control.track" => style.control.track = color,
            "control.fill" => style.control.fill = color,
            "control.mark" => style.control.mark = color,
            _ => unreachable!("`{leaf}` is no color, and names no role"),
        }
    }
    style
}

/// The fields of a style that an element of `kind` reads, beyond its layout, its paint and its
/// transition, which every kind reads, and a layer's backdrop, which a layer's root reads, always a
/// container: `text` for its text's style and color, then each of the selection's, the control's
/// and the icons' that it draws, in some of its states. A rule that names the kind and sets another
/// is refused. Each color among them holds the role that the kind's defaults give it ([`base`]),
/// the others none.
pub(crate) fn read_by(kind: KindName) -> &'static [(&'static str, Option<Token<Color>>)] {
    const SECTION: &[(&str, Option<Token<Color>>)] = &[
        ("text", Some(CANVAS_TEXT)),
        ("control.size", None),
        ("control.radius", None),
        ("control.track", Some(BUTTON_FACE)),
        ("control.mark", Some(CANVAS_TEXT)),
        ("icons.font", None),
        ("icons.down", None),
        ("icons.right", None),
        ("icons.left", None),
    ];
    const BARS: &[(&str, Option<Token<Color>>)] = &[
        ("control.size", None),
        ("control.radius", None),
        ("control.track", Some(BUTTON_FACE)),
        ("control.mark", Some(CANVAS_TEXT)),
    ];
    match kind {
        KindName::Label | KindName::Button | KindName::ToggleButton | KindName::Segment => {
            &[("text", Some(CANVAS_TEXT))]
        }
        KindName::Container
        | KindName::Image
        | KindName::TabBar
        | KindName::Splitter
        | KindName::Menu
        | KindName::MenuBar
        | KindName::SectionHeader
        | KindName::Segments
        | KindName::Surface => &[],
        KindName::Scroll | KindName::List => BARS,
        KindName::TextField => &[
            ("text", Some(CANVAS_TEXT)),
            ("text.selection", Some(HIGHLIGHT)),
            ("control.size", None),
            ("control.track", Some(GRAY_TEXT)),
            ("control.mark", Some(CANVAS_TEXT)),
            ("icons.font", None),
            ("icons.search", None),
            ("icons.clear", None),
        ],
        KindName::NumberField => &[
            ("text", Some(CANVAS_TEXT)),
            ("text.selection", Some(HIGHLIGHT)),
            ("control.size", None),
            ("control.track", Some(GRAY_TEXT)),
            ("control.mark", Some(CANVAS_TEXT)),
        ],
        KindName::CheckBox => &[
            ("text", Some(CANVAS_TEXT)),
            ("control.size", None),
            ("control.radius", None),
            ("control.track", Some(BUTTON_FACE)),
            ("control.fill", Some(ACCENT)),
            ("control.mark", Some(HIGHLIGHT_TEXT)),
            ("icons.font", None),
            ("icons.tick", None),
        ],
        KindName::Slider => &[
            ("control.size", None),
            ("control.radius", None),
            ("control.track", Some(BUTTON_FACE)),
            ("control.fill", Some(ACCENT)),
            ("control.mark", Some(CANVAS_TEXT)),
        ],
        KindName::Section | KindName::FloatingPanel => SECTION,
        KindName::Tab => &[
            ("text", Some(CANVAS_TEXT)),
            ("control.size", None),
            ("control.track", Some(BUTTON_FACE)),
            ("control.mark", Some(CANVAS_TEXT)),
        ],
        KindName::Split => &[
            ("control.size", None),
            ("control.track", Some(BUTTON_FACE)),
            ("control.fill", Some(ACCENT)),
        ],
        KindName::MenuItem => &[
            ("text", Some(CANVAS_TEXT)),
            ("control.size", None),
            ("control.track", Some(GRAY_TEXT)),
            ("control.mark", Some(CANVAS_TEXT)),
            ("icons.font", None),
            ("icons.tick", None),
            ("icons.right", None),
            ("icons.left", None),
        ],
        KindName::Choice => &[
            ("text", Some(CANVAS_TEXT)),
            ("control.size", None),
            ("control.mark", Some(CANVAS_TEXT)),
            ("icons.font", None),
            ("icons.down", None),
        ],
        KindName::Progress => &[
            ("control.size", None),
            ("control.radius", None),
            ("control.track", Some(BUTTON_FACE)),
            ("control.fill", Some(ACCENT)),
        ],
        KindName::ColorWell => &[("control.size", None), ("control.radius", None)],
        KindName::ColorPart => &[("control.radius", None), ("control.fill", Some(FOCUS))],
    }
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
    /// What it sets of a [`Style`], in the order of its document
    /// (`docs/adr/0229-styles-typed-as-declarations.md`).
    pub(crate) declarations: Vec<StyleDeclaration>,
}

/// The registry of the types that style sheets and themes hold.
pub(crate) fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register::<Style>();
    registry.register::<Selector>();
    registry
}

/// What an interface is styled with: its rules, and the tokens that the themes lacked.
pub(crate) struct Styling {
    /// The rules of the style sheets, their themes' values in them.
    pub(crate) rules: Vec<Rule>,
    /// The tokens that the themes lack, which took their fallbacks, in the order declared.
    pub(crate) lacking: Vec<&'static str>,
}

/// Reads the rules of `sheets`, in their order and the order of their files, their names replaced
/// by the values of `themes`, the last that gives a name giving its value; and the value of each
/// token of `declared`, read once, in its kind, into `tokens`, emptied first, whose room it keeps
/// (`docs/adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md`). The themes are read for
/// the tokens declared alone: a token that they lack takes its fallback, and a fallback that names
/// a token takes that token's value if it was declared, else its own fallback.
pub(crate) fn read(
    registry: &Registry,
    sheets: &[StyleSheet],
    themes: &[Theme],
    declared: &[Erased],
    tokens: &mut TokenValues,
) -> Result<Styling, DocumentError> {
    let values = Values::new(registry, themes)?;
    values.check()?;
    let mut lacking = Vec::new();
    tokens.0.clear();
    // The values that the themes give first, so that a fallback finds them whatever their order.
    for token in declared {
        match values.token(token.name, token.kind) {
            Some(Ok(value)) => {
                tokens.0.insert(token.name, value);
            }
            Some(Err(problem)) => {
                let what = if ROLES.each().any(|role| role.name == token.name) {
                    "a role, a color that the components read".to_owned()
                } else {
                    format!("a token, a {} that code reads", token.kind.name())
                };
                return Err(values.fault(
                    values.owners[token.name],
                    token.name,
                    format!("`{}` is {what}: {problem}", token.name),
                ));
            }
            None => lacking.push(token.name),
        }
    }
    for &name in &lacking {
        let token = declared
            .iter()
            .find(|token| token.name == name)
            .expect("lacking, among the tokens declared");
        let value = token.fallback_value(&|name, kind| tokens.get(name, kind));
        tokens.0.insert(name, value);
    }
    // What every rule's part is laid over, to read the values that it sets whole.
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
            let refuse = |problem: String| {
                sheet_error(document.error_at(&[&name, "style"], Problem::Invalid(problem)))
            };
            let declarations = declarations(registry, &default, &style).map_err(refuse)?;
            if let Some(kind) = selector.kind
                && let Some(unread) = declarations
                    .iter()
                    .find(|declaration| !reads(kind, declaration))
            {
                return Err(refuse(format!(
                    "a rule for `{kind:?}` sets `{}`, which a `{kind:?}` does not read",
                    unread.path()
                )));
            }
            for key in document.keys(&[&name]) {
                if key != "select" && key != "style" {
                    return Err(sheet_error(document.error_at(
                        &[&name, &key],
                        Problem::Invalid(format!("a rule holds `select` and `style`, not `{key}`")),
                    )));
                }
            }
            rules.push(Rule {
                selector,
                declarations,
            });
        }
    }
    Ok(Styling { rules, lacking })
}

/// The declarations of what `part` sets, a part of a style as reflection reads one, in its order,
/// each holding the value that the part laid over `default`, the default style's value, gives it.
///
/// # Errors
///
/// What is wrong, if the part sets some of a color's channels and not all, or leaves out a field of
/// what it sets whole, an option's value, without a default to give it: such a part lays over no
/// style, and is refused when the interface reads it, not when a frame styles.
pub(crate) fn declarations(
    registry: &Registry,
    default: &Value,
    part: &Value,
) -> Result<Vec<StyleDeclaration>, String> {
    if let Some(path) = partial_color(registry, "UiStyle", part, "") {
        return Err(format!(
            "`{path}` sets some of a color's channels: a rule sets a color whole, as \
             `\"#rrggbb\"`, a value's name or `{{ r, g, b, a }}`"
        ));
    }
    let mut whole = default.clone();
    registry
        .overlay("UiStyle", &mut whole, part)
        .map_err(|error| error.to_string())?;
    let whole = Style::from_value(&whole).expect("a style, read as one");
    let mut declarations = Vec::new();
    whole.named(part, &mut |declaration| declarations.push(declaration));
    Ok(declarations)
}

/// The path of the first color that `part`, a part of a value of the type `ty` at `path`, sets some
/// of the channels of and not all; none when it sets every color it holds whole.
fn partial_color(registry: &Registry, ty: &str, part: &Value, path: &str) -> Option<String> {
    let inner = ty
        .strip_prefix("Option<")
        .and_then(|inner| inner.strip_suffix('>'));
    if let (Some(inner), Value::Option(Some(held))) = (inner, part) {
        return partial_color(registry, inner, held, path);
    }
    let Value::Struct(parts) = part else {
        return None;
    };
    if ty == "LinearColor" {
        return (parts.len() < 4).then(|| path.to_owned());
    }
    let slime_reflect::Kind::Struct(fields) = &registry.get(ty)?.info().kind else {
        return None;
    };
    parts.iter().find_map(|(name, part)| {
        let field = fields.iter().find(|field| field.name == *name)?;
        let path = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}.{name}")
        };
        partial_color(registry, &field.ty, part, &path)
    })
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
                return Err(self.fault(index, name, problem));
            }
        }
        Ok(())
    }

    /// The value that the themes give the token `name`, read in its kind; none where they do not
    /// give it.
    ///
    /// # Errors
    ///
    /// What is wrong, if its value is not of its kind, holds a number that is not finite, or is a
    /// negative duration: such a value would reach every element that names the token, and make
    /// each style differ from itself.
    fn token(&self, name: &str, kind: KindId) -> Option<Result<Resolved, String>> {
        if !self.owners.contains_key(name) {
            return None;
        }
        let number = |value: Value| match value {
            // `named` refuses a number that is not finite.
            Value::F32(number) => Ok(number),
            _ => Err(format!("the value `{name}` is no number")),
        };
        Some(match kind {
            KindId::Color => self
                .named(name, "LinearColor")
                .and_then(|value| {
                    LinearColor::from_value(&value)
                        .map_err(|_| format!("the value `{name}` is no color"))
                })
                .and_then(|color| {
                    [color.r, color.g, color.b, color.a]
                        .iter()
                        .all(|channel| channel.is_finite())
                        .then_some(Resolved::Color(color))
                        .ok_or_else(|| {
                            format!("the color `{name}` has a channel that is no finite number")
                        })
                }),
            KindId::Length => self
                .named(name, "f32")
                .and_then(number)
                .map(Resolved::Length),
            KindId::Number => self
                .named(name, "f32")
                .and_then(number)
                .map(Resolved::Number),
            KindId::Duration => self
                .named(name, "f32")
                .and_then(number)
                .and_then(|seconds| {
                    std::time::Duration::try_from_secs_f32(seconds)
                        .map(Resolved::Duration)
                        .map_err(|_| {
                            if seconds < 0.0 {
                                format!("the duration `{name}` is negative: a time in seconds")
                            } else {
                                format!("the duration `{name}` is too long to hold, in seconds")
                            }
                        })
                }),
        })
    }

    /// `problem` placed at the value `name` of the theme of `index`.
    fn fault(&self, index: usize, name: &str, problem: String) -> DocumentError {
        let theme = &self.themes[index];
        let error = match theme.source.parse(self.registry) {
            Ok(document) => document.error_at(&[name], Problem::Invalid(problem)),
            Err(error) => error,
        };
        DocumentError {
            document: theme.name.clone(),
            error,
        }
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
        // A number that is not finite would reach every element that a rule naming it styles,
        // and make each style differ from itself.
        if matches!(value, Value::F32(number) if !number.is_finite())
            || matches!(value, Value::F64(number) if !number.is_finite())
        {
            return Err(format!("the value `{name}` is no finite number"));
        }
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
