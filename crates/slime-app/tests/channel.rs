//! The development channel of a headless run (`docs/adr/0122-the-development-channel.md`): a run
//! started with a token listens and says where; a tool that gives the token pauses, steps and stops
//! it, reads and changes its world, also by the texts of commands, presses its keys, reads its log,
//! and saves and loads its run; one without the token is refused; a tool's edits wait while the
//! simulation says so; a run started without a token says nothing and does not listen.
//!
//! A run reads its token from its environment, which only a process's start sets, so each test runs
//! this test program again as a child, restricted to the run, its environment set.

use std::collections::VecDeque;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use slime_app::{AppConfig, AppError, Simulation, SimulationContext, run};
use slime_channel::{
    ANNOUNCEMENT, ChannelError, Client, Event, GameTime, Named, Notice, Pending, Reply, Request,
    TOKEN_ENV_VAR, Token, launch,
};
use slime_color::LinearColor;
use slime_platform::KeyCode;
use slime_world::{Light, Name, Transform, World};

/// The tests, as they name themselves to the game.
fn tester() -> Named {
    Named::new("slime-app's tests of the channel", "")
}

const CHILD_ENV: &str = "SLIME_APP_TEST_CHILD";

/// A lantern, which moves 1 m along x at each fixed step while W is held, logs each key put down,
/// and saves where it stands, its x as text.
struct Lantern;

/// The lantern of `world`.
fn lantern(world: &World) -> slime_world::Entity {
    world
        .find(&"lantern".parse().expect("a key"))
        .expect("spawned")
}

impl Simulation for Lantern {
    fn start(
        &mut self,
        _ctx: &mut SimulationContext<'_>,
        world: &mut World,
    ) -> Result<(), AppError> {
        let lantern = world.spawn_keyed("lantern")?;
        world.insert(lantern, Transform::IDENTITY);
        world.insert(
            lantern,
            Light::Point {
                color: LinearColor::WHITE,
                intensity: 30.0,
                range: None,
            },
        );
        Ok(())
    }

    fn fixed_step(&mut self, ctx: &mut SimulationContext<'_>, world: &mut World) {
        if ctx.input().keys().held(KeyCode::KeyW) {
            let lantern = lantern(world);
            world
                .get_mut::<Transform>(lantern)
                .expect("placed")
                .translation
                .x += 1.0;
        }
    }

    fn update(&mut self, ctx: &mut SimulationContext<'_>, _world: &mut World) {
        if ctx.input().keys().pressed(KeyCode::KeyW) {
            tracing::info!("W goes down");
        }
        // Headless frames run back to back: a little rest each keeps the test light.
        thread::sleep(Duration::from_millis(1));
    }

    fn save(&mut self, world: &World, file: &Path) -> Result<(), AppError> {
        let x = world
            .get::<Transform>(lantern(world))
            .expect("placed")
            .translation
            .x;
        std::fs::write(file, x.to_string())?;
        Ok(())
    }

    fn load(&mut self, world: &mut World, file: &Path) -> Result<(), AppError> {
        let x = std::fs::read_to_string(file)?.parse()?;
        let lantern = lantern(world);
        world
            .get_mut::<Transform>(lantern)
            .expect("placed")
            .translation
            .x = x;
        Ok(())
    }
}

/// The child's run: until a tool stops it.
fn child() {
    run(&mut Lantern, &mut (), &AppConfig::default()).expect("the run starts");
}

/// This test program, restricted to `test`, as a child that runs a game.
fn command(test: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("the test program"));
    command
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .env_remove(slime_app::LOG_ENV_VAR)
        .stderr(Stdio::null());
    command
}

/// Asks `request`, and returns the reply.
fn ask(client: &Client, request: Request) -> Reply {
    client.request(&request).expect("answered").0
}

/// Where the lantern stands along x, as the run that `client` drives answers.
fn x(client: &Client) -> f64 {
    let Reply::Entity { components, .. } = ask(
        client,
        Request::Entity {
            entity: "lantern".to_owned(),
        },
    ) else {
        panic!("the lantern");
    };
    let transform = components
        .iter()
        .find(|component| component.name == "Transform")
        .expect("placed");
    let x = transform
        .value
        .split("x = ")
        .nth(1)
        .and_then(|rest| rest.split([',', ' ']).next())
        .expect("an x");
    x.parse().expect("a number")
}

#[test]
fn a_tool_with_the_token_drives_a_headless_run() {
    if std::env::var_os(CHILD_ENV).is_some() {
        return child();
    }
    let mut launched = launch(
        &mut command("a_tool_with_the_token_drives_a_headless_run"),
        Duration::from_secs(60),
    )
    .expect("the run listens");
    assert!(launched.address.ip().is_loopback());
    let other = Token::generate().expect("random bits");
    assert!(matches!(
        Client::connect(launched.address, &other, &tester(), &tester()),
        Err(slime_channel::ChannelError::Refused(_))
    ));
    let client =
        Client::connect(launched.address, &launched.token, &tester(), &tester()).expect("accepted");

    assert_eq!(ask(&client, Request::Pause), Reply::Done);
    let Reply::State { state } = ask(&client, Request::State) else {
        panic!("a state");
    };
    assert!(state.paused);
    assert_eq!(state.entities, 1);
    let steps = state.steps;
    // W held, twenty steps, more than a frame runs: the step is answered once they have all run,
    // and the lantern moved 20 m.
    let w = || "KeyW".to_owned();
    assert_eq!(ask(&client, Request::Press { key: w() }), Reply::Done);
    assert_eq!(ask(&client, Request::Step { steps: 20 }), Reply::Done);
    let Reply::State { state } = ask(&client, Request::State) else {
        panic!("a state");
    };
    assert_eq!(state.steps, steps + 20);
    assert_eq!(ask(&client, Request::Release { key: w() }), Reply::Done);
    let Reply::Entity {
        entity, components, ..
    } = ask(
        &client,
        Request::Entity {
            entity: "lantern".to_owned(),
        },
    )
    else {
        panic!("the lantern");
    };
    assert_eq!(entity.identity, "lantern");
    let transform = components
        .iter()
        .find(|component| component.name == "Transform")
        .expect("placed");
    assert!(
        transform.value.contains("translation = { x = 20.0"),
        "{}",
        transform.value
    );
    // Saved there, by the simulation, to a file of the test's own; moved on, then loaded: back at
    // 20 m. A file that holds no save is refused, the run left as it is.
    let saved = std::env::temp_dir().join(format!("slime-app-channel-{}.txt", std::process::id()));
    let file = || saved.to_string_lossy().into_owned();
    assert_eq!(ask(&client, Request::Save { file: file() }), Reply::Done);
    assert_eq!(ask(&client, Request::Press { key: w() }), Reply::Done);
    assert_eq!(ask(&client, Request::Step { steps: 5 }), Reply::Done);
    assert_eq!(ask(&client, Request::Release { key: w() }), Reply::Done);
    assert_eq!(x(&client), 25.0);
    assert_eq!(ask(&client, Request::Load { file: file() }), Reply::Done);
    assert_eq!(x(&client), 20.0);
    std::fs::write(&saved, "not a number").expect("written");
    assert!(matches!(
        ask(&client, Request::Load { file: file() }),
        Reply::Failed { .. }
    ));
    assert_eq!(x(&client), 20.0);
    std::fs::remove_file(&saved).expect("removed");

    // A value given as text, read with the component's type, within its range.
    let dim = |value: &str| Request::SetProperty {
        entity: "lantern".to_owned(),
        component: "Light".to_owned(),
        path: "intensity".to_owned(),
        value: value.to_owned(),
    };
    assert_eq!(ask(&client, dim("12.5")), Reply::Done);
    let Reply::Entity { components, .. } = ask(
        &client,
        Request::Entity {
            entity: "lantern".to_owned(),
        },
    ) else {
        panic!("the lantern");
    };
    let light = components
        .iter()
        .find(|component| component.name == "Light")
        .expect("a light");
    assert!(light.value.contains("intensity = 12.5"), "{}", light.value);
    // Out of its range, of another type, not a value.
    for wrong in ["-1.0", "true", "\"bright\"", "bright"] {
        assert!(
            matches!(ask(&client, dim(wrong)), Reply::Failed { .. }),
            "{wrong}"
        );
    }
    // Commands applied as their texts say, in their order, up to the first refused.
    let set = |entity: &str, intensity: f64| {
        format!(
            "[SetProperty]\nentity = \"{entity}\"\ncomponent = \"Light\"\npath = \"intensity\"\nvalue = {intensity:?}\n"
        )
    };
    let commands = vec![
        set("lantern", 20.0),
        set("nowhere", 1.0),
        set("lantern", 3.0),
    ];
    let reply = ask(&client, Request::Apply { commands });
    assert!(
        matches!(&reply, Reply::Failed { message, .. }
            if message.starts_with("command 1:") && message.contains("after it, 1, not applied")),
        "{reply:?}"
    );
    let Reply::Entity { components, .. } = ask(
        &client,
        Request::Entity {
            entity: "lantern".to_owned(),
        },
    ) else {
        panic!("the lantern");
    };
    assert!(
        components
            .iter()
            .any(|component| component.value.contains("intensity = 20.0")),
        "{components:?}"
    );
    let Reply::Entities { entities, next } = ask(
        &client,
        Request::Entities {
            filter: "LANT".to_owned(),
            cursor: None,
            limit: None,
            bytes: None,
        },
    ) else {
        panic!("entities");
    };
    assert_eq!((entities.len(), next), (1, None));
    let Reply::Logs { lines, .. } = ask(
        &client,
        Request::Logs {
            after: 0,
            limit: None,
            bytes: None,
        },
    ) else {
        panic!("logs");
    };
    assert!(
        lines.iter().any(|line| line.text.contains("W goes down")),
        "{lines:?}"
    );
    assert!(
        lines
            .windows(2)
            .all(|pair| pair[1].number == pair[0].number + 1)
    );
    assert!(matches!(
        ask(
            &client,
            Request::Capture {
                view: String::new()
            }
        ),
        Reply::Failed { .. }
    ));
    assert!(matches!(
        ask(
            &client,
            Request::Press {
                key: "W".to_owned()
            }
        ),
        Reply::Failed { .. }
    ));

    // Steps that no run would end, dropped when another tool lets game time run again: the step is
    // answered so.
    let stepping = thread::spawn(move || {
        let reply = ask(&client, Request::Step { steps: u32::MAX });
        (client, reply)
    });
    let other =
        Client::connect(launched.address, &launched.token, &tester(), &tester()).expect("accepted");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(ask(&other, Request::Resume), Reply::Done);
    let (client, dropped) = stepping.join().expect("answered");
    assert!(
        matches!(&dropped, Reply::Failed { message, .. } if message.contains("dropped")),
        "{dropped:?}"
    );

    assert_eq!(ask(&client, Request::Stop), Reply::Done);
    let status = launched.child.wait().expect("the run ends");
    assert!(status.success(), "{status}");
}

/// An editor whose gesture is under way while W is held, and whose steps, named by a digit, are
/// written after the lantern's x: 0, then 1, then 12 for the steps `1` and `2`.
#[derive(Default)]
struct Holder {
    gesture: bool,
}

impl Simulation for Holder {
    fn start(
        &mut self,
        ctx: &mut SimulationContext<'_>,
        world: &mut World,
    ) -> Result<(), AppError> {
        Lantern.start(ctx, world)
    }

    fn update(&mut self, ctx: &mut SimulationContext<'_>, _world: &mut World) {
        self.gesture = ctx.input().keys().held(KeyCode::KeyW);
        thread::sleep(Duration::from_millis(1));
    }

    fn edit(&mut self, world: &mut World, edit: &slime_channel::Edit) -> Result<Reply, AppError> {
        let slime_channel::Edit::Step { label, .. } = edit else {
            return Err("steps alone".into());
        };
        let digit: f64 = label.parse()?;
        let lantern = lantern(world);
        let mut transform = world.get_mut::<Transform>(lantern).expect("placed");
        transform.translation.x = transform.translation.x * 10.0 + digit;
        Ok(Reply::Done)
    }

    fn edits_wait(&self) -> bool {
        self.gesture
    }
}

/// A tool's edits and the values it sets wait while the simulation says so, in their order, and
/// are answered once it no longer does; what the tool reads meanwhile is answered at once
/// (`docs/adr/0161-the-open-step-and-the-handles.md`).
#[test]
fn a_tool_s_edits_wait_while_the_simulation_says_so() {
    if std::env::var_os(CHILD_ENV).is_some() {
        run(&mut Holder::default(), &mut (), &AppConfig::default()).expect("the run starts");
        return;
    }
    let mut launched = launch(
        &mut command("a_tool_s_edits_wait_while_the_simulation_says_so"),
        Duration::from_secs(60),
    )
    .expect("the run listens");
    let connect = || {
        Client::connect(launched.address, &launched.token, &tester(), &tester()).expect("accepted")
    };
    let reader = connect();
    let step = |label: &str| Request::Edit {
        edit: slime_channel::Edit::Step {
            label: label.to_owned(),
            commands: Vec::new(),
            selection: None,
        },
    };
    // Answered at once while nothing is under way.
    assert_eq!(ask(&reader, step("1")), Reply::Done);
    assert_eq!(x(&reader), 1.0);
    let w = || "KeyW".to_owned();
    assert_eq!(ask(&reader, Request::Press { key: w() }), Reply::Done);
    // Two edits, the second asked once the first waits.
    let edits: Vec<_> = ["2", "3"]
        .into_iter()
        .map(|label| {
            let client = connect();
            let request = step(label);
            let edit = thread::spawn(move || ask(&client, request));
            thread::sleep(Duration::from_millis(100));
            edit
        })
        .collect();
    // A value set waits too; a read of the history is answered at once, as the simulation answers
    // it (this one refuses all but steps).
    let client = connect();
    let dim = thread::spawn(move || {
        ask(
            &client,
            Request::SetProperty {
                entity: "lantern".to_owned(),
                component: "Light".to_owned(),
                path: "intensity".to_owned(),
                value: "12.5".to_owned(),
            },
        )
    });
    let history = Request::Edit {
        edit: slime_channel::Edit::History,
    };
    assert!(matches!(ask(&reader, history), Reply::Failed { .. }));
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        x(&reader),
        1.0,
        "nothing applied while the gesture is under way"
    );
    assert!(edits.iter().all(|edit| !edit.is_finished()) && !dim.is_finished());
    assert_eq!(ask(&reader, Request::Release { key: w() }), Reply::Done);
    for edit in edits {
        assert_eq!(edit.join().expect("answered"), Reply::Done);
    }
    assert_eq!(dim.join().expect("answered"), Reply::Done);
    assert_eq!(x(&reader), 123.0, "applied in their order");
    assert_eq!(ask(&reader, Request::Stop), Reply::Done);
    let status = launched.child.wait().expect("the run ends");
    assert!(status.success(), "{status}");
}

/// On one connection, a read is answered while an edit of the same connection waits for a drag's
/// end; an edit cancelled meanwhile is answered so at once, and one of a connection that closed is
/// dropped: neither is applied once the drag ends
/// (`docs/adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md`).
#[test]
fn a_read_passes_a_waiting_edit_and_a_cancelled_edit_is_never_applied() {
    if std::env::var_os(CHILD_ENV).is_some() {
        run(&mut Holder::default(), &mut (), &AppConfig::default()).expect("the run starts");
        return;
    }
    let mut launched = launch(
        &mut command("a_read_passes_a_waiting_edit_and_a_cancelled_edit_is_never_applied"),
        Duration::from_secs(60),
    )
    .expect("the run listens");
    let connect = || {
        Client::connect(launched.address, &launched.token, &tester(), &tester()).expect("accepted")
    };
    let client = connect();
    let step = |label: &str| Request::Edit {
        edit: slime_channel::Edit::Step {
            label: label.to_owned(),
            commands: Vec::new(),
            selection: None,
        },
    };
    let soon = Duration::from_secs(30);
    assert_eq!(ask(&client, step("1")), Reply::Done);
    assert_eq!(
        ask(
            &client,
            Request::Press {
                key: "KeyW".to_owned()
            }
        ),
        Reply::Done
    );
    let edit = client.send(&step("2")).expect("sent");
    thread::sleep(Duration::from_millis(100));
    // A read on the same connection, answered while the edit waits.
    let read = client
        .send(&Request::Entity {
            entity: "lantern".to_owned(),
        })
        .expect("sent");
    assert!(matches!(
        read.wait_for(soon).expect("answered meanwhile").0,
        Reply::Entity { .. }
    ));
    edit.cancel();
    assert!(
        matches!(edit.wait_for(soon), Err(ChannelError::Cancelled)),
        "answered while the drag goes on"
    );
    // An edit whose connection closes while it waits.
    let other = connect();
    let _dropped = other.send(&step("3")).expect("sent");
    thread::sleep(Duration::from_millis(100));
    drop(other);
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        ask(
            &client,
            Request::Release {
                key: "KeyW".to_owned()
            }
        ),
        Reply::Done
    );
    // Each answered at a frame's start: what waited would be applied by the second.
    for _ in 0..2 {
        ask(&client, Request::State);
    }
    assert_eq!(x(&client), 1.0, "neither edit applied");
    assert_eq!(ask(&client, Request::Stop), Reply::Done);
    let status = launched.child.wait().expect("the run ends");
    assert!(status.success(), "{status}");
}

/// A crowd of keyed entities, each placed, which a tool lists page after page; it times its frames
/// by the wall's clock, as a headless run gives each frame its fixed step, and saves their median
/// and the longest since the last save.
#[derive(Default)]
struct Crowd {
    last: Option<std::time::Instant>,
    frames: Vec<Duration>,
}

/// How many entities the crowd holds: past the 1 000 that a list gives, each list a costly request.
const CROWD: usize = 5_000;

impl Simulation for Crowd {
    fn start(
        &mut self,
        _ctx: &mut SimulationContext<'_>,
        world: &mut World,
    ) -> Result<(), AppError> {
        for at in 0..CROWD {
            let entity = world.spawn_keyed(&format!("crowd {at}"))?;
            world.insert(entity, Transform::IDENTITY);
        }
        Ok(())
    }

    fn update(&mut self, _ctx: &mut SimulationContext<'_>, _world: &mut World) {
        let now = std::time::Instant::now();
        if let Some(last) = self.last.replace(now) {
            self.frames.push(now - last);
        }
        // As the lantern's frames, a little rest each.
        thread::sleep(Duration::from_millis(1));
    }

    fn save(&mut self, _world: &World, file: &Path) -> Result<(), AppError> {
        let mut frames = std::mem::take(&mut self.frames);
        frames.sort();
        let median = frames.get(frames.len() / 2).copied().unwrap_or_default();
        let longest = frames.last().copied().unwrap_or_default();
        std::fs::write(
            file,
            format!(
                "{} {:.3} {:.3}",
                frames.len(),
                median.as_secs_f64() * 1e3,
                longest.as_secs_f64() * 1e3
            ),
        )?;
        Ok(())
    }
}

/// The frames of a run that agents keep at the channel's bounds: as many connections as it
/// welcomes, each with as many requests in flight as it takes, each a page of at most a thousand
/// entities.
/// It prints the frames' median and longest by the wall's clock over six seconds, after two of
/// warming; by hand, in a release build, at rest (`docs/measurements.md`, M6d.9a1b1):
/// `cargo test --release -p slime-app --features development --test channel -- --ignored frames_with_agents --nocapture`.
#[test]
#[ignore = "a measure, run by hand at rest"]
fn frames_with_agents_at_the_bounds() {
    if std::env::var_os(CHILD_ENV).is_some() {
        run(&mut Crowd::default(), &mut (), &AppConfig::default()).expect("the run starts");
        return;
    }
    // The child runs this test too, which it skips unless told that it is ignored.
    let mut child = command("frames_with_agents_at_the_bounds");
    child.arg("--ignored");
    let mut launched = launch(&mut child, Duration::from_secs(60)).expect("the run listens");
    let clients: Vec<Arc<Client>> = (0..8)
        .map(|_| {
            Arc::new(
                Client::connect(launched.address, &launched.token, &tester(), &tester())
                    .expect("accepted"),
            )
        })
        .collect();
    let stop = Arc::new(AtomicBool::new(false));
    let agents: Vec<_> = clients
        .iter()
        .skip(1)
        .map(|client| {
            let (client, stop) = (Arc::clone(client), Arc::clone(&stop));
            thread::spawn(move || {
                let list = Request::Entities {
                    filter: String::new(),
                    cursor: None,
                    limit: None,
                    bytes: None,
                };
                let mut flying = VecDeque::new();
                while !stop.load(Ordering::Relaxed) {
                    while flying.len() < 16 {
                        flying.push_back(client.send(&list).expect("sent"));
                    }
                    let oldest: Pending = flying.pop_front().expect("in flight");
                    oldest.wait().expect("listed");
                }
                for pending in flying {
                    let _ = pending.wait();
                }
            })
        })
        .collect();
    let stats = std::env::temp_dir().join(format!("slime-frames-{}.txt", std::process::id()));
    let save = || {
        ask(
            &clients[0],
            Request::Save {
                file: stats.display().to_string(),
            },
        )
    };
    thread::sleep(Duration::from_secs(2));
    assert_eq!(save(), Reply::Done);
    thread::sleep(Duration::from_secs(6));
    assert_eq!(save(), Reply::Done);
    stop.store(true, Ordering::Relaxed);
    for agent in agents {
        agent.join().expect("the agent ends");
    }
    let measured = std::fs::read_to_string(&stats).expect("the frames' times");
    let _ = std::fs::remove_file(&stats);
    let [count, median, longest]: [&str; 3] = measured
        .split(' ')
        .collect::<Vec<_>>()
        .try_into()
        .expect("three figures");
    println!(
        "frames with 7 agents at the bounds and one that measures: {count} frames, {median} ms \
         the median, {longest} ms the longest"
    );
    assert_eq!(ask(&clients[0], Request::Stop), Reply::Done);
    let status = launched.child.wait().expect("the run ends");
    assert!(status.success(), "{status}");
}

/// Connects to the run that `launched` started, telling each event that it sends through the
/// receiver given back.
fn listening(launched: &slime_channel::Launched) -> (Client, std::sync::mpsc::Receiver<GameTime>) {
    let (told, events) = std::sync::mpsc::channel();
    let told = std::sync::Mutex::new(told);
    let client = Client::connect_with(
        launched.address,
        &launched.token,
        &tester(),
        &tester(),
        move |notice| {
            if let Notice::Event(Event::Time(time)) = notice {
                let _ = told.lock().expect("not poisoned").send(time);
            }
        },
    )
    .expect("accepted");
    (client, events)
}

/// How many times each thread of the channel of the process `pid` left its wait, by its id, as
/// Linux counts them (`/proc/<pid>/task/<tid>/status`); none elsewhere. Reading them wakes none.
fn channel_switches(pid: u32) -> Vec<(String, u64)> {
    let Ok(tasks) = std::fs::read_dir(format!("/proc/{pid}/task")) else {
        return Vec::new();
    };
    let mut switches: Vec<(String, u64)> = tasks
        .flatten()
        .filter(|task| {
            std::fs::read_to_string(task.path().join("comm"))
                .is_ok_and(|name| name.starts_with("slime dev chann"))
        })
        .filter_map(|task| {
            let status = std::fs::read_to_string(task.path().join("status")).ok()?;
            let count = status
                .lines()
                .find_map(|line| line.strip_prefix("voluntary_ctxt_switches:"))?
                .trim()
                .parse()
                .ok()?;
            Some((task.file_name().to_string_lossy().into_owned(), count))
        })
        .collect();
    switches.sort();
    switches
}

/// The frame that the run that `client` drives says that it is at.
fn frame(client: &Client) -> u64 {
    let Reply::State { state } = ask(client, Request::State) else {
        panic!("a state");
    };
    state.frame
}

/// A run whose frames run on demand tells each connection subscribed to `play/time` how game time
/// stands, once it subscribed, then as a tool changes it, at the end of the frame that changed it,
/// without a frame more; a second connection is told the current value, and a run at rest runs no
/// frame for its subscriptions
/// (`docs/adr/0254-events-declared-in-the-welcome-replaced-in-the-carrier-published-at-the-frames-end.md`).
#[test]
fn a_run_on_demand_tells_its_subscribers_how_game_time_stands() {
    if std::env::var_os(CHILD_ENV).is_some() {
        let config = AppConfig {
            on_demand: true,
            ..AppConfig::default()
        };
        run(&mut Lantern, &mut (), &config).expect("the run starts");
        return;
    }
    let mut launched = launch(
        &mut command("a_run_on_demand_tells_its_subscribers_how_game_time_stands"),
        Duration::from_secs(60),
    )
    .expect("the run listens");
    let wait = Duration::from_secs(10);
    let (client, events) = listening(&launched);
    assert_eq!(client.events(), ["play/time"]);
    let at = |paused, speed| GameTime {
        paused,
        speed,
        held: false,
    };
    client.subscribe(&["play/time"]).expect("subscribed");
    assert_eq!(events.recv_timeout(wait).expect("told"), at(false, 1.0));
    assert_eq!(ask(&client, Request::Pause), Reply::Done);
    assert_eq!(events.recv_timeout(wait).expect("told"), at(true, 1.0));
    assert_eq!(ask(&client, Request::Speed { speed: 0.5 }), Reply::Done);
    assert_eq!(events.recv_timeout(wait).expect("told"), at(true, 0.5));
    // A second connection is told the current value, the first nothing.
    let (other, others) = listening(&launched);
    other.subscribe(&["play/time"]).expect("subscribed");
    assert_eq!(others.recv_timeout(wait).expect("told"), at(true, 0.5));
    // At rest, subscribed, once the frames that the subscriptions asked for ran, the run runs no
    // frame: two reads apart are a frame apart.
    thread::sleep(Duration::from_millis(300));
    let before = frame(&client);
    // Read until two reads agree: the writer of the last answer notifies once more before it
    // waits.
    let mut switches = channel_switches(launched.child.id());
    // At most a second: a thread that never settles fails the assertion below.
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(50));
        let again = channel_switches(launched.child.id());
        if again == switches {
            break;
        }
        switches = again;
    }
    if cfg!(target_os = "linux") {
        // Its listener, the watch of its input, and a reader and a writer for each of the two
        // connections.
        assert_eq!(switches.len(), 6, "{switches:?}");
    }
    thread::sleep(Duration::from_secs(1));
    assert_eq!(
        channel_switches(launched.child.id()),
        switches,
        "the channel's threads woke at rest"
    );
    assert_eq!(frame(&client), before + 1, "no frame at rest");
    assert!(
        events.try_recv().is_err(),
        "nothing told while nothing changed"
    );
    assert_eq!(ask(&client, Request::Stop), Reply::Done);
    let status = launched.child.wait().expect("the run ends");
    assert!(status.success(), "{status}");
}

/// 2,500 entities, `e0` to `e2499`, after one whose name is larger than the line that
/// a tool reads.
struct Huge;

/// The size of the huge name: past the 16 MiB of a tool's line.
const HUGE: usize = 20 << 20;

impl Simulation for Huge {
    fn start(
        &mut self,
        _ctx: &mut SimulationContext<'_>,
        world: &mut World,
    ) -> Result<(), AppError> {
        let huge = world.spawn_keyed("huge")?;
        world.insert(huge, Name("é".repeat(HUGE / 2)));
        for index in 0..2500 {
            let entity = world.spawn();
            world.insert(entity, Name(format!("e{index}")));
        }
        Ok(())
    }

    fn update(&mut self, _ctx: &mut SimulationContext<'_>, _world: &mut World) {
        // Headless frames run back to back: a little rest each keeps the test light.
        thread::sleep(Duration::from_millis(1));
    }
}

/// A list read by pages goes past an entity whose name is larger than the tool's line, cut and
/// marked, and gives every entity once (`docs/adr/0260-a-long-list-read-by-pages-a-cursor-bound-to-its-maker-pages-bounded-by-time-m6d9a1b3b-cut-in-two.md`).
#[test]
fn a_list_goes_past_an_entity_larger_than_the_tool_s_line() {
    if std::env::var_os(CHILD_ENV).is_some() {
        run(&mut Huge, &mut (), &AppConfig::default()).expect("the run starts");
        return;
    }
    let mut launched = launch(
        &mut command("a_list_goes_past_an_entity_larger_than_the_tool_s_line"),
        Duration::from_secs(60),
    )
    .expect("the run listens");
    let client =
        Client::connect(launched.address, &launched.token, &tester(), &tester()).expect("accepted");
    let (mut listed, mut cursor, mut pages) = (Vec::new(), None, 0);
    loop {
        let reply = ask(
            &client,
            Request::Entities {
                filter: String::new(),
                cursor: cursor.take(),
                limit: None,
                bytes: None,
            },
        );
        let Reply::Entities { entities, next } = reply else {
            panic!("a page, not {reply:?}");
        };
        pages += 1;
        listed.extend(entities);
        match next {
            Some(next) => cursor = Some(next),
            None => break,
        }
        assert!(pages < 1000, "a list that ends");
    }
    assert_eq!(listed.len(), 2501);
    assert_eq!(listed[0].identity, "huge");
    assert_eq!(listed[0].cut, ["name"]);
    assert!(!listed[0].name.is_empty() && listed[0].name.len() < HUGE);
    let names: Vec<String> = listed[1..]
        .iter()
        .map(|entity| entity.name.clone())
        .collect();
    let expected: Vec<String> = (0..2500).map(|index| format!("e{index}")).collect();
    assert_eq!(names, expected, "each once, in the order of the slots");
    assert!(pages >= 3, "{pages} pages of at most a thousand");
    assert_eq!(ask(&client, Request::Stop), Reply::Done);
    assert!(launched.child.wait().expect("the run ends").success());
}

#[test]
fn a_run_ends_once_the_tool_that_started_it_closes_its_input() {
    if std::env::var_os(CHILD_ENV).is_some() {
        return child();
    }
    let mut launched = launch(
        &mut command("a_run_ends_once_the_tool_that_started_it_closes_its_input"),
        Duration::from_secs(60),
    )
    .expect("the run listens");
    // The tool ends, as a crash would end it: the pipe of the game's input closes, and the run
    // stops as at a termination signal, its `stop` methods run.
    drop(launched.child.stdin.take());
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = launched.child.try_wait().expect("the run") {
            break status;
        }
        if started.elapsed() > Duration::from_secs(30) {
            let _ = launched.child.kill();
            panic!("the run went on without the tool that started it");
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "{status}");
}

#[test]
fn a_run_started_without_a_token_does_not_listen() {
    if std::env::var_os(CHILD_ENV).is_some() {
        // Ten frames, without a tool.
        let config = AppConfig {
            frame_limit: Some(10),
            ..AppConfig::default()
        };
        run(&mut Lantern, &mut (), &config).expect("the run starts");
        return;
    }
    let output = command("a_run_started_without_a_token_does_not_listen")
        .env_remove(TOKEN_ENV_VAR)
        .output()
        .expect("the run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains(ANNOUNCEMENT.trim()), "{stdout}");
}
