//! A function's body reloaded by `subsecond` under `dx serve --hot-patch` (ADR-0212), its state
//! kept by the signals' names (ADR-0210). Slatty's library is played to a game's page; then, at
//! each run, the bench changes its search field's placeholder in the view's source file and saves
//! it, as an author's editor would. dx compiles the change, links it against the running program's
//! addresses and sends the new table of the functions' addresses. A thread of the bench receives
//! it and wakes the host, which applies it between two frames and reloads the view through that
//! table: the new body runs, each kept signal taking its value back by its name. A run fails if
//! the frame does not show the new placeholder with the state kept, or if the reload notes
//! anything.
//!
//! Timed on the system's monotonic clock, from the save to the end of the frame that shows the
//! change, and its parts: the patch's arrival, what dx counts of its own work, the patch applied,
//! the reload's call, and the frame. A session's first patch is timed apart.
//!
//! ```sh
//! nix develop
//! dx serve --hot-patch --interactive false -p slime-benches --features hot \
//!     --args="--prototypes --hot --runs 21"
//! ```
//!
//! The bench's crate holds as many lines of code as Slatty's interface: `build.rs` adds views of
//! the library's shape; `SLIME_HOT_SCREENS=<count>` sets how many, 0 for the bench's own size.
//! subsecond patches only with debug assertions: the bench runs in the `dev` profile, as an author
//! runs it. The bench puts the source back as it was once it ends; stopped by a signal, it leaves
//! the last placeholder that it wrote.

use std::cell::Cell;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use serde::Deserialize as _;
use subsecond::{HotFn, JumpTable};
use tungstenite::{Message, WebSocket};

use super::host::Host;
use super::reload::{Kept, kept, played, settled};
use super::signals::{self, HostWake, LIBRARY, Signals};
use super::{SETTLE, median};

/// The view's source file, whose placeholder each run changes.
const SOURCE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/prototypes/signals/library.rs"
);

/// The placeholder as the source holds it, and the one that every other run writes.
const PLACEHOLDERS: [&str; 2] = ["Search a game", "Find a game"];

/// How long a run waits for dx's patch before it fails.
const PATCH_LIMIT: Duration = Duration::from_secs(300);

/// What dx's thread gives the host.
enum FromDx {
    /// dx started to compile a change.
    Compiling(Instant),
    /// A patch for this process, as it arrived, and the milliseconds that dx says it took.
    Patch {
        table: JumpTable,
        arrived: Instant,
        dx_ms: u64,
    },
    /// dx asks for a full rebuild, which a body's change should not.
    Rebuild,
    /// The connection ended, with why.
    Closed(String),
}

/// Connects to dx's development server, whose address dx gives the program it runs, and receives
/// its messages on a thread of their own, waking `host` at each one that it gives.
fn connect(host: Arc<HostWake>) -> Result<Receiver<FromDx>, String> {
    let address = std::env::var("DIOXUS_DEVSERVER_IP")
        .ok()
        .zip(std::env::var("DIOXUS_DEVSERVER_PORT").ok())
        .ok_or("no dx development server: run under `dx serve --hot-patch`")?;
    let build: u64 = std::env::var("DIOXUS_BUILD_ID")
        .ok()
        .and_then(|build| build.parse().ok())
        .unwrap_or(0);
    let process = std::process::id();
    // dx links each patch against the running program's addresses: it needs where `main` was
    // loaded.
    let endpoint = format!(
        "ws://{}:{}/_dioxus?aslr_reference={}&build_id={build}&pid={process}",
        address.0,
        address.1,
        subsecond::aslr_reference(),
    );
    let (socket, _) =
        tungstenite::connect(endpoint).map_err(|error| format!("dx's websocket: {error}"))?;
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("dx".to_owned())
        .spawn(move || receive(socket, build, process, &sender, &host))
        .map_err(|error| format!("dx's thread: {error}"))?;
    Ok(receiver)
}

/// Reads dx's messages until the connection ends or the host is gone.
fn receive(
    mut socket: WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>,
    build: u64,
    process: u32,
    sender: &mpsc::Sender<FromDx>,
    host: &HostWake,
) {
    loop {
        let given = match socket.read() {
            Ok(Message::Text(text)) => match given(&text, build, process) {
                Ok(Some(given)) => given,
                Ok(None) => continue,
                Err(error) => FromDx::Closed(error),
            },
            Ok(Message::Close(_)) => FromDx::Closed("dx closed its websocket".to_owned()),
            Ok(_) => continue,
            Err(error) => FromDx::Closed(format!("dx's websocket: {error}")),
        };
        let closed = matches!(given, FromDx::Closed(_));
        if sender.send(given).is_err() {
            return;
        }
        host.wake();
        if closed {
            return;
        }
    }
}

/// What a message of dx's gives the host, if anything. dx 0.7.10 writes its messages as `serde`
/// writes `dioxus_devtools_types::DevserverMsg`, an enum tagged by its variants' names; only the
/// three that a patch's run needs are read.
fn given(text: &str, build: u64, process: u32) -> Result<Option<FromDx>, String> {
    let message: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("dx's message: {error}"))?;
    match &message {
        serde_json::Value::String(name) if name == "HotPatchStart" => {
            Ok(Some(FromDx::Compiling(Instant::now())))
        }
        serde_json::Value::String(name) if name == "FullReloadCommand" => Ok(Some(FromDx::Rebuild)),
        serde_json::Value::Object(fields) => {
            let Some(patch) = fields.get("HotReload") else {
                return Ok(None);
            };
            let ours = patch["for_build_id"].as_u64() == Some(build)
                && patch["for_pid"].as_u64() == Some(u64::from(process));
            if !ours || patch["jump_table"].is_null() {
                return Ok(None);
            }
            let table = JumpTable::deserialize(&patch["jump_table"])
                .map_err(|error| format!("dx's table: {error}"))?;
            Ok(Some(FromDx::Patch {
                table,
                arrived: Instant::now(),
                dx_ms: patch["ms_elapsed"].as_u64().unwrap_or(0),
            }))
        }
        _ => Ok(None),
    }
}

/// The view's source, put back as it was when the bench drops it, a run failed or not.
struct Source {
    path: PathBuf,
    original: String,
}

impl Source {
    /// The source at `path`, which must hold the first placeholder once.
    fn read(path: &Path) -> Result<Self, String> {
        let original = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if original.matches(&quoted(PLACEHOLDERS[0])).count() != 1 {
            return Err(format!(
                "{} holds `{}` once",
                path.display(),
                PLACEHOLDERS[0]
            ));
        }
        Ok(Self {
            path: path.to_owned(),
            original,
        })
    }

    /// Saves the source with `placeholder`, as an editor that writes a new file and renames it
    /// over the old one, so that dx never reads half a file; the time once it is saved.
    fn save(&self, placeholder: &str) -> Result<Instant, String> {
        let text = self
            .original
            .replace(&quoted(PLACEHOLDERS[0]), &quoted(placeholder));
        written(&self.path, &text)
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        if let Err(error) = written(&self.path, &self.original) {
            eprintln!("{} not put back: {error}", self.path.display());
        }
    }
}

fn quoted(text: &str) -> String {
    format!("\"{text}\"")
}

/// Writes `text` into the workspace's `target`, which dx does not watch, then renames it over
/// `path`.
fn written(path: &Path, text: &str) -> Result<Instant, String> {
    let name = path.file_name().ok_or("a source file has a name")?;
    let beside = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target")
        .join(name)
        .with_extension("hot");
    std::fs::write(&beside, text).map_err(|error| format!("{}: {error}", beside.display()))?;
    std::fs::rename(&beside, path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Instant::now())
}

/// A run: a change saved, to the frame that shows it.
struct Run {
    /// From the save to dx's first word that it compiles.
    noticed: Option<Duration>,
    /// From the save to the patch's arrival in the program.
    arrived: Duration,
    /// What dx says the patch took.
    dx: Duration,
    /// The patch applied: the library loaded, the table installed.
    applied: Duration,
    /// The reload's call: the kept values written, the view before disposed, the new one run.
    call: Duration,
    /// The first frame, which shows the change.
    frame: Duration,
    /// From the save to the end of that frame.
    whole: Duration,
}

/// Saves the placeholder `placeholder`, waits for dx's patch, applies it, reloads the library's
/// view through subsecond's table, and runs the frame that shows it; an error if the state was
/// not kept or the patch did not show.
fn run(
    host: &mut Host<Signals<()>>,
    dx: &Receiver<FromDx>,
    source: &Source,
    placeholder: &'static str,
) -> Result<Run, String> {
    let before = kept(host);
    // The library's games are a program's data, loaded at its start, which a reload does not
    // load again: made before the save, outside the time.
    let games = Cell::new(Some(signals::library_games()));
    let wake = host.program.host_wake();
    let saved = source.save(placeholder)?;
    let mut noticed = None;
    let (table, arrived, dx_ms) = loop {
        match dx.try_recv() {
            Ok(FromDx::Compiling(at)) => noticed = Some(at.saturating_duration_since(saved)),
            Ok(FromDx::Patch {
                table,
                arrived,
                dx_ms,
            }) => break (table, arrived, dx_ms),
            Ok(FromDx::Rebuild) => return Err("dx asked for a full rebuild".to_owned()),
            Ok(FromDx::Closed(why)) => return Err(why),
            Err(TryRecvError::Disconnected) => return Err("dx's thread ended".to_owned()),
            Err(TryRecvError::Empty) => {
                if saved.elapsed() > PATCH_LIMIT {
                    return Err(format!("no patch within {PATCH_LIMIT:?}"));
                }
                // As a window's host waits for its proxy, which dx's thread wakes.
                wake.wait_at_most(PATCH_LIMIT);
            }
        }
    };
    let applying = Instant::now();
    #[expect(
        unsafe_code,
        reason = "applying a patch replaces functions, which only dx's table built for this \
                  program makes sound (ADR-0212)"
    )]
    // SAFETY: the table is dx's, built against this very program, whose build and process it
    // names (`given` checks both). It is applied between two frames, while no function that it
    // replaces runs on this thread, and the bench's other thread runs none of them.
    let patched = unsafe { subsecond::apply_patch(table) };
    patched.map_err(|error| format!("the patch: {error}"))?;
    let applied = applying.elapsed();
    let calling = Instant::now();
    host.program.reload(move || {
        let games = games.take().expect("a reload runs the view once");
        // Through subsecond's table: the latest body of the library's view.
        ((), HotFn::current(signals::library::library).call((games,)))
    });
    let call = calling.elapsed();
    let framing = Instant::now();
    host.frame(&[]);
    let shown = Instant::now();
    host.settle(SETTLE);
    let after = kept(host);
    let expected = Kept {
        placeholder: Some(placeholder.to_owned()),
        ..before
    };
    if after != expected {
        return Err(format!(
            "not shown with its state kept: {expected:?}, then {after:?}"
        ));
    }
    let notes = host.program.notes();
    if !notes.is_empty() {
        return Err(format!("the reload noted: {notes:?}"));
    }
    Ok(Run {
        noticed,
        arrived: arrived.saturating_duration_since(saved),
        dx: Duration::from_millis(dx_ms),
        applied,
        call,
        frame: shown - framing,
        whole: shown - saved,
    })
}

/// Plays the library, then reloads a changed body `runs` times under dx, and prints the medians
/// as Markdown, the first patch apart.
pub(crate) fn print_times(runs: usize) -> Result<(), String> {
    if !cfg!(debug_assertions) {
        return Err("subsecond patches only with debug assertions: run the dev profile".to_owned());
    }
    // The views that bring the crate to Slatty's size, kept in the program.
    std::hint::black_box(&signals::sized::SCREENS);
    let mut host = played(&LIBRARY);
    let dx = connect(host.program.host_wake())?;
    // dx registers the program's addresses once it has read the connection: a change saved
    // before is not patched.
    std::thread::sleep(Duration::from_secs(1));
    settled(&mut host);
    let source = Source::read(Path::new(SOURCE))?;
    let mut all = Vec::with_capacity(runs + 1);
    for count in 0..=runs {
        // Each run changes the placeholder from the one that the last showed.
        let placeholder = PLACEHOLDERS[(count + 1) % 2];
        all.push(run(&mut host, &dx, &source, placeholder)?);
        eprintln!(
            "run {count}: {:.0} ms from the save to its frame",
            all[count].whole.as_secs_f64() * 1e3
        );
        // The machine back at rest before the next save.
        std::thread::sleep(Duration::from_millis(500));
    }
    // The source back as it was, and the program with it, so that dx's next session starts from
    // the file on disk.
    if PLACEHOLDERS[(runs + 1) % 2] != PLACEHOLDERS[0] {
        run(&mut host, &dx, &source, PLACEHOLDERS[0])?;
    }
    drop(source);
    let first = all.remove(0);
    println!("### signals: a function's body reloaded by subsecond\n");
    println!(
        "| Patch | dx noticed | Patch arrived | dx's own count | Applied | Reload's call | Its \
         frame | From the save to the frame |"
    );
    println!("|---|---|---|---|---|---|---|---|");
    let ms = |time: Duration| format!("{:.1}", time.as_secs_f64() * 1e3);
    let noticed = |run: &Run| run.noticed.map_or("—".to_owned(), ms);
    println!(
        "| The session's first | {} | {} | {} | {} | {} | {} | {} |",
        noticed(&first),
        ms(first.arrived),
        ms(first.dx),
        ms(first.applied),
        ms(first.call),
        ms(first.frame),
        ms(first.whole),
    );
    let times = |of: fn(&Run) -> Duration| median(all.iter().map(of).collect());
    let noticed_all: Vec<Duration> = all.iter().filter_map(|run| run.noticed).collect();
    println!(
        "| The {} after it | {} | {} | {} | {} | {} | {} | {} |",
        all.len(),
        if noticed_all.len() == all.len() {
            median(noticed_all)
        } else {
            "—".to_owned()
        },
        times(|run| run.arrived),
        times(|run| run.dx),
        times(|run| run.applied),
        times(|run| run.call),
        times(|run| run.frame),
        times(|run| run.whole),
    );
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_for_another_process_is_not_ours() {
        let text = r#"{"HotReload":{"templates":[],"assets":[],"ms_elapsed":3,"jump_table":null,"for_build_id":0,"for_pid":1}}"#;
        assert!(matches!(given(text, 0, 2), Ok(None)));
        assert!(matches!(
            given("\"HotPatchStart\"", 0, 2),
            Ok(Some(FromDx::Compiling(_)))
        ));
        assert!(matches!(
            given("\"FullReloadCommand\"", 0, 2),
            Ok(Some(FromDx::Rebuild))
        ));
        assert!(matches!(given("\"Shutdown\"", 0, 2), Ok(None)));
    }

    #[test]
    fn a_source_saved_is_put_back_once_dropped() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/hot-test-source.rs");
        let original = "fn view() { placeholder(\"Search a game\") }\n";
        std::fs::write(&path, original).expect("target takes a file");
        let source = Source::read(&path).expect("the source holds the placeholder once");
        source.save("Find a game").expect("saved");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            "fn view() { placeholder(\"Find a game\") }\n"
        );
        drop(source);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), original);
        std::fs::remove_file(&path).expect("removed");
    }
}
