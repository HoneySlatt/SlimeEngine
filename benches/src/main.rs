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

mod loading;
mod measure;
mod render;
mod ui;
mod world;

use std::process::ExitCode;

use measure::{Context, Row};

/// A benchmark: its name, by which the command line selects it, what it measures, and the function
/// that measures it.
struct Benchmark {
    name: &'static str,
    about: &'static str,
    /// Whether it runs only when an argument is its whole name, not the start of it: the zone's
    /// needs the zone fetched and cooked first.
    on_request: bool,
    run: fn(&mut Context) -> Result<Vec<Row>, String>,
}

const BENCHMARKS: &[Benchmark] = &[
    Benchmark {
        name: "render.instances.10k",
        about: "10 000 cubes of one mesh, lit by a sun that casts shadows, under a sky",
        on_request: false,
        run: |context| render::instances(context, 10_000, 30, render::View::Grid),
    },
    Benchmark {
        name: "render.unseen.10k",
        about: "the 10 000 cubes of `render.instances.10k`, none of them in view",
        on_request: false,
        run: |context| render::instances(context, 10_000, 30, render::View::Sky),
    },
    Benchmark {
        name: "render.instances.100k",
        about: "100 000 cubes, as `render.instances.10k`",
        on_request: false,
        run: |context| render::instances(context, 100_000, 8, render::View::Grid),
    },
    Benchmark {
        name: "render.shaders",
        about: "the renderer's pipelines made again, from unchanged shaders and after an edit",
        on_request: false,
        run: render::shaders,
    },
    Benchmark {
        name: "world.frame.100k",
        about: "a frame built from 100 000 entities that place a mesh each, under 1 000 parents",
        on_request: false,
        run: world::frame,
    },
    Benchmark {
        name: "world.transforms.100k",
        about: "the world's transforms computed after the 1 000 parents of 100 000 entities moved",
        on_request: false,
        run: world::transforms,
    },
    Benchmark {
        name: "world.changes.100k",
        about: "100 000 lights, 1 % of them changed, then found by a reader of the whole world",
        on_request: false,
        run: world::changes,
    },
    Benchmark {
        name: "world.children",
        about: "a parent of 10 000, then 20 000 children emptied, each child given no parent",
        on_request: false,
        run: world::children,
    },
    Benchmark {
        name: "world.placements",
        about: "a scene file that places a prefab 2 000, then 8 000 times, each changed, resolved",
        on_request: false,
        run: world::placements,
    },
    Benchmark {
        name: "world.save.100k",
        about: "a level of 100 000 entities, 1 % of them moved, saved and loaded",
        on_request: false,
        run: world::save,
    },
    Benchmark {
        name: "world.undo.1k",
        about: "a subtree of 1 000 entities in a level of 100 000, deleted then restored",
        on_request: false,
        run: world::undo,
    },
    Benchmark {
        name: "world.sky",
        about: "frames built around a sun that does not move, between two fixed steps",
        on_request: false,
        run: world::sky,
    },
    Benchmark {
        name: "ui.text.labels.1k",
        about: "1 000 labels of 12 to 30 letters laid out, each on a line of its own",
        on_request: false,
        run: ui::labels_1k,
    },
    Benchmark {
        name: "ui.text.paragraphs",
        about: "paragraphs of 2 000 characters in seven writing systems laid out, then broken again",
        on_request: false,
        run: ui::paragraphs,
    },
    Benchmark {
        name: "ui.text.raster",
        about: "Latin letters, Chinese characters and emoji rasterized at 14, 32 and 64 pixels",
        on_request: false,
        run: ui::raster,
    },
    Benchmark {
        name: "loading.batch",
        about: "a load of meshes started with a manifest of 10 000, then 100 000 assets",
        on_request: false,
        run: loading::batch,
    },
    Benchmark {
        name: "loading.textures",
        about: "16 squares, each with a texture of 4096² of its own, loaded while frames are drawn",
        on_request: false,
        run: loading::textures,
    },
    Benchmark {
        name: "loading.zone",
        about: "the meshes of Hunna Wood's zone, cooked in target/cooked/hunnawood, placed once \
                each, loaded while frames are drawn",
        on_request: true,
        run: loading::zone,
    },
];

const USAGE: &str = "usage: slime-benches [--list] [<name or name prefix>...]";

fn main() -> ExitCode {
    slime_app::init_diagnostics();
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument == "--list") {
        for benchmark in BENCHMARKS {
            let note = if benchmark.on_request {
                " (only when named whole)"
            } else {
                ""
            };
            println!("{}: {}{note}", benchmark.name, benchmark.about);
        }
        return ExitCode::SUCCESS;
    }
    if let Some(option) = arguments.iter().find(|argument| argument.starts_with('-')) {
        eprintln!("unknown option {option}\n{USAGE}");
        return ExitCode::FAILURE;
    }
    let selected: Vec<&Benchmark> = BENCHMARKS
        .iter()
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
        eprintln!("no benchmark is named, or has a name that starts, with {arguments:?}\n{USAGE}");
        return ExitCode::FAILURE;
    }
    let mut context = match Context::new() {
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
    for benchmark in selected {
        eprintln!("running {}", benchmark.name);
        match (benchmark.run)(&mut context) {
            Ok(rows) => {
                for row in rows {
                    println!("| `{}` | {} | {} |", benchmark.name, row.what, row.value);
                }
            }
            Err(error) => {
                eprintln!("{} failed: {error}", benchmark.name);
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
