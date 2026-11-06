//! The first measures of the quality bar's memory and power (`docs/ROADMAP.md`, section 4;
//! `docs/adr/0193-first-measures-of-resources.md`), through the engine's MCP server, as an agent
//! drives the editor: it starts `hunnawood-editor`, in a window, on a copy of *Hunna Wood*'s zone,
//! and reads its memory and its video memory once it drew the zone whole and rests; then over 100
//! undos and redos of a step, after 300 that let what they make once grow to its most, and over 10
//! runs of Play, after one that builds the game; then starts *Hunna Wood* itself, at its 60 frames
//! a second, and reads its own.
//!
//! - Memory is read from `/proc/<pid>/status`: the resident size, its parts, anonymous, of files
//!   and shared, and the most that it reached.
//! - Video memory, from `/proc/<pid>/fdinfo`, as the GPU's driver counts it for each client of the
//!   kernel's DRM, which the process's files of `/dev/dri` open: what is resident in the GPU's
//!   memory, what the process asked for in it, and what is resident in the system's memory that the
//!   GPU maps (GTT). A CPU driver, lavapipe, opens none: the run then says so.
//! - With `--power <file>`, the GPU's power, a file of the driver's sensor in `/sys/class/hwmon` in
//!   microwatts, read every 250 ms for 30 s: before anything starts, with the editor at rest, with
//!   the game running, and once both ended.
//! - With `--heaptrack <file>`, the editor runs under `heaptrack`, which records its heap into the
//!   file; the run then says at which of the editor's seconds each phase ends, so that the heap
//!   can be read at each. `heaptrack` slows the editor and counts in its memory: the figures of
//!   such a run are not the editor's own, and it reads no power.
//!
//! `scripts/resources.sh` runs it in a headless Weston of its own, on the machine's GPU, on a
//! machine at rest, which absolute figures need; the editor's frames take tens of seconds each on
//! lavapipe. It needs the editor and the game built in its own
//! profile, beside the server, and the zone cooked. It is a measure, not a check: it fails only when
//! the editor or the game does not do what it is asked; no check runs it. It has no harness, and
//! is not in the default run of `cargo test` (`test = false` in Cargo.toml).

#[path = "common/rest.rs"]
mod rest;
#[path = "common/session.rs"]
#[expect(dead_code, reason = "this session reads no image")]
mod session;

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::json;

use session::{Session, server};

/// How many times a step is undone and done again, as the quality bar's budget of memory says,
/// unless `--undos` says otherwise.
const UNDOS: usize = 100;
/// How many times it is first: as the frames that they draw follow each other fast, wgpu's pool of
/// command buffers, which it never shrinks, grows to its most within the first few hundred
/// (`docs/measurements.md`, M6c.Q2e).
const WARM_UNDOS: usize = 300;
/// How many times the zone is played, as the same budget says, after one that builds the game,
/// unless `--plays` says otherwise.
const PLAYS: usize = 10;
/// How long the power is read for, at each phase.
const POWER_WINDOW: Duration = Duration::from_secs(30);
/// How often the power is read.
const POWER_PERIOD: Duration = Duration::from_millis(250);
/// How long a program may take to be still after what it was asked last, the zone's textures
/// arriving included.
const SETTLE: Duration = Duration::from_secs(120);
/// How long the game runs at its schedule before its memory is read: what loads after the zone is
/// drawn, its textures at their full size, arrives meanwhile.
const GAME_LOADS: Duration = Duration::from_secs(10);
/// How long a Play may take to give control, the first one building the game.
const PLAY_LIMIT: Duration = Duration::from_secs(1800);
/// How long the game that Play runs may take to end once stopped.
const STOP_LIMIT: Duration = Duration::from_secs(60);

/// What the run was asked, on its command line.
struct Options {
    /// The sensor's file of the GPU's power, in microwatts.
    power: Option<PathBuf>,
    /// The file into which `heaptrack` records the editor's heap.
    heaptrack: Option<PathBuf>,
    /// How many undos and redos are measured.
    undos: usize,
    /// How many runs of Play are measured.
    plays: usize,
}

impl Options {
    fn read() -> Self {
        const USAGE: &str = "usage: resources [--power <file>] [--heaptrack <file>] \
                             [--undos <count>] [--plays <count>]";
        let mut options = Self {
            power: None,
            heaptrack: None,
            undos: UNDOS,
            plays: PLAYS,
        };
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .unwrap_or_else(|| panic!("{argument} needs a value; {USAGE}"));
            let count = || {
                value
                    .parse()
                    .unwrap_or_else(|_| panic!("{argument}: {USAGE}"))
            };
            match argument.as_str() {
                "--power" => options.power = Some(value.into()),
                "--heaptrack" => options.heaptrack = Some(value.into()),
                "--undos" => options.undos = count(),
                "--plays" => options.plays = count(),
                _ => panic!("{USAGE}"),
            }
        }
        assert!(
            options.power.is_none() || options.heaptrack.is_none(),
            "a run under heaptrack reads no power: it slows the editor"
        );
        options
    }
}

/// A process's memory, in KiB, as the system and the GPU's driver count it.
#[derive(Clone, Copy, Default)]
struct Memory {
    /// What is resident in the system's memory (`VmRSS`).
    resident: u64,
    /// Of it, what no file backs: the heap, the stacks (`RssAnon`).
    anonymous: u64,
    /// Of it, files mapped: the program and its libraries (`RssFile`).
    files: u64,
    /// Of it, memory shared with other processes, the GPU's driver's mappings among them
    /// (`RssShmem`).
    shared: u64,
    /// The most that was ever resident (`VmHWM`).
    peak: u64,
    /// The video memory, if the process opened a GPU of the kernel's DRM.
    video: Option<Video>,
}

/// A process's video memory, in KiB, summed over its clients of the kernel's DRM.
#[derive(Clone, Copy, Default)]
struct Video {
    /// What is resident in the GPU's own memory (`drm-resident-vram`).
    resident: u64,
    /// What the process asked for in it, resident or evicted to the system's (`drm-total-vram`).
    asked: u64,
    /// What is resident in the system's memory that the GPU maps (`drm-resident-gtt`).
    mapped: u64,
}

/// A field of `/proc/<pid>/status` or of a file of `fdinfo`, `name:` followed by a size, in KiB:
/// `status` writes `kB`; the kernel's DRM writes bytes, `KiB` or `MiB`, whichever divides the size
/// (`drm_print_memory_stats`). `None` if the field is missing; a panic if it holds no size.
fn kib(text: &str, name: &str) -> Option<u64> {
    let value = text
        .lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(':'))?
        .trim();
    let (figure, unit) = value.split_once(' ').unwrap_or((value, ""));
    let figure: u64 = figure
        .parse()
        .unwrap_or_else(|_| panic!("{name}: not a size: {value:?}"));
    Some(match unit.trim() {
        "" => figure.div_ceil(1024),
        "kB" | "KiB" => figure,
        "MiB" => figure * 1024,
        unit => panic!("{name}: an unknown unit {unit:?}"),
    })
}

/// The memory of `pid`, now.
fn memory(pid: u32) -> Memory {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))
        .unwrap_or_else(|error| panic!("the process {pid} cannot be read: {error}"));
    let field = |name| kib(&status, name).unwrap_or_else(|| panic!("no {name} in {status}"));
    Memory {
        resident: field("VmRSS"),
        anonymous: field("RssAnon"),
        files: field("RssFile"),
        shared: field("RssShmem"),
        peak: field("VmHWM"),
        video: video(pid),
    }
}

/// The video memory of `pid`, from each of its files that open a GPU through the kernel's DRM;
/// several files that the same client of a device opened count once. `None` if it opened none; a
/// figure that the driver does not give counts as none.
fn video(pid: u32) -> Option<Video> {
    let mut clients = BTreeMap::new();
    for file in fs::read_dir(format!("/proc/{pid}/fd")).ok()?.flatten() {
        let opens_a_gpu =
            fs::read_link(file.path()).is_ok_and(|target| target.starts_with("/dev/dri"));
        let info = format!("/proc/{pid}/fdinfo/{}", file.file_name().to_string_lossy());
        let Some(info) = opens_a_gpu.then(|| fs::read_to_string(info).ok()).flatten() else {
            continue;
        };
        let field = |name: &str| {
            info.lines()
                .find_map(|line| line.strip_prefix(name)?.strip_prefix(':'))
                .map(|value| value.trim().to_owned())
        };
        // A client's identity is unique on its device alone.
        let Some(client) = field("drm-client-id").map(|id| (field("drm-pdev"), id)) else {
            continue;
        };
        clients.insert(
            client,
            Video {
                resident: kib(&info, "drm-resident-vram").unwrap_or(0),
                asked: kib(&info, "drm-total-vram").unwrap_or(0),
                mapped: kib(&info, "drm-resident-gtt").unwrap_or(0),
            },
        );
    }
    (!clients.is_empty()).then(|| {
        clients
            .values()
            .fold(Video::default(), |sum, client| Video {
                resident: sum.resident + client.resident,
                asked: sum.asked + client.asked,
                mapped: sum.mapped + client.mapped,
            })
    })
}

/// KiB as MiB, with a decimal.
struct Mib(u64);

impl fmt::Display for Mib {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[expect(clippy::cast_precision_loss, reason = "a figure printed to a tenth")]
        let mib = self.0 as f64 / 1024.0;
        write!(formatter, "{mib:.1} MiB")
    }
}

/// KiB gained or lost, as MiB with their sign.
struct Change(u64, u64);

impl fmt::Display for Change {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(before, after) = *self;
        let sign = if after < before { "-" } else { "+" };
        write!(formatter, "{sign}{}", Mib(after.abs_diff(before)))
    }
}

impl Memory {
    /// Prints this memory, read at `when`.
    fn print(&self, when: &str) {
        println!(
            "{when}: resident {}, of it anonymous {}, files {}, shared {}; at most {}",
            Mib(self.resident),
            Mib(self.anonymous),
            Mib(self.files),
            Mib(self.shared),
            Mib(self.peak)
        );
        match self.video {
            Some(video) => println!(
                "  video memory: resident {}, asked for {}; mapped from the system's {}",
                Mib(video.resident),
                Mib(video.asked),
                Mib(video.mapped)
            ),
            None => println!("  video memory: none counted, the GPU's driver opened no DRM device"),
        }
    }

    /// Prints what changed from `before` to this memory, over `what`.
    fn print_change(&self, before: &Self, what: &str) {
        println!(
            "over {what}: resident {}, anonymous {}, files {}, shared {}",
            Change(before.resident, self.resident),
            Change(before.anonymous, self.anonymous),
            Change(before.files, self.files),
            Change(before.shared, self.shared)
        );
        if let (Some(was), Some(is)) = (before.video, self.video) {
            println!(
                "  video memory: resident {}, asked for {}, mapped {}",
                Change(was.resident, is.resident),
                Change(was.asked, is.asked),
                Change(was.mapped, is.mapped)
            );
        }
    }
}

/// Reads the GPU's power from `sensor` for [`POWER_WINDOW`], and prints its mean, lowest and
/// highest, `what` saying what runs meanwhile.
fn power(sensor: &Path, what: &str) {
    let started = Instant::now();
    let mut readings = Vec::new();
    while started.elapsed() < POWER_WINDOW {
        let text = fs::read_to_string(sensor)
            .unwrap_or_else(|error| panic!("{}: {error}", sensor.display()));
        let microwatts: u64 = text
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("{}: not a figure: {text}", sensor.display()));
        readings.push(microwatts);
        std::thread::sleep(POWER_PERIOD);
    }
    #[expect(clippy::cast_precision_loss, reason = "watts printed to a tenth")]
    let watts = |microwatts: u64| microwatts as f64 / 1e6;
    let mean = readings.iter().sum::<u64>() / readings.len() as u64;
    println!(
        "the GPU's power, {what}: mean {:.1} W, from {:.1} to {:.1} W, {} readings over {POWER_WINDOW:?}",
        watts(mean),
        watts(*readings.iter().min().expect("readings")),
        watts(*readings.iter().max().expect("readings")),
        readings.len()
    );
}

/// The process that runs `program` among `pid` and its descendants: the program itself, or the
/// one that `heaptrack` starts.
fn process_of(pid: u32, program: &Path) -> u32 {
    let program = fs::canonicalize(program).expect("the program's path");
    let runs = |pid: u32| fs::read_link(format!("/proc/{pid}/exe")).is_ok_and(|exe| exe == program);
    let parent_of = |pid: u32| {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // `ppid`, the 4th field, the 2nd after the name.
        rest::stat_fields(&stat).get(1)?.parse::<u32>().ok()
    };
    let started = Instant::now();
    loop {
        let mut descendants = vec![pid];
        let mut found = 0;
        while found < descendants.len() {
            let parent = descendants[found];
            found += 1;
            if runs(parent) {
                return parent;
            }
            for entry in fs::read_dir("/proc").expect("/proc").flatten() {
                let child = entry
                    .file_name()
                    .to_str()
                    .and_then(|name| name.parse().ok());
                if let Some(child) = child.filter(|&child| parent_of(child) == Some(parent)) {
                    descendants.push(child);
                }
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "no process of {pid} runs {}",
            program.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The seconds that the process `pid` has run for, from its start that the system counts in ticks
/// of `/proc/<pid>/stat`, to the system's uptime: what places a phase in `heaptrack`'s record.
fn seconds_run(pid: u32) -> f64 {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).expect("the process's stat");
    // `starttime`, the 22nd field, the 20th after the name, in ticks of 100 a second: what Linux
    // gives on every architecture that the engine runs on (`sysconf(_SC_CLK_TCK)`).
    let ticks: f64 = rest::stat_fields(&stat)
        .get(19)
        .and_then(|ticks| ticks.parse().ok())
        .expect("a start");
    let uptime: f64 = fs::read_to_string("/proc/uptime")
        .expect("the uptime")
        .split_whitespace()
        .next()
        .and_then(|seconds| seconds.parse().ok())
        .expect("an uptime");
    uptime - ticks / 100.0
}

/// The measuring run: what it asks of the editor and the game, how it waits for them, and what it
/// prints.
struct Run {
    session: Session,
    /// The editor's process, or the game's.
    pid: u32,
    /// The number of the last line of the log already read.
    read: u64,
    /// Whether the editor runs under `heaptrack`.
    heaptrack: bool,
}

impl Run {
    /// Calls the tool `name` with `arguments`, and returns its first text.
    fn tool(&mut self, name: &str, arguments: serde_json::Value) -> String {
        let (said, _) = self.session.tool(name, arguments);
        said.into_iter().next().unwrap_or_default()
    }

    /// Waits until a line of the log after those already read holds `what`, for at most `limit`,
    /// and returns it; the warnings and errors that the program logs meanwhile are printed.
    fn wait_for(&mut self, what: &str, limit: Duration) -> String {
        let started = Instant::now();
        let mut last = Vec::new();
        loop {
            let read = self.tool("read_logs", json!({ "after": self.read }));
            for line in read.lines() {
                let Some((number, text)) = line.split_once(": ") else {
                    continue;
                };
                self.read = number.parse().unwrap_or(self.read);
                if text.trim_start().starts_with("WARN") || text.trim_start().starts_with("ERROR") {
                    println!("  logged: {text}");
                }
                if text.contains(what) {
                    return text.to_owned();
                }
                last.push(text.to_owned());
                if last.len() > 20 {
                    last.remove(0);
                }
            }
            assert!(
                started.elapsed() < limit,
                "{what:?} not logged within {limit:?}; the last lines:\n{}",
                last.join("\n")
            );
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// Waits until the program is at rest, what it was asked done, its data loaded and its
    /// notifications gone, each at its own deadline, which wakes it once more; a run under
    /// `heaptrack`, whose thread writes the record, is never at rest, and waits as long as a
    /// notification shows.
    fn settle(&self) {
        if self.heaptrack {
            std::thread::sleep(Duration::from_secs(10));
            return;
        }
        let started = Instant::now();
        while let Err(woke) = rest::at_rest(self.pid, SETTLE) {
            // The process itself ended, not one of its threads.
            let process = format!("the process {} ", self.pid);
            let ended = [
                format!("{process}ended"),
                format!("{process}cannot be read"),
            ]
            .iter()
            .any(|gone| woke.starts_with(gone));
            assert!(
                !ended && started.elapsed() < SETTLE,
                "not at rest within {SETTLE:?}: {woke}"
            );
        }
    }

    /// Stops the program that `start_game` started, and checks that it ended by itself, cleanly: a
    /// program killed would have cut `heaptrack`'s record short.
    fn stop(&mut self) {
        let ended = self.tool("stop_game", json!({}));
        assert!(
            ended.starts_with("the game ended: exit status: 0"),
            "{ended}"
        );
    }

    /// Settles the program, and returns its memory, printed as read at `when`; a run under
    /// `heaptrack` says at which of the editor's seconds.
    fn measure(&self, when: &str) -> Memory {
        self.settle();
        let memory = memory(self.pid);
        memory.print(when);
        if self.heaptrack {
            println!(
                "  heaptrack: {:.1} s after the editor started",
                seconds_run(self.pid)
            );
        }
        memory
    }

    /// Plays the zone from the editor until the game gives control, then stops it, and waits until
    /// it stopped cleanly.
    fn play(&mut self) {
        assert_eq!(
            self.tool("run_operation", json!({ "name": "play.start" })),
            "done"
        );
        self.wait_for("Play: the game gave control", PLAY_LIMIT);
        assert_eq!(
            self.tool("run_operation", json!({ "name": "play.stop" })),
            "done"
        );
        let ended = self.wait_for("Play: the game stopped", STOP_LIMIT);
        assert!(ended.contains(" INFO "), "not stopped cleanly: {ended}");
    }

    /// The frame that `game_state` says the game is at.
    fn frame(&mut self) -> u64 {
        let state = self.tool("game_state", json!({}));
        state
            .strip_prefix("frame ")
            .and_then(|rest| rest.split(',').next())
            .and_then(|number| number.parse().ok())
            .unwrap_or_else(|| panic!("no frame in {state:?}"))
    }
}

fn main() {
    let options = Options::read();
    let server = server();
    let target = server.parent().expect("the target directory");
    let [editor, game] = ["hunnawood-editor", "hunnawood"].map(|name| {
        let program = target.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        assert!(
            program.is_file(),
            "{} is not built: `cargo build -p {name}` first, in this run's profile",
            program.display()
        );
        program
    });
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let cooked = workspace.join("target/cooked/hunnawood");
    assert!(
        cooked.is_dir(),
        "{} holds no cooked zone: games/hunnawood/README.md says how",
        cooked.display()
    );
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("slime-mcp-resources");
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("a directory of the run's own");
    let zone = workspace.join("games/hunnawood/assets/zone");
    for name in ["zone.scene", "zone.scene.meta"] {
        fs::copy(zone.join(name), directory.join(name)).expect("the zone copied");
    }
    let path = |path: &Path| path.to_str().expect("a path of text").to_owned();

    if let Some(sensor) = &options.power {
        power(sensor, "before anything starts");
    }

    let mut command = Vec::new();
    if let Some(record) = &options.heaptrack {
        command.extend(["heaptrack", "--record-only", "--quiet", "--output"].map(str::to_owned));
        command.push(path(record));
    }
    command.extend([
        path(&editor),
        "--level".to_owned(),
        path(&directory.join("zone.scene")),
        "--assets".to_owned(),
        path(&cooked),
        "--journal".to_owned(),
        path(&directory.join("zone.journal")),
    ]);
    let (session, mut server, _) = Session::start(&workspace, "resources");
    let mut run = Run {
        session,
        pid: 0,
        read: 0,
        heaptrack: options.heaptrack.is_some(),
    };
    let started = run.tool("start_game", json!({ "command": command }));
    println!("{started}");
    run.pid = process_of(rest::launched(&started), &editor);
    run.wait_for("the level is drawn whole", SETTLE);
    run.measure("the editor on the zone, at rest");
    if let Some(sensor) = &options.power {
        power(sensor, "the editor at rest on the zone");
    }

    // A step, its value changed, undone and done again: as many times first as fill what the frames
    // that follow each other fast make once, then as many times as the budget says.
    let campfire = "zone/land/campfire light";
    run.tool(
        "edit",
        json!({
            "label": "Dim the campfire",
            "commands": [format!(
                "[SetProperty]\nentity = \"{campfire}\"\ncomponent = \"hunnawood::Flickers\"\n\
                 path = \"intensity\"\nvalue = 90.0\n"
            )],
        }),
    );
    let undo_and_redo = |run: &mut Run, times: usize| {
        for _ in 0..times {
            run.tool("undo", json!({}));
            run.tool("redo", json!({}));
        }
    };
    undo_and_redo(&mut run, WARM_UNDOS);
    let before = run.measure(&format!("after a step and {WARM_UNDOS} undos and redos"));
    undo_and_redo(&mut run, options.undos);
    let undos = options.undos;
    let after = run.measure(&format!("after {undos} undos and redos more"));
    after.print_change(&before, &format!("{undos} undos and redos"));
    let flicker = run.tool("inspect_entity", json!({ "entity": campfire }));
    assert!(flicker.contains("intensity = 90.0"), "redone: {flicker}");

    // Play, once to build the game and fill what a first Play makes, then as many times as the
    // budget says.
    run.play();
    let before = run.measure("after a Play, which built the game");
    for _ in 0..options.plays {
        run.play();
    }
    let plays = options.plays;
    let after = run.measure(&format!("after {plays} Plays more"));
    after.print_change(&before, &format!("{plays} Plays"));
    run.stop();

    // The game, at its schedule of 60 frames a second, its zone drawn, the hero standing.
    if options.heaptrack.is_none() {
        let command = [
            path(&game),
            "--new".to_owned(),
            "--save".to_owned(),
            path(&directory.join("run.save")),
            "--assets".to_owned(),
            path(&cooked),
        ];
        let started = run.tool("start_game", json!({ "command": command }));
        run.pid = rest::pid(&started, &game);
        run.read = 0;
        run.wait_for("the zone is drawn", SETTLE);
        std::thread::sleep(GAME_LOADS);
        memory(run.pid).print("the game on its zone, running");
        if let Some(sensor) = &options.power {
            let (first, started) = (run.frame(), Instant::now());
            power(sensor, "the game running at its 60 frames a second");
            let frames = run.frame() - first;
            #[expect(clippy::cast_precision_loss, reason = "a rate printed to a tenth")]
            let rate = frames as f64 / started.elapsed().as_secs_f64();
            println!("  the game ran {frames} frames meanwhile, {rate:.1} a second");
        }
        run.stop();
    }

    drop(run);
    let status = server.wait().expect("the server ends");
    assert!(status.success(), "the server ended with {status}");
    if let Some(sensor) = &options.power {
        std::thread::sleep(Duration::from_secs(5));
        power(sensor, "once both ended");
    }
    let _ = fs::remove_dir_all(&directory);
}
