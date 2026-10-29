//! The editor's panels: its hierarchy over a world of 100 000 entities
//! (`docs/adr/0167-the-hierarchy.md`), in the editor's look.

use std::hint::black_box;
use std::time::Instant;

use slime_editor::Hierarchy;
use slime_ui::look::{self, Colors, Density};
use slime_ui::style::{Dimension, FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{LayoutDirection, RootSettings, Ui};
use slime_world::{Name, World};

use crate::measure::{Context, Row};
use crate::ui::times;

/// The hierarchy over 100 000 named entities, 1 000 roots of 99 children each, every root
/// unfolded, as it is first built: a frame standing still, one scrolled by a row, one after an
/// entity was renamed, which builds the rows in sight again, one that runs a search over every
/// name, and one after an entity was spawned, which makes the rows again.
pub fn hierarchy(_: &mut Context) -> Result<Vec<Row>, String> {
    const ROOTS: usize = 1_000;
    const CHILDREN: usize = 99;
    let mut world = World::new();
    let mut renamed = None;
    for root in 0..ROOTS {
        let parent = world.spawn();
        world.insert(parent, Name(format!("group {root}")));
        for child in 0..CHILDREN {
            let entity = world.spawn();
            world.insert(entity, Name(format!("entity {root}.{child}")));
            world
                .set_parent(entity, Some(parent))
                .map_err(|error| error.to_string())?;
            renamed.get_or_insert(entity);
        }
    }
    let renamed = renamed.ok_or("no entity")?;
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let mut ui = Ui::new(fonts);
    ui.set_style(
        &[look::style_sheet()],
        &look::themes(Colors::Dark, Density::Comfortable),
    )
    .map_err(|error| error.to_string())?;
    let settings = RootSettings {
        size: [340.0, 1000.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let mut column = Style::default();
    column.layout.direction = FlexDirection::Column;
    let root = ui.add_root("main", settings, column);
    let mut slot = None;
    ui.build(root, 0, |children| {
        let mut content = Style::default();
        content.layout.direction = FlexDirection::Column;
        content.layout.grow = 1.0;
        content.layout.min_size[1] = Dimension::Pixels(0.0);
        slot = Some(children.add_slot("hierarchy", slime_ui::tree::Element::container(content)));
    });
    let slot = slot.ok_or("no slot")?;
    let mut hierarchy = Hierarchy::default();
    let frame = |ui: &mut Ui, world: &World, hierarchy: &mut Hierarchy| {
        hierarchy.build(ui, slot, world, &[], Instant::now());
        black_box(ui.frame());
    };
    // The first frames make the rows and lay the list out, the next builds the rows that it shows.
    for _ in 0..3 {
        frame(&mut ui, &world, &mut hierarchy);
    }
    let (_, stats) = ui.frame();
    let still = times(|| frame(&mut ui, &world, &mut hierarchy));
    let list = ui
        .meaning()
        .into_iter()
        .find(|meaning| meaning.name == "hierarchy")
        .ok_or("no list")?
        .id;
    let mut offset = 0.0;
    let scrolled = times(|| {
        offset += 22.0;
        ui.scroll_to(list, [0.0, offset]);
        frame(&mut ui, &world, &mut hierarchy);
    });
    let mut number = 0;
    let renaming = times(|| {
        number += 1;
        world.insert(renamed, Name(format!("renamed {number}")));
        frame(&mut ui, &world, &mut hierarchy);
    });
    // The search, once the typing paused: every name read, the rows found made and built.
    let mut number = 0;
    let searched = times(|| {
        number += 1;
        let paused = Instant::now() - slime_editor::SEARCH_PAUSE;
        hierarchy.set_search(&format!("entity {}", number % 10), paused);
        frame(&mut ui, &world, &mut hierarchy);
    });
    hierarchy.set_search("", Instant::now() - slime_editor::SEARCH_PAUSE);
    frame(&mut ui, &world, &mut hierarchy);
    let spawned = times(|| {
        world.spawn();
        frame(&mut ui, &world, &mut hierarchy);
    });
    let mut rows = vec![Row::new("elements", stats.elements.to_string())];
    rows.extend(Row::times("a frame standing still", &still));
    rows.extend(Row::times("a frame scrolled by a row", &scrolled));
    rows.extend(Row::times("a frame after an entity was renamed", &renaming));
    rows.extend(Row::times(
        "a frame that runs a search, once the typing paused",
        &searched,
    ));
    rows.extend(Row::times(
        "a frame after an entity was spawned, the rows made again",
        &spawned,
    ));
    Ok(rows)
}
