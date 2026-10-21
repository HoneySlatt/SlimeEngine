//! The development channel of a headless run (`docs/adr/0122-the-development-channel.md`): a run
//! started with a token listens and says where; a tool that gives the token pauses, steps and
//! stops it, reads and changes its world, presses its keys and reads its log; one without the token
//! is refused; a run started without a token says nothing and does not listen.
//!
//! A run reads its token from its environment, which only a process's start sets, so each test runs
//! this test program again as a child, restricted to the run, its environment set.

use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use slime_app::{AppConfig, AppError, Simulation, SimulationContext, run};
use slime_channel::{ANNOUNCEMENT, Client, Reply, Request, TOKEN_ENV_VAR, Token, launch};
use slime_color::LinearColor;
use slime_platform::KeyCode;
use slime_world::{Light, Transform, World};

const CHILD_ENV: &str = "SLIME_APP_TEST_CHILD";

/// A lantern, which moves 1 m along x at each fixed step while W is held, and logs each key put
/// down.
struct Lantern;

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
            let lantern = world
                .find(&"lantern".parse().expect("a key"))
                .expect("spawned");
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
        ask(&mut client, Request::Capture),
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
