//! The development channel of a headless run (`docs/adr/0122-the-development-channel.md`): a run
//! started with a token listens and says where; a tool that gives the token pauses, steps and stops
//! it, reads and changes its world, also by the texts of commands, presses its keys, reads its log,
//! and saves and loads its run; one without the token is refused; a tool's edits wait while the
//! simulation says so; a run started without a token says nothing and does not listen.
//!
//! A run reads its token from its environment, which only a process's start sets, so each test runs
//! this test program again as a child, restricted to the run, its environment set.

use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use slime_app::{AppConfig, AppError, Simulation, SimulationContext, run};
use slime_channel::{ANNOUNCEMENT, Client, Reply, Request, TOKEN_ENV_VAR, Token, launch};
use slime_color::LinearColor;
use slime_platform::KeyCode;
use slime_world::{Light, Transform, World};

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
fn ask(client: &mut Client, request: Request) -> Reply {
    client.request(&request).expect("answered").0
}

/// Where the lantern stands along x, as the run that `client` drives answers.
fn x(client: &mut Client) -> f64 {
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
        Client::connect(launched.address, &other),
        Err(slime_channel::ChannelError::Refused(_))
    ));
    let mut client = Client::connect(launched.address, &launched.token).expect("accepted");

    assert_eq!(ask(&mut client, Request::Pause), Reply::Done);
    let Reply::State { state } = ask(&mut client, Request::State) else {
        panic!("a state");
    };
    assert!(state.paused);
    assert_eq!(state.entities, 1);
    let steps = state.steps;
    // W held, twenty steps, more than a frame runs: the step is answered once they have all run,
    // and the lantern moved 20 m.
    let w = || "KeyW".to_owned();
    assert_eq!(ask(&mut client, Request::Press { key: w() }), Reply::Done);
    assert_eq!(ask(&mut client, Request::Step { steps: 20 }), Reply::Done);
    let Reply::State { state } = ask(&mut client, Request::State) else {
        panic!("a state");
    };
    assert_eq!(state.steps, steps + 20);
    assert_eq!(ask(&mut client, Request::Release { key: w() }), Reply::Done);
    let Reply::Entity {
        entity, components, ..
    } = ask(
        &mut client,
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
    assert_eq!(
        ask(&mut client, Request::Save { file: file() }),
        Reply::Done
    );
    assert_eq!(ask(&mut client, Request::Press { key: w() }), Reply::Done);
    assert_eq!(ask(&mut client, Request::Step { steps: 5 }), Reply::Done);
    assert_eq!(ask(&mut client, Request::Release { key: w() }), Reply::Done);
    assert_eq!(x(&mut client), 25.0);
    assert_eq!(
        ask(&mut client, Request::Load { file: file() }),
        Reply::Done
    );
    assert_eq!(x(&mut client), 20.0);
    std::fs::write(&saved, "not a number").expect("written");
    assert!(matches!(
        ask(&mut client, Request::Load { file: file() }),
        Reply::Failed { .. }
    ));
    assert_eq!(x(&mut client), 20.0);
    std::fs::remove_file(&saved).expect("removed");

    // A value given as text, read with the component's type, within its range.
    let dim = |value: &str| Request::SetProperty {
        entity: "lantern".to_owned(),
        component: "Light".to_owned(),
        path: "intensity".to_owned(),
        value: value.to_owned(),
    };
    assert_eq!(ask(&mut client, dim("12.5")), Reply::Done);
    let Reply::Entity { components, .. } = ask(
        &mut client,
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
            matches!(ask(&mut client, dim(wrong)), Reply::Failed { .. }),
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
    let reply = ask(&mut client, Request::Apply { commands });
    assert!(
        matches!(&reply, Reply::Failed { message }
            if message.starts_with("command 1:") && message.contains("after it, 1, not applied")),
        "{reply:?}"
    );
    let Reply::Entity { components, .. } = ask(
        &mut client,
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
    let Reply::Entities { entities, more } = ask(
        &mut client,
        Request::Entities {
            filter: "LANT".to_owned(),
        },
    ) else {
        panic!("entities");
    };
    assert_eq!((entities.len(), more), (1, 0));
    let Reply::Logs { lines } = ask(&mut client, Request::Logs { after: 0 }) else {
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
            &mut client,
            Request::Capture {
                view: String::new()
            }
        ),
        Reply::Failed { .. }
    ));
    assert!(matches!(
        ask(
            &mut client,
            Request::Press {
                key: "W".to_owned()
            }
        ),
        Reply::Failed { .. }
    ));

    // Steps that no run would end, dropped when another tool lets game time run again: the step is
    // answered so.
    let stepping = thread::spawn(move || {
        let reply = ask(&mut client, Request::Step { steps: u32::MAX });
        (client, reply)
    });
    let mut other = Client::connect(launched.address, &launched.token).expect("accepted");
    thread::sleep(Duration::from_millis(50));
    assert_eq!(ask(&mut other, Request::Resume), Reply::Done);
    let (mut client, dropped) = stepping.join().expect("answered");
    assert!(
        matches!(&dropped, Reply::Failed { message } if message.contains("dropped")),
        "{dropped:?}"
    );

    assert_eq!(ask(&mut client, Request::Stop), Reply::Done);
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
    let connect = || Client::connect(launched.address, &launched.token).expect("accepted");
    let mut reader = connect();
    let step = |label: &str| Request::Edit {
        edit: slime_channel::Edit::Step {
            label: label.to_owned(),
            commands: Vec::new(),
            selection: None,
        },
    };
    // Answered at once while nothing is under way.
    assert_eq!(ask(&mut reader, step("1")), Reply::Done);
    assert_eq!(x(&mut reader), 1.0);
    let w = || "KeyW".to_owned();
    assert_eq!(ask(&mut reader, Request::Press { key: w() }), Reply::Done);
    // Two edits, the second asked once the first waits.
    let edits: Vec<_> = ["2", "3"]
        .into_iter()
        .map(|label| {
            let mut client = connect();
            let request = step(label);
            let edit = thread::spawn(move || ask(&mut client, request));
            thread::sleep(Duration::from_millis(100));
            edit
        })
        .collect();
    // A value set waits too; a read of the history is answered at once, as the simulation answers
    // it (this one refuses all but steps).
    let mut client = connect();
    let dim = thread::spawn(move || {
        ask(
            &mut client,
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
    assert!(matches!(ask(&mut reader, history), Reply::Failed { .. }));
    thread::sleep(Duration::from_millis(100));
    assert_eq!(
        x(&mut reader),
        1.0,
        "nothing applied while the gesture is under way"
    );
    assert!(edits.iter().all(|edit| !edit.is_finished()) && !dim.is_finished());
    assert_eq!(ask(&mut reader, Request::Release { key: w() }), Reply::Done);
    for edit in edits {
        assert_eq!(edit.join().expect("answered"), Reply::Done);
    }
    assert_eq!(dim.join().expect("answered"), Reply::Done);
    assert_eq!(x(&mut reader), 123.0, "applied in their order");
    assert_eq!(ask(&mut reader, Request::Stop), Reply::Done);
    let status = launched.child.wait().expect("the run ends");
    assert!(status.success(), "{status}");
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
