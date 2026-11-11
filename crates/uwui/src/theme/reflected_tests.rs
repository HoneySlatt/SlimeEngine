//! The reader of themes in TOML through reflection that M6d.3b1 replaced, kept as the oracle of
//! the reader of KDL while reflection is in uwui
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`): random stacks of
//! themes, written in both notations, give the same tokens and the same rules' declarations, bit
//! for bit. Its sheets are TOML, read as `theme::read` reads them until M6d.3b2.

use std::cell::RefCell;
use std::collections::HashMap;

use slime_color::LinearColor;
use slime_reflect::{Problem, Reflect, Registry, Value};
use slime_serialize::{Document, Mode, ReadError, References};

use super::{
    Erased, KindId, ROLES, Resolved, Rule, Selector, StyleSheet, Styling, TokenValues, call,
    declarations, lighten, mix, reads, srgb,
};
use crate::document::{DocumentError, Fault};
use crate::style::Style;

/// The document of TOML that `text` holds, parsed strictly with the types of `registry`.
fn parse<'a>(registry: &'a Registry, text: &'a str) -> Result<Document<'a>, ReadError> {
    Document::parse(registry, text, Mode::Strict)
}

/// Reads `sheets` and `themes`, a stack of themes in TOML, each `(name, text)`, as `theme::read`
/// read them before M6d.3b1.
pub(super) fn read(
    registry: &Registry,
    sheets: &[StyleSheet],
    themes: &[(&str, &str)],
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
            error: Fault::Reflected(error),
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

/// The values of stacked themes, worked out as the rules and the themes name them: `"$name"` for a
/// value, `"#rrggbb"` and `"#rrggbbaa"` for a color in sRGB, its alpha straight, and the color
/// functions.
struct Values<'a> {
    registry: &'a Registry,
    themes: &'a [(&'a str, &'a str)],
    /// The theme that gives each name: the last that holds it.
    owners: HashMap<String, usize>,
    /// The names being worked out, innermost last: one named again names itself.
    working: RefCell<Vec<String>>,
    /// The values worked out, by name and type, each worked out once.
    done: RefCell<HashMap<(String, String), Value>>,
}

impl<'a> Values<'a> {
    /// The values of `themes`, each parsed to learn the names it gives.
    fn new(
        registry: &'a Registry,
        themes: &'a [(&'a str, &'a str)],
    ) -> Result<Self, DocumentError> {
        let mut owners = HashMap::new();
        for (index, theme) in themes.iter().enumerate() {
            let document = parse(registry, theme.1).map_err(|error| DocumentError {
                document: theme.0.to_owned(),
                error: Fault::Reflected(error),
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
        let error = match parse(self.registry, theme.1) {
            Ok(document) => document.error_at(&[name], Problem::Invalid(problem)),
            Err(error) => error,
        };
        DocumentError {
            document: theme.0.to_owned(),
            error: Fault::Reflected(error),
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
        let mut document = parse(self.registry, theme.1).ok()?;
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
        let read = parse(self.registry, theme.1)
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
                    theme.0
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

/// A generator of numbers that repeats from its seed: xorshift.
struct Numbers(u64);

impl Numbers {
    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % bound as u64) as usize
    }

    fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }

    fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
        from[self.below(from.len())]
    }
}

/// The names that random themes give, by kind: a name keeps its kind in every theme of a stack.
const COLORS: [&str; 6] = ["c0", "c1", "c2", "c3", "c4", "c5"];
const LENGTHS: [&str; 3] = ["l0", "l1", "l2"];
const NUMBERS: [&str; 3] = ["n0", "n1", "n2"];
const DURATIONS: [&str; 2] = ["d0", "d1"];
const TEXTS: [&str; 2] = ["t0", "t1"];

/// A share, as a colour function writes one: a number, a percentage, or a number's name.
fn random_share(numbers: &mut Numbers) -> String {
    match numbers.below(3) {
        0 => numbers.pick(&["0", "0.2", "0.035", "1", "1.5"]).to_owned(),
        1 => numbers
            .pick(&["20%", "35%", "0%", "100%", "22.5%"])
            .to_owned(),
        _ => format!("${}", numbers.pick(&NUMBERS)),
    }
}

/// The name of a value of `names` that another, the one at `place`, names: mostly one before it,
/// so that most stacks hold no cycle of names, and now and then any.
fn random_name(numbers: &mut Numbers, names: &[&'static str], place: usize) -> &'static str {
    if place == 0 || numbers.chance(10) {
        names[numbers.below(names.len())]
    } else {
        names[numbers.below(place)]
    }
}

/// A color, as a theme writes one for the color at `place`, its functions nested `depth` deep at
/// most.
fn random_color(numbers: &mut Numbers, place: usize, depth: usize) -> String {
    match numbers.below(if depth == 0 { 2 } else { 3 }) {
        0 => {
            let hex: String = (0..if numbers.chance(2) { 6 } else { 8 })
                .map(|_| char::from(b"0123456789ABCDEF"[numbers.below(16)]))
                .collect();
            format!("#{hex}")
        }
        1 => format!("${}", random_name(numbers, &COLORS, place)),
        _ => {
            let color = random_color(numbers, place, depth - 1);
            let share = random_share(numbers);
            match numbers.below(4) {
                0 => format!(
                    "mix({color}, {}, {share})",
                    random_color(numbers, place, depth - 1)
                ),
                1 => format!("lighten({color}, {share})"),
                2 => format!("darken({color}, {share})"),
                _ => format!("alpha({color}, {share})"),
            }
        }
    }
}

/// A random theme, the same values in TOML and in KDL, and the names it gives.
fn random_theme(numbers: &mut Numbers) -> (String, String, Vec<&'static str>) {
    let mut toml = String::new();
    let mut given = Vec::new();
    let mut groups: Vec<(&str, Vec<String>)> = Vec::new();
    let mut group = |name: &'static str, lines: Vec<String>| {
        if !lines.is_empty() {
            groups.push((name, lines));
        }
    };
    let mut colors = Vec::new();
    for (place, name) in COLORS.into_iter().enumerate() {
        if !numbers.chance(5) {
            let value = random_color(numbers, place, 2);
            toml.push_str(&format!("{name} = \"{value}\"\n"));
            let kdl = match value.strip_prefix('$') {
                Some(_) => value.clone(),
                None => format!("\"{value}\""),
            };
            colors.push(format!("{name} {kdl}"));
            given.push(name);
        }
    }
    group("color", colors);
    // Lengths, numbers and durations: a literal, or a name of their kind.
    // A number does not name another: the reflected reader follows a share's names by recursion,
    // and overflows its stack on a cycle of them.
    let mut numeric = |names: &[&'static str],
                       literals: &[&str],
                       unit: &str,
                       may_name: bool,
                       given: &mut Vec<&'static str>| {
        let mut lines = Vec::new();
        for (place, &name) in names.iter().enumerate() {
            if !numbers.chance(5) {
                if may_name && numbers.chance(3) {
                    let other = random_name(numbers, names, place);
                    toml.push_str(&format!("{name} = \"${other}\"\n"));
                    lines.push(format!("{name} ${other}"));
                } else {
                    let literal = numbers.pick(literals);
                    toml.push_str(&format!("{name} = {literal}\n"));
                    lines.push(format!("{name} {unit}{literal}"));
                }
                given.push(name);
            }
        }
        lines
    };
    let lengths = numeric(
        &LENGTHS,
        &["0", "1", "2.5", "12", "-3", "11.5"],
        "",
        true,
        &mut given,
    );
    let numbers_given = numeric(
        &NUMBERS,
        &["0", "0.5", "500", "1", "0.08"],
        "",
        false,
        &mut given,
    );
    let durations = numeric(
        &DURATIONS,
        &["0", "0.12", "0.2", "1.5"],
        "(s)",
        true,
        &mut given,
    );
    group("length", lengths);
    group("number", numbers_given);
    group("duration", durations);
    let mut texts = Vec::new();
    for name in TEXTS {
        if numbers.chance(2) {
            toml.push_str(&format!("{name} = \"tick\"\n"));
            texts.push(format!("{name} tick"));
        }
    }
    group("text", texts);
    let kdl = groups
        .iter()
        .map(|(group, lines)| format!("{group} {{\n    {}\n}}\n", lines.join("\n    ")))
        .collect();
    (toml, kdl, given)
}

/// A style sheet whose rules name each of `names`, in a field of its kind.
fn naming_sheet(names: &[&str]) -> String {
    let mut sheet = String::new();
    for name in names {
        let field = match &name[..1] {
            "c" => "paint.background",
            "l" => "paint.border_width",
            "n" => "paint.opacity",
            "d" => "transition.duration",
            _ => continue,
        };
        sheet.push_str(&format!(
            "[uses_{name}]\nselect = {{}}\nstyle.{field} = \"${name}\"\n"
        ));
    }
    sheet
}

/// A token of each name in `names`, of its kind, falling back on nothing.
fn tokens_of(names: &[&'static str]) -> Vec<Erased> {
    use super::{Color, Duration, Fallback, Length, Number, Token};
    names
        .iter()
        .filter_map(|&name| match &name[..1] {
            "c" => Some(
                Token::<Color>::new(name, Fallback::Value(LinearColor::rgba(0.0, 0.0, 0.0, 0.0)))
                    .erased(),
            ),
            "l" => Some(Token::<Length>::new(name, Fallback::Value(0.0)).erased()),
            "n" => Some(Token::<Number>::new(name, Fallback::Value(0.0)).erased()),
            "d" => Some(
                Token::<Duration>::new(name, Fallback::Value(std::time::Duration::ZERO)).erased(),
            ),
            _ => None,
        })
        .collect()
}

#[test]
fn random_stacks_of_themes_read_as_the_reflected_reader_read_them() {
    use crate::style::Same;
    let seed = std::env::var("UWUI_THEME_SEED")
        .ok()
        .and_then(|seed| seed.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |time| time.as_nanos() as u64)
                | 1
        });
    let mut numbers = Numbers(seed);
    let registry = super::registry();
    let (mut agreed, mut refused) = (0, 0);
    for case in 0..2_000 {
        let mut toml = Vec::new();
        let mut kdl = Vec::new();
        let mut given: Vec<&'static str> = Vec::new();
        for _ in 0..1 + numbers.below(3) {
            let (toml_text, kdl_text, names) = random_theme(&mut numbers);
            toml.push(toml_text);
            kdl.push(kdl_text);
            given.extend(names);
        }
        given.sort_unstable();
        given.dedup();
        // Some names that no theme gives, which the tokens lack and no rule names.
        let declared = tokens_of(&[given.as_slice(), &["c5", "l2", "d1"]].concat());
        let sheet = [StyleSheet::from_text("random.style", naming_sheet(&given))];
        let toml_themes: Vec<(&str, &str)> = toml
            .iter()
            .map(|text| ("random.theme", text.as_str()))
            .collect();
        let kdl_themes: Vec<super::Theme> = kdl
            .iter()
            .map(|text| super::Theme::from_text("random.theme.kdl", text.as_str()))
            .collect();
        let (mut old_tokens, mut new_tokens) = (TokenValues::default(), TokenValues::default());
        let old = read(&registry, &sheet, &toml_themes, &declared, &mut old_tokens);
        let new = super::read(&registry, &sheet, &kdl_themes, &declared, &mut new_tokens);
        let replay = format!(
            "case {case}, from the seed UWUI_THEME_SEED={seed}:\nTOML:\n{}\nKDL:\n{}",
            toml.join("---\n"),
            kdl.join("---\n")
        );
        match (old, new) {
            (Ok(old), Ok(new)) => {
                agreed += 1;
                assert_eq!(old.lacking, new.lacking, "{replay}");
                for token in &declared {
                    let (old, new) = (old_tokens.value(token), new_tokens.value(token));
                    assert!(
                        old.same(&new),
                        "{}: {old:?} and {new:?}\n{replay}",
                        token.name
                    );
                }
                assert_eq!(old.rules.len(), new.rules.len(), "{replay}");
                for (old, new) in old.rules.iter().zip(&new.rules) {
                    assert!(
                        old.declarations.same(&new.declarations),
                        "{:?} and {:?}\n{replay}",
                        old.declarations,
                        new.declarations
                    );
                }
            }
            (Err(_), Err(_)) => refused += 1,
            (Ok(_), Err(error)) => panic!("only the reader of KDL refused: {error}\n{replay}"),
            (Err(error), Ok(_)) => panic!("only the reflected reader refused: {error}\n{replay}"),
        }
    }
    // Both kinds of cases are met, so that neither is left untested.
    eprintln!("{agreed} read, {refused} refused, from the seed {seed}");
    assert!(
        agreed > 200 && refused > 200,
        "{agreed} read, {refused} refused"
    );
}
