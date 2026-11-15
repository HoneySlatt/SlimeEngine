//! A program in memory, for the tests: an interface that `uwui` answers, behind both ends of the
//! session of uwui's core for agents, the tool's and the program's, so that each request crosses
//! the core's messages as on a connection, without a socket.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use uwui::agents::{Answer as UiAnswer, answer};
use uwui::host::{Event, Input, TargetId, TargetSize};
use uwui::style::{Dimension, StyleBuilder};
use uwui::text::Fonts;
use uwui::time::{EventTime, Instant};
use uwui::tree::{Element, RootSettings, Ui};
use uwui_agent::interface::{self, Reply};
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

/// A request, and where its answer goes.
struct Job {
    method: String,
    params: Value,
    answered: mpsc::Sender<Result<Answer, String>>,
}

/// A program in memory, its interface on a thread of its own, which owns it: each request is
/// answered at once, so that none is ever in flight for a cancel to reach.
#[derive(Clone)]
pub struct Memory {
    jobs: Arc<Mutex<mpsc::Sender<Job>>>,
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
        thread::spawn(move || serve(build(), &taken, items));
        Self {
            jobs: Arc::new(Mutex::new(jobs)),
        }
    }

    /// Sends the request `method` with `params`, as a carrier does.
    pub fn send(&self, method: &str, params: Value) -> Result<Sent, String> {
        let (answered, answer) = mpsc::channel();
        let job = Job {
            method: method.to_owned(),
            params,
            answered,
        };
        self.jobs
            .lock()
            .map_err(|_| "the program's queue is poisoned".to_owned())?
            .send(job)
            .map_err(|_| "the program ended".to_owned())?;
        Ok(Sent::new(
            move || answer.recv().map_err(|_| "the program ended".to_owned())?,
            || {},
        ))
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

/// Welcomes the tool, then answers each job with `ui`, its pages of at most `items` items, until
/// no sender is left.
fn serve(mut ui: Ui, jobs: &mpsc::Receiver<Job>, items: Option<u64>) {
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
    for job in jobs {
        let answer = answered(
            &mut ui,
            &mut program,
            &mut tool,
            (reads, items),
            job.method,
            job.params,
        );
        let _ = job.answered.send(answer);
    }
}

/// The answer that `ui` gives to `method` and `params`, through both ends of the session.
fn answered(
    ui: &mut Ui,
    program: &mut ProgramSession,
    tool: &mut ToolSession,
    (reads, items): (Limits, Option<u64>),
    method: String,
    params: Value,
) -> Result<Answer, String> {
    let (id, line) = tool
        .request(&method, params)
        .map_err(|error| error.to_string())?;
    let (id, outcome, bytes) = match program
        .receive(
            read(
                &line,
                Limits {
                    line: 1 << 20,
                    bytes: 0,
                },
            ),
            |_| false,
        )
        .map_err(|error| error.to_string())?
    {
        FromTool::Request {
            id: received,
            family,
            name,
            params,
            ..
        } => {
            assert_eq!(received, id);
            let (outcome, bytes) = interface(ui, &format!("{family}/{name}"), params, reads, items);
            (id, outcome, bytes)
        }
        FromTool::Answered { reply } => {
            // Refused by the session itself, as a family not served.
            return match tool.receive(read(&reply, READS)) {
                Ok(FromProgram::Response { outcome, bytes, .. }) => Ok(Answer { outcome, bytes }),
                other => Err(format!("the session answered {other:?}")),
            };
        }
        other => return Err(format!("the program read {other:?}")),
    };
    let (mut written, count) = program
        .answer(id, outcome, bytes.len())
        .map_err(|error| error.to_string())?;
    program.written(id);
    written.extend_from_slice(&bytes[..count]);
    match tool.receive(read(&written, READS)) {
        Ok(FromProgram::Response { outcome, bytes, .. }) => Ok(Answer { outcome, bytes }),
        other => Err(format!("the tool read {other:?}")),
    }
}

/// The interface's answer to the request `method` with `params`, its pages of at most `items`
/// items: its outcome, and the bytes that follow it, a capture's pixels, each a gray of its
/// column.
fn interface(
    ui: &mut Ui,
    method: &str,
    params: Value,
    reads: Limits,
    items: Option<u64>,
) -> (Result<Value, Fault>, Vec<u8>) {
    let request: interface::Request = match uwui_agent::read_call(method, params) {
        Ok(request) => request,
        Err(fault) => return (Err(fault), Vec::new()),
    };
    let page = Page::new(PROGRAM, reads.line).within(items, None);
    let written = |reply: &Reply| {
        uwui_agent::write(reply).map_err(|error| Fault::new(Fault::INVALID, error.to_string()))
    };
    match answer(ui, &request, page, || false) {
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
