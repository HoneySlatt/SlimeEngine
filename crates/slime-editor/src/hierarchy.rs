//! The hierarchy (`docs/adr/0167-the-hierarchy.md`): the world's entities as a tree of rows, which
//! the editor flattens from the world's children and builds only where they show, folded and
//! unfolded, selected, searched and renamed.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use slime_devui::entity_name;
use slime_reflect::{Reflect, Value};
use slime_ui::action::{Action, ActionKind, FieldValue, Intent};
use slime_ui::look;
use slime_ui::style::{Dimension, FlexDirection, Style};
use slime_ui::tree::{Element, ElementId, Role, Ui};
use slime_world::{Command, Entity, Name, PersistentId, Tick, World};

use crate::shared::Request;

/// The height of a row, in logical pixels.
const ROW_HEIGHT: f32 = 22.0;

/// How far a level of the tree indents its rows, in logical pixels.
const INDENT: f32 = 14.0;

/// The width of the mark that folds and unfolds a row, in logical pixels, which a row without
/// children leaves empty.
const FOLD_WIDTH: f32 = 18.0;

/// How long the hierarchy's search waits once the typing paused before it runs
/// ([`Hierarchy::set_search`]).
pub const SEARCH_PAUSE: Duration = Duration::from_millis(125);

/// The class of the hierarchy's list, the scope of its shortcuts, `editor.keys`' `[hierarchy]`.
pub(crate) const SCOPE: &str = "hierarchy";

/// A row of the hierarchy: an entity, its depth, and whether it has children.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Row {
    entity: Entity,
    depth: usize,
    children: bool,
}

/// The elements of a row built last.
#[derive(Clone, Copy, Debug)]
struct Built {
    entity: Entity,
    item: ElementId,
    fold: Option<ElementId>,
    name: ElementId,
}

/// What the rows built last showed, which a change of builds them again.
#[derive(Clone, Debug, PartialEq)]
struct Shown {
    rows: u64,
    names: Tick,
    selection: Vec<Entity>,
    renaming: Option<Entity>,
}

/// The editor's hierarchy (`docs/adr/0167-the-hierarchy.md`): the world's entities as a tree of
/// rows, depth first from its roots, the world's roots unfolded as it is first built, which the
/// editor's window shows, folds, selects, searches and renames, and which `slime-benches` measures
/// over a world of 100 000 entities. It makes its rows again only when the world's structure, the
/// folds or the search change, and builds only those that show, their names read as they are built,
/// so that a rename builds again the rows in sight alone.
#[derive(Debug, Default)]
pub struct Hierarchy {
    rows: Vec<Row>,
    /// The world's structure, and the count of the folds and the search, when the rows were made.
    made: Option<(Tick, u64)>,
    /// The entities whose children show, the user's.
    open: HashSet<Entity>,
    /// Whether the world's roots were opened, as the level opened.
    rooted: bool,
    /// Counted up each time the folds or what the search found change.
    changes: u64,
    /// The search's text, when it was last typed while it waits, what it found while it holds
    /// text, and the world's structure and names that it ran on.
    search: String,
    typed: Option<Instant>,
    found: Option<Vec<Entity>>,
    searched: (Tick, Tick),
    /// An entity whose row is to show at the next build, its ancestors unfolded.
    reveal: Option<Entity>,
    /// The row last clicked, from which Shift and a click select.
    anchor: Option<Entity>,
    /// The entity whose row holds a field of its name, while it is renamed, and whether the field
    /// was given the focus.
    renaming: Option<Entity>,
    rename_focused: bool,
    /// The entity whose row takes the focus once it is built, as the arrows or a rename's end ask.
    focus: Option<Entity>,
    /// Whether the selection that the core shows next is the hierarchy's own, which it shows
    /// already.
    quiet: bool,
    /// The count of the builds of the hierarchy's elements and of its rows, and what was shown.
    revision: u64,
    rows_revision: u64,
    made_rows: u64,
    shown: Option<Shown>,
    search_field: Option<ElementId>,
    list: Option<ElementId>,
    built: Vec<Built>,
}

impl Hierarchy {
    /// A level opened in place of the other: nothing folded or searched of the other is kept; the
    /// counts of the builds go on, so that the elements of the other are built again.
    pub(crate) fn opened(&mut self) {
        *self = Self {
            revision: self.revision + 1,
            rows_revision: self.rows_revision + 1,
            made_rows: self.made_rows + 1,
            ..Self::default()
        };
    }

    /// Builds the hierarchy under `slot`, an element made by a slot of its parent
    /// (`slime_ui::tree::Children::add_slot`), from `world`, the rows of `selection` selected, at
    /// `now`, which a search typed waits a pause from: its search field and its list, then the
    /// rows that show, which the list's layout knows once it is laid out, a frame later.
    ///
    /// # Panics
    ///
    /// If `slot` names nothing.
    pub fn build(
        &mut self,
        ui: &mut Ui,
        slot: ElementId,
        world: &World,
        selection: &[Entity],
        now: Instant,
    ) {
        if !self.rooted {
            self.rooted = true;
            self.open.extend(roots(world));
            self.changes += 1;
        }
        self.run_search(world, now);
        if let Some(entity) = self.reveal {
            for ancestor in ancestors(world, entity) {
                if self.open.insert(ancestor) {
                    self.changes += 1;
                }
            }
        }
        let made = Some((world.structure(), self.changes));
        if self.made != made {
            self.rows = match &self.found {
                Some(found) => found
                    .iter()
                    .filter(|&&entity| world.contains(entity))
                    .map(|&entity| Row {
                        entity,
                        depth: 0,
                        children: false,
                    })
                    .collect(),
                None => flatten(world, &self.open),
            };
            self.made = made;
            self.made_rows += 1;
            self.revision += 1;
        }
        let (mut field, mut list) = (self.search_field, self.list);
        let rows = self.rows.len();
        let search = self.search.clone();
        ui.build(slot, self.revision, |children| {
            let field_element = Element::search_field(search.as_str(), Style::default())
                .placeholder("Search: a name, c:component, p:prefab")
                .named("search the hierarchy");
            field = Some(children.add("search", field_element, |_| {}));
            let mut style = Style::default();
            style.layout.grow = 1.0;
            style.layout.min_size[1] = Dimension::Pixels(0.0);
            let element = Element::list(rows, ROW_HEIGHT, style)
                .role(Role::Tree)
                .class(SCOPE)
                .named("hierarchy");
            list = Some(children.add_slot("rows", element));
        });
        (self.search_field, self.list) = (field, list);
        let Some(list) = self.list else {
            return;
        };
        let shown = Shown {
            rows: self.made_rows,
            names: world.names(),
            selection: selection.to_vec(),
            renaming: self.renaming,
        };
        if self.shown.as_ref() != Some(&shown) {
            self.rows_revision += 1;
            self.shown = Some(shown);
        }
        let mut built = None;
        ui.build(list, self.rows_revision, |children| {
            let mut rows = Vec::new();
            for index in children.rows() {
                let Some(&row) = self.rows.get(index) else {
                    continue;
                };
                let open = self.open.contains(&row.entity);
                rows.push(build_row(
                    children,
                    index,
                    row,
                    world,
                    self.renaming,
                    &self.found,
                    open,
                ));
            }
            built = Some(rows);
        });
        if let Some(built) = built {
            self.built = built;
            for row in &self.built {
                let selected = selection.contains(&row.entity);
                let expanded = self.open.contains(&row.entity);
                for (element, expanded) in [(row.item, expanded), (row.name, false)] {
                    let mut state = ui.state(element).unwrap_or_default();
                    (state.selected, state.expanded) = (selected, expanded);
                    ui.set_state(element, state);
                }
            }
        }
        // The rename's field takes the focus once; the rename ends once the focus left it, as a
        // click elsewhere does, the field committing what was typed.
        if let Some(entity) = self.renaming
            && let Some(row) = self.built.iter().find(|row| row.entity == entity)
        {
            if !self.rename_focused {
                ui.set_focus(Some(row.name));
                self.rename_focused = true;
            } else if ui.focus() != Some(row.name) {
                self.renaming = None;
            }
        }
        if let Some(entity) = self.focus
            && let Some(row) = self.built.iter().find(|row| row.entity == entity)
        {
            ui.set_focus(Some(row.name));
            self.focus = None;
        }
        if let Some(entity) = self.reveal.take()
            && let Some(index) = self.rows.iter().position(|row| row.entity == entity)
        {
            ui.show_row(list, index);
        }
    }

    /// Searches `text`, typed at `at`, as the search field does: the search runs at the first build
    /// once the typing paused, [`SEARCH_PAUSE`] after `at`; an empty text shows the tree again.
    pub fn set_search(&mut self, text: &str, at: Instant) {
        text.clone_into(&mut self.search);
        self.typed = Some(at);
    }

    /// Shows the row of `entity` at the next build, its ancestors unfolded, as a selection made
    /// elsewhere asks.
    pub(crate) fn reveal(&mut self, entity: Entity) {
        if !std::mem::take(&mut self.quiet) {
            self.reveal = Some(entity);
        }
    }

    /// Takes from `actions` those of the hierarchy's elements: the search typed, a row pressed,
    /// which selects as `modifiers` say, with `selection` selected now, a fold pressed, and the
    /// rename's field committed or given up. Gives the core's requests that they make.
    pub(crate) fn act(
        &mut self,
        actions: &mut Vec<Action>,
        world: &World,
        selection: &[Entity],
        modifiers: slime_platform::Modifiers,
        now: Instant,
    ) -> Vec<Request> {
        let mut requests = Vec::new();
        actions.retain(|action| {
            let Some(element) = action.element else {
                return true;
            };
            // What else reaches its elements, a shortcut's command among them, goes on to the
            // window.
            if Some(element) == self.search_field {
                let ActionKind::Changed(FieldValue::Text(text)) = &action.kind else {
                    return true;
                };
                self.set_search(text, now);
                return false;
            }
            if let Some(row) = self.built.iter().find(|row| row.fold == Some(element)) {
                if action.kind != ActionKind::Press {
                    return true;
                }
                let entity = row.entity;
                if !self.open.remove(&entity) {
                    self.open.insert(entity);
                }
                self.changes += 1;
                return false;
            }
            let Some(row) = self.built.iter().find(|row| row.name == element).copied() else {
                return true;
            };
            if self.renaming == Some(row.entity) {
                match &action.kind {
                    ActionKind::Committed(FieldValue::Text(text)) => {
                        requests.extend(rename(world, row.entity, text));
                    }
                    ActionKind::Intent(Intent::Cancel | Intent::Accept) => {}
                    _ => return true,
                }
                self.renaming = None;
                self.focus = Some(row.entity);
                return false;
            }
            if action.kind != ActionKind::Press {
                return true;
            }
            let chosen = self.select(row.entity, selection, modifiers);
            // Its own selection needs no showing.
            self.quiet = true;
            requests.push(Request::Select(identities(world, &chosen)));
            false
        });
        requests
    }

    /// What a press of `entity`'s row selects, `selection` selected now: the entity alone; with
    /// Ctrl, the selection with it added or taken out; with Shift, the rows from the one last
    /// pressed to this one.
    fn select(
        &mut self,
        entity: Entity,
        selection: &[Entity],
        modifiers: slime_platform::Modifiers,
    ) -> Vec<Entity> {
        if modifiers.shift
            && let Some(anchor) = self.anchor
            && let (Some(from), Some(to)) = (self.index_of(anchor), self.index_of(entity))
        {
            let (low, high) = (from.min(to), from.max(to));
            return self.rows[low..=high].iter().map(|row| row.entity).collect();
        }
        self.anchor = Some(entity);
        if modifiers.control {
            let mut chosen: Vec<Entity> = selection.to_vec();
            match chosen.iter().position(|&each| each == entity) {
                Some(at) => {
                    chosen.remove(at);
                }
                None => chosen.push(entity),
            }
            return chosen;
        }
        vec![entity]
    }

    /// Runs the hierarchy's operation `name` on what is selected, `selection`: the row above or
    /// below selected, a row folded or its parent selected, unfolded or its first child selected,
    /// the rename of the entity selected started. Gives the core's requests that it makes; none for
    /// an operation that is not the hierarchy's.
    pub(crate) fn run(
        &mut self,
        name: &str,
        world: &World,
        selection: &[Entity],
    ) -> Option<Vec<Request>> {
        let current = selection.last().copied();
        let at = current.and_then(|entity| self.index_of(entity));
        let chosen = match name {
            "hierarchy.previous" => match at {
                Some(at) => at.checked_sub(1),
                None => self.rows.len().checked_sub(1),
            }
            .map(|index| self.rows[index].entity),
            "hierarchy.next" => at
                .map_or(Some(0), |at| (at + 1 < self.rows.len()).then_some(at + 1))
                .filter(|&index| index < self.rows.len())
                .map(|index| self.rows[index].entity),
            "hierarchy.fold" => {
                let entity = current?;
                if self.open.remove(&entity) && self.has_children(entity) {
                    self.changes += 1;
                    None
                } else {
                    world.parent(entity)
                }
            }
            "hierarchy.unfold" => {
                let entity = current?;
                if !self.has_children(entity) {
                    None
                } else if self.open.insert(entity) {
                    self.changes += 1;
                    None
                } else {
                    world.children(entity).next()
                }
            }
            "edit.rename" => {
                self.renaming = current;
                self.rename_focused = false;
                if let Some(entity) = current {
                    self.reveal = Some(entity);
                }
                return Some(Vec::new());
            }
            _ => return None,
        };
        Some(match chosen {
            Some(entity) => {
                self.anchor = Some(entity);
                self.reveal = Some(entity);
                self.focus = Some(entity);
                vec![Request::Select(identities(world, &[entity]))]
            }
            None => {
                self.focus = current;
                Vec::new()
            }
        })
    }

    /// When the search runs, once the typing paused, which the next frame is asked for; none while
    /// it waits for nothing.
    pub(crate) fn waits(&self) -> Option<Instant> {
        self.typed.map(|typed| typed + SEARCH_PAUSE)
    }

    /// The place of `entity`'s row, if it shows.
    fn index_of(&self, entity: Entity) -> Option<usize> {
        self.rows.iter().position(|row| row.entity == entity)
    }

    /// Whether `entity`'s row has children, as the rows were last made.
    fn has_children(&self, entity: Entity) -> bool {
        self.rows
            .iter()
            .find(|row| row.entity == entity)
            .is_some_and(|row| row.children)
    }

    /// Runs the search once the typing paused, at `now`, and again when the world's structure or
    /// names moved since it ran.
    fn run_search(&mut self, world: &World, now: Instant) {
        let paused = self
            .typed
            .is_some_and(|typed| now.duration_since(typed) >= SEARCH_PAUSE);
        let moved = self.found.is_some() && self.searched != (world.structure(), world.names());
        if !paused && !moved {
            return;
        }
        self.typed = None;
        self.found = (!self.search.trim().is_empty()).then(|| search(world, &self.search));
        self.searched = (world.structure(), world.names());
        self.changes += 1;
    }
}

/// Adds `row`, the row of number `index`, to `children`: its indent, the mark that folds or unfolds
/// it if it has children, and its name, or the field that renames it; a row that the search found
/// says the path of its parent too.
fn build_row(
    children: &mut slime_ui::tree::Children<'_>,
    index: usize,
    row: Row,
    world: &World,
    renaming: Option<Entity>,
    found: &Option<Vec<Entity>>,
    open: bool,
) -> Built {
    let mut style = Style::default();
    style.layout.direction = FlexDirection::Row;
    let name = entity_name(world, row.entity);
    let mut item = Element::container(style)
        .role(Role::TreeItem)
        .named(name.clone())
        .level(row.depth + 1);
    if row.children {
        item = item.expandable();
    }
    let (mut fold, mut name_element) = (None, None);
    let element = children.add(index, item, |children| {
        let mut indent = Style::default();
        indent.layout.size[0] = Dimension::Pixels(row.depth as f32 * INDENT);
        indent.layout.shrink = 0.0;
        children.add("indent", Element::container(indent), |_| {});
        let mut mark = Style::default();
        mark.layout.size[0] = Dimension::Pixels(FOLD_WIDTH);
        mark.layout.shrink = 0.0;
        if row.children {
            let chevron = if open {
                "chevron-down"
            } else {
                "chevron-right"
            };
            let icon = look::icon(chevron).map(String::from).unwrap_or_default();
            let name = if open { "fold" } else { "unfold" };
            let button = Element::button(icon, mark)
                .class("row")
                .class("fold")
                .named(name);
            fold = Some(children.add("fold", button, |_| {}));
        } else {
            children.add("leaf", Element::container(mark), |_| {});
        }
        let mut grows = Style::default();
        grows.layout.grow = 1.0;
        let shown = match found {
            Some(_) => match world.parent(row.entity) {
                Some(parent) => format!("{name}  —  {}", path_of(world, parent)),
                None => name.clone(),
            },
            None => name.clone(),
        };
        let element = if renaming == Some(row.entity) {
            // Its name, or nothing for an entity without one, not its handle.
            let text = world
                .get::<Name>(row.entity)
                .map_or_else(String::new, |name| name.0.clone());
            Element::text_field(text, grows).named("rename")
        } else {
            Element::button(shown, grows)
                .class("row")
                .named(name.as_str())
        };
        name_element = Some(children.add("name", element, |_| {}));
        // A placement's root, marked as what a prefab placed.
        if world.placement(row.entity).is_some() {
            let icon = look::icon("package").map(String::from).unwrap_or_default();
            let mark = Element::label(icon, Style::default())
                .class("icon")
                .named("placement");
            children.add("placement", mark, |_| {});
        }
    });
    Built {
        entity: row.entity,
        item: element,
        fold,
        name: name_element.unwrap_or(element),
    }
}

/// The world's roots: its entities without a parent, in the world's order.
fn roots(world: &World) -> Vec<Entity> {
    world
        .entities()
        .filter(|&entity| world.parent(entity).is_none())
        .collect()
}

/// The ancestors of `entity`, its parent first.
fn ancestors(world: &World, entity: Entity) -> Vec<Entity> {
    let mut ancestors = Vec::new();
    let mut at = world.parent(entity);
    while let Some(parent) = at {
        ancestors.push(parent);
        at = world.parent(parent);
    }
    ancestors
}

/// The rows of `world`'s tree, depth first from its roots, each parent's children in their order,
/// the children of those that `open` holds alone shown.
fn flatten(world: &World, open: &HashSet<Entity>) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut stack: Vec<(Entity, usize)> = roots(world).into_iter().rev().map(|e| (e, 0)).collect();
    while let Some((entity, depth)) = stack.pop() {
        let children = world.children(entity).len() > 0;
        rows.push(Row {
            entity,
            depth,
            children,
        });
        if children && open.contains(&entity) {
            stack.extend(world.children(entity).rev().map(|child| (child, depth + 1)));
        }
    }
    rows
}

/// The entities of `world` that `text` finds: each of its terms, separated by spaces, holds, any
/// case: a name's text; `c:` and a text, a component whose type's name holds it; `p:` and a text,
/// the prefab that a placement's root places, whose name holds it.
pub(crate) fn search(world: &World, text: &str) -> Vec<Entity> {
    let terms: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    world
        .entities()
        .filter(|&entity| {
            terms.iter().all(|term| {
                if let Some(component) = term.strip_prefix("c:") {
                    world
                        .component_names(entity)
                        .iter()
                        .any(|name| holds(name, component))
                } else if let Some(prefab) = term.strip_prefix("p:") {
                    world
                        .placement(entity)
                        .is_some_and(|placement| holds(placement.prefab().name(), prefab))
                } else {
                    match world.get::<Name>(entity) {
                        Some(name) if !name.0.is_empty() => holds(&name.0, term),
                        _ => holds(&entity_name(world, entity), term),
                    }
                }
            })
        })
        .collect()
}

/// Whether `text` holds `lower`, a text in lower case, whatever the case of `text`: without a copy
/// of `text` where both are ASCII, which a search over 100 000 names spares as many copies
/// (`editor.hierarchy.100k`).
fn holds(text: &str, lower: &str) -> bool {
    if text.is_ascii() && lower.is_ascii() {
        lower.is_empty()
            || text
                .as_bytes()
                .windows(lower.len())
                .any(|window| window.eq_ignore_ascii_case(lower.as_bytes()))
    } else {
        text.to_lowercase().contains(lower)
    }
}

/// The path of `entity`, its ancestors' names and its own, from the root.
fn path_of(world: &World, entity: Entity) -> String {
    let mut names: Vec<String> = ancestors(world, entity)
        .into_iter()
        .rev()
        .map(|ancestor| entity_name(world, ancestor))
        .collect();
    names.push(entity_name(world, entity));
    names.join(" / ")
}

/// The persistent identities of `entities`, those that have one.
fn identities(world: &World, entities: &[Entity]) -> Vec<PersistentId> {
    entities
        .iter()
        .filter_map(|&entity| world.persistent_id(entity))
        .collect()
}

/// The step that renames `entity` to `text`, if it has an identity and the text is a new name:
/// its `Name` set, or given where it has none.
fn rename(world: &World, entity: Entity, text: &str) -> Option<Request> {
    let id = world.persistent_id(entity)?;
    let text = text.trim();
    if text.is_empty() || world.get::<Name>(entity).is_some_and(|name| name.0 == text) {
        return None;
    }
    let command = if world.get::<Name>(entity).is_some() {
        Command::SetProperty {
            entity: id,
            component: "Name".to_owned(),
            path: "0".parse().expect("the path of a name's text"),
            value: Value::Text(text.to_owned()),
        }
    } else {
        Command::Insert {
            entity: id,
            component: "Name".to_owned(),
            value: Name(text.to_owned()).to_value(),
        }
    };
    Some(Request::Apply {
        label: format!("Rename to {text}"),
        commands: vec![command],
        selection: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world of a root with two children, the first with a child of its own.
    fn tree() -> (World, [Entity; 4]) {
        let mut world = World::new();
        let names = ["yard", "well", "lamp", "post"];
        let entities = names.map(|name| {
            let entity = world.spawn();
            world.insert(entity, Name(name.to_owned()));
            entity
        });
        let [yard, well, lamp, post] = entities;
        world.set_parent(well, Some(yard)).expect("alive");
        world.set_parent(lamp, Some(yard)).expect("alive");
        world.set_parent(post, Some(well)).expect("alive");
        (world, entities)
    }

    #[test]
    fn the_rows_are_the_tree_depth_first_the_folded_children_left_out() {
        let (world, [yard, well, lamp, post]) = tree();
        let entities = |rows: &[Row]| {
            rows.iter()
                .map(|row| (row.entity, row.depth))
                .collect::<Vec<_>>()
        };
        let open = HashSet::from([yard]);
        assert_eq!(
            entities(&flatten(&world, &open)),
            [(yard, 0), (well, 1), (lamp, 1)]
        );
        let open = HashSet::from([yard, well]);
        assert_eq!(
            entities(&flatten(&world, &open)),
            [(yard, 0), (well, 1), (post, 2), (lamp, 1)]
        );
        assert!(flatten(&world, &open)[1].children);
        assert!(!flatten(&world, &open)[3].children);
    }

    #[test]
    fn a_text_holds_another_whatever_its_case() {
        assert!(holds("Lamp Post", "post"));
        assert!(holds("anything", ""));
        assert!(!holds("lamp", "post"));
        assert!(holds("Énorme Lampe", "énorme"), "beyond ASCII");
    }

    #[test]
    fn the_search_finds_names_components_and_every_term() {
        let (world, [yard, well, lamp, post]) = tree();
        assert_eq!(search(&world, "WELL"), [well]);
        assert_eq!(search(&world, "c:name"), [yard, well, lamp, post]);
        assert_eq!(search(&world, "c:name l"), [well, lamp]);
        assert!(search(&world, "c:light").is_empty());
        assert!(search(&world, "p:lamp").is_empty(), "no placement");
    }
}
