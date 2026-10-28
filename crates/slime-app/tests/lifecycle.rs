//! Tests of the lifecycle of a game's simulation, driven by `slime_app::run` in headless mode, and
//! of the phases of a frame, game time and commands, driven by a `Host`. The windowed lifecycle,
//! with a presentation, is checked by `windowed_lifecycle.rs`, which `scripts/check.sh` runs in a
//! headless compositor (ADR-0024), and the errors of a windowed run by `windowed_errors.rs`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use slime_app::{AppConfig, AppError, Host, RunError, Simulation, SimulationContext, run};
use slime_platform::KeyCode;
use slime_reflect::Value;
use slime_world::{Command, Entity, Transform, World};

const STEP: Duration = Duration::from_millis(10);

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Start,
    FixedStep(Duration),
    Update {
        index: u64,
        delta: Duration,
        elapsed: Duration,
    },
    Stop,
}

/// Records every call it receives, and requests an exit or fails where it is told to.
#[derive(Default)]
struct Recorder {
    calls: Vec<Call>,
    exit_in_start: bool,
    fail_in_start: bool,
    exit_in_update_of_frame: Option<u64>,
    fail_in_update_of_frame: Option<u64>,
    fail_in_stop: bool,
    /// The fraction of a fixed step left to run, as each `update` reads it.
    alphas: Vec<f64>,
}

impl Simulation for Recorder {
    fn start(
        &mut self,
        ctx: &mut SimulationContext<'_>,
        _world: &mut World,
    ) -> Result<(), AppError> {
        self.calls.push(Call::Start);
        if self.fail_in_start {
            return Err("no save file".into());
        }
        if self.exit_in_start {
            ctx.request_exit();
        }
        Ok(())
    }

    fn fixed_step(&mut self, ctx: &mut SimulationContext<'_>, _world: &mut World) {
        self.calls.push(Call::FixedStep(ctx.time().game().step()));
    }

    fn update(&mut self, ctx: &mut SimulationContext<'_>, _world: &mut World) {
        let frame = *ctx.time().real();
        self.calls.push(Call::Update {
            index: frame.index(),
            delta: frame.delta(),
            elapsed: frame.elapsed(),
        });
        self.alphas.push(ctx.time().game().alpha());
        if self.exit_in_update_of_frame == Some(frame.index()) {
            ctx.request_exit();
        }
        if self.fail_in_update_of_frame == Some(frame.index()) {
            ctx.exit_with_error("the save file is corrupt");
            // Only the first error is returned.
            ctx.exit_with_error("the second error");
        }
    }

    fn stop(&mut self, ctx: &mut SimulationContext<'_>, _world: &mut World) {
        self.calls.push(Call::Stop);
        if self.fail_in_stop {
            ctx.exit_with_error("cannot write the save file");
        }
    }
}

/// Sends the engine's log to the test harness, which shows it only for a failing test, instead of
/// the real standard error, which a `SLIME_LOG=trace` left in the shell would flood. `run` keeps
/// the subscriber that exists.
fn quiet_logs() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
}

fn config(frame_limit: Option<u64>) -> AppConfig {
    quiet_logs();
    AppConfig {
        frame_limit,
        fixed_step: STEP,
        ..AppConfig::default()
    }
}

fn update(index: u64) -> Call {
    Call::Update {
        index,
        delta: STEP,
        elapsed: STEP * (index as u32 + 1),
    }
}

#[test]
fn runs_start_then_one_fixed_step_and_one_update_per_frame_then_stop() {
    let mut game = Recorder::default();
    run(&mut game, &mut (), &config(Some(3))).expect("this game starts");
    assert_eq!(
        game.calls,
        [
            Call::Start,
            Call::FixedStep(STEP),
            update(0),
            Call::FixedStep(STEP),
            update(1),
            Call::FixedStep(STEP),
            update(2),
            Call::Stop,
        ]
    );
}

#[test]
fn an_exit_request_lets_the_current_frame_finish_and_runs_no_other() {
    let mut game = Recorder {
        exit_in_update_of_frame: Some(1),
        ..Recorder::default()
    };
    run(&mut game, &mut (), &config(None)).expect("this game starts");
    assert_eq!(
        game.calls,
        [
            Call::Start,
            Call::FixedStep(STEP),
            update(0),
            Call::FixedStep(STEP),
            update(1),
            Call::Stop,
        ]
    );
}

#[test]
fn an_exit_requested_during_start_runs_no_frame() {
    let mut game = Recorder {
        exit_in_start: true,
        ..Recorder::default()
    };
    run(&mut game, &mut (), &config(None)).expect("this game starts");
    assert_eq!(game.calls, [Call::Start, Call::Stop]);
}

#[test]
fn a_start_error_stops_the_run_without_frames_or_stop() {
    let mut game = Recorder {
        fail_in_start: true,
        ..Recorder::default()
    };
    let error = run(&mut game, &mut (), &config(None)).expect_err("the start error is returned");
    assert!(
        matches!(error, RunError::Init(_)),
        "unexpected error: {error:?}"
    );
    assert_eq!(
        std::error::Error::source(&error).map(ToString::to_string),
        Some("no save file".to_owned())
    );
    assert_eq!(game.calls, [Call::Start]);
}

#[test]
fn a_frame_limit_of_zero_runs_no_frame() {
    let mut game = Recorder::default();
    run(&mut game, &mut (), &config(Some(0))).expect("this game starts");
    assert_eq!(game.calls, [Call::Start, Call::Stop]);
}

#[test]
fn headless_time_is_simulated_and_identical_between_runs() {
    let mut first = Recorder::default();
    let mut second = Recorder::default();
    run(&mut first, &mut (), &config(Some(600))).expect("this game starts");
    run(&mut second, &mut (), &config(Some(600))).expect("this game starts");

    assert_eq!(first.calls, second.calls);
    assert_eq!(first.calls.len(), 1 + 600 * 2 + 1);
    assert_eq!(first.calls[first.calls.len() - 2], update(599));
}

#[test]
fn update_reads_the_fraction_of_a_fixed_step_left_to_run() {
    let mut game = Recorder::default();
    // Frames limited to 4 ms of a 10 ms step run a step every two or three frames.
    let limited = AppConfig {
        max_frame_delta: Duration::from_millis(4),
        ..config(Some(5))
    };
    run(&mut game, &mut (), &limited).expect("this game starts");
    assert_eq!(game.alphas, [0.4, 0.8, 0.2, 0.6, 0.0]);

    let mut game = Recorder::default();
    run(&mut game, &mut (), &config(Some(3))).expect("this game starts");
    assert_eq!(game.alphas, [0.0, 0.0, 0.0]);
}

/// The run's error, and the message of its source.
fn app_error(result: Result<(), RunError>) -> String {
    let error = result.expect_err("the game's error is returned");
    assert!(
        matches!(error, RunError::App(_)),
        "unexpected error: {error:?}"
    );
    std::error::Error::source(&error)
        .expect("the game's error is the source")
        .to_string()
}

#[test]
fn an_error_in_a_frame_lets_it_finish_runs_stop_and_is_returned() {
    let mut game = Recorder {
        fail_in_update_of_frame: Some(1),
        ..Recorder::default()
    };
    let result = run(&mut game, &mut (), &config(None));
    assert_eq!(app_error(result), "the save file is corrupt");
    assert_eq!(
        game.calls,
        [
            Call::Start,
            Call::FixedStep(STEP),
            update(0),
            Call::FixedStep(STEP),
            update(1),
            Call::Stop,
        ]
    );
}

#[test]
fn an_error_in_stop_fails_a_run_that_ended_well() {
    let mut game = Recorder {
        fail_in_stop: true,
        ..Recorder::default()
    };
    let result = run(&mut game, &mut (), &config(Some(1)));
    assert_eq!(app_error(result), "cannot write the save file");
}

/// Moves a hero 1 m along x at each fixed step and a marker in each `update`; records, in each
/// `update`, where the world draws the hero at the frame's fraction of a step, and the phases as
/// they ran.
#[derive(Default)]
struct Mover {
    hero: Option<Entity>,
    marker: Option<Entity>,
    /// Where the world draws the hero in each `update`.
    drawn: Vec<f64>,
    /// The phases as they ran: `step <x>` for a fixed step, `update <x>` for an update, with the
    /// hero's x as each found it.
    phases: Vec<String>,
}

impl Mover {
    fn x(world: &World, entity: Option<Entity>, alpha: f64) -> f64 {
        world
            .interpolated_transform(entity.expect("spawned"), alpha)
            .expect("placed")
            .translation
            .x
    }
}

impl Simulation for Mover {
    fn start(
        &mut self,
        _ctx: &mut SimulationContext<'_>,
        world: &mut World,
    ) -> Result<(), AppError> {
        for (key, entity) in [("hero", &mut self.hero), ("marker", &mut self.marker)] {
            let spawned = world.spawn_keyed(key)?;
            world.insert(spawned, Transform::IDENTITY);
            *entity = Some(spawned);
        }
        Ok(())
    }

    fn fixed_step(&mut self, _ctx: &mut SimulationContext<'_>, world: &mut World) {
        let hero = self.hero.expect("spawned");
        let mut transform = world.get_mut::<Transform>(hero).expect("placed");
        self.phases
            .push(format!("step {}", transform.translation.x));
        transform.translation.x += 1.0;
    }

    fn update(&mut self, ctx: &mut SimulationContext<'_>, world: &mut World) {
        let alpha = ctx.time().game().alpha();
        self.drawn.push(Self::x(world, self.hero, alpha));
        let hero = world.get::<Transform>(self.hero.expect("spawned"));
        let x = hero.expect("placed").translation.x;
        self.phases.push(format!("update {x}"));
        // Moved outside a fixed step: drawn at once where it goes.
        let marker = self.marker.expect("spawned");
        let x = ctx.time().real().index() as f64 * 10.0;
        world
            .get_mut::<Transform>(marker)
            .expect("placed")
            .translation
            .x = x;
        assert_eq!(Self::x(world, self.marker, alpha), x);
    }
}

#[test]
fn the_world_draws_what_moved_during_a_fixed_step_between_its_two_transforms() {
    let mut game = Mover::default();
    // Frames limited to 4 ms of a 10 ms step: the fractions 0.4, 0.8, 0.2, 0.6 and 0.0, a step in
    // the third frame and in the fifth.
    let limited = AppConfig {
        max_frame_delta: Duration::from_millis(4),
        ..config(Some(5))
    };
    run(&mut game, &mut (), &limited).expect("this game starts");
    // Not moved, not moved, 0.2 of the first step, 0.6 of it, then the second step's start.
    let expected = [0.0, 0.0, 0.2, 0.6, 1.0];
    for (drawn, expected) in game.drawn.iter().zip(expected) {
        assert!((drawn - expected).abs() < 1e-9, "{:?}", game.drawn);
    }
    assert_eq!(game.drawn.len(), 5);
}

/// A host of `game`, started, its fixed steps of `STEP`.
fn host(game: &mut impl Simulation) -> Host {
    let mut host = Host::new(&config(None));
    host.start(game).expect("this game starts");
    host
}

/// The command that sets the hero's x.
fn place_hero(x: f64) -> Command {
    Command::SetProperty {
        entity: "hero".parse().expect("an identity"),
        component: "Transform".to_owned(),
        path: "translation.x".parse().expect("a path"),
        value: Value::F64(x),
    }
}

#[test]
fn a_frame_applies_the_commands_then_runs_its_fixed_steps_then_its_update() {
    let mut game = Mover::default();
    let mut host = host(&mut game);
    host.frame(&mut game, STEP * 2);
    // Sent during the frame: applied at the next one's start, before its steps.
    host.context_mut().send(place_hero(10.0));
    assert_eq!(game.phases, ["step 0", "step 1", "update 2"]);
    host.frame(&mut game, STEP);
    assert!(host.context().refused().is_empty());
    assert_eq!(game.phases[3..], ["step 10", "update 11"]);
    // The transforms computed after the update.
    let hero = game.hero.expect("spawned");
    assert_eq!(
        host.world()
            .global_transform(hero)
            .map(|at| at.translation.x),
        Some(11.0)
    );
}

#[test]
fn a_refused_command_changes_nothing_and_is_handed_back_for_one_frame() {
    let mut game = Mover::default();
    let mut host = host(&mut game);
    host.context_mut().pause();
    let Command::SetProperty {
        component,
        path,
        value,
        ..
    } = place_hero(3.0)
    else {
        unreachable!("`place_hero` sets a property");
    };
    let nobody = Command::SetProperty {
        entity: "nobody".parse().expect("an identity"),
        component,
        path,
        value,
    };
    host.context_mut().send(nobody.clone());
    host.context_mut().send(place_hero(4.0));
    host.frame(&mut game, STEP);
    let refused: Vec<&Command> = host.context().refused().iter().map(|(c, _)| c).collect();
    assert_eq!(refused, [&nobody]);
    assert_eq!(
        game.phases,
        ["update 4"],
        "the next command is applied all the same"
    );
    host.frame(&mut game, STEP);
    assert!(host.context().refused().is_empty());
}

#[test]
fn a_headless_run_on_demand_runs_a_frame_only_when_something_asks_for_one() {
    /// Woken four times, 40 ms apart, from a thread of its own, then once more, 200 ms later, to
    /// stop.
    #[derive(Default)]
    struct Woken {
        frames: u32,
        steps: u32,
        done: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl Simulation for Woken {
        fn start(
            &mut self,
            ctx: &mut SimulationContext<'_>,
            _: &mut World,
        ) -> Result<(), AppError> {
            let (waker, done) = (ctx.waker(), Arc::clone(&self.done));
            self.thread = Some(std::thread::spawn(move || {
                for _ in 0..4 {
                    std::thread::sleep(Duration::from_millis(40));
                    waker.wake();
                }
                std::thread::sleep(Duration::from_millis(200));
                done.store(true, Ordering::SeqCst);
                waker.wake();
            }));
            Ok(())
        }
        fn fixed_step(&mut self, _: &mut SimulationContext<'_>, _: &mut World) {
            self.steps += 1;
        }
        fn update(&mut self, ctx: &mut SimulationContext<'_>, _: &mut World) {
            self.frames += 1;
            if self.done.load(Ordering::SeqCst) {
                ctx.request_exit();
            }
        }
    }
    let mut woken = Woken::default();
    let config = AppConfig {
        max_frame_rate: 100,
        on_demand: true,
        ..AppConfig::default()
    };
    let started = std::time::Instant::now();
    run(&mut woken, &mut (), &config).expect("this program starts");
    woken.thread.take().expect("started").join().expect("woken");
    assert!(
        started.elapsed() >= Duration::from_millis(360),
        "{:?}",
        started.elapsed()
    );
    // The first frame, then one for each of the five wakes, 40 ms apart or more, of which two
    // might merge on a loaded machine: a run that missed its wakes would run fewer, and one paced
    // at 100 frames per second would have run 36.
    assert!((4..=6).contains(&woken.frames), "{} frames", woken.frames);
    assert_eq!(woken.steps, woken.frames, "one fixed step per frame");
}

/// A simulation that keeps the commands sent to it rather than applying them to the world, as an
/// editor makes steps of them, and refuses those that name no one.
#[derive(Default)]
struct Keeper {
    kept: Vec<Command>,
}

impl Simulation for Keeper {
    fn apply(&mut self, _: &mut World, command: &Command) -> Result<(), AppError> {
        if command.entity().to_string() == "nobody" {
            return Err("nobody is here".into());
        }
        self.kept.push(command.clone());
        Ok(())
    }
}

#[test]
fn a_simulation_applies_the_commands_sent_to_it_its_own_way() {
    let mut keeper = Keeper::default();
    let mut host = host(&mut keeper);
    let Command::SetProperty {
        component,
        path,
        value,
        ..
    } = place_hero(3.0)
    else {
        unreachable!("`place_hero` sets a property");
    };
    host.context_mut().send(place_hero(3.0));
    host.context_mut().send(Command::SetProperty {
        entity: "nobody".parse().expect("an identity"),
        component,
        path,
        value,
    });
    host.frame(&mut keeper, STEP);
    assert_eq!(keeper.kept, [place_hero(3.0)]);
    let refused: Vec<String> = host
        .context()
        .refused()
        .iter()
        .map(|(command, why)| format!("{}: {why}", command.entity()))
        .collect();
    assert_eq!(refused, ["nobody: nobody is here"]);
}

#[test]
fn at_a_quarter_of_the_speed_a_step_runs_at_every_fourth_frame_and_what_moves_is_drawn_between() {
    let mut game = Mover::default();
    let mut host = host(&mut game);
    host.context_mut().set_speed(0.25);
    for _ in 0..12 {
        host.frame(&mut game, STEP);
    }
    let phases: Vec<&str> = game
        .phases
        .iter()
        .map(|phase| phase.split(' ').next().expect("a phase"))
        .collect();
    let four = ["update", "update", "update", "step", "update"];
    assert_eq!(phases, [four, four, four].concat());
    // A quarter of a step further at each frame: the hero moves 1 m a step.
    let expected = [
        0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0,
    ];
    for (drawn, expected) in game.drawn.iter().zip(expected) {
        assert!((drawn - expected).abs() < 1e-9, "{:?}", game.drawn);
    }
    assert_eq!(host.context().time().game().steps(), 3);
}

#[test]
fn paused_no_step_runs_and_one_step_runs_exactly_one() {
    let mut game = Mover::default();
    let mut host = host(&mut game);
    host.context_mut().pause();
    for _ in 0..3 {
        host.frame(&mut game, STEP);
    }
    assert_eq!(game.phases, ["update 0"; 3], "the update runs, no step");
    host.context_mut().step();
    host.frame(&mut game, STEP);
    host.frame(&mut game, STEP);
    assert_eq!(game.phases[3..], ["step 0", "update 1", "update 1"]);
    assert_eq!(host.context().time().game().steps(), 1);
    assert_eq!(host.context().time().game().delta(), Duration::ZERO);
    host.context_mut().resume();
    host.frame(&mut game, STEP);
    assert_eq!(game.phases[6..], ["step 1", "update 2"]);
}

/// Records, at each update, whether E was put down in the frame and whether it is held.
#[derive(Default)]
struct Keys {
    seen: Vec<(bool, bool)>,
}

impl Simulation for Keys {
    fn update(&mut self, ctx: &mut SimulationContext<'_>, _world: &mut World) {
        let keys = ctx.input().keys();
        self.seen
            .push((keys.pressed(KeyCode::KeyE), keys.held(KeyCode::KeyE)));
    }
}

#[test]
fn a_key_put_down_is_pressed_in_the_next_frame_alone_and_held_until_let_go() {
    let mut game = Keys::default();
    let mut host = host(&mut game);
    host.input_mut().keys_mut().press(KeyCode::KeyE);
    host.frame(&mut game, STEP);
    host.frame(&mut game, STEP);
    host.input_mut().keys_mut().release(KeyCode::KeyE);
    host.frame(&mut game, STEP);
    assert_eq!(game.seen, [(true, true), (false, true), (false, false)]);
}

#[test]
fn at_60_hz_and_a_quarter_of_the_speed_what_moves_is_drawn_a_quarter_of_a_step_further_each_frame()
{
    let mut game = Mover::default();
    let config = AppConfig::default();
    let mut host = Host::new(&config);
    host.start(&mut game).expect("this game starts");
    host.context_mut().set_speed(0.25);
    for _ in 0..8 {
        host.frame(&mut game, config.fixed_step);
    }
    // A step at the fourth frame and the eighth, but for the nanosecond that rounding adds.
    let expected = [0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0];
    for (drawn, expected) in game.drawn.iter().zip(expected) {
        assert!((drawn - expected).abs() < 1e-6, "{:?}", game.drawn);
    }
}

#[test]
fn the_commands_sent_after_the_last_frame_are_applied_before_the_simulation_stops() {
    let mut game = Mover::default();
    let mut host = host(&mut game);
    host.frame(&mut game, STEP);
    host.context_mut().send(place_hero(7.0));
    host.stop(&mut game);
    let hero = game.hero.expect("spawned");
    let x = host
        .world()
        .get::<Transform>(hero)
        .map(|at| at.translation.x);
    assert_eq!(x, Some(7.0));
}
