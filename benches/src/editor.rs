//! The editor's interface: its hierarchy over a world of 100 000 entities
//! (`docs/adr/0167-the-hierarchy.md`), in the editor's look, and its whole window on *Hunna Wood*'s
//! zone and on levels of 10 000 and 100 000 entities (`docs/adr/0186-counters-and-ratios-in-ci.md`).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use slime_editor::{Hierarchy, InterfaceFrame, Offscreen};
use slime_gfx::Gpu;
use slime_world::{Name, World};
use uwui::look::{self, Colors, Density};
use uwui::style::{Dimension, FlexDirection, Style};
use uwui::text::Fonts;
use uwui::tree::{LayoutDirection, RootSettings, Ui};

use crate::allocations::counted;
use crate::measure::{Context, Row, timed};
use crate::play::{copy, fresh, yard_with};
use crate::ui::{RUNS, still_counters, times};

/// The hierarchy over 100 000 named entities, 1 000 roots of 99 children each, every root
/// unfolded, as it is first built: a frame standing still, one scrolled by a row, one after an
/// entity was renamed, which builds the rows in sight again, one that runs a search over every
/// name, and one after an entity was spawned, which makes the rows again.
pub fn hierarchy(context: &mut Context) -> Result<Vec<Row>, String> {
    let runs = context.runs(RUNS);
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
        slot = Some(children.add_slot("hierarchy", uwui::tree::Element::container(content)));
    });
    let slot = slot.ok_or("no slot")?;
    let mut hierarchy = Hierarchy::default();
    let frame = |ui: &mut Ui, world: &World, hierarchy: &mut Hierarchy| {
        hierarchy.build(ui, slot, world, &[], Instant::now());
        ui.frame()
    };
    // The first frames make the rows and lay the list out, the next builds the rows that it shows.
    for _ in 0..3 {
        frame(&mut ui, &world, &mut hierarchy);
    }
    let still = times(runs, || frame(&mut ui, &world, &mut hierarchy));
    let counters = still_counters(|| frame(&mut ui, &world, &mut hierarchy));
    let list = ui
        .meaning()
        .into_iter()
        .find(|meaning| meaning.name == "hierarchy")
        .ok_or("no list")?
        .id;
    let mut offset = 0.0;
    let scrolled = times(runs, || {
        offset += 22.0;
        ui.scroll_to(list, [0.0, offset]);
        frame(&mut ui, &world, &mut hierarchy);
    });
    let mut number = 0;
    let renaming = times(runs, || {
        number += 1;
        world.insert(renamed, Name(format!("renamed {number}")));
        frame(&mut ui, &world, &mut hierarchy);
    });
    // The search, once the typing paused: every name read, the rows found made and built.
    let mut number = 0;
    let searched = times(runs, || {
        number += 1;
        let paused = Instant::now() - slime_editor::SEARCH_PAUSE;
        hierarchy.set_search(&format!("entity {}", number % 10), paused);
        frame(&mut ui, &world, &mut hierarchy);
    });
    hierarchy.set_search("", Instant::now() - slime_editor::SEARCH_PAUSE);
    frame(&mut ui, &world, &mut hierarchy);
    let spawned = times(runs, || {
        world.spawn();
        frame(&mut ui, &world, &mut hierarchy);
    });
    let mut rows = counters;
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

/// The editor's window at the high tier's screen, 3840 × 2160, in logical pixels at scale 2
/// (`docs/ROADMAP.md`, section 4).
const SCREEN: [u32; 2] = [1920, 1080];
const SCALE: f32 = 2.0;

/// How many times the cost of the editor's interface may grow from a level of 10 000 entities to
/// one of 100 000: the quality bar's ratio of the editor's interface (`docs/ROADMAP.md`, section
/// 4), which `--counters` checks.
const AT_100K: f64 = 1.5;

/// The editor's whole interface on *Hunna Wood*'s zone, cooked in `target/cooked/hunnawood`, as
/// its editor's program opens it, at the high tier's screen: the counters of a frame standing
/// still, and the CPU of a frame standing still, of one after the selection changed and of one
/// after a step: whole, the core's part, then the interface built, styled, laid out and gathered,
/// then drawn through the canvas, and its part building the interface.
pub fn window_zone(context: &mut Context) -> Result<Vec<Row>, String> {
    let root = Context::root();
    let cooked = root.join("target/cooked/hunnawood");
    if !cooked.join(slime_assets::MANIFEST_FILE).is_file() {
        return Err(format!(
            "{} holds no cooked zone: fetch it with `scripts/fetch-assets.sh zone/`, then cook it \
             with `cargo run --release -p slime-cook -- cook games/hunnawood/assets/zone \
             target/cooked/hunnawood`",
            cooked.display()
        ));
    }
    let directory = fresh("editor-window-zone")?;
    let level = copy(
        &root.join("games/hunnawood/assets/zone/zone.scene"),
        &directory,
    )?;
    let mut window = open(
        context,
        &hunnawood_editor::program(),
        &level,
        &cooked,
        &directory,
    )?;
    let mut rows = settle(&context.gpu, &mut window, "zone/lantern 1", "")?;
    let [frames] = series(&context.gpu, &mut [window], context.runs(RUNS))?;
    rows.extend(frame_rows(&frames, ""));
    Ok(rows)
}

/// The editor's whole interface on the examples' yard with 10 000, then 100 000 entities more,
/// under one parent for every hundred, at the high tier's screen, both windows open and their
/// frames in turn, so that what slows the machine meanwhile slows both: the counters and times of
/// [`window_zone`] at each, and the ratio of the time of each kind's interface built at 100 000 to
/// its time at 10 000, which the quality bar bounds and `--counters` checks, its series kept whole.
/// The core's part and the canvas's draw are left out of the ratio: the first runs the fixed steps
/// over the whole world, as a game's does, and the second does not grow with the level.
pub fn window_entities(context: &mut Context) -> Result<Vec<Row>, String> {
    let assets = Context::root().join("target/benches/sample");
    context.sample()?;
    let program = slime_editor::Program {
        name: "editor",
        level: PathBuf::new(),
        key: "yard",
        assets: assets.clone(),
        sources: None,
        register: |_| {},
        lighting: slime_editor::Lighting::default(),
        play: None,
        build: None,
        layout: None,
    };
    let sizes = [(10_000, ", 10k"), (100_000, ", 100k")];
    let mut rows = Vec::new();
    let mut windows = Vec::new();
    for (entities, name) in sizes {
        let directory = fresh(&format!("editor-window-{}", &name[2..]))?;
        let level = yard_with(&directory, entities)?;
        let mut window = open(context, &program, &level, &assets, &directory)?;
        rows.extend(settle(&context.gpu, &mut window, "yard/sample", name)?);
        windows.push(window);
    }
    let windows: &mut [Offscreen; 2] = windows
        .as_mut_slice()
        .try_into()
        .map_err(|_| "two windows")?;
    let [smaller, larger] = series(&context.gpu, windows, RUNS)?;
    for ((_, name), frames) in sizes.iter().zip([&smaller, &larger]) {
        rows.extend(frame_rows(frames, name));
    }
    for (kind, (smaller, larger)) in FRAMES.iter().zip(smaller.iter().zip(&larger)) {
        let built = |frames: &[Timed]| -> Vec<Duration> {
            frames.iter().map(|frame| frame.built).collect()
        };
        rows.push(Row::ratio(
            format!("{kind}: its interface built at 100 000 entities to at 10 000"),
            &built(larger),
            &built(smaller),
            AT_100K,
        ));
    }
    Ok(rows)
}

/// What the frames of [`series`] follow, in the order of its series of times.
const FRAMES: [&str; 3] = [
    "a frame standing still",
    "a frame after the selection changed",
    "a frame after a step",
];

/// The editor's `program` opened on `level` and `assets`, its journal in `directory`, at the high
/// tier's screen, on the benchmarks' GPU.
fn open(
    context: &Context,
    program: &slime_editor::Program,
    level: &Path,
    assets: &Path,
    directory: &Path,
) -> Result<Offscreen, String> {
    let arguments = [
        "--level".into(),
        level.as_os_str().to_owned(),
        "--assets".into(),
        assets.as_os_str().to_owned(),
        "--journal".into(),
        directory.join("journal").into_os_string(),
    ];
    Offscreen::open(program, arguments, &context.gpu, SCREEN, SCALE)
}

/// One frame of the editor's window, timed: what its interface did, its CPU whole, its part
/// building, styling, laying out and gathering the interface, and the allocations of that part.
struct Timed {
    frame: InterfaceFrame,
    whole: Duration,
    built: Duration,
    allocations: u64,
}

/// A frame of `window`, the core's part, then its interface built then drawn through the canvas,
/// timed, then waited for on the GPU. The allocations are those of the interface built alone: the
/// core's part runs fixed steps by the clock, and the draw allocates in the GPU's driver, both of
/// which vary from one machine to another.
fn timed_frame(gpu: &Gpu, window: &mut Offscreen) -> Result<Timed, String> {
    let (((frame, built, allocations), drawn), whole) = timed(|| {
        window.update();
        let ((frame, built), allocations) = counted(|| timed(|| window.frame()));
        ((frame, built, allocations.calls), window.draw())
    });
    drawn?;
    gpu.wait_idle().map_err(|error| error.to_string())?;
    Ok(Timed {
        frame,
        whole,
        built,
        allocations,
    })
}

/// Selects the entity `selected` of `window`, by its persistent identity, and runs frames until
/// nothing is left to do: the counters of a frame standing still then, `name` after what each
/// counts.
fn settle(
    gpu: &Gpu,
    window: &mut Offscreen,
    selected: &str,
    name: &str,
) -> Result<Vec<Row>, String> {
    window.select(&[selected])?;
    // The first frames build every panel and lay them out, the next build the rows that lists
    // show, the last leave nothing to do.
    for _ in 0..4 {
        timed_frame(gpu, window)?;
    }
    let still = timed_frame(gpu, window)?;
    Ok(vec![
        Row::count(format!("elements{name}"), still.frame.stats.elements),
        Row::count(
            format!("elements visited by a frame standing still{name}"),
            still.frame.stats.visited,
        ),
        Row::count(
            format!("items of a frame standing still{name}"),
            still.frame.items,
        ),
        Row::count(
            format!("allocations of a frame standing still{name}"),
            still.allocations,
        ),
    ])
}

/// `runs` frames of each kind of [`FRAMES`] in each of `windows`, the windows in turn at each
/// frame: the selection moves to the next row of the hierarchy and back, and a step nudges what is
/// selected, then is undone. Gives the frames of each kind, in each window.
fn series<const N: usize>(
    gpu: &Gpu,
    windows: &mut [Offscreen; N],
    runs: usize,
) -> Result<[[Vec<Timed>; 3]; N], String> {
    let mut frames: [[Vec<Timed>; 3]; N] = std::array::from_fn(|_| Default::default());
    for run in 0..runs {
        for (window, frames) in windows.iter_mut().zip(&mut frames) {
            frames[0].push(timed_frame(gpu, window)?);
        }
        let selection = if run % 2 == 0 {
            "hierarchy.next"
        } else {
            "hierarchy.previous"
        };
        for (window, frames) in windows.iter_mut().zip(&mut frames) {
            window.run(selection)?;
            frames[1].push(timed_frame(gpu, window)?);
        }
        for (window, frames) in windows.iter_mut().zip(&mut frames) {
            window.run("edit.nudge_right")?;
            frames[2].push(timed_frame(gpu, window)?);
        }
        for window in windows.iter_mut() {
            window.run("edit.undo")?;
            timed_frame(gpu, window)?;
        }
    }
    Ok(frames)
}

/// The rows of the times of `frames`, of each kind of [`FRAMES`], `name` after what each says.
fn frame_rows(frames: &[Vec<Timed>; 3], name: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for (kind, frames) in FRAMES.iter().zip(frames) {
        let whole: Vec<Duration> = frames.iter().map(|frame| frame.whole).collect();
        let built: Vec<Duration> = frames.iter().map(|frame| frame.built).collect();
        rows.extend(Row::times(&format!("{kind}, its CPU whole{name}"), &whole));
        rows.extend(Row::times(
            &format!("{kind}, its interface built, of it{name}"),
            &built,
        ));
    }
    rows
}
