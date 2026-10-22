//! The world panel: the entities of a world listed by name, and the one selected shown, its
//! identity, its prefab and its components edited through reflection
//! (`docs/adr/0081-stage-5a-the-world.md`, `docs/adr/0090-stage-5b-scenes-and-prefabs.md`); and
//! the same widgets for a value outside a world, as a view's settings
//! (`docs/adr/0127-the-settings-of-a-view.md`).

use std::fmt;

use slime_assets::AssetId;
use slime_color::LinearColor;
use slime_reflect::{FieldInfo, Kind, Path, Range, Reflect, Registry, Value, VariantInfo};
use slime_world::glam::{DVec3, EulerRot, Quat, Vec3};
use slime_world::{Command, Entity, Name, PersistentId, Tick, World};

/// The width the panel opens with, in points.
const WIDTH: f32 = 380.0;
/// The height of the list of entities, in points, past which it scrolls.
const LIST_HEIGHT: f32 = 160.0;
/// The height the panel opens with at most, in points; it opens as high as the screen otherwise.
const HEIGHT: f32 = 640.0;
/// The types whose values are edited whole, on one line: their parts are numbers that mean little
/// alone, and a rotation's would not stay one if set one at a time.
const WHOLE: [&str; 4] = ["Vec3", "DVec3", "Quat", "LinearColor"];

/// A panel of the overlay that lists the entities of a world by name, with a filter, and shows the
/// one selected: its persistent identity, the prefab that it places if it is a placement's root
/// ([`World::placement`]), and its components, through the world's registry ([`World::registry`]):
/// any component, the engine's or a game's, without knowing its type.
///
/// Booleans, numbers, text, enum variants and options are edited; vectors, rotations and colors
/// whole, a rotation as three angles in degrees, turned about Y, then X, from −90° to 90°, then
/// Z. A number's unit follows it, and its field's documentation and range show when the cursor is
/// over its name. Text is set at each key typed. References to assets are shown, and a reference
/// to an entity selects it when clicked. An enum that takes another variant keeps the fields of the
/// same name and type, and starts the others as an option that starts holding a value does: at
/// false, empty, or the first of 0, 1 and the field's bounds that its range holds. A type's check
/// may refuse what they start at, as a spot light's refuses its two cones at one angle: a light
/// does not become a spot light here. A variant with a reference among its new fields cannot be
/// taken.
///
/// The panel changes nothing: it returns each change as a [`Command`], which names the entity by
/// its persistent identity, and which the caller sends to its host (`slime_app::Context::send`),
/// or applies ([`World::apply`]); the world refuses a value out of its field's range or that
/// breaks a rule of its type, and the caller hands the refusal back ([`WorldPanel::refused`]) to
/// show it. An entity without an identity, which a world has only while a scene spawns it, is
/// shown and not edited. The panel does not undo, nor add, remove or save components.
///
/// It keeps the entity selected by its handle and its identity: when the handle no longer names
/// the entity of that identity, as after a load or a hot reload replaced the world, it finds the
/// entity again by its identity.
///
/// It makes its list of entities again only when the world's structure moves
/// ([`World::structure`]): a world of many entities costs a frame of the panel little while they
/// keep their names and parents. Shown another world, as after a hot reload, it lists the new one
/// once their structures' ticks or their numbers of entities differ.
#[derive(Debug, Default)]
pub struct WorldPanel {
    filter: String,
    /// Each entity listed, what the panel calls it, and that in lower case for the filter.
    list: Vec<(Entity, String, String)>,
    /// The world's structure, and its number of entities, when the list was made.
    listed: Option<(Tick, usize)>,
    /// The entity selected, by its handle, which may be of a world replaced since, and its
    /// identity, once the panel showed it.
    selected: Option<Entity>,
    selected_id: Option<PersistentId>,
    /// Why the world refused the last edit, until the next one.
    refusal: Option<String>,
}

impl WorldPanel {
    /// The entity whose components the panel shows, if one is selected, alive or not, by the
    /// handle that it last found it under.
    pub fn selected(&self) -> Option<Entity> {
        self.selected
    }

    /// Shows the components of `entity`.
    pub fn select(&mut self, entity: Entity) {
        self.selected = Some(entity);
        self.selected_id = None;
        self.refusal = None;
    }

    /// The entity selected in `world`: the handle while it names the entity of the identity seen,
    /// or else the entity of that identity, found again.
    fn find_selected(&mut self, world: &World) -> Option<Entity> {
        let entity = self.selected?;
        let Some(id) = &self.selected_id else {
            self.selected_id = world.persistent_id(entity);
            return world.contains(entity).then_some(entity);
        };
        if world.persistent_id(entity).as_ref() == Some(id) {
            return Some(entity);
        }
        let found = world.find(id)?;
        self.selected = Some(found);
        Some(found)
    }

    /// Shows `error`, why the world refused an edit, until the panel asks for another one.
    pub fn refused(&mut self, error: &dyn fmt::Display) {
        self.refusal = Some(error.to_string());
    }

    /// Builds the panel, a window of `ctx`, from `world`, and returns the commands that its widgets
    /// ask for in this pass, in their order, for the caller to send.
    ///
    /// egui may lay a frame out twice ([`Overlay::build`](crate::Overlay::build)): the caller adds
    /// the commands of each call to those of the frame, rather than keep the last call's.
    pub fn show(&mut self, ctx: &egui::Context, world: &World) -> Vec<Command> {
        let mut edits = Vec::new();
        let right = ctx.content_rect().right();
        egui::Window::new("World")
            .default_pos(egui::pos2((right - WIDTH - 12.0).max(12.0), 12.0))
            .default_width(WIDTH)
            .default_height((ctx.content_rect().height() - 24.0).clamp(LIST_HEIGHT, HEIGHT))
            .vscroll(true)
            .show(ctx, |ui| self.contents(ui, world, &mut edits));
        if !edits.is_empty() {
            self.refusal = None;
        }
        edits
    }

    fn contents(&mut self, ui: &mut egui::Ui, world: &World, edits: &mut Vec<Command>) {
        ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("filter by name"));
        let filter = self.filter.to_lowercase();
        // Found again before the list shows which entity is selected.
        let selected = self.find_selected(world);
        egui::ScrollArea::vertical()
            .id_salt("entities")
            .max_height(LIST_HEIGHT)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let now = Some((world.structure(), world.len()));
                if self.listed != now {
                    self.list = world
                        .entities()
                        .map(|entity| {
                            let name = name(world, entity);
                            let lower = name.to_lowercase();
                            (entity, name, lower)
                        })
                        .collect();
                    self.listed = now;
                }
                let mut clicked = None;
                for (entity, name, lower) in &self.list {
                    if lower.contains(&filter)
                        && ui
                            .selectable_label(selected == Some(*entity), name.as_str())
                            .clicked()
                    {
                        clicked = Some(*entity);
                    }
                }
                if let Some(entity) = clicked {
                    self.select(entity);
                }
            });
        ui.separator();
        // Again: a click on the list selects another.
        let Some(entity) = self.find_selected(world) else {
            ui.label("Select an entity to see its components.");
            return;
        };
        ui.heading(name(world, entity));
        // What files and saves name it by, and what it places, if it is a placement's root.
        let id = world.persistent_id(entity);
        match &id {
            Some(id) => ui.label(format!("Identity: {id}")),
            None => ui.weak("No identity yet: not edited"),
        };
        if let Some(placement) = world.placement(entity) {
            let prefab = placement.prefab();
            ui.label(format!(
                "Places the prefab `{}`, {}",
                prefab.name(),
                prefab.id()
            ));
        }
        if let Some(refusal) = &self.refusal {
            ui.colored_label(ui.visuals().error_fg_color, refusal);
        }
        let mut editor = Editor {
            registry: world.registry(),
            world: Some(world),
            edits: Vec::new(),
            select: None,
        };
        for component in world.component_names(entity) {
            let Some(value) = world.component_value(entity, component) else {
                continue;
            };
            let header = egui::CollapsingHeader::new(component)
                .id_salt(component)
                .default_open(true)
                .show(ui, |ui| {
                    editor.value(ui, component, &value, &Path::default(), None);
                });
            if let Some(registration) = world.registry().get(component)
                && !registration.info().doc.is_empty()
            {
                header
                    .header_response
                    .on_hover_text(&registration.info().doc);
            }
            let asked = editor.edits.drain(..);
            if let Some(id) = &id {
                edits.extend(asked.map(|(path, value)| Command::SetProperty {
                    entity: id.clone(),
                    component: component.to_owned(),
                    path,
                    value,
                }));
            }
        }
        if let Some(entity) = editor.select {
            self.select(entity);
        }
    }
}

/// What the panel calls `entity`: its name, or its handle if it has none.
fn name(world: &World, entity: Entity) -> String {
    match world.get::<Name>(entity) {
        Some(Name(name)) if !name.is_empty() => name.clone(),
        _ => format!("{entity:?}"),
    }
}

/// The widgets of `value`, a value of a type that reflection describes, as the world panel shows a
/// component's ([`WorldPanel`] says how each kind of value is edited), under `id`, which tells
/// apart the widgets of values shown side by side: changes `value` as they ask, each change checked
/// against its field's range, and returns whether they changed it. A change that the type refuses,
/// as a number out of its field's range, leaves it as it is.
///
/// It describes the type at each call, which suits the few small values of an overlay's panel,
/// such as a view's settings (`slime_render::ViewSettings`): not a value of many parts at each
/// frame.
pub fn edit_reflected<T: Reflect>(ui: &mut egui::Ui, id: &str, value: &mut T) -> bool {
    let mut registry = Registry::new();
    registry.register::<T>();
    let reflected = value.to_value();
    let mut editor = Editor {
        registry: &registry,
        world: None,
        edits: Vec::new(),
        select: None,
    };
    ui.push_id(id, |ui| {
        editor.value(ui, &T::type_name(), &reflected, &Path::default(), None);
    });
    set_edits(&registry, value, reflected, editor.edits)
}

/// Sets each of `edits`, a part's path and its new value, in `reflected`, the value of `value`,
/// whose type `registry` describes, and `value` to the result: returns whether one was set. One
/// that the type refuses, as a number out of its field's range, is left out.
fn set_edits<T: Reflect>(
    registry: &Registry,
    value: &mut T,
    mut reflected: Value,
    edits: Vec<(Path, Value)>,
) -> bool {
    let ty = T::type_name();
    let mut changed = false;
    for (path, part) in edits {
        changed |= registry.set(&ty, &mut reflected, &path, part).is_ok();
    }
    match T::from_value(&reflected) {
        Ok(new) if changed => {
            *value = new;
            true
        }
        _ => false,
    }
}

/// Builds the widgets of a component's value, or of a value outside a world, and collects the
/// edits they ask for.
struct Editor<'a> {
    registry: &'a Registry,
    /// The world whose entities a reference names; none for a value outside a world.
    world: Option<&'a World>,
    /// The parts to set, by their paths from the component, and their new values.
    edits: Vec<(Path, Value)>,
    /// The entity of a reference clicked, which the panel selects.
    select: Option<Entity>,
}

impl Editor<'_> {
    /// The widgets of `value`, a value of the type named `ty` at `path` in the component, which is
    /// `field`, or an element of it, or the component itself if `None`.
    fn value(
        &mut self,
        ui: &mut egui::Ui,
        ty: &str,
        value: &Value,
        path: &Path,
        field: Option<&FieldInfo>,
    ) {
        let unit = field.and_then(|field| field.unit.as_deref());
        let id = ui.make_persistent_id(path.to_string());
        if let Some(changed) = whole(ui, id, ty, value, unit) {
            self.edits.extend(changed.map(|new| (path.clone(), new)));
            return;
        }
        let Some(registration) = self.registry.get(ty) else {
            return;
        };
        let new = match (&registration.info().kind, value) {
            (_, Value::Bool(on)) => {
                let mut on = *on;
                ui.add(egui::Checkbox::without_text(&mut on))
                    .changed()
                    .then_some(Value::Bool(on))
            }
            (_, Value::Int(number)) => edited(ui, *number, unit).map(Value::Int),
            (_, Value::UInt(number)) => edited(ui, *number, unit).map(Value::UInt),
            (_, Value::F32(number)) => edited(ui, *number, unit).map(Value::F32),
            (_, Value::F64(number)) => edited(ui, *number, unit).map(Value::F64),
            (_, Value::Text(text)) => {
                let mut text = text.clone();
                ui.text_edit_singleline(&mut text)
                    .changed()
                    .then_some(Value::Text(text))
            }
            (_, Value::Asset(_)) => {
                match AssetId::from_value(value) {
                    Ok(asset) => ui.monospace(asset.to_string()),
                    Err(error) => ui.weak(error.to_string()),
                };
                None
            }
            (_, Value::Entity(_)) => {
                self.reference(ui, value);
                None
            }
            (Kind::Struct(fields), Value::Struct(values)) => {
                if fields.is_empty() {
                    ui.weak("holds no value");
                }
                self.fields(ui, fields, values, path);
                None
            }
            (Kind::Enum(variants), Value::Enum(current, values)) => {
                let chosen = self.variant(ui, variants, current, values, path);
                if let Some(variant) = variants.iter().find(|variant| variant.name == *current) {
                    self.fields(ui, &variant.fields, values, path);
                }
                chosen
            }
            (Kind::Option(inner), Value::Option(held)) => {
                self.option(ui, inner, held.as_deref(), path, field);
                None
            }
            (Kind::List { element, .. }, Value::List(elements)) => {
                if elements.is_empty() {
                    ui.weak("empty");
                }
                for (index, element_value) in elements.iter().enumerate() {
                    let at = path.clone().index(index);
                    self.field(
                        ui,
                        &format!("[{index}]"),
                        element,
                        element_value,
                        &at,
                        field,
                    );
                }
                None
            }
            _ => {
                ui.weak(value.kind());
                None
            }
        };
        self.edits.extend(new.map(|new| (path.clone(), new)));
    }

    /// The fields of a struct or a variant, `fields` as its type describes them and `values` as
    /// its value holds them, at `path`.
    fn fields(
        &mut self,
        ui: &mut egui::Ui,
        fields: &[FieldInfo],
        values: &[(String, Value)],
        path: &Path,
    ) {
        for field in fields {
            if let Some((_, value)) = values.iter().find(|(name, _)| *name == field.name) {
                let at = path.clone().field(field.name.clone());
                self.field(ui, &field.name, &field.ty, value, &at, Some(field));
            }
        }
    }

    /// A part of a value under its `label`: on the label's line if it fits there, below it
    /// otherwise, in a header that folds.
    fn field(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        ty: &str,
        value: &Value,
        path: &Path,
        field: Option<&FieldInfo>,
    ) {
        if self.inline(ty, value) {
            ui.horizontal(|ui| {
                described(ui.label(label), field);
                self.value(ui, ty, value, path, field);
            });
        } else {
            let header = egui::CollapsingHeader::new(label)
                .id_salt(path.to_string())
                .default_open(true)
                .show(ui, |ui| self.value(ui, ty, value, path, field));
            described(header.header_response, field);
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

    /// The variant `current` of an enum whose variants are `variants`, and the variants it can
    /// take instead, in a menu; returns the value of the one chosen, if one is.
    fn variant(
        &self,
        ui: &mut egui::Ui,
        variants: &[VariantInfo],
        current: &str,
        values: &[(String, Value)],
        path: &Path,
    ) -> Option<Value> {
        let mut chosen = None;
        egui::ComboBox::from_id_salt(format!("variant {path}"))
            .selected_text(current)
            .show_ui(ui, |ui| {
                for variant in variants {
                    let selected = variant.name == current;
                    let value = if selected {
                        None
                    } else {
                        variant_value(self.registry, variant, values)
                    };
                    let item = egui::Button::selectable(selected, variant.name.as_str());
                    let mut response = ui.add_enabled(selected || value.is_some(), item);
                    if !variant.doc.is_empty() {
                        response = response.on_hover_text(&variant.doc);
                    }
                    if response.clicked() {
                        chosen = value;
                    }
                }
            });
        chosen
    }

    /// An option of values of the type named `inner`, with a box that says whether it holds one,
    /// and the value it holds.
    fn option(
        &mut self,
        ui: &mut egui::Ui,
        inner: &str,
        held: Option<&Value>,
        path: &Path,
        field: Option<&FieldInfo>,
    ) {
        let registry = self.registry;
        let start = || initial(registry, inner, field.and_then(|field| field.range));
        let mut holds = held.is_some();
        let toggle = egui::Checkbox::without_text(&mut holds);
        if ui
            .add_enabled(held.is_some() || start().is_some(), toggle)
            .changed()
        {
            let new = if holds {
                start().map(|value| Value::Option(Some(Box::new(value))))
            } else {
                Some(Value::Option(None))
            };
            self.edits.extend(new.map(|new| (path.clone(), new)));
        }
        if let Some(held) = held {
            let first = self.edits.len();
            self.value(ui, inner, held, path, field);
            // An edit of the held value as a whole sets the option, which holds it; the paths of
            // its parts go through the option.
            for (at, new) in &mut self.edits[first..] {
                if at == path {
                    let value = std::mem::replace(new, Value::Option(None));
                    *new = Value::Option(Some(Box::new(value)));
                }
            }
        }
    }

    /// A reference to an entity: its name, which selects it when clicked, if it is alive; its
    /// handle outside a world.
    fn reference(&mut self, ui: &mut egui::Ui, value: &Value) {
        match (Entity::from_value(value), self.world) {
            (Ok(entity), Some(world)) if world.contains(entity) => {
                if ui.link(name(world, entity)).clicked() {
                    self.select = Some(entity);
                }
            }
            (Ok(entity), Some(_)) => {
                ui.weak(format!("{entity:?}, despawned"));
            }
            (Ok(entity), None) => {
                ui.weak(format!("{entity:?}"));
            }
            (Err(error), _) => {
                ui.weak(error.to_string());
            }
        }
    }
}

/// The widgets of a value of one of the types edited whole ([`WHOLE`]), with `unit`, under `id`;
/// `None` for a value of another type, and otherwise its new value if the widgets changed it.
fn whole(
    ui: &mut egui::Ui,
    id: egui::Id,
    ty: &str,
    value: &Value,
    unit: Option<&str>,
) -> Option<Option<Value>> {
    let changed = match ty {
        "Vec3" => {
            let mut vector = Vec3::from_value(value).ok()?;
            let Vec3 { x, y, z } = &mut vector;
            numbers(ui, [x, y, z], unit).then(|| vector.to_value())
        }
        "DVec3" => {
            let mut vector = DVec3::from_value(value).ok()?;
            let DVec3 { x, y, z } = &mut vector;
            numbers(ui, [x, y, z], unit).then(|| vector.to_value())
        }
        "Quat" => turned(ui, id, Quat::from_value(value).ok()?).map(|turned| turned.to_value()),
        "LinearColor" => {
            let color = LinearColor::from_value(value).ok()?;
            let mut rgba = egui::Rgba::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
            let alpha = egui::color_picker::Alpha::OnlyBlend;
            egui::color_picker::color_edit_button_rgba(ui, &mut rgba, alpha)
                .changed()
                .then(|| {
                    let [r, g, b, a] = rgba.to_rgba_unmultiplied();
                    LinearColor { r, g, b, a }.to_value()
                })
        }
        _ => return None,
    };
    Some(changed)
}

/// The widgets of `rotation`: its angles in degrees, about Y, then X, from −90° to 90°, then Z;
/// returns the rotation they make if they changed.
///
/// The angles last edited are kept under `id` while the rotation is the one they made: computed
/// from it again, they would come back inexact, and the widget would drop the text being typed,
/// and past 90° about X as other angles, which a drag would then turn back at each frame.
fn turned(ui: &mut egui::Ui, id: egui::Id, rotation: Quat) -> Option<Quat> {
    let kept = ui.data(|data| data.get_temp::<(Quat, [f32; 3])>(id));
    let mut degrees = match kept {
        Some((made, degrees)) if made == rotation => degrees,
        _ => angles(rotation),
    };
    let [y, x, z] = &mut degrees;
    let mut changed = false;
    for (axis, angle) in [("Y", y), ("X", x), ("Z", z)] {
        let mut widget = egui::DragValue::new(angle)
            .speed(0.5)
            .prefix(format!("{axis} "))
            .suffix("°");
        if axis == "X" {
            widget = widget.range(-90.0..=90.0);
        }
        changed |= ui.add(widget).changed();
    }
    if !changed {
        return None;
    }
    let made = self::rotation(degrees);
    ui.data_mut(|data| data.insert_temp(id, (made, degrees)));
    Some(made)
}

/// A number's widget, which the cursor drags or the keyboard types into, with `unit` after it;
/// returns the new number if it changed. The widget does not hold the number within its field's
/// range: the world refuses one out of it, and says why.
fn edited<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    mut number: N,
    unit: Option<&str>,
) -> Option<N> {
    drag(ui, &mut number, unit).then_some(number)
}

/// Three numbers' widgets, side by side; returns whether one changed.
fn numbers<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    numbers: [&mut N; 3],
    unit: Option<&str>,
) -> bool {
    let mut changed = false;
    for number in numbers {
        changed |= drag(ui, number, unit);
    }
    changed
}

/// A number's widget, with `unit` after it; returns whether it changed the number.
fn drag<N: egui::emath::Numeric>(ui: &mut egui::Ui, number: &mut N, unit: Option<&str>) -> bool {
    let before = *number;
    // A drag of a hundred points changes a float by about its own size, and by 1 at least.
    let speed = if N::INTEGRAL {
        0.1
    } else {
        (before.to_f64().abs() * 0.01).max(0.01)
    };
    // The number shown as it is, NaN included, rather than brought within the widget's range, which
    // would change it without a hand on it.
    let mut widget = egui::DragValue::new(number)
        .speed(speed)
        .clamp_existing_to_range(false);
    if let Some(unit) = unit {
        widget = widget.suffix(format!(" {unit}"));
    }
    let changed = ui.add(widget).changed();
    // egui finds a number that is not one (NaN) changed at each frame, since it is not equal to
    // itself.
    let after = *number;
    changed && after != before && !(after.to_f64().is_nan() && before.to_f64().is_nan())
}

/// `rotation`'s angles in degrees, turned about Y, then X, then Z.
fn angles(rotation: Quat) -> [f32; 3] {
    let (y, x, z) = rotation.to_euler(EulerRot::YXZ);
    [y, x, z].map(f32::to_degrees)
}

/// The rotation turned by `angles` in degrees about Y, then X, then Z.
fn rotation(angles: [f32; 3]) -> Quat {
    let [y, x, z] = angles.map(f32::to_radians);
    Quat::from_euler(EulerRot::YXZ, y, x, z).normalize()
}

/// Shows the documentation of `field`, its range, and whether saves leave it out, when the cursor
/// is over `response`, its name.
fn described(response: egui::Response, field: Option<&FieldInfo>) {
    let Some(field) = field else {
        return;
    };
    let mut text = field.doc.clone();
    if let Some(range) = field.range {
        text.push_str(&format!("\nRange: {range}."));
    }
    if field.transient {
        text.push_str("\nNot saved.");
    }
    let text = text.trim();
    if !text.is_empty() {
        response.on_hover_text(text);
    }
}

/// A value of `variant`, whose fields are those of `values`, the variant that a value holds, of
/// the same name and type, and start as [`initial`] says otherwise; `None` if one cannot.
fn variant_value(
    registry: &Registry,
    variant: &VariantInfo,
    values: &[(String, Value)],
) -> Option<Value> {
    let fields = variant
        .fields
        .iter()
        .map(|field| {
            let kept = values
                .iter()
                .find(|(name, value)| {
                    *name == field.name && registry.check(&field.ty, value).is_ok()
                })
                .map(|(_, value)| value.clone());
            let value = kept.or_else(|| initial(registry, &field.ty, field.range))?;
            Some((field.name.clone(), value))
        })
        .collect::<Option<_>>()?;
    Some(Value::Enum(variant.name.clone(), fields))
}

/// A value of the type named `ty` to start a part at, when an option starts holding one or an enum
/// takes a variant with new fields: false, empty text, an empty list, an option that holds none,
/// the first variant that can start, and for a number the first of 0, 1 and the bounds of `range`
/// that `range` holds. `None` for a reference to an asset or an entity, which has no value to
/// start at, and for a value that holds one.
fn initial(registry: &Registry, ty: &str, range: Option<Range>) -> Option<Value> {
    // Each candidate made a number of the field's kind first, as the world checks it in that kind.
    let number = |of_kind: fn(f64) -> Value| {
        let range = range.unwrap_or(Range::ALL);
        [0.0, 1.0, range.min, range.max]
            .into_iter()
            .map(of_kind)
            .find(|number| number.number().is_some_and(|number| range.holds(number)))
            .unwrap_or_else(|| of_kind(0.0))
    };
    Some(match &registry.get(ty)?.info().kind {
        Kind::Bool => Value::Bool(false),
        Kind::Int { .. } => number(|number| Value::Int(number as i64)),
        Kind::UInt { .. } => number(|number| Value::UInt(number as u64)),
        Kind::F32 => number(|number| Value::F32(number as f32)),
        Kind::F64 => number(Value::F64),
        Kind::Text => Value::Text(String::new()),
        Kind::Option(_) => Value::Option(None),
        Kind::List { len: None, .. } => Value::List(Vec::new()),
        Kind::List {
            element,
            len: Some(len),
        } => Value::List(vec![initial(registry, element, range)?; *len]),
        Kind::Struct(fields) => Value::Struct(
            fields
                .iter()
                .map(|field| {
                    Some((
                        field.name.clone(),
                        initial(registry, &field.ty, field.range)?,
                    ))
                })
                .collect::<Option<_>>()?,
        ),
        Kind::Enum(variants) => variants
            .iter()
            .find_map(|variant| variant_value(registry, variant, &[]))?,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use slime_assets::Extra;
    use slime_world::{Component, Extras, Hidden, Light, Meshes, Transform};

    use super::*;

    /// A component that only derives, with a value of each kind the panel shows.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Everything")]
    struct Everything {
        on: bool,
        count: i32,
        small: u8,
        ratio: f64,
        /// How far it reaches.
        #[reflect(range(above = 0.0), unit = "m")]
        reach: Option<f32>,
        label: String,
        mesh: AssetId,
        target: Entity,
        offset: DVec3,
        axis: Vec3,
        turn: Quat,
        tint: Option<LinearColor>,
        mode: Mode,
        weights: Vec<f32>,
        corners: [u16; 2],
        pair: (String, Extra),
        #[reflect(transient, default)]
        seen: bool,
    }
    impl Component for Everything {}

    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Mode")]
    enum Mode {
        Off,
        Fixed(f32),
        Aimed { at: Entity, speed: f32 },
    }

    /// A world with an entity for each of the engine's components, each variant of a light, and
    /// one for a component that only derives, which refers to the first.
    fn world() -> (World, Vec<Entity>) {
        let mut world = World::new();
        let named = world.spawn();
        world.insert(named, Name("door".to_owned()));
        world.insert(
            named,
            Transform::from_translation(DVec3::new(1.0, 2.0, 3.0)),
        );
        world.insert(named, Hidden);
        world.insert(
            named,
            Meshes {
                ids: vec![AssetId::from_u128(7)],
                casts_shadows: true,
            },
        );
        world.insert(
            named,
            Extras(vec![
                ("open".to_owned(), Extra::Bool(true)),
                ("height".to_owned(), Extra::Number(2.5)),
                ("kind".to_owned(), Extra::Text("oak".to_owned())),
            ]),
        );
        let mut entities = vec![named];
        for light in [
            Light::Directional {
                color: LinearColor::WHITE,
                illuminance: 1.0,
                casts_shadows: true,
                shadow_distance: Some(20.0),
                in_sky: true,
            },
            Light::Point {
                color: LinearColor::WHITE,
                intensity: 30.0,
                range: None,
            },
            Light::Spot {
                color: LinearColor::WHITE,
                intensity: 30.0,
                range: Some(10.0),
                inner_cone_angle: 0.2,
                outer_cone_angle: 0.4,
                casts_shadows: false,
            },
        ] {
            let lit = world.spawn();
            world.insert(lit, light);
            entities.push(lit);
        }
        let everything = world.spawn();
        world.insert(
            everything,
            Everything {
                on: true,
                count: -3,
                small: 7,
                ratio: 0.5,
                reach: Some(4.0),
                label: "every kind".to_owned(),
                mesh: AssetId::from_u128(9),
                target: named,
                offset: DVec3::X,
                axis: Vec3::Y,
                turn: Quat::from_rotation_y(0.5),
                tint: Some(LinearColor::WHITE),
                mode: Mode::Aimed {
                    at: named,
                    speed: 2.0,
                },
                weights: vec![0.25, 0.75],
                corners: [1, 2],
                pair: ("name".to_owned(), Extra::Number(1.0)),
                seen: false,
            },
        );
        entities.push(everything);
        (world, entities)
    }

    /// Builds `panel` over `world` for a frame of a context without a window, and returns the
    /// commands it asks for.
    fn frame(ctx: &egui::Context, panel: &mut WorldPanel, world: &World) -> Vec<Command> {
        let mut edits = Vec::new();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            edits.extend(panel.show(ui.ctx(), world));
        })
        .drop_without_applying_deltas();
        edits
    }

    #[test]
    fn the_panel_is_built_for_every_component() {
        let (world, entities) = world();
        let mut shown: Vec<&str> = entities
            .iter()
            .flat_map(|&entity| world.component_names(entity))
            .collect();
        shown.sort_unstable();
        shown.dedup();
        assert_eq!(
            shown,
            [
                "Extras",
                "Hidden",
                "Light",
                "Meshes",
                "Name",
                "Transform",
                "test::Everything"
            ],
            "every component type is shown"
        );
        let ctx = egui::Context::default();
        let mut panel = WorldPanel::default();
        assert!(
            frame(&ctx, &mut panel, &world).is_empty(),
            "nothing selected"
        );
        for &entity in &entities {
            panel.select(entity);
            // egui sizes the window and its grids on the first frames.
            for _ in 0..3 {
                assert_eq!(frame(&ctx, &mut panel, &world), [], "nothing is touched");
            }
        }
    }

    #[test]
    fn a_placement_shows_its_identity_and_its_prefab() {
        let (yard, lamp) = (AssetId::from_u128(1), AssetId::from_u128(2));
        let mut scenes = slime_world::SceneLibrary::new();
        scenes.insert_file(
            lamp,
            "lamp",
            "[entities.post.components]\nName = \"post\"\n",
        );
        scenes.insert_file(
            yard,
            "yard",
            "[assets]\nlamp = { id = \"00000000-0000-0000-0000-000000000002\" }\n\n\
             [entities.porch]\nprefab = \"lamp\"\n",
        );
        let mut world = World::new();
        let scene = scenes.resolve(world.registry(), yard).expect("resolved");
        world.spawn_scene("yard", &scene).expect("spawned");
        let porch = world
            .find(&slime_world::PersistentId::new(["yard", "porch"]))
            .expect("the placement");
        let mut panel = WorldPanel::default();
        panel.select(porch);
        let ctx = egui::Context::default();
        assert_eq!(frame(&ctx, &mut panel, &world), []);
        // The texts that the next frame draws.
        let mut texts = Vec::new();
        let mut shapes: Vec<egui::Shape> = ctx
            .run_ui(egui::RawInput::default(), |ui| {
                panel.show(ui.ctx(), &world);
            })
            .shapes
            .into_iter()
            .map(|clipped| clipped.shape)
            .collect();
        while let Some(shape) = shapes.pop() {
            match shape {
                egui::Shape::Text(text) => texts.push(text.galley.text().to_owned()),
                egui::Shape::Vec(inner) => shapes.extend(inner),
                _ => {}
            }
        }
        for shown in [
            "Identity: yard/porch",
            "Places the prefab `lamp`, 00000000-0000-0000-0000-000000000002",
        ] {
            assert!(texts.iter().any(|text| text == shown), "{shown}: {texts:?}");
        }
    }

    #[test]
    fn a_despawned_entity_shows_nothing() {
        let (mut world, entities) = world();
        let mut panel = WorldPanel::default();
        panel.select(entities[0]);
        world.despawn(entities[0]);
        let ctx = egui::Context::default();
        assert_eq!(frame(&ctx, &mut panel, &world), []);
        assert_eq!(panel.selected(), Some(entities[0]), "kept, as a handle");
    }

    #[test]
    fn the_entity_selected_is_found_again_by_its_identity_in_a_world_that_replaces_it() {
        let lamp = Light::Point {
            color: LinearColor::WHITE,
            intensity: 30.0,
            range: None,
        };
        let mut world = World::new();
        let first = world.spawn_keyed("lamp").expect("a key");
        world.insert(first, lamp);
        let mut panel = WorldPanel::default();
        panel.select(first);
        let ctx = egui::Context::default();
        frame(&ctx, &mut panel, &world);
        // Another world, as a load makes, where the handle names another entity.
        let mut replaced = World::new();
        let post = replaced.spawn_keyed("post").expect("a key");
        let found = replaced.spawn_keyed("lamp").expect("a key");
        replaced.insert(found, lamp);
        assert_eq!(post, first, "the handle names the post there");
        frame(&ctx, &mut panel, &replaced);
        assert_eq!(panel.selected(), Some(found));
        replaced.despawn(found);
        frame(&ctx, &mut panel, &replaced);
        assert_eq!(
            panel.selected(),
            Some(found),
            "kept while no entity has the identity"
        );
    }

    #[test]
    fn values_start_as_values_of_their_types() {
        let (world, _) = world();
        let registry = world.registry();
        for registration in registry.iter() {
            let name = &registration.info().name;
            if let Some(value) = initial(registry, name, None) {
                registry
                    .check(name, &value)
                    .unwrap_or_else(|error| panic!("`{name}` starts at a value of it: {error}"));
            }
        }
        let starts = |name: &str| initial(registry, name, None);
        assert_eq!(starts("AssetId"), None, "no asset to refer to");
        assert_eq!(starts("test::Everything"), None, "it holds a reference");
        assert_eq!(
            starts("test::Mode"),
            Some(Value::Enum("Off".to_owned(), Vec::new()))
        );
        assert_eq!(
            starts("[u16; 2]"),
            Some(Value::List(vec![Value::UInt(0); 2]))
        );
        let above = Range {
            min: 0.0,
            min_excluded: true,
            ..Range::ALL
        };
        assert_eq!(initial(registry, "f32", Some(above)), Some(Value::F32(1.0)));
        // A bound that a 32-bit float does not hold: the float nearest it, which the range holds.
        let blend = Range {
            min: 0.7,
            max: 0.9,
            ..Range::ALL
        };
        let start = initial(registry, "f32", Some(blend)).expect("a number");
        assert_eq!(start, Value::F32(0.7));
        assert!(start.number().is_some_and(|number| blend.holds(number)));
    }

    #[test]
    fn a_new_variant_keeps_the_fields_it_shares() {
        let (mut world, entities) = world();
        let point = entities[2];
        let light = world.component_value(point, "Light").expect("held");
        let Value::Enum(_, values) = &light else {
            panic!("a light is an enum");
        };
        let Some(Kind::Enum(variants)) = world.registry().get("Light").map(|r| &r.info().kind)
        else {
            panic!("a light is an enum");
        };
        let variant = |name: &str| {
            let variant = variants.iter().find(|v| v.name == name).expect("a variant");
            variant_value(world.registry(), variant, values).expect("its fields start")
        };
        let (spot, directional) = (variant("Spot"), variant("Directional"));
        // Its two cones start at the same angle, which a spot light cannot have.
        let refused = world
            .set_property(point, "Light", &Path::default(), spot)
            .expect_err("a spot light's inner cone is narrower than its outer one");
        assert!(refused.to_string().contains("its cones"), "{refused}");
        world
            .set_property(point, "Light", &Path::default(), directional)
            .expect("a light");
        assert_eq!(
            world.get::<Light>(point),
            Some(&Light::Directional {
                color: LinearColor::WHITE,
                illuminance: 0.0,
                casts_shadows: false,
                shadow_distance: None,
                in_sky: false,
            }),
            "the color is kept"
        );
        let Some(Kind::Enum(modes)) = world.registry().get("test::Mode").map(|r| &r.info().kind)
        else {
            panic!("an enum");
        };
        assert_eq!(
            variant_value(world.registry(), &modes[1], &[]),
            Some(Value::Enum(
                "Fixed".to_owned(),
                vec![("0".to_owned(), Value::F32(0.0))]
            ))
        );
        assert_eq!(
            variant_value(world.registry(), &modes[2], &[]),
            None,
            "no entity to aim at"
        );
    }

    #[test]
    fn a_rotation_is_edited_as_three_angles() {
        let turned = rotation([30.0, 20.0, 10.0]);
        assert!(turned.is_normalized());
        for (angle, expected) in angles(turned).into_iter().zip([30.0, 20.0, 10.0]) {
            assert!((angle - expected).abs() < 1e-3, "{angle} for {expected}");
        }
        let expected = Quat::from_rotation_y(30f32.to_radians())
            * Quat::from_rotation_x(20f32.to_radians())
            * Quat::from_rotation_z(10f32.to_radians());
        assert!(
            turned.angle_between(expected) < 1e-4,
            "about Y, then X, then Z"
        );
    }

    /// A value of the type named `ty` edited alone, in a context without a window, frame after
    /// frame, the edits its widgets ask for set in it as a world sets them in a component.
    struct Bench {
        ctx: egui::Context,
        world: World,
        ty: String,
        value: Value,
    }

    impl Bench {
        fn new(ty: &str, value: Value) -> Self {
            let mut world = World::new();
            world.register::<Everything>();
            Self {
                ctx: egui::Context::default(),
                world,
                ty: ty.to_owned(),
                value,
            }
        }

        /// Draws a frame given `events`, and sets in the value the edits its widgets ask for.
        fn frame(&mut self, events: Vec<egui::Event>) {
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
            let input = egui::RawInput {
                events,
                screen_rect: Some(screen),
                ..egui::RawInput::default()
            };
            let mut edits = Vec::new();
            self.ctx
                .run_ui(input, |ui| {
                    let mut editor = Editor {
                        registry: self.world.registry(),
                        world: Some(&self.world),
                        edits: Vec::new(),
                        select: None,
                    };
                    editor.value(ui, &self.ty, &self.value, &Path::default(), None);
                    edits.extend(editor.edits);
                })
                .drop_without_applying_deltas();
            for (path, new) in edits {
                let registry = self.world.registry();
                registry
                    .set(&self.ty, &mut self.value, &path, new)
                    .expect("a value of its type");
            }
        }

        /// The rectangles of the widgets of the last frame that the cursor clicks or drags, in
        /// their order.
        fn widgets(&self) -> Vec<egui::Rect> {
            self.ctx.viewport(|viewport| {
                viewport
                    .prev_pass
                    .widgets
                    .get_layer(egui::LayerId::background())
                    .filter(|widget| widget.sense.senses_click() || widget.sense.senses_drag())
                    .map(|widget| widget.rect)
                    .collect()
            })
        }

        /// Presses the primary button at `at`, moves the cursor by each of `moves`, a frame each,
        /// and releases it: a click when there are none.
        fn drag(&mut self, at: egui::Pos2, moves: &[egui::Vec2]) {
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            self.frame(vec![egui::Event::PointerMoved(at), button(at, true)]);
            let mut pos = at;
            for &step in moves {
                pos += step;
                self.frame(vec![egui::Event::PointerMoved(pos)]);
            }
            self.frame(vec![button(pos, false)]);
        }

        /// Types `text`, a character a frame, then Enter.
        fn type_text(&mut self, text: &str) {
            for character in text.chars() {
                self.frame(vec![egui::Event::Text(character.to_string())]);
            }
            self.frame(vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
    }

    #[test]
    fn an_angle_is_set_as_typed() {
        let mut bench = Bench::new("Quat", rotation([30.0, 0.0, 0.0]).to_value());
        bench.frame(Vec::new());
        let widgets = bench.widgets();
        assert_eq!(widgets.len(), 3, "an angle about each axis");
        // A click on the angle about Y selects its text, which what is typed replaces.
        bench.drag(widgets[0].center(), &[]);
        bench.type_text("45.5");
        let typed = Quat::from_value(&bench.value).expect("a rotation");
        let expected = rotation([45.5, 0.0, 0.0]);
        assert!(typed.angle_between(expected) < 1e-5, "{:?}", angles(typed));
    }

    #[test]
    fn an_angle_dragged_about_x_stops_at_90_degrees() {
        let mut bench = Bench::new("Quat", rotation([10.0, 85.0, 0.0]).to_value());
        bench.frame(Vec::new());
        let about_x = bench.widgets()[1];
        // Half a degree a point: 48 points would turn it by 24°.
        bench.drag(about_x.center(), &[egui::vec2(4.0, 0.0); 12]);
        let dragged = Quat::from_value(&bench.value).expect("a rotation");
        let expected = rotation([10.0, 90.0, 0.0]);
        assert!(
            dragged.angle_between(expected) < 1e-5,
            "{:?}",
            angles(dragged)
        );
    }

    #[test]
    fn a_number_typed_into_an_option_sets_the_option() {
        let held = |number| Value::Option(Some(Box::new(Value::F32(number))));
        let mut bench = Bench::new("Option<f32>", held(4.0));
        bench.frame(Vec::new());
        let widgets = bench.widgets();
        assert_eq!(
            widgets.len(),
            2,
            "whether it holds a number, and the number"
        );
        bench.drag(widgets[1].center(), &[]);
        bench.type_text("6");
        assert_eq!(bench.value, held(6.0));
        bench.drag(widgets[0].center(), &[]);
        assert_eq!(
            bench.value,
            Value::Option(None),
            "it holds none once unticked"
        );
    }

    #[test]
    fn a_number_that_is_not_one_is_left_as_it_is() {
        let mut bench = Bench::new("f32", Value::F32(f32::NAN));
        for _ in 0..3 {
            bench.frame(Vec::new());
        }
        assert!(matches!(bench.value, Value::F32(number) if number.is_nan()));
    }

    /// Settings outside a world, as a view's.
    #[derive(Clone, Debug, PartialEq, Reflect)]
    #[reflect(name = "test::Knobs")]
    struct Knobs {
        /// How large.
        #[reflect(range(min = 0.25, max = 2.0))]
        scale: f32,
        /// Whether it is lit.
        lit: bool,
    }

    #[test]
    fn a_value_outside_a_world_takes_the_edits_that_its_ranges_hold() {
        let mut registry = Registry::new();
        registry.register::<Knobs>();
        let mut knobs = Knobs {
            scale: 1.0,
            lit: false,
        };
        let path = |text: &str| text.parse::<Path>().expect("a path");
        let edits = vec![
            (path("scale"), Value::F32(5.0)),
            (path("lit"), Value::Bool(true)),
        ];
        let reflected = knobs.to_value();
        assert!(set_edits(&registry, &mut knobs, reflected, edits));
        assert_eq!(
            knobs,
            Knobs {
                scale: 1.0,
                lit: true
            },
            "the scale out of its range is left out"
        );
        let reflected = knobs.to_value();
        let edits = vec![(path("scale"), Value::F32(0.1))];
        assert!(!set_edits(&registry, &mut knobs, reflected, edits));
        assert_eq!(knobs.scale, 1.0);
        let reflected = knobs.to_value();
        let edits = vec![(path("scale"), Value::F32(0.5))];
        assert!(set_edits(&registry, &mut knobs, reflected, edits));
        assert_eq!(knobs.scale, 0.5);
    }
}
