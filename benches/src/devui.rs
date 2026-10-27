//! The dev overlay's panels on the engine's interface
//! (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`): the world panel over a world of
//! 100 000 entities, in the editor's look.

use std::hint::black_box;

use slime_devui::interface::WorldPanel;
use slime_ui::look::{self, Colors, Density};
use slime_ui::style::{FlexDirection, Style};
use slime_ui::text::Fonts;
use slime_ui::tree::{LayoutDirection, RootSettings, Ui};
use slime_world::glam::DVec3;
use slime_world::{Light, Name, Transform, World};

use crate::measure::{Context, Row};
use crate::ui::times;

/// The world panel over a world of 100 000 named entities, the one selected holding a transform
/// and a light: a frame standing still, one whose world changed, which builds the entity selected
/// again, one scrolled by a row, and one after an entity was renamed, which makes the list again.
pub fn world_panel(_: &mut Context) -> Result<Vec<Row>, String> {
    const ENTITIES: usize = 100_000;
    let mut world = World::new();
    let mut selected = None;
    for number in 0..ENTITIES {
        let entity = world.spawn();
        world.insert(entity, Name(format!("entity {number}")));
        if number == 0 {
            world.insert(
                entity,
                Transform::from_translation(DVec3::new(1.0, 2.0, 3.0)),
            );
            world.insert(
                entity,
                Light::Point {
                    color: slime_color::LinearColor::WHITE,
                    intensity: 30.0,
                    range: None,
                },
            );
            selected = Some(entity);
        }
    }
    let selected = selected.ok_or("no entity")?;
    let mut fonts = Fonts::new();
    look::register_fonts(&mut fonts);
    let mut ui = Ui::new(fonts);
    ui.set_style(
        &[look::style_sheet()],
        &look::themes(Colors::Dark, Density::Comfortable),
    )
    .map_err(|error| error.to_string())?;
    let settings = RootSettings {
        size: [380.0, 1000.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let mut column = Style::default();
    column.layout.direction = FlexDirection::Column;
    let root = ui.add_root(settings, column);
    let mut panel = WorldPanel::default();
    panel.select(selected);
    let frame = |ui: &mut Ui, world: &World, panel: &mut WorldPanel| {
        let mut actions = ui.take_actions();
        black_box(panel.act(&mut actions));
        panel.build(ui, root, world);
        black_box(ui.frame());
    };
    // The first frame lays the list out, the second builds the rows that it shows.
    frame(&mut ui, &world, &mut panel);
    frame(&mut ui, &world, &mut panel);
    let (_, stats) = ui.frame();
    let still = times(|| frame(&mut ui, &world, &mut panel));
    let mut x = 0.0;
    let changed = times(|| {
        x += 0.5;
        world.insert(
            selected,
            Transform::from_translation(DVec3::new(x, 2.0, 3.0)),
        );
        frame(&mut ui, &world, &mut panel);
    });
    let list = ui
        .meaning()
        .into_iter()
        .find(|meaning| meaning.name == "entities")
        .ok_or("no list")?
        .id;
    let mut offset = 0.0;
    let scrolled = times(|| {
        offset += 22.0;
        ui.scroll_to(list, [0.0, offset]);
        frame(&mut ui, &world, &mut panel);
    });
    // The world's structure moved: the list made again, every name read and filtered.
    let mut number = 0;
    let renamed = times(|| {
        number += 1;
        world.insert(selected, Name(format!("renamed {number}")));
        frame(&mut ui, &world, &mut panel);
    });
    let mut rows = vec![Row::new("elements", stats.elements.to_string())];
    rows.extend(Row::times("a frame standing still", &still));
    rows.extend(Row::times("a frame whose world changed", &changed));
    rows.extend(Row::times("a frame scrolled by a row", &scrolled));
    rows.extend(Row::times("a frame after an entity was renamed", &renamed));
    Ok(rows)
}
