//! Rearranging the level's tree (`docs/adr/0168-rearranging-the-hierarchy.md`): where an entity may
//! move, and the commands that move entities among a parent's children, each where it was in the
//! world, which a drag of the hierarchy's rows and the operations of Alt and the arrows make.

use std::collections::{HashMap, HashSet};

use slime_world::{Command, Entity, World};

use crate::handles;

/// Where an entity moves among a parent's children: before one of them, after one, or last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Spot {
    Before(Entity),
    After(Entity),
    Last,
}

/// Why `entity` of `world` cannot move under `parent` at `spot`; none when it can. An entity at the
/// world's root, as the level's root is, stays, as does one that a placement holds but for its
/// root, which a scene file cannot move out of its place in the prefab; a parent that is the
/// entity or one of its descendants would hold it; a placement's entities keep their prefab's
/// order, before the file's, so that nothing goes before or after one of them; an entity or a
/// parent without an identity is out of the level.
pub(crate) fn refusal(world: &World, entity: Entity, parent: Entity, spot: Spot) -> Option<String> {
    let name = |entity: Entity| {
        world
            .persistent_id(entity)
            .map_or_else(|| format!("{entity:?}"), |id| id.to_string())
    };
    if world.persistent_id(entity).is_none() || world.persistent_id(parent).is_none() {
        return Some(format!("`{}` is out of the level", name(entity)));
    }
    if world.parent(entity).is_none() {
        return Some(format!("`{}` is a root, which stays", name(entity)));
    }
    if held(world, entity) {
        return Some(format!(
            "`{}` belongs to a placement, out of which it cannot be moved",
            name(entity)
        ));
    }
    if std::iter::successors(Some(parent), |&each| world.parent(each)).any(|each| each == entity) {
        return Some(format!("`{}` cannot hold itself", name(entity)));
    }
    if let Spot::Before(sibling) | Spot::After(sibling) = spot {
        if sibling == entity {
            return Some(format!("`{}` cannot move beside itself", name(entity)));
        }
        if held(world, sibling) {
            return Some(format!(
                "`{}` belongs to a placement, whose entities keep their prefab's order",
                name(sibling)
            ));
        }
    }
    None
}

/// Whether a placement holds `entity` of `world`, but for its root.
fn held(world: &World, entity: Entity) -> bool {
    world
        .placement_root(entity)
        .is_some_and(|root| root != entity)
}

/// The commands that make each move, an entity, its new parent and its spot among the parent's
/// children, in their order, the entity where it was in the world: each rank is counted as the
/// world counts it once the commands before it applied, without the entity. A move that
/// [`refusal`] refuses is left out; none at all when no entity changes its parent or its rank.
pub(crate) fn moved(world: &World, moves: &[(Entity, Entity, Spot)]) -> Vec<Command> {
    // The children of each parent that a move reads, as the moves before leave them.
    let mut children: HashMap<Entity, Vec<Entity>> = HashMap::new();
    let mut commands = Vec::new();
    let mut changed = false;
    for &(entity, parent, spot) in moves {
        if refusal(world, entity, parent, spot).is_some() {
            continue;
        }
        let (Some(id), Some(parent_id)) =
            (world.persistent_id(entity), world.persistent_id(parent))
        else {
            continue;
        };
        // Where it stood among its parent's children, as the moves before left them.
        let was = world.parent(entity);
        let mut stood = None;
        if let Some(was) = was {
            let siblings = children
                .entry(was)
                .or_insert_with(|| world.children(was).collect());
            stood = siblings.iter().position(|&each| each == entity);
            siblings.retain(|&each| each != entity);
        }
        let siblings = children
            .entry(parent)
            .or_insert_with(|| world.children(parent).collect());
        let rank = match spot {
            Spot::Before(sibling) => siblings.iter().position(|&each| each == sibling),
            Spot::After(sibling) => siblings
                .iter()
                .position(|&each| each == sibling)
                .map(|at| at + 1),
            Spot::Last => None,
        }
        .unwrap_or(siblings.len());
        siblings.insert(rank, entity);
        changed |= was != Some(parent) || stood != Some(rank);
        commands.push(Command::Reparent {
            entity: id.clone(),
            parent: Some(parent_id.clone()),
            rank: Some(rank),
        });
        if was != Some(parent)
            && let Some(kept) =
                handles::kept(world, entity, &handles::placed_at(world, Some(parent)))
        {
            commands.push(handles::set_transform(id, &kept));
        }
    }
    if !changed {
        commands.clear();
    }
    commands
}

/// The moves of the operation `name` of the hierarchy on `selected`, the outermost of what is
/// selected: `hierarchy.move_up` before its previous sibling, `hierarchy.move_down` after its next,
/// `hierarchy.outdent` beside its parent, after it, `hierarchy.indent` into its previous sibling,
/// last. Entities selected one after the other move together, in their order; none passes over
/// another selected.
///
/// # Panics
///
/// For a name that is none of these four.
pub(crate) fn moves(world: &World, selected: &[Entity], name: &str) -> Vec<(Entity, Entity, Spot)> {
    let chosen: HashSet<Entity> = selected.iter().copied().collect();
    // Each parent of what is selected, with its children as they stand, in the order in which the
    // selection first names them.
    let mut parents: Vec<(Entity, Vec<Entity>)> = Vec::new();
    let mut seen = HashSet::new();
    for &entity in selected {
        let Some(parent) = world.parent(entity) else {
            continue;
        };
        if seen.insert(parent) {
            parents.push((parent, world.children(parent).collect()));
        }
    }
    let mut moves = Vec::new();
    for (parent, siblings) in parents {
        let is_chosen = |at: usize| chosen.contains(&siblings[at]);
        match name {
            // Each run of entities selected one after the other passes the sibling before it, its
            // last first, each before the one after it; down, its first first, each after the one
            // before it, so that they keep their order.
            "hierarchy.move_up" => {
                for at in (1..siblings.len()).rev() {
                    if is_chosen(at) && !is_chosen(at - 1) {
                        let mut end = at;
                        while end + 1 < siblings.len() && is_chosen(end + 1) {
                            end += 1;
                        }
                        for each in (at..=end).rev() {
                            let spot = if each == end {
                                Spot::Before(siblings[at - 1])
                            } else {
                                Spot::Before(siblings[each + 1])
                            };
                            moves.push((siblings[each], parent, spot));
                        }
                    }
                }
            }
            "hierarchy.move_down" => {
                for at in 0..siblings.len().saturating_sub(1) {
                    if is_chosen(at) && !is_chosen(at + 1) {
                        let mut start = at;
                        while start > 0 && is_chosen(start - 1) {
                            start -= 1;
                        }
                        for each in start..=at {
                            let spot = if each == start {
                                Spot::After(siblings[at + 1])
                            } else {
                                Spot::After(siblings[each - 1])
                            };
                            moves.push((siblings[each], parent, spot));
                        }
                    }
                }
            }
            "hierarchy.outdent" => {
                let Some(grandparent) = world.parent(parent) else {
                    continue;
                };
                for (at, &entity) in siblings.iter().enumerate().rev() {
                    if is_chosen(at) {
                        moves.push((entity, grandparent, Spot::After(parent)));
                    }
                }
            }
            "hierarchy.indent" => {
                let mut into = None;
                for (at, &entity) in siblings.iter().enumerate() {
                    if !is_chosen(at) {
                        into = Some(entity);
                    } else if let Some(into) = into {
                        moves.push((entity, into, Spot::Last));
                    }
                }
            }
            name => panic!("`{name}` is no move of the hierarchy"),
        }
    }
    moves
}

/// `entities`, entities of `world`, in the order of a walk of its tree, depth first, each parent's
/// children in their order: what a drag of several carries, in that order. It reads the children
/// of each of their ancestors once.
pub(crate) fn tree_order(world: &World, entities: &[Entity]) -> Vec<Entity> {
    let mut ranks: HashMap<Entity, usize> = HashMap::new();
    // Its root, then its ancestors' ranks and its own, from the root down.
    let mut key = |entity: Entity| -> (Entity, Vec<usize>) {
        let mut root = entity;
        let mut key = Vec::new();
        while let Some(parent) = world.parent(root) {
            if !ranks.contains_key(&root) {
                ranks.extend(
                    world
                        .children(parent)
                        .enumerate()
                        .map(|(at, child)| (child, at)),
                );
            }
            key.push(ranks[&root]);
            root = parent;
        }
        key.reverse();
        (root, key)
    };
    let mut keyed: Vec<((Entity, Vec<usize>), Entity)> = entities
        .iter()
        .map(|&entity| (key(entity), entity))
        .collect();
    keyed.sort();
    keyed.into_iter().map(|(_, entity)| entity).collect()
}

#[cfg(test)]
mod tests {
    use slime_world::Name;

    use super::*;

    /// A level, `yard`, of five children, `a` to `e`, each named; a spawned entity has an identity.
    fn row() -> (World, Entity, [Entity; 5]) {
        let mut world = World::new();
        let yard = world.spawn();
        let children = ["a", "b", "c", "d", "e"].map(|name| {
            let entity = world.spawn();
            world.insert(entity, Name(name.to_owned()));
            world.set_parent(entity, Some(yard)).expect("alive");
            entity
        });
        (world, yard, children)
    }

    /// The names of `yard`'s children once `moves` of a new [`row`] applied.
    fn applied(moves: impl Fn([Entity; 5], &World) -> Vec<(Entity, Entity, Spot)>) -> Vec<String> {
        let (mut world, yard, children) = row();
        for command in moved(&world, &moves(children, &world)) {
            world.apply(&command).expect("applied");
        }
        world
            .children(yard)
            .map(|child| world.get::<Name>(child).expect("named").0.clone())
            .collect()
    }

    #[test]
    fn moves_count_their_ranks_as_the_world_does_once_those_before_applied() {
        let (world, yard, [a, b, c, _, e]) = row();
        // Several before one, after one, and last, in their order.
        assert_eq!(
            applied(|[a, b, _, d, _], _| vec![
                (d, yard, Spot::Before(b)),
                (a, yard, Spot::Before(b))
            ]),
            ["d", "a", "b", "c", "e"]
        );
        assert_eq!(
            applied(|[a, b, _, d, _], _| vec![
                (a, yard, Spot::After(d)),
                (b, yard, Spot::After(a))
            ]),
            ["c", "d", "a", "b", "e"]
        );
        assert_eq!(
            applied(|[a, b, ..], _| vec![(b, yard, Spot::Last), (a, yard, Spot::Last)]),
            ["c", "d", "e", "b", "a"]
        );
        // Into another, and nothing where nothing moves.
        assert_eq!(
            applied(|[a, b, .., e], _| vec![(a, e, Spot::Last), (b, e, Spot::Last)]),
            ["c", "d", "e"]
        );
        assert!(moved(&world, &[(c, yard, Spot::After(b))]).is_empty());
        assert!(moved(&world, &[(e, yard, Spot::Last)]).is_empty());
        assert_eq!(moved(&world, &[(a, yard, Spot::After(b))]).len(), 1);
    }

    #[test]
    fn a_move_refuses_a_root_a_cycle_and_a_place_beside_itself() {
        let (mut world, yard, [a, b, ..]) = row();
        world.set_parent(b, Some(a)).expect("alive");
        assert!(refusal(&world, yard, a, Spot::Last).is_some(), "a root");
        assert!(
            refusal(&world, a, b, Spot::Last).is_some(),
            "into its child"
        );
        assert!(refusal(&world, a, a, Spot::Last).is_some(), "into itself");
        assert!(
            refusal(&world, a, yard, Spot::Before(a)).is_some(),
            "beside itself"
        );
        assert!(refusal(&world, b, yard, Spot::Before(a)).is_none());
    }

    #[test]
    fn the_keys_move_what_is_selected_together_in_its_order() {
        let keyed = |name: &'static str, picked: fn([Entity; 5]) -> Vec<Entity>| {
            applied(move |children, world| moves(world, &picked(children), name))
        };
        assert_eq!(
            keyed("hierarchy.move_up", |[_, _, c, d, _]| vec![c, d]),
            ["a", "c", "d", "b", "e"]
        );
        assert_eq!(
            keyed("hierarchy.move_up", |[a, _, c, ..]| vec![a, c]),
            ["a", "c", "b", "d", "e"]
        );
        assert_eq!(
            keyed("hierarchy.move_down", |[_, b, c, ..]| vec![b, c]),
            ["a", "d", "b", "c", "e"]
        );
        assert_eq!(
            keyed("hierarchy.move_down", |[.., c, _, e]| vec![c, e]),
            ["a", "b", "d", "c", "e"]
        );
        assert_eq!(
            keyed("hierarchy.indent", |[_, _, c, d, _]| vec![c, d]),
            ["a", "b", "e"]
        );
        let (mut world, yard, [a, b, c, d, e]) = row();
        for name in ["hierarchy.indent", "hierarchy.outdent"] {
            for command in moved(&world, &moves(&world, &[c, d], name)) {
                world.apply(&command).expect("applied");
            }
            if name == "hierarchy.indent" {
                assert_eq!(world.children(b).collect::<Vec<_>>(), [c, d]);
            }
        }
        assert_eq!(world.children(yard).collect::<Vec<_>>(), [a, b, c, d, e]);
    }

    #[test]
    fn the_tree_order_walks_depth_first() {
        let (mut world, _, [a, b, c, d, e]) = row();
        world.set_parent(e, Some(b)).expect("alive");
        assert_eq!(tree_order(&world, &[d, e, a, c]), [a, e, c, d]);
    }
}
