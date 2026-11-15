//! A program of the tests' own, on the development channel's listener alone, which the engine's
//! MCP server starts with `start_game` as it starts a game: a test's own binary, run again with
//! the token that the server gives it. It answers at once, with no frame, and holds the edits
//! that come while a key is held, as an editor holds an agent's edits while a person drags,
//! applying them once the key is let go, but those that their tool cancelled meanwhile, which it
//! answers so. Its report `seen` tells what it applied, holds and cancelled; `world/entities`
//! gives [`ENTITIES`] entities on one page.

use std::io::Read as _;
use std::sync::mpsc;
use std::time::Duration;

use slime_channel::{
    ANNOUNCEMENT, Edit, EntitySummary, Incoming, Listener, Reply, Request, TOKEN_ENV_VAR, Token,
};

/// How many entities a page of `world/entities` gives.
pub const ENTITIES: usize = 100;

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

/// Runs as a program that never says where it listens, when the server started this binary with a
/// token, until its tool closes its input; says not, at once, in a run of the tests.
pub fn stay_silent_if_started() -> bool {
    if std::env::var_os(TOKEN_ENV_VAR).is_none() {
        return false;
    }
    let _ = std::io::stdin().read_to_end(&mut Vec::new());
    true
}

/// Runs as the program when the server started this binary with a token, until its tool asks it
/// to stop or closes its input, and says so; says not, at once, in a run of the tests.
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
                    edit: Edit::Step { .. },
                } if seen.dragging => seen.held.push(incoming),
                Request::Edit {
                    edit: Edit::Step { .. },
                } => {
                    seen.applied.push(label(&incoming));
                    incoming.answer(Reply::Done);
                }
                Request::Press { .. } => {
                    seen.dragging = true;
                    incoming.answer(Reply::Done);
                }
                Request::Release { .. } => {
                    seen.dragging = false;
                    for held in std::mem::take(&mut seen.held) {
                        seen.applied.push(label(&held));
                        held.answer(Reply::Done);
                    }
                    incoming.answer(Reply::Done);
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
                Request::Stop => {
                    incoming.answer(Reply::Done);
                    // Its answer written before the program ends.
                    std::thread::sleep(Duration::from_millis(200));
                    return true;
                }
                _ => incoming.answer(Reply::refused("the tests' program does not do it")),
            }
        }
    }
    // The listener, which holds what wakes the program, is gone.
    true
}
