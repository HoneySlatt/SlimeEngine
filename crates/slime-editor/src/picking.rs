//! Picking in a viewport (`docs/adr/0159-picking-and-the-selection.md`): what a button that selects
//! asks for over it, the left one by default, a click, a drag that draws a marquee, or a click
//! through what lies behind, Alt and a click; and what each selects of what is found there. It
//! needs no GPU: the window asks the viewport what is drawn where, the GPU answering a click and a
//! marquee a frame or two later and the CPU's ray Alt and a click at once, and hands the entities
//! found back here.

use slime_platform::{Modifiers, MouseButton};
use slime_world::{Entity, World};

/// How far the pointer moves with the left button held, in logical pixels, before a click becomes
/// a marquee, and the mouse during Alt and the left button before a click becomes an orbit.
pub(crate) const DRAG: f32 = 4.0;

/// What a button that selects asked for over the viewport, once let go, its points in logical
/// pixels from the viewport's corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Wanted {
    /// A click: what is drawn at the point; with Shift, added to what is selected, or taken out of
    /// it.
    Click { at: [f32; 2], adding: bool },
    /// A marquee: what is drawn between two corners; with Shift, added to what is selected.
    Marquee {
        from: [f32; 2],
        to: [f32; 2],
        adding: bool,
    },
    /// Alt and a click: what the ray through the point meets, the nearest first.
    Through { at: [f32; 2] },
}

/// A button held over the viewport that the table of shortcuts binds to `select`, or to `through`,
/// from its press (`docs/adr/0160-two-viewports-and-blenders-way.md`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Selecting {
    from: [f32; 2],
    to: [f32; 2],
    adding: bool,
    /// The button held.
    button: MouseButton,
    /// Whether it goes through what lies behind, a click alone.
    through: bool,
    /// Whether the pointer went [`DRAG`] away from the press: a marquee, from then on, or nothing
    /// for a click through.
    dragged: bool,
}

impl Selecting {
    /// `button` pressed at `at` with `modifiers` held, to select, or to go `through` what lies
    /// behind: Shift adds to what is selected.
    pub(crate) fn new(
        at: [f32; 2],
        modifiers: Modifiers,
        button: MouseButton,
        through: bool,
    ) -> Self {
        Self {
            from: at,
            to: at,
            adding: modifiers.shift,
            button,
            through,
            dragged: false,
        }
    }

    /// The button held.
    pub(crate) fn button(&self) -> MouseButton {
        self.button
    }

    /// The pointer moved to `at`.
    pub(crate) fn moved(&mut self, at: [f32; 2]) {
        self.to = at;
        self.dragged |= (at[0] - self.from[0]).hypot(at[1] - self.from[1]) >= DRAG;
    }

    /// The marquee's corners, once the pointer has gone far enough to draw one.
    pub(crate) fn marquee(&self) -> Option<([f32; 2], [f32; 2])> {
        (self.dragged && !self.through).then_some((self.from, self.to))
    }

    /// Let go at `at`: what it asks for; nothing for a click through that the pointer dragged.
    pub(crate) fn release(mut self, at: [f32; 2]) -> Option<Wanted> {
        self.moved(at);
        Some(match (self.through, self.dragged) {
            (true, true) => return None,
            (true, false) => Wanted::Through { at: self.from },
            (false, true) => Wanted::Marquee {
                from: self.from,
                to: self.to,
                adding: self.adding,
            },
            (false, false) => Wanted::Click {
                at: self.from,
                adding: self.adding,
            },
        })
    }
}

/// The outermost placement that `entity` belongs to, or `entity` itself, which belongs to none.
fn outermost(world: &World, entity: Entity) -> Entity {
    world.placement_root(entity).unwrap_or(entity)
}

/// What is selected once `wanted` found `found`, while `selection` is: what is drawn at a click's
/// point or in a marquee, each entity once, or what the ray meets, the nearest first.
///
/// - A click selects the outermost placement of what it found; a click again on what lies below
///   the one entity selected, the next one down from it towards what it found, a level deeper at
///   each click, and the outermost placement again past it. With Shift, the outermost placement
///   is added to what is selected, or taken out of it.
/// - A marquee selects the outermost placements of what it found; with Shift, added to what is
///   selected.
/// - Alt and a click select the outermost placement behind the one selected, along the ray, or
///   the nearest when none of them is selected: a click after another goes through them.
///
/// Where nothing is found, nothing is selected, but with Shift, which changes nothing.
pub(crate) fn select(
    world: &World,
    wanted: &Wanted,
    found: &[Entity],
    selection: &[Entity],
) -> Vec<Entity> {
    let placements = || {
        let mut placements: Vec<Entity> = Vec::new();
        for &entity in found {
            let placement = outermost(world, entity);
            if !placements.contains(&placement) {
                placements.push(placement);
            }
        }
        placements
    };
    match *wanted {
        Wanted::Click { adding: true, .. } => {
            let mut chosen = selection.to_vec();
            if let Some(&picked) = found.first() {
                let placement = outermost(world, picked);
                match chosen.iter().position(|&selected| selected == placement) {
                    Some(place) => {
                        chosen.remove(place);
                    }
                    None => chosen.push(placement),
                }
            }
            chosen
        }
        Wanted::Click { adding: false, .. } => found
            .first()
            .map(|&picked| deeper(world, picked, selection))
            .into_iter()
            .collect(),
        Wanted::Marquee { adding, .. } => {
            let mut chosen = if adding {
                selection.to_vec()
            } else {
                Vec::new()
            };
            for placement in placements() {
                if !chosen.contains(&placement) {
                    chosen.push(placement);
                }
            }
            chosen
        }
        Wanted::Through { .. } => {
            let along = placements();
            let selected = match selection {
                [selected] => along
                    .iter()
                    .position(|&placement| placement == outermost(world, *selected)),
                _ => None,
            };
            let next = selected.map_or(0, |place| (place + 1) % along.len().max(1));
            along.get(next).copied().into_iter().collect()
        }
    }
}

/// What a click on what draws `picked` selects while `selection` is: the outermost placement that
/// it belongs to; or, when one entity is selected, above `picked`, the next one down from it
/// towards `picked`.
fn deeper(world: &World, picked: Entity, selection: &[Entity]) -> Entity {
    if let [selected] = *selection {
        // `picked` and the entities above it, up to the one selected.
        let path: Vec<Entity> = std::iter::successors(Some(picked), |&entity| world.parent(entity))
            .take_while(|&entity| entity != selected)
            .collect();
        let below = path
            .last()
            .filter(|&&last| last != selected && world.parent(last) == Some(selected));
        if let Some(&next) = below {
            return next;
        }
    }
    outermost(world, picked)
}

#[cfg(test)]
mod tests {
    use slime_assets::AssetId;
    use slime_world::{Name, PersistentId, SceneLibrary};

    use super::*;

    const LAMP: AssetId = AssetId::from_u128(2);
    const YARD: AssetId = AssetId::from_u128(1);

    /// A lamp: a post, a cage on it, and a light in the cage.
    const LAMP_FILE: &str = r#"
[entities.post.components]
Name = "post"

[entities.cage]
parent = "post"

[entities.cage.components]
Name = "cage"

[entities.light]
parent = "cage"

[entities.light.components]
Name = "light"
"#;

    /// A lamp by the gate, and a well that no prefab places.
    const YARD_FILE: &str = r#"
[assets]
lamp = { id = "00000000-0000-0000-0000-000000000002", path = "lamp.scene" }

[entities]
gate = { prefab = "lamp" }
well = { components = { Name = "well" } }
"#;

    /// The yard, spawned under `yard`, and the entities of the lamp at the gate by their names,
    /// then the well.
    fn yard() -> (World, [Entity; 5]) {
        let mut scenes = SceneLibrary::new();
        scenes.insert_file(LAMP, "lamp", LAMP_FILE);
        scenes.insert_file(YARD, "yard", YARD_FILE);
        let mut world = World::new();
        let scene = scenes.resolve(world.registry(), YARD).expect("the yard");
        world.spawn_scene("yard", &scene).expect("spawned");
        let find = |id: &str| {
            world
                .find(&id.parse::<PersistentId>().expect("an identity"))
                .unwrap_or_else(|| panic!("{id}"))
        };
        let gate = find("yard/gate");
        let named = |name: &str| {
            world
                .query::<&Name>()
                .find(|(_, named)| named.0 == name)
                .map(|(entity, _)| entity)
                .unwrap_or_else(|| panic!("{name}"))
        };
        let entities = [
            gate,
            named("post"),
            named("cage"),
            named("light"),
            named("well"),
        ];
        (world, entities)
    }

    fn click(adding: bool) -> Wanted {
        Wanted::Click {
            at: [0.0; 2],
            adding,
        }
    }

    #[test]
    fn a_click_selects_the_outermost_placement_then_a_level_deeper_at_each_click_again() {
        let (world, [gate, post, cage, light, well]) = yard();
        let mut selection = Vec::new();
        let mut chosen = Vec::new();
        for _ in 0..6 {
            selection = select(&world, &click(false), &[light], &selection);
            chosen.push(selection[0]);
        }
        // The placement, then down to the light, then round again; the placement's root is the
        // prefab's root, or holds it.
        let down: Vec<Entity> = if gate == post {
            vec![gate, cage, light, gate, cage, light]
        } else {
            vec![gate, post, cage, light, gate, post]
        };
        assert_eq!(chosen, down);
        // What no prefab places is selected itself; what is selected elsewhere does not lead
        // deeper.
        assert_eq!(select(&world, &click(false), &[well], &[cage]), [well]);
        assert_eq!(select(&world, &click(false), &[light], &[well]), [gate]);
        // A click on nothing selects nothing.
        assert!(select(&world, &click(false), &[], &[gate]).is_empty());
    }

    #[test]
    fn shift_and_a_click_add_the_outermost_placement_or_take_it_out() {
        let (world, [gate, _, cage, light, well]) = yard();
        assert_eq!(
            select(&world, &click(true), &[light], &[well]),
            [well, gate]
        );
        assert_eq!(select(&world, &click(true), &[cage], &[well, gate]), [well]);
        assert_eq!(select(&world, &click(true), &[], &[well]), [well]);
    }

    #[test]
    fn a_marquee_selects_the_outermost_placements_of_what_it_holds() {
        let (world, [gate, post, _, light, well]) = yard();
        let marquee = |adding| Wanted::Marquee {
            from: [0.0; 2],
            to: [10.0; 2],
            adding,
        };
        assert_eq!(
            select(&world, &marquee(false), &[light, well, post], &[gate]),
            [gate, well]
        );
        assert_eq!(
            select(&world, &marquee(true), &[light], &[well]),
            [well, gate]
        );
        assert!(select(&world, &marquee(false), &[], &[well]).is_empty());
    }

    #[test]
    fn alt_and_a_click_go_through_what_the_ray_meets_a_click_after_another() {
        let (world, [gate, _, cage, light, well]) = yard();
        let through = Wanted::Through { at: [0.0; 2] };
        let met = [light, cage, well];
        assert_eq!(select(&world, &through, &met, &[]), [gate]);
        assert_eq!(select(&world, &through, &met, &[gate]), [well]);
        // A level deeper selected counts as its placement.
        assert_eq!(select(&world, &through, &met, &[cage]), [well]);
        assert_eq!(select(&world, &through, &met, &[well]), [gate]);
        assert!(select(&world, &through, &[], &[well]).is_empty());
    }

    #[test]
    fn a_press_becomes_a_marquee_once_the_pointer_goes_far_enough() {
        let mut shift = Modifiers::default();
        shift.shift = true;
        let mut selecting = Selecting::new([10.0, 10.0], shift, MouseButton::Left, false);
        selecting.moved([12.0, 11.0]);
        assert_eq!(selecting.marquee(), None);
        assert_eq!(
            selecting.release([12.0, 11.0]),
            Some(Wanted::Click {
                at: [10.0, 10.0],
                adding: true
            })
        );
        let mut selecting =
            Selecting::new([10.0, 10.0], Modifiers::default(), MouseButton::Left, false);
        selecting.moved([30.0, 5.0]);
        assert_eq!(selecting.marquee(), Some(([10.0, 10.0], [30.0, 5.0])));
        // Back near the press, it is a marquee still.
        assert_eq!(
            selecting.release([11.0, 10.0]),
            Some(Wanted::Marquee {
                from: [10.0, 10.0],
                to: [11.0, 10.0],
                adding: false
            })
        );
        // A click through, as Blender's way binds one: no marquee, and nothing once dragged.
        let through = Selecting::new([10.0, 10.0], Modifiers::default(), MouseButton::Left, true);
        assert_eq!(
            through.release([11.0, 10.0]),
            Some(Wanted::Through { at: [10.0, 10.0] })
        );
        let mut through =
            Selecting::new([10.0, 10.0], Modifiers::default(), MouseButton::Left, true);
        through.moved([30.0, 5.0]);
        assert_eq!(through.marquee(), None);
        assert_eq!(through.release([30.0, 5.0]), None);
    }
}
