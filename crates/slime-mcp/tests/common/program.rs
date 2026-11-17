//! A program of the tests' own, on the development channel's listener alone, which the engine's
//! MCP server starts with `start_game` as it starts a game: a test's own binary, run again with
//! the token that the server gives it. It answers at once, with no frame, and holds the edits
//! that come while a key is held, as an editor holds an agent's edits while a person drags,
//! applying them once the key is let go, but those that their tool cancelled meanwhile, which it
//! answers so. Its report `seen` tells what it applied, holds and cancelled; `world/entities`
//! gives [`ENTITIES`] entities on one page. It holds each request to run steps of game time until
//! it ends, and writes on its standard error, its log, how many it holds ([`STEPS_HELD`]). It
//! plays a game as an editor does, a listener of its own whose report `seen` is [`PLAYED`], which
//! `edit/played` gives; it holds that request too while the key is down, which an editor of the
//! engine never does, so that a test can make it wait. It ends once its standard input closes, as
//! the engine's programs do.

use std::io::Read as _;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;

use slime_channel::{
    ANNOUNCEMENT, Edit, EntitySummary, Incoming, Listener, Reply, Request, TOKEN_ENV_VAR, Token,
};

/// How many entities a page of `world/entities` gives.
pub const ENTITIES: usize = 100;

/// What the program writes in its log each time it holds one more step, before their count.
pub const STEPS_HELD: &str = "steps held: ";

/// The report `seen` of the game that the program plays.
pub const PLAYED: &str = "the played game";

/// What the program saw, which it reports as its report `seen`.
#[derive(Default)]
struct Seen {
    /// The labels of the steps applied, in their order.
    applied: Vec<String>,
    /// The labels of the steps that their tool cancelled while they were held.
    cancelled: Vec<String>,
    /// The steps held while the key is down, the drag.
    held: Vec<Incoming>,
    /// Whether the key is down.
    dragging: bool,
    /// The steps of game time asked for, which it never runs.
    steps: Vec<Incoming>,
}

impl Seen {
    fn report(&self) -> String {
        let held: Vec<String> = self.held.iter().map(label).collect();
        format!(
            "applied: [{}]; held: [{}]; cancelled: [{}]",
            self.applied.join(", "),
            held.join(", "),
            self.cancelled.join(", ")
        )
    }
}

/// The label of a step's request.
fn label(incoming: &Incoming) -> String {
    match incoming.request() {
        Request::Edit {
            edit: Edit::Step { label, .. },
        } => label.clone(),
        other => format!("{other:?}"),
    }
}

/// The name of the file where the silent program writes the port of a socket that it holds while
/// it runs, in the data directory that the session gives the server and the programs that it
/// starts: a test that cannot connect there any more knows that the program ended.
pub const SILENT_PORT: &str = "the silent program's port";

/// Runs as a program that never says where it listens, when the server started this binary with a
/// token, until its tool closes its input or kills it, holding a socket whose port it writes in
/// [`SILENT_PORT`]; says not, at once, in a run of the tests.
pub fn stay_silent_if_started() -> bool {
    if std::env::var_os(TOKEN_ENV_VAR).is_none() {
        return false;
    }
    let socket = TcpListener::bind("127.0.0.1:0").expect("a socket");
    let port = socket.local_addr().expect("its address").port();
    // Each connection taken and closed at once, so that the queue of those waiting never fills:
    // a connection then fails only once the program ended.
    std::thread::spawn(move || {
        for connection in socket.incoming() {
            drop(connection);
        }
    });
    let file = PathBuf::from(std::env::var_os("XDG_DATA_HOME").expect("the session's data"))
        .join(SILENT_PORT);
    std::fs::create_dir_all(file.parent().expect("a directory")).expect("the data directory");
    // Written whole, then named, so that a test never reads half of it.
    let written = file.with_extension("new");
    std::fs::write(&written, port.to_string()).expect("the port written");
    std::fs::rename(&written, &file).expect("the port named");
    let _ = std::io::stdin().read_to_end(&mut Vec::new());
    true
}

/// Opens the game that the program plays, on a listener of its own, which answers its report
/// `seen` with [`PLAYED`] on a thread of its own; gives the reply of `edit/played`, its address
/// and its token.
fn play() -> Reply {
    let token = Token::generate().expect("a token");
    let (wake, woken) = mpsc::channel();
    let listener = Listener::open(token.clone(), "the tests' played game", move || {
        let _ = wake.send(());
    })
    .expect("it listens");
    let played = Reply::Played {
        address: listener.address().to_string(),
        token: token.to_string(),
    };
    std::thread::spawn(move || {
        while woken.recv().is_ok() {
            while let Some(incoming) = listener.next() {
                match incoming.request() {
                    Request::Report { .. } => incoming.answer(Reply::Report {
                        text: PLAYED.to_owned(),
                    }),
                    _ => incoming.answer(Reply::refused("the tests' played game does not do it")),
                }
            }
        }
    });
    played
}

/// Applies `incoming`, an edit, once no drag holds it: a step, which `seen` notes, or the game
/// that the program plays, `played`.
fn apply(incoming: Incoming, seen: &mut Seen, played: &Reply) {
    if matches!(incoming.request(), Request::Edit { edit: Edit::Played }) {
        incoming.answer(played.clone());
        return;
    }
    seen.applied.push(label(&incoming));
    incoming.answer(Reply::Done);
}

/// Runs as the program when the server started this binary with a token, until its tool closes
/// its input, and says so; says not, at once, in a run of the tests.
pub fn run_if_started() -> bool {
    let Ok(token) = std::env::var(TOKEN_ENV_VAR) else {
        return false;
    };
    let token: Token = token.parse().expect("a token");
    let (wake, woken) = mpsc::channel();
    let listener = Listener::open(token, "the tests' program", move || {
        let _ = wake.send(());
    })
    .expect("it listens");
    println!("{ANNOUNCEMENT}{}", listener.address());
    // The server holds the program's input: it ends once the server does.
    std::thread::spawn(|| {
        let _ = std::io::stdin().read_to_end(&mut Vec::new());
        std::process::exit(0);
    });
    let played = play();
    let mut seen = Seen::default();
    // Woken by each request that comes, and by each cancel.
    while woken.recv().is_ok() {
        for incoming in std::mem::take(&mut seen.held) {
            if incoming.cancelled() {
                seen.cancelled.push(label(&incoming));
                incoming.cancel();
            } else {
                seen.held.push(incoming);
            }
        }
        while let Some(incoming) = listener.next() {
            match incoming.request() {
                Request::Edit {
                    edit: Edit::Step { .. } | Edit::Played,
                } if seen.dragging => seen.held.push(incoming),
                Request::Edit {
                    edit: Edit::Step { .. } | Edit::Played,
                } => apply(incoming, &mut seen, &played),
                Request::Press { .. } => {
                    seen.dragging = true;
                    incoming.answer(Reply::Done);
                }
                Request::Release { .. } => {
                    seen.dragging = false;
                    for held in std::mem::take(&mut seen.held) {
                        apply(held, &mut seen, &played);
                    }
                    incoming.answer(Reply::Done);
                }
                Request::Step { .. } => {
                    seen.steps.push(incoming);
                    eprintln!("{STEPS_HELD}{}", seen.steps.len());
                }
                Request::Report { .. } => {
                    let text = seen.report();
                    incoming.answer(Reply::Report { text });
                }
                Request::Entities { .. } => {
                    let entities = (0..ENTITIES)
                        .map(|number| EntitySummary {
                            identity: format!("yard/entity {number}"),
                            name: format!("entity {number}"),
                            cut: Vec::new(),
                        })
                        .collect();
                    incoming.answer(Reply::Entities {
                        entities,
                        next: None,
                    });
                }
                _ => incoming.answer(Reply::refused("the tests' program does not do it")),
            }
        }
    }
    // The listener, which holds what wakes the program, is gone.
    true
}
