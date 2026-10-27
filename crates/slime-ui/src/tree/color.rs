//! Color wells (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`): the swatch, the
//! picker that it opens below it in a layer of its own, the picker's square of saturation and
//! value, its strips of hue and alpha, its fields of the channels and of the hexadecimal code; the
//! color, which the program reads as actions.

use slime_color::{LinearColor, linear_to_srgb, srgb_to_linear};
use slime_input::{Key, KeyEvent, NamedKey};

use super::control::Control;
use super::{Along, ColorPart, Element, ElementId, Kind, Layer, Place, Side, Ui};
use crate::Rect;
use crate::action::{Action, ActionKind, FieldValue};
use crate::display::{BoxPrimitive, GradientPrimitive, Primitive};
use crate::style::{Dimension, FlexDirection, Style};

/// The size of the square of saturation and value, in logical pixels.
const SQUARE: [f32; 2] = [200.0, 150.0];

/// The height of a strip of hue or alpha, in logical pixels.
const STRIP: f32 = 14.0;

/// How far an arrow moves a part of the picker: a hundredth of its length, ten with Shift.
const KEY_STEP: f32 = 0.01;

/// The colors of the checkerboard that shows through a color's alpha.
const CHECKS: [LinearColor; 2] = [
    LinearColor::rgb(0.8, 0.8, 0.8),
    LinearColor::rgb(0.45, 0.45, 0.45),
];

/// A color as a picker shows it: its hue, in degrees from 0 to 360, and its saturation, value and
/// alpha, from 0 to 1, on its sRGB encoding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hsva {
    pub(crate) h: f32,
    pub(crate) s: f32,
    v: f32,
    a: f32,
}

impl Hsva {
    /// The hue, saturation, value and alpha of `color`, each channel held within 0 and 1; `hue`
    /// where the color is a gray, which has none, and `saturation` where it is black, which has
    /// none either.
    pub(crate) fn of(color: LinearColor, hue: f32, saturation: f32) -> Self {
        let [r, g, b] = [color.r, color.g, color.b].map(encoded);
        let max = r.max(g).max(b);
        let delta = max - r.min(g).min(b);
        let h = if delta <= 0.0 {
            hue
        } else if max == r {
            60.0 * ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        Self {
            h,
            s: if max > 0.0 { delta / max } else { saturation },
            v: max,
            a: unit(color.a),
        }
    }

    /// The linear color of its hue, saturation, value and alpha.
    fn color(self) -> LinearColor {
        let [r, g, b] = rgb_of(self.h, self.s, self.v).map(srgb_to_linear);
        LinearColor::rgba(r, g, b, self.a)
    }
}

/// The sRGB encoding of a hue at `h` degrees, of saturation `s` and value `v`.
fn rgb_of(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.rem_euclid(360.0) / 60.0;
    let chroma = v * s;
    let x = chroma * (1.0 - (h % 2.0 - 1.0).abs());
    let [r, g, b] = match h as u32 {
        0 => [chroma, x, 0.0],
        1 => [x, chroma, 0.0],
        2 => [0.0, chroma, x],
        3 => [0.0, x, chroma],
        4 => [x, 0.0, chroma],
        _ => [chroma, 0.0, x],
    };
    let m = v - chroma;
    [r + m, g + m, b + m]
}

/// The sRGB encoding of a linear channel, held within 0 and 1; 0 for one that is no number.
fn encoded(channel: f32) -> f32 {
    linear_to_srgb(unit(channel))
}

/// `value` within 0 and 1; 0 for one that is no number.
fn unit(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

/// Whether two colors are the same, their channels compared by their bits: a `NaN` is the same as
/// itself.
pub(crate) fn same(a: LinearColor, b: LinearColor) -> bool {
    [a.r, a.g, a.b, a.a]
        .iter()
        .zip([b.r, b.g, b.b, b.a])
        .all(|(a, b)| a.to_bits() == b.to_bits())
}

/// The hexadecimal code of `color`: its sRGB encoding, `#rrggbb`, and `#rrggbbaa` where its alpha is
/// not 1, each channel held within 0 and 1.
pub(crate) fn hex(color: LinearColor) -> String {
    let byte = |encoded: f32| (encoded * 255.0).round() as u8;
    let [r, g, b] = [color.r, color.g, color.b].map(|channel| byte(encoded(channel)));
    let a = byte(unit(color.a));
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

/// The color that a hexadecimal code names, read as sRGB: `#rgb`, `#rgba`, `#rrggbb` or
/// `#rrggbbaa`, the `#` optional, either case; none for another text.
pub(crate) fn parse_hex(text: &str) -> Option<LinearColor> {
    let digits = text.trim();
    let digits = digits.strip_prefix('#').unwrap_or(digits);
    if !digits.chars().all(|digit| digit.is_ascii_hexdigit()) {
        return None;
    }
    let pair = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
    let single = |at: usize| {
        u8::from_str_radix(&digits[at..=at], 16)
            .ok()
            .map(|d| d * 17)
    };
    let channels: Vec<u8> = match digits.len() {
        3 | 4 => (0..digits.len()).map(single).collect::<Option<_>>()?,
        6 | 8 => (0..digits.len() / 2)
            .map(|place| pair(place * 2))
            .collect::<Option<_>>()?,
        _ => return None,
    };
    let channel = |place: usize| f32::from(channels[place]) / 255.0;
    let a = channels.get(3).map_or(1.0, |_| channel(3));
    Some(LinearColor::rgba(
        srgb_to_linear(channel(0)),
        srgb_to_linear(channel(1)),
        srgb_to_linear(channel(2)),
        a,
    ))
}

/// The picker that a color well opened: the well, the picker's layer, its parts, its fields of the
/// channels and of the hexadecimal code, and the revision of its build.
pub(super) struct Picker {
    well: ElementId,
    parts: Vec<ElementId>,
    channels: [ElementId; 4],
    code: ElementId,
    revision: u64,
}

impl Ui {
    /// The color of the color well `id`, as the user left it; none for another element.
    pub fn color(&self, id: ElementId) -> Option<LinearColor> {
        match self.node(id)?.control? {
            Control::Color { color, .. } => Some(color),
            _ => None,
        }
    }

    /// The hue, saturation, value and alpha that the picker of the color well `id` shows.
    fn well_hsva(&self, id: ElementId) -> Option<Hsva> {
        match self.node(id)?.control? {
            Control::Color {
                color,
                hue,
                saturation,
                ..
            } => Some(Hsva::of(color, hue, saturation)),
            _ => None,
        }
    }

    /// A press of the color well `id`: its picker opens below it, or closes when it is open.
    pub(super) fn press_well(&mut self, id: ElementId) {
        if let Some(picker) = self.owned_layer(id) {
            self.close_layer(picker, true);
            return;
        }
        let layer = Layer {
            place: Place::Beside {
                element: id,
                side: Side::Below,
                along: Along::Start,
            },
            modal: false,
            dismiss: true,
        };
        let root = self.open_owned_layer(id, layer, Style::default());
        self.picker = Some(Picker {
            well: id,
            parts: Vec::new(),
            channels: [root; 4],
            code: root,
            revision: 0,
        });
        self.build_picker();
        self.change_state(id, |state| state.expanded = true);
    }

    /// Builds the open picker again, from its well's color.
    fn build_picker(&mut self) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        let well = picker.well;
        let revision = picker.revision + 1;
        let (Some(root), Some(color)) = (self.owned_layer(well), self.color(well)) else {
            return;
        };
        let code = hex(color);
        let channels = [color.r, color.g, color.b, color.a];
        let mut parts = Vec::new();
        let mut fields = Vec::new();
        let mut code_field = root;
        self.build(root, revision, |children| {
            let mut column = Style::default();
            column.layout.direction = FlexDirection::Column;
            column.layout.gap = [8.0, 0.0];
            children.add(
                "picker",
                Element::container(column).class("picker"),
                |picker| {
                    for (key, part, size, name) in [
                        ("square", ColorPart::Square, SQUARE, "saturation and value"),
                        ("hue", ColorPart::Hue, [SQUARE[0], STRIP], "hue"),
                        ("alpha", ColorPart::Alpha, [SQUARE[0], STRIP], "alpha"),
                    ] {
                        let mut style = Style::default();
                        style.layout.size = size.map(Dimension::Pixels);
                        style.layout.shrink = 0.0;
                        let element = Element::of(Kind::ColorPart(part), style).named(name);
                        parts.push(picker.add(key, element, |_| {}));
                    }
                    // The channels in two rows of two, each wide enough for its number, shown
                    // to three decimals: the code gives the color whole.
                    let names = [
                        ("R", "red"),
                        ("G", "green"),
                        ("B", "blue"),
                        ("A", "alpha channel"),
                    ];
                    for (line, pair) in names.chunks(2).enumerate() {
                        let mut row = Style::default();
                        row.layout.gap = [0.0, 4.0];
                        row.layout.size[0] = Dimension::Pixels(SQUARE[0]);
                        picker.add(line, Element::container(row), |row| {
                            for (column, &(prefix, name)) in pair.iter().enumerate() {
                                let place = line * 2 + column;
                                let mut style = Style::default();
                                style.layout.grow = 1.0;
                                style.layout.basis = Dimension::Pixels(0.0);
                                style.layout.min_size[0] = Dimension::Pixels(0.0);
                                let shown = (f64::from(channels[place]) * 1000.0).round() / 1000.0;
                                let field = Element::number_field(shown, 0.01, style)
                                    .prefix(prefix)
                                    .named(name);
                                fields.push(row.add(prefix, field, |_| {}));
                            }
                        });
                    }
                    let field = Element::text_field(code.as_str(), Style::default())
                        .class("fixed")
                        .named("hexadecimal code");
                    code_field = picker.add("code", field, |_| {});
                },
            );
        });
        if let Some(picker) = self.picker.as_mut() {
            picker.revision = revision;
            picker.parts = parts;
            if let [r, g, b, a] = fields[..] {
                picker.channels = [r, g, b, a];
            }
            picker.code = code_field;
        }
    }

    /// Sets the color well `id` to `color`, its picker showing the hue and the saturation of
    /// `shown`, as the user does: the program reads [`ActionKind::Changed`] where the color changed,
    /// then [`ActionKind::Committed`] for `commit`; the picker, open, shows it. Gives whether `id`
    /// is a color well.
    pub(crate) fn set_color(
        &mut self,
        id: ElementId,
        color: LinearColor,
        shown: Hsva,
        commit: bool,
    ) -> bool {
        let Some(node) = self.node_mut(id) else {
            return false;
        };
        let Some(Control::Color {
            color: held,
            hue,
            saturation,
            ..
        }) = node.control.as_mut()
        else {
            return false;
        };
        let changed = !same(*held, color);
        let turned = *hue != shown.h || *saturation != shown.s;
        *held = color;
        (*hue, *saturation) = (shown.h, shown.s);
        if changed {
            self.interaction.actions.push(Action {
                element: Some(id),
                kind: ActionKind::Changed(FieldValue::Color(color)),
            });
        }
        if commit {
            self.interaction.actions.push(Action {
                element: Some(id),
                kind: ActionKind::Committed(FieldValue::Color(color)),
            });
        }
        if changed || turned {
            self.mark_paint(id);
            self.mark_meaning(id);
            let parts = self
                .picker
                .as_ref()
                .filter(|picker| picker.well == id)
                .map(|picker| picker.parts.clone())
                .unwrap_or_default();
            for part in parts {
                self.mark_paint(part);
                self.mark_meaning(part);
            }
            if self.picker.as_ref().is_some_and(|picker| picker.well == id) {
                self.build_picker();
            }
        }
        true
    }

    /// Sets the color well `id` to `color` and commits it, as the user does, its picker keeping its
    /// hue where the color is a gray. Gives whether `id` is a color well.
    pub(crate) fn commit_color(&mut self, id: ElementId, color: LinearColor) -> bool {
        let (hue, saturation) = self
            .well_hsva(id)
            .map_or((0.0, 0.0), |held| (held.h, held.s));
        self.set_color(id, color, Hsva::of(color, hue, saturation), true)
    }

    /// The color well whose picker holds the part `id`, and which part it is.
    fn part_of(&self, id: ElementId) -> Option<(ElementId, ColorPart)> {
        let Kind::ColorPart(part) = self.node(id)?.element.kind else {
            return None;
        };
        let well = self.picker.as_ref()?.well;
        self.picker
            .as_ref()?
            .parts
            .contains(&id)
            .then_some((well, part))
    }

    /// The pointer pressed, or moved while it holds the part `id` of a picker, at (`x`, `y`) in
    /// pixels of the surface: the color under it, changed, not committed.
    pub(crate) fn part_drag(&mut self, id: ElementId, x: f32, y: f32) {
        let Some((well, part)) = self.part_of(id) else {
            return;
        };
        let (Some(node), Some(mut hsva)) = (self.node(id), self.well_hsva(well)) else {
            return;
        };
        let rect = node.content_box();
        let along = |at: f32, from: f32, length: f32| {
            if length > 0.0 {
                ((at - from) / length).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        let across = along(x, rect.x, rect.width);
        match part {
            ColorPart::Square => {
                hsva.s = across;
                hsva.v = 1.0 - along(y, rect.y, rect.height);
            }
            ColorPart::Hue => hsva.h = across * 360.0,
            ColorPart::Alpha => hsva.a = across,
        }
        self.set_color(well, hsva.color(), hsva, false);
    }

    /// The pointer let go of the part `id` of a picker, which it held: its well's color committed.
    /// Gives whether `id` is such a part.
    pub(super) fn part_release(&mut self, id: ElementId) -> bool {
        let Some((well, _)) = self.part_of(id) else {
            return false;
        };
        if let (Some(color), Some(hsva)) = (self.color(well), self.well_hsva(well)) {
            self.set_color(well, color, hsva, true);
        }
        true
    }

    /// A key reached the part `id` of a picker, which holds the focus: the arrows move it by a
    /// hundredth of its length, ten with Shift, the square's up and down its value, Home and End a
    /// strip to its ends, each committed. Gives whether it took the key.
    pub(super) fn part_key(&mut self, id: ElementId, event: &KeyEvent) -> bool {
        let Key::Named(named) = &event.logical else {
            return false;
        };
        let Some((well, part)) = self.part_of(id) else {
            return false;
        };
        let Some(mut hsva) = self.well_hsva(well) else {
            return false;
        };
        let step = if self.interaction.modifiers.shift {
            KEY_STEP * 10.0
        } else {
            KEY_STEP
        };
        let (across, up) = match named {
            NamedKey::ArrowRight => (step, 0.0),
            NamedKey::ArrowLeft => (-step, 0.0),
            NamedKey::ArrowUp => (0.0, step),
            NamedKey::ArrowDown => (0.0, -step),
            NamedKey::Home if part != ColorPart::Square => (-1.0, 0.0),
            NamedKey::End if part != ColorPart::Square => (1.0, 0.0),
            _ => return false,
        };
        match part {
            ColorPart::Square => {
                hsva.s = (hsva.s + across).clamp(0.0, 1.0);
                hsva.v = (hsva.v + up).clamp(0.0, 1.0);
            }
            // On a strip, up moves as the right does.
            ColorPart::Hue => hsva.h = (hsva.h + (across + up) * 360.0).clamp(0.0, 360.0),
            ColorPart::Alpha => hsva.a = (hsva.a + across + up).clamp(0.0, 1.0),
        }
        self.set_color(well, hsva.color(), hsva, true);
        true
    }

    /// Turns the actions of the open picker's fields into its well's: a channel or a hexadecimal
    /// code committed sets the color, which the program reads as the well's; what the fields hold
    /// as they are typed in never reaches it. A code that names no color is given back.
    pub(super) fn settle_picker_actions(&mut self) {
        let Some(picker) = self.picker.as_ref() else {
            return;
        };
        let (well, channels, code) = (picker.well, picker.channels, picker.code);
        let actions = std::mem::take(&mut self.interaction.actions);
        let mut kept = Vec::with_capacity(actions.len());
        for action in actions {
            let Some(element) = action.element else {
                kept.push(action);
                continue;
            };
            let channel = channels.iter().position(|&each| each == element);
            if channel.is_none() && element != code {
                kept.push(action);
                continue;
            }
            let Some(color) = self.color(well) else {
                continue;
            };
            match (channel, &action.kind) {
                (Some(place), ActionKind::Committed(FieldValue::Number(number))) => {
                    let mut values = [color.r, color.g, color.b, color.a];
                    values[place] = *number as f32;
                    let [r, g, b, a] = values;
                    self.commit_color(well, LinearColor::rgba(r, g, b, a));
                }
                (None, ActionKind::Committed(FieldValue::Text(text))) => match parse_hex(text) {
                    Some(color) => {
                        self.commit_color(well, color);
                    }
                    None => self.build_picker(),
                },
                _ => {}
            }
            kept.append(&mut self.interaction.actions);
        }
        self.interaction.actions = kept;
    }

    /// Forgets the picker of the color well `well`, whose layer closed.
    pub(super) fn forget_picker(&mut self, well: ElementId) {
        if self
            .picker
            .as_ref()
            .is_some_and(|picker| picker.well == well)
        {
            self.picker = None;
        }
    }

    /// What screen readers and agents read of the part `id` of a picker: its value, the square's
    /// saturation and value, the hue in degrees, the alpha; none for another element.
    pub(crate) fn part_value(&self, id: ElementId) -> Option<String> {
        let (well, part) = self.part_of(id)?;
        let hsva = self.well_hsva(well)?;
        let percent = |value: f32| (value * 100.0).round();
        Some(match part {
            ColorPart::Square => format!(
                "saturation {}%, value {}%",
                percent(hsva.s),
                percent(hsva.v)
            ),
            ColorPart::Hue => format!("{}°", hsva.h.round()),
            ColorPart::Alpha => format!("{}%", percent(hsva.a)),
        })
    }

    /// What a color well or a part of its picker draws, besides its box: a well's swatch over a
    /// checkerboard; the square's gradient of saturation and value and its ring; a strip's gradient
    /// and its mark.
    pub(crate) fn color_primitives(&mut self, id: ElementId) -> Vec<Primitive> {
        let scale = self.scale_of(id);
        let Some(node) = self.node(id) else {
            return Vec::new();
        };
        let radius = node.style.control.radius * scale;
        let rect = node.content_box();
        let focus = (node.state.focus_visible && matches!(node.element.kind, Kind::ColorPart(_)))
            .then_some(node.style.control.fill);
        let mut primitives = Vec::new();
        match node.element.kind {
            Kind::ColorWell { .. } => {
                let Some(color) = self.color(id) else {
                    return primitives;
                };
                if color.a < 1.0 {
                    checkerboard(&mut primitives, rect, radius);
                }
                primitives.push(boxed(rect, [radius; 4], color));
            }
            Kind::ColorPart(part) => {
                let Some(hsva) = self.part_of(id).and_then(|(well, _)| self.well_hsva(well)) else {
                    return primitives;
                };
                match part {
                    ColorPart::Square => {
                        let hue = Hsva {
                            s: 1.0,
                            v: 1.0,
                            a: 1.0,
                            ..hsva
                        }
                        .color();
                        let black = LinearColor::BLACK;
                        primitives.push(Primitive::Gradient(GradientPrimitive {
                            rect,
                            radii: [radius; 4],
                            corners: [LinearColor::WHITE, hue, black, black],
                        }));
                        let at = [
                            rect.x + hsva.s * rect.width,
                            rect.y + (1.0 - hsva.v) * rect.height,
                        ];
                        ring(&mut primitives, at, 6.0 * scale, scale);
                    }
                    ColorPart::Hue => {
                        let pure = |step: usize| {
                            let [r, g, b] =
                                rgb_of(step as f32 * 60.0, 1.0, 1.0).map(srgb_to_linear);
                            LinearColor::rgb(r, g, b)
                        };
                        let width = rect.width / 6.0;
                        for step in 0..6 {
                            let radii = match step {
                                0 => [radius, 0.0, 0.0, radius],
                                5 => [0.0, radius, radius, 0.0],
                                _ => [0.0; 4],
                            };
                            let (from, to) = (pure(step), pure(step + 1));
                            let part =
                                Rect::new(rect.x + step as f32 * width, rect.y, width, rect.height);
                            primitives.push(Primitive::Gradient(GradientPrimitive {
                                rect: part,
                                radii,
                                corners: [from, to, to, from],
                            }));
                        }
                        mark(&mut primitives, rect, hsva.h / 360.0, scale);
                    }
                    ColorPart::Alpha => {
                        checkerboard(&mut primitives, rect, radius);
                        let opaque = Hsva { a: 1.0, ..hsva }.color();
                        let clear = Hsva { a: 0.0, ..hsva }.color();
                        primitives.push(Primitive::Gradient(GradientPrimitive {
                            rect,
                            radii: [radius; 4],
                            corners: [clear, opaque, opaque, clear],
                        }));
                        mark(&mut primitives, rect, hsva.a, scale);
                    }
                }
            }
            _ => {}
        }
        // A part that the keyboard's focus is on, ringed inside its edges in the control's fill:
        // what a control draws is cut to its content.
        if let Some(color) = focus {
            primitives.push(Primitive::Box(BoxPrimitive {
                rect,
                radii: [radius; 4],
                fill: LinearColor::rgba(0.0, 0.0, 0.0, 0.0),
                border_width: 2.0 * scale,
                border: color,
            }));
        }
        primitives
    }
}

/// A box of `rect`, rounded by `radii`, filled with `fill`.
fn boxed(rect: Rect, radii: [f32; 4], fill: LinearColor) -> Primitive {
    Primitive::Box(BoxPrimitive {
        rect,
        radii,
        fill,
        border_width: 0.0,
        border: fill,
    })
}

/// A checkerboard over `rect`, two rows of squares, its end squares rounded by `radius`.
fn checkerboard(primitives: &mut Vec<Primitive>, rect: Rect, radius: f32) {
    let side = rect.height / 2.0;
    if side <= 0.0 {
        return;
    }
    let columns = (rect.width / side).ceil() as usize;
    for column in 0..columns {
        let x = rect.x + column as f32 * side;
        let width = side.min(rect.x + rect.width - x);
        for row in 0..2 {
            let mut radii = [0.0; 4];
            if column == 0 {
                radii[if row == 0 { 0 } else { 3 }] = radius;
            }
            if column + 1 == columns {
                radii[if row == 0 { 1 } else { 2 }] = radius;
            }
            let square = Rect::new(x, rect.y + row as f32 * side, width, side);
            primitives.push(boxed(square, radii, CHECKS[(column + row) % 2]));
        }
    }
}

/// The ring that marks the square's color, of `radius` pixels, centred at `at`: white around black,
/// seen on any color.
fn ring(primitives: &mut Vec<Primitive>, at: [f32; 2], radius: f32, scale: f32) {
    for (grown, color) in [(0.0, LinearColor::BLACK), (-scale, LinearColor::WHITE)] {
        let size = (radius + grown) * 2.0;
        primitives.push(Primitive::Box(BoxPrimitive {
            rect: Rect::new(at[0] - size / 2.0, at[1] - size / 2.0, size, size),
            radii: [size / 2.0; 4],
            fill: LinearColor::rgba(0.0, 0.0, 0.0, 0.0),
            border_width: 2.0 * scale,
            border: color,
        }));
    }
}

/// The mark of a strip's value, at `along` its length: a bar across it, white around black.
fn mark(primitives: &mut Vec<Primitive>, rect: Rect, along: f32, scale: f32) {
    let width = 4.0 * scale;
    let x = rect.x + along.clamp(0.0, 1.0) * rect.width - width / 2.0;
    let bar = Rect::new(x, rect.y - scale, width, rect.height + 2.0 * scale);
    primitives.push(Primitive::Box(BoxPrimitive {
        rect: bar,
        radii: [width / 2.0; 4],
        fill: LinearColor::rgba(0.0, 0.0, 0.0, 0.0),
        border_width: scale,
        border: LinearColor::BLACK,
    }));
    let inner = Rect::new(
        bar.x + scale,
        bar.y + scale,
        width - 2.0 * scale,
        bar.height - 2.0 * scale,
    );
    primitives.push(Primitive::Box(BoxPrimitive {
        rect: inner,
        radii: [width / 2.0; 4],
        fill: LinearColor::rgba(0.0, 0.0, 0.0, 0.0),
        border_width: scale,
        border: LinearColor::WHITE,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_color_reads_back_through_its_hue_saturation_and_value() {
        for color in [
            LinearColor::rgba(0.5, 0.2, 0.05, 0.75),
            LinearColor::rgb(0.0, 1.0, 0.25),
            LinearColor::rgb(0.3, 0.3, 0.9),
        ] {
            let back = Hsva::of(color, 0.0, 0.0).color();
            for (a, b) in [
                (color.r, back.r),
                (color.g, back.g),
                (color.b, back.b),
                (color.a, back.a),
            ] {
                assert!((a - b).abs() < 1e-4, "{color:?} came back as {back:?}");
            }
        }
        // A gray has no hue: the one kept stays.
        assert_eq!(
            Hsva::of(LinearColor::rgb(0.2, 0.2, 0.2), 120.0, 0.0).h,
            120.0
        );
        // Black has no saturation either: the one kept stays.
        assert_eq!(Hsva::of(LinearColor::BLACK, 0.0, 0.5).s, 0.5);
        // Read on the sRGB encoding: half the light is a value of 188 of 255, not 128.
        let half = Hsva::of(LinearColor::rgb(0.5, 0.5, 0.5), 0.0, 0.0);
        assert!((half.v * 255.0 - 187.5).abs() < 0.5);
    }

    #[test]
    fn a_hexadecimal_code_names_a_color_of_srgb() {
        let orange = parse_hex("#ff8000").expect("a code");
        assert_eq!(hex(orange), "#ff8000");
        assert_eq!(parse_hex("FF8000"), Some(orange));
        assert_eq!(hex(parse_hex("#f80").expect("a short code")), "#ff8800");
        let clear = parse_hex("#00000080").expect("a code with alpha");
        assert!((clear.a - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(hex(clear), "#00000080");
        assert_eq!(
            hex(parse_hex("#0f08").expect("a short code with alpha")),
            "#00ff0088"
        );
        for wrong in ["", "#", "#12", "#12345", "#gg0000", "red", "#ff80001"] {
            assert_eq!(parse_hex(wrong), None, "{wrong}");
        }
        // Half the light is sRGB's 188, `bc`.
        assert_eq!(hex(LinearColor::rgb(0.5, 0.5, 0.5)), "#bcbcbc");
    }
}
