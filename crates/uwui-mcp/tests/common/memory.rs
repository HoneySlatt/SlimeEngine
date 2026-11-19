//! A program in memory, for the tests: an interface that `uwui` answers, behind both ends of the
//! session of uwui's core for agents, the tool's and the program's, so that each request crosses
//! the core's messages as on a connection, without a socket. It holds the waits on its interface
//! in flight, as a program's host does, and takes their cancels through the core.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use uwui::agents::{Answer as UiAnswer, WaitId, Waiting, Waits, answer};
use uwui::host::{Event, Input, TargetId, TargetSize};
use uwui::style::{Dimension, StyleBuilder};
use uwui::text::Fonts;
use uwui::time::{EventTime, Instant};
use uwui::tree::{Element, RootSettings, Ui};
use uwui_agent::interface::{self, Reply, Request};
use uwui_agent::page::Page;
use uwui_agent::{
    CORE_VERSION, Family, Fault, FromProgram, FromTool, Hello, InFlight, Limits, Named,
    ProgramSession, Reader, ToolSession, Value,
};
use uwui_mcp::{Answer, Sent};

/// This file, as the compiler names it, which an element's own style written here names: its
/// separators are the system's.
pub const FILE: &str = file!();

/// The number of the process, as a carrier draws it.
const PROGRAM: u64 = 0x5eed;
/// The secret that the program asks.
const SECRET: &str = "memory";
/// The most that the tool reads, as `slime-channel`'s client reads: 16 MiB a line, 1 GiB after it.
const READS: Limits = Limits {
    line: 16 << 20,
    bytes: 1 << 30,
};
/// The targets of the interface, which the program frames after each request, as a host would.
const TARGETS: [TargetId; 2] = [TargetId::MAIN, TargetId(2)];

/// What the program's thread is given.
enum Job {
    /// A request, numbered by the tool's carrier, and where its answer goes.
    Request {
        number: u64,
        method: String,
        params: Value,
        answered: mpsc::Sender<Result<Answer, String>>,
    },
    /// The tool's cancel of the request of that number.
    Cancel { number: u64 },
}

/// A request that the program holds, a wait.
struct Held {
    /// The carrier's number.
    number: u64,
    /// The core's.
    id: u64,
    wait: WaitId,
    request: Request,
    answered: mpsc::Sender<Result<Answer, String>>,
}

/// A program in memory, its interface on a thread of its own, which owns it: each request is
/// answered at once, but a wait on the interface, held in flight until it is met or cancelled.
#[derive(Clone)]
pub struct Memory {
    jobs: Arc<Mutex<mpsc::Sender<Job>>>,
    next: Arc<AtomicU64>,
    held: Arc<AtomicUsize>,
}

impl Memory {
    /// A program whose interface `build` makes, welcomed by the tool's hello.
    pub fn start(build: impl FnOnce() -> Ui + Send + 'static) -> Self {
        Self::paged(build, None)
    }

    /// A program as [`Memory::start`] makes, whose pages of a list hold at most `items` items, as
    /// a program cuts a page short once its time passed.
    pub fn paged(build: impl FnOnce() -> Ui + Send + 'static, items: Option<u64>) -> Self {
        let (jobs, taken) = mpsc::channel::<Job>();
        let held = Arc::new(AtomicUsize::new(0));
        let holding = Arc::clone(&held);
        thread::spawn(move || serve(build(), &taken, items, &holding));
        Self {
            jobs: Arc::new(Mutex::new(jobs)),
            next: Arc::new(AtomicU64::new(0)),
            held,
        }
    }

    /// How many waits the program holds now, as its thread last left them.
    pub fn held(&self) -> usize {
        self.held.load(Ordering::SeqCst)
    }

    /// Sends the request `method` with `params`, as a carrier does: its cancel reaches the
    /// program through the core's cancel.
    pub fn send(&self, method: &str, params: Value) -> Result<Sent, String> {
        let (answered, answer) = mpsc::channel();
        let number = self.next.fetch_add(1, Ordering::SeqCst);
        self.job(Job::Request {
            number,
            method: method.to_owned(),
            params,
            answered,
        })?;
        let jobs = Arc::clone(&self.jobs);
        Ok(Sent::new(
            move || answer.recv().map_err(|_| "the program ended".to_owned())?,
            move || {
                if let Ok(jobs) = jobs.lock() {
                    let _ = jobs.send(Job::Cancel { number });
                }
            },
        ))
    }

    fn job(&self, job: Job) -> Result<(), String> {
        self.jobs
            .lock()
            .map_err(|_| "the program's queue is poisoned".to_owned())?
            .send(job)
            .map_err(|_| "the program ended".to_owned())
    }
}

/// What one end wrote, read by a reader of the other's limits.
fn read(bytes: &[u8], limits: Limits) -> uwui_agent::Frame {
    let mut reader = Reader::new(limits);
    reader.push(bytes);
    reader
        .message()
        .expect("a message of the core")
        .expect("whole")
}

/// Both ends of the core's session, and what the program reads.
struct Session {
    program: ProgramSession,
    tool: ToolSession,
    reads: Limits,
    items: Option<u64>,
}

impl Session {
    /// The page that a request of the program reads.
    fn page(&self) -> Page {
        Page::new(PROGRAM, self.reads.line).within(self.items, None)
    }

    /// Writes the program's answer to the core's request `id`, and gives what the tool reads of
    /// it.
    fn answer(
        &mut self,
        id: u64,
        outcome: Result<Value, Fault>,
        bytes: &[u8],
    ) -> Result<Answer, String> {
        let (mut written, count) = self
            .program
            .answer(id, outcome, bytes.len())
            .map_err(|error| error.to_string())?;
        self.program.written(id);
        written.extend_from_slice(&bytes[..count]);
        match self.tool.receive(read(&written, READS)) {
            Ok(FromProgram::Response { outcome, bytes, .. }) => Ok(Answer { outcome, bytes }),
            other => Err(format!("the tool read {other:?}")),
        }
    }
}

/// Welcomes the tool, then takes each job with `ui`, its pages of at most `items` items, until no
/// sender is left: after each request, frames the targets, and answers the waits met; `held`
/// counts the waits that it holds.
fn serve(mut ui: Ui, jobs: &mpsc::Receiver<Job>, items: Option<u64>, held: &AtomicUsize) {
    let families = vec![Family::new(interface::FAMILY, interface::VERSION)];
    let mut program =
        ProgramSession::new("memory", families.clone(), Vec::new(), InFlight::DEFAULT);
    let mut tool = ToolSession::new();
    let hello = tool
        .hello(&Hello {
            core: CORE_VERSION,
            agent: Named::new("a test", "1"),
            client: Named::new("uwui-mcp's tests", "1"),
            secret: SECRET.to_owned(),
            families,
            limits: READS,
        })
        .expect("a hello");
    let FromTool::Welcomed { reads, reply, .. } = program
        .receive(read(&hello, Limits::HELLO), |secret| secret == SECRET)
        .expect("read")
    else {
        panic!("not welcomed");
    };
    assert!(matches!(
        tool.receive(read(&reply, READS)),
        Ok(FromProgram::Welcomed(_))
    ));
    let mut session = Session {
        program,
        tool,
        reads,
        items,
    };
    let mut waits = Waits::default();
    let mut holding: Vec<Held> = Vec::new();
    for job in jobs {
        match job {
            Job::Request {
                number,
                method,
                params,
                answered,
            } => {
                if let Some(answer) = taken(
                    &mut ui,
                    &mut waits,
                    &mut session,
                    (number, method, params, &answered),
                    &mut holding,
                ) {
                    let _ = answered.send(answer);
                }
                for target in TARGETS {
                    ui.frame(target);
                    for met in waits.check(&mut ui, target).met {
                        let at = holding
                            .iter()
                            .position(|held| held.wait == met.wait)
                            .expect("a wait held");
                        let held = holding.remove(at);
                        let outcome =
                            uwui::agents::met(&ui, &held.request, &met, session.page(), || false)
                                .and_then(|reply| written(&reply));
                        let _ = held.answered.send(session.answer(held.id, outcome, &[]));
                    }
                }
            }
            Job::Cancel { number } => {
                let Some(at) = holding.iter().position(|held| held.number == number) else {
                    // Answered already.
                    continue;
                };
                let id = holding[at].id;
                // The cancel crosses the core, as on a connection.
                let cancel = session
                    .tool
                    .cancel(id)
                    .expect("written")
                    .expect("the request waits");
                let read_cancel = session.program.receive(read(&cancel, READS), |_| false);
                assert!(
                    matches!(read_cancel, Ok(FromTool::Cancel { id: cancelled }) if cancelled == id),
                    "{read_cancel:?}"
                );
                let held = holding.remove(at);
                assert!(waits.cancel(&mut ui, held.wait), "held in the waits");
                let fault = Fault::new(Fault::CANCELLED, "the tool cancelled the wait");
                let _ = held.answered.send(session.answer(id, Err(fault), &[]));
            }
        }
        held.store(holding.len(), Ordering::SeqCst);
    }
    waits.clear(Some(&mut ui));
}

/// Takes the request `method` with `params`, numbered `number` by the carrier: its answer, or
/// none for a wait held in `holding`, whose answer goes to `answered` once met.
fn taken(
    ui: &mut Ui,
    waits: &mut Waits,
    session: &mut Session,
    (number, method, params, answered): (u64, String, Value, &mpsc::Sender<Result<Answer, String>>),
    holding: &mut Vec<Held>,
) -> Option<Result<Answer, String>> {
    let (id, line) = match session.tool.request(&method, params) {
        Ok(sent) => sent,
        Err(error) => return Some(Err(error.to_string())),
    };
    let received = session.program.receive(
        read(
            &line,
            Limits {
                line: 1 << 20,
                bytes: 0,
            },
        ),
        |_| false,
    );
    let (family, name, params) = match received {
        Ok(FromTool::Request {
            id: received,
            family,
            name,
            params,
            ..
        }) => {
            assert_eq!(received, id);
            (family, name, params)
        }
        Ok(FromTool::Answered { reply }) => {
            // Refused by the session itself, as a family not served.
            return Some(match session.tool.receive(read(&reply, READS)) {
                Ok(FromProgram::Response { outcome, bytes, .. }) => Ok(Answer { outcome, bytes }),
                other => Err(format!("the session answered {other:?}")),
            });
        }
        other => return Some(Err(format!("the program read {other:?}"))),
    };
    let request: Request = match uwui_agent::read_call(&format!("{family}/{name}"), params) {
        Ok(request) => request,
        Err(fault) => return Some(session.answer(id, Err(fault), &[])),
    };
    let page = session.page();
    if let Request::Wait { .. } = request {
        let outcome = match uwui::agents::wait(ui, waits, &request, page, || false) {
            Ok(Waiting::Reply(reply)) => written(&reply),
            Ok(Waiting::Held(wait)) => {
                holding.push(Held {
                    number,
                    id,
                    wait,
                    request,
                    answered: answered.clone(),
                });
                return None;
            }
            Err(fault) => Err(fault),
            Ok(_) => unreachable!("a way that this test does not know"),
        };
        return Some(session.answer(id, outcome, &[]));
    }
    let (outcome, bytes) = interface(ui, &request, page);
    Some(session.answer(id, outcome, &bytes))
}

/// `reply`, as the family writes it.
fn written(reply: &Reply) -> Result<Value, Fault> {
    uwui_agent::write(reply).map_err(|error| Fault::new(Fault::INVALID, error.to_string()))
}

/// The interface's answer to `request` on `page`: its outcome, and the bytes that follow it, a
/// capture's pixels, each a gray of its column.
fn interface(ui: &mut Ui, request: &Request, page: Page) -> (Result<Value, Fault>, Vec<u8>) {
    match answer(ui, request, page, || false) {
        Ok(UiAnswer::Reply(reply)) => (written(&reply), Vec::new()),
        Ok(UiAnswer::Capture(shown)) => {
            let pixels = (0..shown.height)
                .flat_map(|_| (0..shown.width).flat_map(|x| [x as u8, x as u8, x as u8, 255]))
                .collect();
            let reply = Reply::Image {
                width: shown.width,
                height: shown.height,
            };
            (written(&reply), pixels)
        }
        Err(fault) => (Err(fault), Vec::new()),
        _ => unreachable!("a case that this test does not know"),
    }
}

/// A box `s` of 100 × 40 logical pixels that does not shrink.
fn sized(s: StyleBuilder) -> StyleBuilder {
    s.width(Dimension::Pixels(100.0))
        .height(Dimension::Pixels(40.0))
        .shrink(0.0)
}

/// An interface of two targets, the main one and target 2, each 400 × 300 logical pixels at
/// scale 1: on the main one, a root `main` of three buttons, `open`, `save` and `close`; on target
/// 2, a root `side` of one button, `help`; each laid out. Target 2 is the active target, given
/// the window's focus last, so that what names no target reads another than the window's.
pub fn panel() -> Ui {
    let mut ui = Ui::new(Fonts::new());
    for target in [TargetId::MAIN, TargetId(2)] {
        ui.set_target_size(target, TargetSize::new([400, 300], 1.0).expect("a size"));
    }
    let root = ui.add_root("main", RootSettings::on(TargetId::MAIN), |s| s);
    ui.build(root, 0, |children| {
        for name in ["open", "save", "close"] {
            children.add(name, Element::button(name).style(sized), |_| {});
        }
    });
    let side = ui.add_root("side", RootSettings::on(TargetId(2)), |s| s);
    ui.build(side, 0, |children| {
        children.add("help", Element::button("help").style(sized), |_| {});
    });
    ui.frame(TargetId::MAIN);
    ui.frame(TargetId(2));
    let time = EventTime::simulated(Instant::from_nanos(1));
    let _ = ui.event(Event::new(TargetId(2), time, Input::Focus(true)));
    assert_eq!(ui.active_target(), TargetId(2));
    ui
}

/// An interface of a root `main` on the main target, 400 × 300 logical pixels at scale 1: a
/// button `open` and a list to choose from, `pick`, of two options; laid out.
pub fn chooser() -> Ui {
    let mut ui = Ui::new(Fonts::new());
    ui.set_target_size(
        TargetId::MAIN,
        TargetSize::new([400, 300], 1.0).expect("a size"),
    );
    let root = ui.add_root("main", RootSettings::on(TargetId::MAIN), |s| s);
    ui.build(root, 0, |children| {
        children.add("open", Element::button("open").style(sized), |_| {});
        let pick = Element::choice(["one", "two"], 0).style(sized);
        children.add("pick", pick, |_| {});
    });
    ui.frame(TargetId::MAIN);
    ui
}

/// An interface of `count` buttons, each named by its number after `named`, on the main target,
/// laid out in a column that scrolls.
pub fn buttons(count: usize, named: &'static str) -> impl FnOnce() -> Ui + Send + 'static {
    move || {
        let mut ui = Ui::new(Fonts::new());
        ui.set_target_size(
            TargetId::MAIN,
            TargetSize::new([400, 300], 1.0).expect("a size"),
        );
        let root = ui.add_root("main", RootSettings::on(TargetId::MAIN), |s| s);
        ui.build(root, 0, |children| {
            for at in 0..count {
                let name = format!("{named} {at}");
                children.add(at, Element::button(name).style(sized), |_| {});
            }
        });
        ui.frame(TargetId::MAIN);
        ui
    }
}
