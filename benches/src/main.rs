//! SlimeEngine's benchmarks (`docs/adr/0104-the-build-ci-and-measurements.md`): each runs a
//! scenario of the size that the engine aims at, many times, and prints what it measured as rows of
//! a Markdown table, which `scripts/measure.sh` puts in `docs/measurements.md` with the build's own
//! figures. `benches/README.md` lists them, and says how their figures are read.
//!
//! ```sh
//! cargo run --release -p slime-benches                 # every benchmark not run on request
//! cargo run --release -p slime-benches -- world        # those whose name starts with `world`
//! cargo run --release -p slime-benches -- --list       # their names, and what each measures
//! cargo run --release -p slime-benches -- loading.zone # one run on request, named whole
//! ```

mod allocations;
mod counters;
mod devui;
mod editor;
mod loading;
mod measure;
mod play;
mod render;
mod ui;
mod world;

use std::process::ExitCode;

use counters::Counter;
use measure::{Check, Context, Row};

/// Counts the allocations of each thread, which the benchmarks report.
#[global_allocator]
static ALLOCATOR: allocations::Counting = allocations::Counting;

/// A benchmark: its name, by which the command line selects it, what it measures, and the function
/// that measures it.
struct Benchmark {
    name: &'static str,
    about: &'static str,
    /// Whether it gives counters, which `--counters` runs it for.
    counts: bool,
    /// Whether it runs only when an argument is its whole name, not the start of it: the zone's
    /// needs the zone fetched and cooked first.
    on_request: bool,
    run: fn(&mut Context) -> Result<Vec<Row>, String>,
}

const BENCHMARKS: &[Benchmark] = &[
    Benchmark {
        name: "render.instances.10k",
        about: "10 000 cubes of one mesh, lit by a sun that casts shadows, under a sky",
        counts: true,
        on_request: false,
        run: |context| render::instances(context, 10_000, 30, render::View::Grid),
    },
    Benchmark {
        name: "render.unseen.10k",
        about: "the 10 000 cubes of `render.instances.10k`, none of them in view",
        counts: true,
        on_request: false,
        run: |context| render::instances(context, 10_000, 30, render::View::Sky),
    },
    Benchmark {
        name: "render.instances.100k",
        about: "100 000 cubes, as `render.instances.10k`",
        counts: true,
        on_request: false,
        run: |context| render::instances(context, 100_000, 8, render::View::Grid),
    },
    Benchmark {
        name: "render.shaders",
        about: "the renderer's pipelines made again, from unchanged shaders and after an edit",
        counts: false,
        on_request: false,
        run: render::shaders,
    },
    Benchmark {
        name: "render.canvas",
        about: "two screens drawn by the canvas: an editor's, 2 000 boxes, 100 shadows, 20 000 glyphs and 16 images, and a game's menu, 290 boxes, 20 shadows, 3 900 glyphs and 50 icons",
        counts: true,
        on_request: false,
        run: render::canvas,
    },
    Benchmark {
        name: "world.frame.100k",
        about: "a frame built from 100 000 entities that place a mesh each, under 1 000 parents",
        counts: true,
        on_request: false,
        run: world::frame,
    },
    Benchmark {
        name: "world.transforms.100k",
        about: "the world's transforms computed after the 1 000 parents of 100 000 entities moved",
        counts: false,
        on_request: false,
        run: world::transforms,
    },
    Benchmark {
        name: "world.changes.100k",
        about: "100 000 lights, 1 % of them changed, then found by a reader of the whole world",
        counts: false,
        on_request: false,
        run: world::changes,
    },
    Benchmark {
        name: "world.children",
        about: "a parent of 10 000, then 20 000 children emptied, each child given no parent",
        counts: false,
        on_request: false,
        run: world::children,
    },
    Benchmark {
        name: "world.placements",
        about: "a scene file that places a prefab 2 000, then 8 000 times, each changed, resolved",
        counts: false,
        on_request: false,
        run: world::placements,
    },
    Benchmark {
        name: "world.save.100k",
        about: "a level of 100 000 entities, 1 % of them moved, saved and loaded",
        counts: true,
        on_request: false,
        run: world::save,
    },
    Benchmark {
        name: "world.undo.1k",
        about: "a subtree of 1 000 entities in a level of 100 000, deleted then restored",
        counts: false,
        on_request: false,
        run: world::undo,
    },
    Benchmark {
        name: "world.deep",
        about: "a hierarchy 10 000 deep spawned from its root down, saved, and drawn after its root moved",
        counts: false,
        on_request: false,
        run: world::deep,
    },
    Benchmark {
        name: "world.sky",
        about: "frames built around a sun that does not move, between two fixed steps",
        counts: false,
        on_request: false,
        run: world::sky,
    },
    Benchmark {
        name: "ui.text.labels.1k",
        about: "1 000 labels of 12 to 30 letters laid out, each on a line of its own",
        counts: false,
        on_request: false,
        run: ui::labels_1k,
    },
    Benchmark {
        name: "ui.text.paragraphs",
        about: "paragraphs of 2 000 characters in seven writing systems laid out, then broken again",
        counts: false,
        on_request: false,
        run: ui::paragraphs,
    },
    Benchmark {
        name: "ui.text.raster",
        about: "Latin letters, Chinese characters and emoji rasterized at 14, 32 and 64 pixels",
        counts: false,
        on_request: false,
        run: ui::raster,
    },
    Benchmark {
        name: "ui.draw.menu",
        about: "a game's menu drawn from a display list, its real text's glyphs from the atlas",
        counts: true,
        on_request: false,
        run: ui::menu,
    },
    Benchmark {
        name: "ui.tree.editor",
        about: "a tree of the editor's size, 12 500 elements: first built, one value changed, still",
        counts: true,
        on_request: false,
        run: ui::tree,
    },
    Benchmark {
        name: "ui.tree.list.100k",
        about: "a list of 100 000 rows, of which only those that show are built: scrolled, still",
        counts: true,
        on_request: false,
        run: ui::list,
    },
    Benchmark {
        name: "devui.world_panel.100k",
        about: "the dev overlay's world panel over 100 000 entities: still, the world changed, \
                scrolled",
        counts: true,
        on_request: false,
        run: devui::world_panel,
    },
    Benchmark {
        name: "editor.hierarchy.100k",
        about: "the editor's hierarchy over 100 000 entities under 1 000 roots, unfolded: still, \
                scrolled, after a rename and after the rows were made again",
        counts: true,
        on_request: false,
        run: editor::hierarchy,
    },
    Benchmark {
        name: "editor.window.entities",
        about: "the editor's whole interface on levels of 10 000 and 100 000 entities: still, \
                after the selection changed and after a step, and how its cost grows",
        counts: true,
        on_request: false,
        run: editor::window_entities,
    },
    Benchmark {
        name: "editor.window.zone",
        about: "the editor's whole interface on Hunna Wood's zone, cooked in \
                target/cooked/hunnawood: still, after the selection changed and after a step",
        counts: true,
        on_request: true,
        run: editor::window_zone,
    },
    Benchmark {
        name: "loading.batch",
        about: "a load of meshes started with a manifest of 10 000, then 100 000 assets",
        counts: false,
        on_request: false,
        run: loading::batch,
    },
    Benchmark {
        name: "loading.textures",
        about: "16 squares, each with a texture of 4096² of its own, loaded while frames are drawn",
        counts: false,
        on_request: false,
        run: loading::textures,
    },
    Benchmark {
        name: "loading.zone",
        about: "the meshes of Hunna Wood's zone, cooked in target/cooked/hunnawood, placed once \
                each, loaded while frames are drawn",
        counts: false,
        on_request: true,
        run: loading::zone,
    },
    Benchmark {
        name: "play.zone",
        about: "Play's time to control on Hunna Wood's zone, cooked in target/cooked/hunnawood, from \
                its editor built beside this program, in a window",
        counts: false,
        on_request: true,
        run: play::zone,
    },
    Benchmark {
        name: "play.level.100k",
        about: "Play's time to control on the examples' yard with 100 000 entities more, from the \
                `editor` example built beside this program, without a window",
        counts: false,
        on_request: true,
        run: play::level,
    },
];

const USAGE: &str =
    "usage: slime-benches [--list | --counters | --write-counters] [<name or name prefix>...]";

/// What the command line asks.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Every figure, as a table.
    Measure,
    /// The counters and ratios, checked against `benches/counters.txt` and their bounds.
    Check,
    /// The counters written into `benches/counters.txt`.
    Write,
}

fn main() -> ExitCode {
    slime_app::init_diagnostics();
    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument == "--list") {
        for benchmark in BENCHMARKS {
            let note = match (benchmark.counts, benchmark.on_request) {
                (true, true) => " (counted; only when named whole)",
                (true, false) => " (counted)",
                (false, true) => " (only when named whole)",
                (false, false) => "",
            };
            println!("{}: {}{note}", benchmark.name, benchmark.about);
        }
        return ExitCode::SUCCESS;
    }
    let mode = match arguments.first().map(String::as_str) {
        Some("--counters") => Mode::Check,
        Some("--write-counters") => Mode::Write,
        _ => Mode::Measure,
    };
    if mode != Mode::Measure {
        arguments.remove(0);
    }
    if let Some(option) = arguments.iter().find(|argument| argument.starts_with('-')) {
        eprintln!("unknown option {option}\n{USAGE}");
        return ExitCode::FAILURE;
    }
    let selected: Vec<&Benchmark> = BENCHMARKS
        .iter()
        .filter(|benchmark| mode == Mode::Measure || benchmark.counts)
        .filter(|benchmark| {
            if benchmark.on_request {
                arguments.iter().any(|name| benchmark.name == name)
            } else {
                arguments.is_empty()
                    || arguments
                        .iter()
                        .any(|prefix| benchmark.name.starts_with(prefix.as_str()))
            }
        })
        .collect();
    if selected.is_empty() {
        let which = if mode == Mode::Measure {
            "no benchmark"
        } else {
            "no benchmark that counts"
        };
        eprintln!("{which} is named, or has a name that starts, with {arguments:?}\n{USAGE}");
        return ExitCode::FAILURE;
    }
    let mut context = match Context::new(mode != Mode::Measure) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("cannot measure: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("{}\n", context.machine());
    println!("| Benchmark | Measure | Value |");
    println!("|---|---|---|");
    let mut failed = false;
    let mut counted = Vec::new();
    for benchmark in &selected {
        eprintln!("running {}", benchmark.name);
        match (benchmark.run)(&mut context) {
            Ok(rows) => {
                for row in rows {
                    match row.check {
                        Check::None if mode != Mode::Measure => continue,
                        Check::None => {}
                        Check::Count(value) => counted.push(Counter {
                            benchmark: benchmark.name.to_owned(),
                            what: row.what.clone(),
                            value,
                        }),
                        Check::Ratio { value, at_most } => {
                            if mode == Mode::Check && row.check.over_budget() {
                                eprintln!(
                                    "`{}`: {} is {value:.2}, over its budget of {at_most}",
                                    benchmark.name, row.what
                                );
                                failed = true;
                            }
                        }
                    }
                    println!("| `{}` | {} | {} |", benchmark.name, row.what, row.value);
                }
            }
            Err(error) => {
                eprintln!("{} failed: {error}", benchmark.name);
                failed = true;
            }
        }
    }
    if mode != Mode::Measure && !failed && !compare(mode, &selected, &counted) {
        failed = true;
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Compares the counters of the benchmarks `run` with those that `benches/counters.txt` records,
/// or writes them there, as `mode` asks: whether they agree, or were written.
fn compare(mode: Mode, run: &[&Benchmark], counted: &[Counter]) -> bool {
    let file = Context::root().join(counters::FILE);
    let recorded = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            eprintln!("cannot read {}: {error}", file.display());
            return false;
        }
    };
    let run: Vec<&str> = run.iter().map(|benchmark| benchmark.name).collect();
    let known: Vec<&str> = BENCHMARKS
        .iter()
        .filter(|benchmark| benchmark.counts)
        .map(|benchmark| benchmark.name)
        .collect();
    if mode == Mode::Write {
        return match counters::written(&recorded, counted, &run, &known)
            .and_then(|text| std::fs::write(&file, text).map_err(|error| error.to_string()))
        {
            Ok(()) => {
                eprintln!("wrote {}", file.display());
                true
            }
            Err(error) => {
                eprintln!("cannot write {}: {error}", file.display());
                false
            }
        };
    }
    match counters::differences(&recorded, counted, &run, &known) {
        Ok(differences) if differences.is_empty() => true,
        Ok(differences) => {
            for difference in differences {
                eprintln!("{difference}");
            }
            eprintln!(
                "a change that changes a counter on purpose writes {} again: \
                 cargo run -p slime-benches -- --write-counters [<name>...]",
                counters::FILE
            );
            false
        }
        Err(error) => {
            eprintln!("{error}");
            false
        }
    }
}
