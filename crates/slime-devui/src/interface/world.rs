//! The world panel on the engine's interface
//! (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`): the entities of a world listed
//! by name, the list building only the rows that show, and the one selected shown, its identity,
//! its prefab and its components edited through reflection.

use std::fmt;

use slime_reflect::Value;
use slime_ui::action::{Action, ActionKind, FieldValue};
use slime_ui::style::{Dimension, FlexDirection, Style};
use slime_ui::tree::{Element, ElementId, Ui};
use slime_world::{Command, Entity, PersistentId, Tick, World};

use super::values::{Asked, ValueEditor};
use crate::world_panel::name;

/// The height of a row of the list of entities, in logical pixels.
const ROW_HEIGHT: f32 = 22.0;
/// The height of the list of entities, in logical pixels, past which it scrolls.
const LIST_HEIGHT: f32 = 160.0;

/// A panel that lists the entities of a world by name, with a filter, and shows the one selected:
/// its persistent identity, the prefab that it places if it is a placement's root
/// ([`World::placement`]), and its components, through the world's registry ([`World::registry`]),
/// each edited as [`ValueEditor`] edits a value: any component, the engine's or a game's, without
/// knowing its type. It is built as elements of the engine's interface, under a parent that the
/// program gives ([`WorldPanel::build`]).
///
/// The list builds only the rows that show, each a button with the entity's name, or its handle,
/// which selects it: a world of 100 000 entities costs a frame of the panel as one of a few
/// hundred. It makes its list of entities again only when the world's structure moves
/// ([`World::structure`]), and filters it again only when the filter or the list changes; the
/// filter keeps the entities whose names hold its text, ignoring case.
///
/// The panel changes nothing: it gives each change as a [`Command`], which names the entity by its
/// persistent identity, and which the caller sends to its host (`slime_app::Context::send`), or
/// applies ([`World::apply`]); the world refuses a value out of its field's range or that breaks a
/// rule of its type, and the caller hands the refusal back ([`WorldPanel::refused`]) to show it
/// until the next edit or selection. An entity without an identity, which a world has only while a
/// scene spawns it, is shown and not edited. A field changes a component only when the user changed
/// it. The panel does not undo, nor add, remove or save components.
///
/// It keeps the entity selected by its handle and its identity: when the handle no longer names
/// the entity of that identity, as after a load or a hot reload replaced the world, it finds the
/// entity again by its identity.
#[derive(Debug, Default)]
pub struct WorldPanel {
    filter: String,
    /// Each entity listed, what the panel calls it, and that in lower case for the filter.
    list: Vec<(Entity, String, String)>,
    /// The world's structure, and its number of entities, when the list was made.
    listed: Option<(Tick, usize)>,
    /// The places in the list of the entities that the filter keeps, and the filter and the list
    /// that they were found for.
    matches: Vec<usize>,
    matched: Option<(String, Option<(Tick, usize)>)>,
    /// The entity selected, by its handle, which may be of a world replaced since, and its
    /// identity, once the panel showed it.
    selected: Option<Entity>,
    selected_id: Option<PersistentId>,
    /// Why the world refused the last edit, until the next one.
    refusal: Option<String>,
    editor: ValueEditor,
    /// The filter's field, the list, and each row of the list built last, with its entity.
    filter_field: Option<ElementId>,
    list_slot: Option<ElementId>,
    rows: Vec<(ElementId, Entity)>,
    /// What the panel last showed, and the revisions of its builds and of its rows'.
    shown: Option<Shown>,
    revision: u64,
    rows_revision: u64,
}

/// What a build of the panel shows, which a change of builds it again.
#[derive(Debug, PartialEq)]
struct Shown {
    /// The world's structure, which the names of the entities that references name follow.
    structure: Tick,
    selected: Option<Details>,
    refusal: Option<String>,
    rows: u64,
}

/// What the panel shows of the entity selected.
#[derive(Debug, PartialEq)]
struct Details {
    entity: Entity,
    name: String,
    identity: Option<PersistentId>,
    /// What the line of the prefab that it places says, if it places one.
    prefab: Option<String>,
    /// Its components, each its name and its value.
    components: Vec<(String, Value)>,
}

impl Details {
    /// What the panel shows of `entity` in `world`.
    fn of(world: &World, entity: Entity) -> Self {
        let components = world
            .component_names(entity)
            .into_iter()
            .filter_map(|component| {
                let value = world.component_value(entity, component)?;
                Some((component.to_owned(), value))
            })
            .collect();
        let prefab = world.placement(entity).map(|placement| {
            let prefab = placement.prefab();
            format!("Places the prefab `{}`, {}", prefab.name(), prefab.id())
        });
        Self {
            entity,
            name: name(world, entity),
            identity: world.persistent_id(entity),
            prefab,
            components,
        }
    }
}

impl WorldPanel {
    /// The entity whose components the panel shows, if one is selected, alive or not, by the
    /// handle that it last found it under.
    pub fn selected(&self) -> Option<Entity> {
        self.selected
    }

    /// Shows the components of `entity`.
    pub fn select(&mut self, entity: Entity) {
        if self.selected != Some(entity) {
            self.editor.forget_angles();
        }
        self.selected = Some(entity);
        self.selected_id = None;
        self.refusal = None;
    }

    /// Shows `error`, why the world refused an edit, until the panel asks for another one.
    pub fn refused(&mut self, error: &dyn fmt::Display) {
        self.refusal = Some(error.to_string());
    }

    /// Builds the panel under `parent`, a root, a layer or an element that the program added for it
    /// (`Children::add_slot`), from `world`: its filter, its list, whose rows it builds where they
    /// show, and the entity selected. It builds again only once what it shows changed: the
    /// components of the entity selected, its name, identity or prefab, the world's structure, the
    /// selection, the filter or a refusal.
    ///
    /// # Panics
    ///
    /// If `parent` names nothing.
    pub fn build(&mut self, ui: &mut Ui, parent: ElementId, world: &World) {
        self.list_entities(world);
        let selected = self.find_selected(world);
        let shown = Shown {
            structure: world.structure(),
            selected: selected.map(|entity| Details::of(world, entity)),
            refusal: self.refusal.clone(),
            rows: self.rows_revision,
        };
        if self.shown.as_ref() != Some(&shown) {
            self.revision += 1;
            self.shown = Some(shown);
        }
        let Some(shown) = self.shown.take() else {
            return;
        };
        let mut ran = false;
        let (mut filter_field, mut list_slot) = (self.filter_field, self.list_slot);
        ui.build(parent, self.revision, |children| {
            ran = true;
            let field = Element::search_field(self.filter.as_str(), Style::default())
                .placeholder("Filter by name")
                .named("filter by name");
            filter_field = Some(children.add("filter", field, |_| {}));
            let mut list = Style::default();
            list.layout.size[1] = Dimension::Pixels(LIST_HEIGHT);
            list.layout.shrink = 0.0;
            let list = Element::list(self.matches.len(), ROW_HEIGHT, list).named("entities");
            list_slot = Some(children.add_slot("list", list));
            let mut details = Style::default();
            details.layout.direction = FlexDirection::Column;
            details.layout.gap = [6.0, 0.0];
            children.add("selected", Element::container(details), |children| {
                let Some(details) = &shown.selected else {
                    let text = "Select an entity to see its components.";
                    let none = Element::label(text, Style::default()).class("muted");
                    children.add("none", none, |_| {});
                    return;
                };
                let heading =
                    Element::label(details.name.as_str(), Style::default()).class("heading");
                children.add("name", heading, |_| {});
                // What files and saves name it by, and what it places, if it is a placement's root.
                let identity = match &details.identity {
                    Some(id) => Element::label(format!("Identity: {id}"), Style::default()),
                    None => Element::label("No identity yet: not edited", Style::default())
                        .class("muted"),
                };
                children.add("identity", identity, |_| {});
                if let Some(prefab) = &details.prefab {
                    children.add(
                        "prefab",
                        Element::label(prefab.as_str(), Style::default()),
                        |_| {},
                    );
                }
                if let Some(refusal) = &shown.refusal {
                    let element = Element::label(refusal.as_str(), Style::default()).class("error");
                    children.add("refusal", element, |_| {});
                }
                let mut components = Style::default();
                components.layout.direction = FlexDirection::Column;
                components.layout.gap = [4.0, 0.0];
                children.add("components", Element::container(components), |children| {
                    self.editor
                        .build_components(children, world, &details.components);
                });
            });
        });
        self.shown = Some(shown);
        self.filter_field = filter_field;
        self.list_slot = list_slot;
        if ran {
            self.editor.settle(ui);
        }
        self.build_rows(ui, selected);
    }

    /// Builds the list's rows that show, again where the list shows others or changed, and marks
    /// the one selected.
    fn build_rows(&mut self, ui: &mut Ui, selected: Option<Entity>) {
        let Some(slot) = self.list_slot else {
            return;
        };
        let mut rows = None;
        ui.build(slot, self.rows_revision, |children| {
            let mut built = Vec::new();
            for row in children.rows() {
                let Some(&(entity, ref text, _)) =
                    self.matches.get(row).and_then(|&at| self.list.get(at))
                else {
                    continue;
                };
                let element = Element::button(text.as_str(), Style::default()).class("row");
                built.push((children.add(row, element, |_| {}), entity));
            }
            rows = Some(built);
        });
        if let Some(rows) = rows {
            self.rows = rows;
        }
        for &(row, entity) in &self.rows {
            if let Some(mut state) = ui.state(row) {
                let chosen = Some(entity) == selected;
                if state.selected != chosen {
                    state.selected = chosen;
                    ui.set_state(row, state);
                }
            }
        }
    }

    /// Takes from `actions`, the actions that the program took from the interface
    /// (`Ui::take_actions`), those of the panel's elements, and gives the commands that they ask
    /// for, in their order: the filter typed, a row pressed, a reference pressed, which select, and
    /// the edits of the entity selected, sent only for an entity with an identity. The others are
    /// left in `actions`, in their order.
    pub fn act(&mut self, actions: &mut Vec<Action>, world: &World) -> Vec<Command> {
        let mut commands = Vec::new();
        let mut select = None;
        actions.retain(|action| {
            let Some(element) = action.element else {
                return true;
            };
            if Some(element) == self.filter_field {
                if let ActionKind::Changed(FieldValue::Text(text)) = &action.kind {
                    self.filter.clone_from(text);
                }
                return false;
            }
            if let Some(&(_, entity)) = self.rows.iter().find(|(row, _)| *row == element) {
                if action.kind == ActionKind::Press {
                    select = Some(entity);
                }
                return false;
            }
            if !self.editor.holds(element) {
                return true;
            }
            match self.editor.asked(action) {
                Some(Asked::Select(entity)) => select = Some(entity),
                Some(Asked::Set { scope, path, value }) => {
                    let id = self.selected.and_then(|entity| world.persistent_id(entity));
                    if let Some(id) = id {
                        commands.push(Command::SetProperty {
                            entity: id,
                            component: scope,
                            path,
                            value,
                        });
                    }
                }
                None => {}
            }
            false
        });
        if !commands.is_empty() {
            self.refusal = None;
        }
        if let Some(entity) = select {
            self.select(entity);
        }
        commands
    }

    /// Makes the list of entities again where the world's structure moved, and finds again those
    /// that the filter keeps where the list or the filter changed.
    fn list_entities(&mut self, world: &World) {
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
        let matched = Some((self.filter.clone(), self.listed));
        if self.matched != matched {
            let filter = self.filter.to_lowercase();
            self.matches = (0..self.list.len())
                .filter(|&at| self.list[at].2.contains(&filter))
                .collect();
            self.matched = matched;
            self.rows_revision += 1;
        }
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
}
