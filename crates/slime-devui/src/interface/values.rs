//! The editors of reflected values on the engine's interface
//! (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`): the elements that edit a value of
//! a type that reflection describes, and the edits that the program reads from their actions.

use std::collections::HashMap;

use slime_assets::AssetId;
use slime_color::LinearColor;
use slime_reflect::{FieldInfo, Kind, Path, Range, Reflect, Registry, Value, VariantInfo};
use slime_ui::action::{Action, ActionKind, FieldValue};
use slime_ui::style::{Align, Dimension, FlexDirection, Style};
use slime_ui::tree::{Children, ChoiceOption, Element, ElementId, Key, Tooltip, Ui, Units};
use slime_world::glam::{DVec3, Quat, Vec3};
use slime_world::{Entity, World};

use crate::world_panel::{WHOLE, angles, initial, name, plain, rotation, set_edits, variant_value};

/// The prefixes of the fields of a vector's components, and the classes that color them.
const AXES: [(&str, &str); 3] = [("X", "axis-x"), ("Y", "axis-y"), ("Z", "axis-z")];

/// The prefixes of the fields of a rotation's angles, in the order that turns it: about Y, X, Z.
const TURNS: [(&str, &str); 3] = [("Y", "axis-y"), ("X", "axis-x"), ("Z", "axis-z")];

/// The prefixes of the fields of a color's channels.
const CHANNELS: [&str; 4] = ["R", "G", "B", "A"];

/// The share of a row that a part's label takes, before what edits it.
const LABEL_SHARE: f32 = 0.32;

/// Editors of reflected values, built as elements of the engine's interface: the elements that
/// edit a value of a type that reflection describes, and the edits that their actions ask for.
///
/// Each kind of value has its editor: a boolean a check box; a number a field of numbers, dragged
/// by its label's prefix or typed, an expression or a number in a unit of its field's family,
/// `10cm` in a field of meters, an angle of a field in radians shown and typed in degrees, an
/// integer rounded; text a field committed by Enter or as it loses the focus; an asset's ID, or
/// its error; a reference to an entity a button with its name that selects it, `despawned` once it
/// is gone; a struct its fields; an enum a list of its variants, those that a new value cannot
/// start in disabled, each with its documentation as help, then its variant's fields; an option a
/// check box, then the value it holds; a list or an array its rows, which none are added to or
/// removed from. Vectors, rotations and colors are edited whole, their parts on one line: a vector
/// as three fields, X, Y and Z, a rotation as three angles in degrees, about Y, then X, from −90°
/// to 90°, then Z, a color as a swatch and its four channels. A part sits on its label's line
/// where it fits, or under a section that folds, open at first; its label's tooltip holds its
/// documentation, its range and whether saves leave it out. A field's unit outside the families
/// that fields of numbers know, `cd`, follows its label.
///
/// A field changes the value only when the user changed it, and gives nothing for a value that
/// it was only given the focus of or tabbed through (`docs/PLAN.md`, G33). The editor changes
/// nothing itself: it gives each edit, a part's path and its new value, which the world or the
/// type then checks; a value out of its field's range is given, and refused there.
#[derive(Debug, Default)]
pub struct ValueEditor {
    /// What each element built last edits.
    edits: HashMap<ElementId, Edit>,
    /// The angles last typed into each rotation, by the scope and the path of the rotation, and
    /// the rotation that they made: computed again from it, they would come back inexact, and past
    /// 90° about X as other angles.
    angles: HashMap<(String, Path), (Quat, [f32; 3])>,
    /// The boxes of options built last, each with whether it is disabled, as one that holds nothing
    /// and has nothing to start holding is.
    boxes: Vec<(ElementId, bool)>,
    /// The label and the value that the elements were last built for, and the revision of that
    /// build.
    built: Option<(String, Value)>,
    revision: u64,
}

/// What an element that the editors built edits.
#[derive(Clone, Debug)]
struct Edit {
    /// The scope of the value, a component's name or empty for a value outside a world.
    scope: String,
    /// The path of the part that it sets.
    path: Path,
    /// How many options hold the part at its path, which its new value is wrapped in.
    wraps: usize,
    target: Target,
}

/// How an element's value reads as the part's new value.
#[derive(Clone, Debug)]
enum Target {
    /// A boolean's check box.
    Bool,
    /// A number's field: the kind of number that the part holds, and whether its field shows in
    /// degrees an angle in radians.
    Number { kind: Number, radians: bool },
    /// A text's field.
    Text,
    /// An enum's list of variants: the value of each that it can take.
    Variant(Vec<Option<Value>>),
    /// An option's check box: the value that it starts holding.
    Option(Option<Value>),
    /// A field of a vector's component: the vector shown, which of its components, and whether it
    /// is a vector of `f64`.
    Component {
        vector: [f64; 3],
        axis: usize,
        double: bool,
    },
    /// A field of a rotation's angle: the angles shown, about Y, X and Z, and which of them.
    Angle { angles: [f32; 3], turn: usize },
    /// A field of a color's channel: the color shown, and which of its channels.
    Channel { color: LinearColor, channel: usize },
    /// A reference's button: the entity that it selects.
    Reference(Entity),
}

/// The kinds of numbers that reflection holds.
#[derive(Clone, Copy, Debug)]
enum Number {
    Int,
    UInt,
    F32,
    F64,
}

/// What an action on the editors' elements asks for.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Asked {
    /// The part at the path, within the scope, set to the value.
    Set {
        scope: String,
        path: Path,
        value: Value,
    },
    /// The entity selected, a reference to which was pressed.
    Select(Entity),
}

impl ValueEditor {
    /// Builds in `slot`, a parent that the program added for the editor (`Children::add_slot`),
    /// the elements that edit `value`, under its `label`, as the editor of its kind does
    /// ([`ValueEditor`] says how each is edited), again only once the label or the value changed.
    /// Their edits are read by [`ValueEditor::apply`].
    ///
    /// It describes the type at each build, which suits the few small values of an overlay's
    /// panel, such as a view's settings (`slime_render::ViewSettings`).
    ///
    /// # Panics
    ///
    /// If `slot` names nothing.
    pub fn build<T: Reflect>(&mut self, ui: &mut Ui, slot: ElementId, label: &str, value: &T) {
        let reflected = value.to_value();
        // Floats compared by their bits, so that a `NaN` does not build it again at each frame.
        let same = self
            .built
            .as_ref()
            .is_some_and(|(built, held)| built == label && held.same(&reflected));
        if !same {
            self.revision += 1;
            self.built = Some((label.to_owned(), reflected.clone()));
        }
        let revision = self.revision;
        let mut ran = false;
        ui.build(slot, revision, |children| {
            ran = true;
            let mut registry = Registry::new();
            registry.register::<T>();
            let ty = T::type_name();
            self.edits.clear();
            self.boxes.clear();
            let mut builder = Builder::new(self, &registry, None, "");
            let path = Path::default();
            builder.part(children, "value", label, &ty, &reflected, At::whole(&path));
        });
        if ran {
            self.settle(ui);
        }
    }

    /// Sets `value` as the actions of `actions` that the editor's elements were given ask, each
    /// checked by the type: a part that it refuses, as a number out of its field's range, is left
    /// as it was. Gives whether one was set; the actions that are not the editor's are left alone.
    pub fn apply<T: Reflect>(&mut self, actions: &[Action], value: &mut T) -> bool {
        let edits: Vec<(Path, Value)> = actions
            .iter()
            .filter_map(|action| match self.asked(action)? {
                Asked::Set { path, value, .. } => Some((path, value)),
                Asked::Select(_) => None,
            })
            .collect();
        if edits.is_empty() {
            return false;
        }
        let mut registry = Registry::new();
        registry.register::<T>();
        let reflected = value.to_value();
        set_edits(&registry, value, reflected, edits)
    }

    /// Whether `id` is an element that the editor built last.
    pub(crate) fn holds(&self, id: ElementId) -> bool {
        self.edits.contains_key(&id)
    }

    /// Forgets the angles typed, as the world panel selects another entity.
    pub(crate) fn forget_angles(&mut self) {
        self.angles.clear();
    }

    /// Adds under `children` the elements of `components`, an entity's in `world`, each its name
    /// and its value, a section for each, folding, open at first, with the documentation of its
    /// type as help; the elements that it built before are forgotten. The builder's caller then
    /// settles their states ([`ValueEditor::settle`]).
    pub(crate) fn build_components(
        &mut self,
        children: &mut Children<'_>,
        world: &World,
        components: &[(String, Value)],
    ) {
        self.edits.clear();
        self.boxes.clear();
        let registry = world.registry();
        for (component, value) in components {
            let component = component.as_str();
            let mut section = Element::section(component, true, column(4.0)).named(component);
            if let Some(registration) = registry.get(component)
                && !registration.info().doc.is_empty()
            {
                section = section.help(component, plain(&registration.info().doc));
            }
            children.add(component, section, |children| {
                let mut builder = Builder::new(self, registry, Some(world), component);
                let path = Path::default();
                builder.value(children, component, value, At::whole(&path));
            });
        }
    }

    /// Disables the boxes of options built last that cannot be used, and enables the others,
    /// which a builder cannot do: a box kept from a build before keeps its state.
    pub(crate) fn settle(&self, ui: &mut Ui) {
        for &(id, disabled) in &self.boxes {
            if let Some(mut state) = ui.state(id)
                && state.disabled != disabled
            {
                state.disabled = disabled;
                ui.set_state(id, state);
            }
        }
    }

    /// What `action` asks for, if one of the editor's elements was given it: a part set, the
    /// value read from the element as its target says, or an entity selected.
    pub(crate) fn asked(&mut self, action: &Action) -> Option<Asked> {
        let edit = self.edits.get(&action.element?)?.clone();
        let value = match (&edit.target, &action.kind) {
            (Target::Reference(entity), ActionKind::Press) => return Some(Asked::Select(*entity)),
            (Target::Bool, ActionKind::Committed(FieldValue::Checked(on))) => Value::Bool(*on),
            (Target::Option(start), ActionKind::Committed(FieldValue::Checked(on))) => {
                if *on {
                    Value::Option(Some(Box::new(start.clone()?)))
                } else {
                    Value::Option(None)
                }
            }
            (Target::Text, ActionKind::Committed(FieldValue::Text(text))) => {
                Value::Text(text.clone())
            }
            (Target::Variant(values), ActionKind::Committed(FieldValue::Chosen(place))) => {
                values.get(*place)?.clone()?
            }
            (target, ActionKind::Committed(FieldValue::Number(number))) => {
                self.number(&edit, target, *number)?
            }
            _ => return None,
        };
        let value = (0..edit.wraps).fold(value, |value, _| Value::Option(Some(Box::new(value))));
        Some(Asked::Set {
            scope: edit.scope,
            path: edit.path,
            value,
        })
    }

    /// The part's new value from the number that the field of `edit` committed.
    fn number(&mut self, edit: &Edit, target: &Target, number: f64) -> Option<Value> {
        Some(match *target {
            Target::Number { kind, radians } => {
                let number = if radians { number.to_radians() } else { number };
                match kind {
                    // A negative number given to an unsigned integer, which the type refuses
                    // with its message.
                    Number::Int => Value::Int(number.round() as i64),
                    Number::UInt if number < 0.0 => Value::Int(number.round() as i64),
                    Number::UInt => Value::UInt(number.round() as u64),
                    Number::F32 => Value::F32(number as f32),
                    Number::F64 => Value::F64(number),
                }
            }
            Target::Component {
                mut vector,
                axis,
                double,
            } => {
                vector[axis] = number;
                self.carry(edit, |target| {
                    if let Target::Component { vector: held, .. } = target {
                        *held = vector;
                    }
                });
                let [x, y, z] = vector;
                if double {
                    DVec3::new(x, y, z).to_value()
                } else {
                    Vec3::new(x as f32, y as f32, z as f32).to_value()
                }
            }
            Target::Angle { mut angles, turn } => {
                angles[turn] = number as f32;
                // About X, within −90° and 90°, where the angles turn it one way only.
                angles[1] = angles[1].clamp(-90.0, 90.0);
                let made = rotation(angles);
                let key = (edit.scope.clone(), edit.path.clone());
                self.angles.insert(key, (made, angles));
                self.carry(edit, |target| {
                    if let Target::Angle { angles: held, .. } = target {
                        *held = angles;
                    }
                });
                made.to_value()
            }
            Target::Channel { color, channel } => {
                let mut channels = [color.r, color.g, color.b, color.a];
                channels[channel] = number as f32;
                let [r, g, b, a] = channels;
                let color = LinearColor { r, g, b, a };
                self.carry(edit, |target| {
                    if let Target::Channel { color: held, .. } = target {
                        *held = color;
                    }
                });
                color.to_value()
            }
            _ => return None,
        })
    }

    /// Updates, with `update`, what the fields of the value that `edit` sets hold of it, as one
    /// of them sets the value whole: the next field committed before the value is built again
    /// sets it from this one, rather than taking this one back.
    fn carry(&mut self, edit: &Edit, update: impl Fn(&mut Target)) {
        for other in self.edits.values_mut() {
            if other.scope == edit.scope && other.path == edit.path {
                update(&mut other.target);
            }
        }
    }
}

/// Where a value lies: its path, the field that it is, or an element of, the name of an element
/// of a list, and how many options at its path hold it.
#[derive(Clone, Copy)]
struct At<'p> {
    path: &'p Path,
    field: Option<&'p FieldInfo>,
    /// For an element of a list, its name: its list's, then its place, `weights [1]`.
    element: Option<&'p str>,
    wraps: usize,
}

impl<'p> At<'p> {
    /// The whole value, at the empty path.
    fn whole(path: &'p Path) -> Self {
        Self {
            path,
            field: None,
            element: None,
            wraps: 0,
        }
    }

    /// The name that its elements are known by, which agents and screen readers read: an element
    /// of a list's, or its field's, or else its type's, `ty`.
    fn name<'n>(&self, ty: &'n str) -> &'n str
    where
        'p: 'n,
    {
        self.element
            .or(self.field.map(|field| field.name.as_str()))
            .unwrap_or(ty)
    }

    /// Its field's unit, if it has one.
    fn unit(&self) -> Option<&'p str> {
        self.field.and_then(|field| field.unit.as_deref())
    }
}

/// Builds the elements of a value, recording what each edits.
struct Builder<'a> {
    edits: &'a mut HashMap<ElementId, Edit>,
    boxes: &'a mut Vec<(ElementId, bool)>,
    angles: &'a HashMap<(String, Path), (Quat, [f32; 3])>,
    registry: &'a Registry,
    /// The world whose entities a reference names; none for a value outside a world.
    world: Option<&'a World>,
    scope: &'a str,
}

impl<'a> Builder<'a> {
    fn new(
        editor: &'a mut ValueEditor,
        registry: &'a Registry,
        world: Option<&'a World>,
        scope: &'a str,
    ) -> Self {
        Self {
            edits: &mut editor.edits,
            boxes: &mut editor.boxes,
            angles: &editor.angles,
            registry,
            world,
            scope,
        }
    }

    /// Records that `id` edits the value at `at`, as `target` reads it.
    fn record(&mut self, id: ElementId, at: At<'_>, target: Target) {
        let edit = Edit {
            scope: self.scope.to_owned(),
            path: at.path.clone(),
            wraps: at.wraps,
            target,
        };
        self.edits.insert(id, edit);
    }

    /// The elements of `value`, a value of the type named `ty`, at `at`.
    fn value(&mut self, children: &mut Children<'_>, ty: &str, value: &Value, at: At<'_>) {
        if self.whole(children, ty, value, at) {
            return;
        }
        let Some(registration) = self.registry.get(ty) else {
            return;
        };
        let name = at.name(ty);
        match (&registration.info().kind, value) {
            (_, Value::Bool(on)) => {
                let element = Element::check_box("", *on, Style::default()).named(name);
                let id = children.add("value", element, |_| {});
                self.record(id, at, Target::Bool);
            }
            (_, Value::Int(number)) => {
                self.number(children, name, *number as f64, Number::Int, at);
            }
            (_, Value::UInt(number)) => {
                self.number(children, name, *number as f64, Number::UInt, at);
            }
            (_, Value::F32(number)) => {
                self.number(children, name, f64::from(*number), Number::F32, at);
            }
            (_, Value::F64(number)) => self.number(children, name, *number, Number::F64, at),
            (_, Value::Text(text)) => {
                let element = Element::text_field(text.as_str(), growing()).named(name);
                let id = children.add("value", element, |_| {});
                self.record(id, at, Target::Text);
            }
            (_, Value::Asset(_)) => {
                let element = match AssetId::from_value(value) {
                    Ok(asset) => Element::label(asset.to_string(), Style::default()).class("fixed"),
                    Err(error) => {
                        Element::label(error.to_string(), Style::default()).class("muted")
                    }
                };
                children.add("value", element, |_| {});
            }
            (_, Value::Entity(_)) => self.reference(children, value, at),
            (Kind::Struct(fields), Value::Struct(values)) => {
                if fields.is_empty() {
                    muted(children, "holds no value");
                }
                self.fields(children, fields, values, at.path);
            }
            (Kind::Enum(variants), Value::Enum(current, values)) => {
                self.variant(children, name, variants, current, values, at);
                if let Some(variant) = variants.iter().find(|variant| variant.name == *current) {
                    self.fields(children, &variant.fields, values, at.path);
                }
            }
            (Kind::Option(inner), Value::Option(held)) => {
                self.option(children, name, inner, held.as_deref(), at);
            }
            (Kind::List { element, .. }, Value::List(elements)) => {
                if elements.is_empty() {
                    muted(children, "empty");
                }
                for (index, element_value) in elements.iter().enumerate() {
                    let path = at.path.clone().index(index);
                    let label = format!("[{index}]");
                    let element_name = format!("{name} {label}");
                    let at = At {
                        path: &path,
                        field: at.field,
                        element: Some(&element_name),
                        wraps: 0,
                    };
                    self.part(children, index, &label, element, element_value, at);
                }
            }
            _ => muted(children, value.kind()),
        }
    }

    /// A part of a value under its `label`, keyed by `key`: on the label's line where it fits,
    /// below it otherwise, in a section that folds, open at first.
    fn part(
        &mut self,
        children: &mut Children<'_>,
        key: impl Into<Key>,
        label: &str,
        ty: &str,
        value: &Value,
        at: At<'_>,
    ) {
        let label = labelled(label, at.field);
        let help = at.field.and_then(described);
        if self.inline(ty, value) {
            let mut row = Style::default();
            row.layout.gap = [0.0, 8.0];
            row.layout.align_items = Align::Center;
            children.add(key, Element::container(row), |children| {
                let mut style = Style::default();
                style.layout.size[0] = Dimension::Share(LABEL_SHARE);
                style.layout.shrink = 0.0;
                let mut element = Element::label(label.as_str(), style).class("muted");
                if let Some(help) = &help {
                    element = element.help(help.title.as_str(), help.text.as_str());
                }
                children.add("label", element, |_| {});
                let mut editors = growing();
                editors.layout.gap = [0.0, 4.0];
                editors.layout.align_items = Align::Center;
                children.add("editors", Element::container(editors), |children| {
                    self.value(children, ty, value, at);
                });
            });
        } else {
            let mut section = Element::section(label.as_str(), true, column(4.0));
            if let Some(help) = &help {
                section = section.help(help.title.as_str(), help.text.as_str());
            }
            children.add(key, section, |children| {
                self.value(children, ty, value, at);
            });
        }
    }

    /// The fields of a struct or a variant, `fields` as its type describes them and `values` as
    /// its value holds them, at `path`.
    fn fields(
        &mut self,
        children: &mut Children<'_>,
        fields: &[FieldInfo],
        values: &[(String, Value)],
        path: &Path,
    ) {
        for field in fields {
            if let Some((_, value)) = values.iter().find(|(name, _)| *name == field.name) {
                let path = path.clone().field(field.name.clone());
                let at = At {
                    path: &path,
                    field: Some(field),
                    element: None,
                    wraps: 0,
                };
                let key = format!("field {}", field.name);
                self.part(children, key, &field.name, &field.ty, value, at);
            }
        }
    }

    /// Whether `value`, of the type named `ty`, fits on its label's line: a value edited whole,
    /// or one with no parts.
    fn inline(&self, ty: &str, value: &Value) -> bool {
        match value {
            _ if WHOLE.contains(&ty) => true,
            Value::Struct(fields) | Value::Enum(_, fields) => fields.is_empty(),
            Value::List(elements) => elements.is_empty(),
            Value::Option(Some(held)) => match self.registry.get(ty).map(|r| &r.info().kind) {
                Some(Kind::Option(inner)) => self.inline(inner, held),
                _ => true,
            },
            _ => true,
        }
    }

    /// A number's field, named `name`, of its field's unit: a unit of a family that fields of
    /// numbers know converts what is typed, and an angle in radians shows in degrees.
    fn number(
        &mut self,
        children: &mut Children<'_>,
        name: &str,
        number: f64,
        kind: Number,
        at: At<'_>,
    ) {
        let radians = at.unit() == Some("rad");
        let mut shown = if radians { number.to_degrees() } else { number };
        // An angle that an `f32` holds is shown as an `f32` too, in degrees: the digits past it say
        // nothing of the angle.
        if matches!(kind, Number::F32) {
            shown = f64::from(shown as f32);
        }
        let step = match kind {
            Number::Int | Number::UInt => 1.0,
            Number::F32 | Number::F64 => step(shown),
        };
        let mut element = Element::number_field(shown, step, growing())
            .units(units(at.unit()))
            .named(name);
        if matches!(kind, Number::F32) {
            element = element.single_precision();
        }
        let id = children.add("value", element, |_| {});
        self.record(id, at, Target::Number { kind, radians });
    }

    /// The elements of a value of one of the types edited whole ([`WHOLE`]), at `at`; whether
    /// `ty` is one of them.
    fn whole(&mut self, children: &mut Children<'_>, ty: &str, value: &Value, at: At<'_>) -> bool {
        let name = at.name(ty);
        let units = units(at.unit());
        match ty {
            "Vec3" | "DVec3" => {
                let double = ty == "DVec3";
                let vector = if double {
                    DVec3::from_value(value).ok().map(|v| v.to_array())
                } else {
                    Vec3::from_value(value)
                        .ok()
                        .map(|v| v.to_array().map(f64::from))
                };
                let Some(vector) = vector else {
                    return false;
                };
                for (axis, (prefix, class)) in AXES.into_iter().enumerate() {
                    let mut element =
                        Element::number_field(vector[axis], step(vector[axis]), growing())
                            .prefix(prefix)
                            .units(units)
                            .class(class)
                            .named(format!("{name} {}", prefix.to_lowercase()));
                    if !double {
                        element = element.single_precision();
                    }
                    let id = children.add(prefix, element, |_| {});
                    let target = Target::Component {
                        vector,
                        axis,
                        double,
                    };
                    self.record(id, at, target);
                }
                true
            }
            "Quat" => {
                let Ok(rotation) = Quat::from_value(value) else {
                    return false;
                };
                let key = (self.scope.to_owned(), at.path.clone());
                let shown = match self.angles.get(&key) {
                    Some(&(made, angles)) if made == rotation => angles,
                    // Without the sign of a zero, which `-0` would show.
                    _ => angles(rotation).map(|angle| angle + 0.0),
                };
                for (turn, (prefix, class)) in TURNS.into_iter().enumerate() {
                    let element = Element::number_field(f64::from(shown[turn]), 0.5, growing())
                        .prefix(prefix)
                        .units(Units::Angle)
                        .single_precision()
                        .class(class)
                        .named(format!("{name} {}", prefix.to_lowercase()));
                    let id = children.add(prefix, element, |_| {});
                    let target = Target::Angle {
                        angles: shown,
                        turn,
                    };
                    self.record(id, at, target);
                }
                true
            }
            "LinearColor" => {
                let Ok(color) = LinearColor::from_value(value) else {
                    return false;
                };
                let mut swatch = Style::default();
                swatch.layout.size = [Dimension::Pixels(20.0), Dimension::Pixels(20.0)];
                swatch.layout.shrink = 0.0;
                swatch.paint.background = color;
                children.add("swatch", Element::container(swatch).class("swatch"), |_| {});
                let channels = [color.r, color.g, color.b, color.a];
                for (channel, prefix) in CHANNELS.into_iter().enumerate() {
                    let element =
                        Element::number_field(f64::from(channels[channel]), 0.01, growing())
                            .prefix(prefix)
                            .single_precision()
                            .named(format!("{name} {}", prefix.to_lowercase()));
                    let id = children.add(prefix, element, |_| {});
                    self.record(id, at, Target::Channel { color, channel });
                }
                true
            }
            _ => false,
        }
    }

    /// The list of the variants of an enum whose variants are `variants`, `current` chosen, those
    /// that a new value cannot start in disabled, each with its documentation as help.
    fn variant(
        &mut self,
        children: &mut Children<'_>,
        name: &str,
        variants: &[VariantInfo],
        current: &str,
        values: &[(String, Value)],
        at: At<'_>,
    ) {
        let taken: Vec<Option<Value>> = variants
            .iter()
            .map(|variant| {
                (variant.name != current)
                    .then(|| variant_value(self.registry, variant, values))
                    .flatten()
            })
            .collect();
        let options: Vec<ChoiceOption> = variants
            .iter()
            .zip(&taken)
            .map(|(variant, value)| ChoiceOption {
                label: variant.name.clone(),
                disabled: variant.name != current && value.is_none(),
                help: (!variant.doc.is_empty()).then(|| Tooltip {
                    title: variant.name.clone(),
                    text: plain(&variant.doc),
                }),
            })
            .collect();
        let chosen = variants
            .iter()
            .position(|variant| variant.name == current)
            .unwrap_or(0);
        let element = Element::choice(options, chosen, growing()).named(name);
        let id = children.add("variant", element, |_| {});
        self.record(id, at, Target::Variant(taken));
    }

    /// An option of values of the type named `inner`, with a box that says whether it holds one,
    /// and the value it holds, its edits wrapped in the option.
    fn option(
        &mut self,
        children: &mut Children<'_>,
        name: &str,
        inner: &str,
        held: Option<&Value>,
        at: At<'_>,
    ) {
        let range = at.field.and_then(|field| field.range);
        let start = initial(self.registry, inner, range);
        let element = Element::check_box("", held.is_some(), Style::default())
            .named(format!("{name} holds a value"));
        let id = children.add("holds", element, |_| {});
        // Nothing to start holding, as a reference: the box cannot be checked.
        self.boxes.push((id, held.is_none() && start.is_none()));
        self.record(id, at, Target::Option(start));
        if let Some(held) = held {
            children.add("held", Element::container(row(4.0)), |children| {
                let inside = At {
                    wraps: at.wraps + 1,
                    ..at
                };
                self.value(children, inner, held, inside);
            });
        }
    }

    /// A reference to an entity: a button with its name, which selects it, if it is alive; its
    /// handle outside a world.
    fn reference(&mut self, children: &mut Children<'_>, value: &Value, at: At<'_>) {
        let text = match (Entity::from_value(value), self.world) {
            (Ok(entity), Some(world)) if world.contains(entity) => {
                let element = Element::button(name(world, entity), Style::default()).class("link");
                let id = children.add("value", element, |_| {});
                self.record(id, at, Target::Reference(entity));
                return;
            }
            (Ok(entity), Some(_)) => format!("{entity:?}, despawned"),
            (Ok(entity), None) => format!("{entity:?}"),
            (Err(error), _) => error.to_string(),
        };
        muted(children, text);
    }
}

/// The help over a part's label: its field's name, then its documentation, its range, and
/// whether saves leave it out; none where there is none of them.
fn described(field: &FieldInfo) -> Option<Tooltip> {
    let mut text = plain(&field.doc);
    // A range in radians, as its field shows them: in degrees.
    match field.range {
        Some(range) if field.unit.as_deref() == Some("rad") => {
            // Rounded as an `f32`, which a bound such as π/2 is: 90, not 90.0000025.
            let degrees = |radians: f64| f64::from(radians.to_degrees() as f32);
            let degrees = Range {
                min: degrees(range.min),
                max: degrees(range.max),
                ..range
            };
            text.push_str(&format!("\nRange: {degrees}, in degrees."));
        }
        Some(range) => text.push_str(&format!("\nRange: {range}.")),
        None => {}
    }
    if field.transient {
        text.push_str("\nNot saved.");
    }
    let text = text.trim();
    (!text.is_empty()).then(|| Tooltip {
        title: field.name.clone(),
        text: text.to_owned(),
    })
}

/// A part's label, with its field's unit where fields of numbers know no family of it.
fn labelled(label: &str, field: Option<&FieldInfo>) -> String {
    match field.and_then(|field| field.unit.as_deref()) {
        Some(unit) if units(Some(unit)) == Units::None => format!("{label} ({unit})"),
        _ => label.to_owned(),
    }
}

/// The family of units of a field's `unit`: meters lengths, radians and degrees angles, seconds
/// times; none for another.
fn units(unit: Option<&str>) -> Units {
    match unit {
        Some("m") => Units::Length,
        Some("rad" | "°" | "deg") => Units::Angle,
        Some("s") => Units::Time,
        _ => Units::None,
    }
}

/// The step of a drag of a number about `number`: the power of ten at most a hundredth of it, and
/// at least a hundredth, so that a drag of a hundred pixels changes it by a tenth of its size to
/// its own size, and gives round numbers.
fn step(number: f64) -> f64 {
    let about = (number.abs() * 0.01).max(0.01);
    if about.is_finite() {
        10f64.powf(about.log10().floor())
    } else {
        0.01
    }
}

/// A style that takes the room left in its row.
fn growing() -> Style {
    let mut style = Style::default();
    style.layout.grow = 1.0;
    style.layout.basis = Dimension::Pixels(0.0);
    style.layout.min_size[0] = Dimension::Pixels(0.0);
    style
}

/// A column of children, `gap` logical pixels apart.
fn column(gap: f32) -> Style {
    let mut style = Style::default();
    style.layout.direction = FlexDirection::Column;
    style.layout.gap = [gap, 0.0];
    style
}

/// A row of children, `gap` logical pixels apart, taking the room left in its own row.
fn row(gap: f32) -> Style {
    let mut style = growing();
    style.layout.gap = [0.0, gap];
    style.layout.align_items = Align::Center;
    style
}

/// A text in the quieter color.
fn muted(children: &mut Children<'_>, text: impl Into<String>) {
    let text = text.into();
    let key = format!("muted {text}");
    children.add(
        key,
        Element::label(text, Style::default()).class("muted"),
        |_| {},
    );
}
