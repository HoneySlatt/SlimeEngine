//! Style sheets and themes: documents that style an interface's elements
//! (`docs/adr/0141-style-sheets-and-themes.md`).
//!
//! A *style sheet* is a document of KDL that holds rules, each a node named as its author likes,
//! applied in the order of the file: what elements it selects, `select`, and the parts of their
//! style that it sets, in the groups of a [`Style`], `layout`, `paint`, `text`, `control`,
//! `transition` and `icons`; what it leaves out it leaves as it is. Changing the theme changes the
//! values that the rules give, the rules unchanged. Its notation follows one set of rules
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`):
//!
//! - a field that holds a value is a property, `border_width=1`; a struct is a child node named by
//!   its field, `padding start=4 end=4`; an array or a list of values is a child node whose
//!   arguments are its elements, `radii 4 4 4 4`;
//! - an enum's variant is an identifier, `direction=Column`; an option left out is left as it is,
//!   and `#null` empties it, a shadow too, `shadow #null`; a shadow is set whole, what it leaves out
//!   taking [`Shadow::default`](crate::style::Shadow)'s value;
//! - a number is a length, in logical pixels, bare or `(px)`, a duration, which writes its unit,
//!   `(ms)120` or `(s)0.12`, never negative, or a number, as its field takes it; a size is a length,
//!   a share of its parent's, `(%)50`, or `Auto`, and a grid's track a length, a share of the space
//!   left, `(fr)1`, or `Auto`;
//! - a color is sRGB's `"#rrggbb"` or `"#rrggbbaa"`, or a color function, below;
//! - `$name` names a theme's value of the field's kind wherever a value goes, a length where a size
//!   or a track goes, a text in a field of text, which writes `$$` for a text that starts with `$`;
//! - a theme's value is never named in a name, a class, a key, a kind or a state: a name that
//!   starts with `$` is refused; a numbered key is a whole number, `key=3`, and a key's name never
//!   starts with `#`.
//!
//! A rule, a group or a field given twice is refused.
//!
//! A *theme* is a document of KDL that holds named values in groups, each the kind of its values,
//! as the design tokens of the W3C's community group give a group its type: `color`, `length` in
//! logical pixels, bare or `(px)`, `duration`, `(ms)120` or `(s)0.12`, never negative, `number`,
//! and `text`, a text that starts with `$` written `$$`. A name is given once in a theme, in one
//! group. A value names another of its kind, `$name`, which takes the unit of its group and writes
//! none.
//!
//! A rule's fields become a list of typed *declarations*, one for each field it sets, read by the
//! code that `uwui-macros`'s derives generate, which the cascade applies in order
//! (`docs/adr/0229-styles-typed-as-declarations.md`). A rule that names a kind
//! sets only what that kind reads of a style: its layout, its paint and its transition, which every
//! kind reads but for a layer's backdrop, which a container alone reads, as a layer's root; and the
//! parts of a style that it draws, a check box its control's colors and its tick, a label its text.
//! A rule that sets more is refused.
//!
//! Code names a theme's value by a *token* of its kind, a color, a length, a duration or a number,
//! in place of a literal ([`Token`]); a crate lists its tokens and a program declares each list to
//! its interface, which reads each token once, in its kind, when it takes its themes: a value of
//! another group refuses them, and a token that they lack takes its fallback
//! (`docs/adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md`). The core declares seven
//! tokens, its *roles*, the colors that its components' defaults read, which every look gives
//! under their names (`docs/adr/0230-the-cores-roles-and-each-kinds-defaults.md`):
//! [`CANVAS_TEXT`] for text and the marks beside it, [`GRAY_TEXT`] for a placeholder and a
//! shortcut, [`BUTTON_FACE`] for what a control is drawn on, [`ACCENT`] for what it is filled with,
//! [`HIGHLIGHT`] for a field's selection, [`HIGHLIGHT_TEXT`] for a tick over the accent, [`FOCUS`]
//! for a ring that a component draws itself. Each kind's defaults take them, under every rule.
//!
//! A color may be computed by a function, in OKLCH, where equal steps look equal
//! (`docs/adr/0149-the-looks-foundations.md`): `mix($a, $b, 20%)`, the color a fifth of the way
//! from `$a` to `$b`, its hue going the shorter way around, a grey taking the other's hue;
//! `lighten($a, 0.08)` and `darken($a, 0.08)`, the color lighter or darker by that much of OKLCH's
//! lightness, from 0 to 1; `alpha($a, 40%)`, the color of that opacity. A color that leaves sRGB
//! loses chroma until it fits, its lightness and hue kept. A share is a number, `0.2`, a
//! percentage, `20%`, or the name of a `number`. Functions nest eight deep at most.
//!
//! Themes stack: of several themes, the last that gives a name gives its value, and the values
//! that name it follow it. The first theme to give a name sets its kind: a later theme that gives
//! it in another group is refused. The stack's names are followed once, without recursion, a chain
//! of names 64 long at most, each named value's chain counted whatever the order of the names. A
//! color that is no finite number, as a function given a share without bounds computes, is
//! refused. Each error is placed at its line and column, and gathered, in the order that
//! [`DocumentErrors`] gives.
//!
//! ```kdl
//! // A style sheet.
//! panel {
//!     select { classes panel; }
//!     paint background=$surface {
//!         radii $radius $radius $radius $radius
//!     }
//! }
//! toolbar-button-pressed {
//!     select kind=Button {
//!         states Pressed
//!         inside class=toolbar
//!     }
//!     paint background=$accent
//!     transition duration=$fast
//! }
//! ```
//!
//! ```kdl
//! // A theme.
//! color {
//!     surface "#26282C"
//!     accent "#4C6EF5"
//!     accent_hover "lighten($accent, 0.06)"
//!     accent_soft "alpha($accent, 20%)"
//! }
//! length {
//!     radius 8
//! }
//! duration {
//!     fast (ms)120
//! }
//! ```
//!
//! ```kdl
//! // A theme stacked after it: the accent changes, and its hover and soft fill with it.
//! color {
//!     accent "#2BB673"
//! }
//! ```

use std::collections::HashMap;

use slime_color::{LinearColor, Oklch};
use slime_reflect::Reflect;
#[cfg(test)]
use slime_reflect::Registry;
#[cfg(test)]
use slime_reflect::Value;
use uwui_macros::Identifiers;

use crate::document::{DocumentError, DocumentErrors};
use crate::style::{Style, StyleDeclaration};

/// A style sheet: rules that select elements and set a part of their style, in the order they are
/// applied, read when an interface takes it ([`Ui::set_style`](crate::tree::Ui::set_style)).
#[derive(Clone, Debug)]
pub struct StyleSheet {
    name: String,
    text: String,
}

/// A theme: the values that the rules of style sheets name, a document of KDL.
#[derive(Clone, Debug)]
pub struct Theme {
    name: String,
    text: String,
}

impl StyleSheet {
    /// The style sheet that `text` holds, named `name` in its errors: its file, usually. A cooked
    /// style sheet is its text too, checked by the cooker
    /// (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
    pub fn from_text(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
        }
    }

    /// Its name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Theme {
    /// The theme that `text` holds, named `name` in its errors: its file, usually. A cooked theme
    /// is its text too, checked by the cooker
    /// (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
    pub fn from_text(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Identifiers)]
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
#[cfg(test)]
mod reflected_tests;
pub(crate) mod sheet;
mod token;
mod values;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Identifiers)]
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

/// The registry of the types that style sheets hold, as reflection reads them: the oracle's of the
/// tests while reflection is in uwui (M6d.3c).
#[cfg(test)]
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
/// (`docs/adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md`). A token that the themes
/// lack takes its fallback, and a fallback that names a token takes that token's value if it was
/// declared, else its own fallback.
///
/// The themes are read first, by [`values`], then the sheets, by [`sheet`], each error gathered
/// (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
pub(crate) fn read(
    sheets: &[StyleSheet],
    themes: &[Theme],
    declared: &[Erased],
    tokens: &mut TokenValues,
) -> Result<Styling, DocumentErrors> {
    let texts: Vec<(&str, &str)> = themes
        .iter()
        .map(|theme| (theme.name.as_str(), theme.text.as_str()))
        .collect();
    let values = values::read(&texts)?;
    let mut errors = DocumentErrors::new();
    let mut lacking = Vec::new();
    tokens.0.clear();
    // The values that the themes give first, so that a fallback finds them whatever their order.
    for token in declared {
        let Some(value) = values.get(token.name) else {
            lacking.push(token.name);
            continue;
        };
        match token_value(value, token.kind) {
            Some(value) => {
                tokens.0.insert(token.name, value);
            }
            None => {
                let what = if ROLES.each().any(|role| role.name == token.name) {
                    "a role, a color that the components read".to_owned()
                } else {
                    format!("a token, a {} that code reads", token.kind.name())
                };
                let (theme, offset) = values
                    .place(token.name)
                    .expect("a value given has its place");
                let (name, text) = texts[theme];
                errors.push(DocumentError::at(
                    name,
                    text,
                    offset,
                    format!(
                        "`{}` is {what}: the themes give it as {}",
                        token.name,
                        value.group().a()
                    ),
                ));
            }
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
    let mut rules = Vec::new();
    for style_sheet in sheets {
        rules.extend(sheet::read(
            &style_sheet.name,
            &style_sheet.text,
            &values,
            &mut errors,
        ));
    }
    if errors.is_empty() {
        Ok(Styling { rules, lacking })
    } else {
        Err(errors)
    }
}

/// The value of a token of `kind` that `value` gives; none if it is of another kind.
fn token_value(value: &values::ThemeValue, kind: KindId) -> Option<Resolved> {
    use values::ThemeValue;
    match (value, kind) {
        (ThemeValue::Color(color), KindId::Color) => Some(Resolved::Color(*color)),
        (ThemeValue::Length(length), KindId::Length) => Some(Resolved::Length(*length)),
        (ThemeValue::Number(number), KindId::Number) => Some(Resolved::Number(*number)),
        (ThemeValue::Duration(seconds), KindId::Duration) => {
            // A theme's duration is never negative and fits, as its reader checked.
            std::time::Duration::try_from_secs_f32(*seconds)
                .ok()
                .map(Resolved::Duration)
        }
        _ => None,
    }
}

/// The declarations of what `part` sets, a part of a style as reflection reads one, in its order,
/// each holding the value that the part laid over `default`, the default style's value, gives it.
///
/// # Errors
///
/// What is wrong, if the part sets some of a color's channels and not all, or leaves out a field of
/// what it sets whole, an option's value, without a default to give it: such a part lays over no
/// style. The reflected reader's, kept as the oracle of the tests while reflection is in uwui.
#[cfg(test)]
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
#[cfg(test)]
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

/// The function and the arguments of `text` if it calls one, `name(a, b)`: the arguments split at
/// the commas outside their own parentheses.
pub(crate) fn call(text: &str) -> Option<(&str, Vec<&str>)> {
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
pub(crate) fn mix(from: LinearColor, to: LinearColor, share: f32) -> LinearColor {
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
pub(crate) fn lighten(color: LinearColor, by: f32) -> LinearColor {
    let color = color.to_oklch();
    Oklch {
        l: (color.l + by).clamp(0.0, 1.0),
        ..color
    }
    .to_linear_in_gamut()
}

/// The color that `hex`, six or eight hexadecimal digits, gives in sRGB, its alpha straight.
pub(crate) fn srgb(hex: &str) -> Option<LinearColor> {
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
