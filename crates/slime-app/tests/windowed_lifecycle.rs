//! The lifecycle of a windowed run, checked in a Wayland compositor: what the application sees in
//! `init`, the order of the calls, the stop conditions, and the frame schedule after a burst of
//! redraws. `scripts/check.sh` runs it in a headless Weston with lavapipe (ADR-0024). It is not in
//! the default run of `cargo test` (`test = false` in Cargo.toml), since it needs a compositor, and
//! it has no harness, since the windowing library needs the main thread.
//!
//! A process can run one windowed application, so each scenario runs in a child process: this
//! program runs itself again with `SLIME_APP_TEST_SCENARIO` set, and checks what the child prints.
//!
//! Linux only, and compiled out elsewhere (ADR-0015): the check needs a Wayland compositor, and
//! `main` does nothing on other platforms. On Windows, the windowed lifecycle is checked by hand
//! in the virtual machine, with the `window` and `scene` examples.

fn main() {
    #[cfg(target_os = "linux")]
    linux::main();
}

#[cfg(target_os = "linux")]
mod linux {
    use std::process::Command;
    use std::time::Instant;

    use slime_app::time::FrameTime;
    use slime_app::{App, AppConfig, AppError, Context, RunError};
    use slime_gfx::{Color, RenderPassDescriptor, RenderTarget};
    use slime_platform::{Event, PlatformError, WindowConfig};

    const SCENARIO_ENV: &str = "SLIME_APP_TEST_SCENARIO";
    const SCENARIOS: [&str; 3] = ["calls", "init-error", "exit-in-init"];

    /// Frames of the `calls` scenario, and those in which the application asks for an extra redraw.
    const FRAME_LIMIT: u64 = 50;
    const STORM: std::ops::Range<u64> = 10..40;

    /// Prints every call it receives, with the time of each frame.
    struct Recorder {
        scenario: &'static str,
        start: Option<Instant>,
        last_update: u64,
    }

    impl App for Recorder {
        fn init(&mut self, ctx: &mut Context) -> Result<(), AppError> {
            println!(
                "call init window={} gpu={} format={:?}",
                ctx.window().is_some(),
                ctx.gpu().is_some(),
                ctx.surface_format()
            );
            match self.scenario {
                "init-error" => return Err("no save file".into()),
                "exit-in-init" => ctx.request_exit(),
                _ => {}
            }
            Ok(())
        }

        fn on_event(&mut self, _ctx: &mut Context, event: &Event) {
            let name = format!("{event:?}");
            let name = name
                .split(|c: char| !c.is_alphanumeric())
                .next()
                .unwrap_or("");
            println!("call event {name}");
        }

        fn fixed_update(&mut self, _ctx: &mut Context, _step: std::time::Duration) {
            println!("call fixed_update");
        }

        fn update(&mut self, ctx: &mut Context, frame: &FrameTime) {
            let now = Instant::now();
            let start = *self.start.get_or_insert(now);
            self.last_update = frame.index();
            println!(
                "call update {} t={}",
                frame.index(),
                (now - start).as_micros()
            );
            if STORM.contains(&frame.index()) {
                ctx.window().expect("windowed run").request_redraw();
            }
        }

        fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
            let grey = Color {
                r: 0.2,
                g: 0.2,
                b: 0.2,
                a: 1.0,
            };
            let pass = RenderPassDescriptor {
                target,
                clear: Some(grey),
                depth: None,
                timer: None,
            };
            ctx.gpu().expect("windowed run").render(&pass, |_pass| {});
            println!("call render {}", self.last_update);
        }

        fn shutdown(&mut self, ctx: &mut Context) {
            println!("call shutdown exit_requested={}", ctx.exit_requested());
        }
    }

    fn config() -> AppConfig {
        AppConfig {
            window: Some(WindowConfig {
                title: "windowed lifecycle".to_owned(),
                width: 320,
                height: 240,
            }),
            frame_limit: Some(FRAME_LIMIT),
            ..AppConfig::default()
        }
    }

    pub fn main() {
        if let Some(scenario) = std::env::var_os(SCENARIO_ENV) {
            let scenario = scenario.to_str().expect("scenario name");
            let scenario = SCENARIOS
                .into_iter()
                .find(|name| *name == scenario)
                .expect("known scenario");
            return child(scenario);
        }

        for scenario in SCENARIOS {
            let output = Command::new(std::env::current_exe().expect("test binary path"))
                .env(SCENARIO_ENV, scenario)
                .env_remove(slime_app::LOG_ENV_VAR)
                .output()
                .expect("child test process runs");
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                output.status.success(),
                "scenario {scenario}: the child failed: {}\n{stdout}\n{stderr}",
                output.status
            );
            let lines: Vec<&str> = stdout.lines().collect();
            match scenario {
                "calls" => check_calls(&lines),
                "init-error" => check_init_error(&lines),
                "exit-in-init" => check_exit_in_init(&lines),
                _ => unreachable!(),
            }
            println!("scenario {scenario} ... ok");
        }
        println!("test windowed_lifecycle ... ok");
    }

    /// Runs one scenario and prints its result; for `calls`, also tries a second windowed run.
    fn child(scenario: &'static str) {
        let mut app = Recorder {
            scenario,
            start: None,
            last_update: 0,
        };
        match slime_app::run(&mut app, &config()) {
            Ok(()) => println!("result Ok"),
            Err(RunError::Init(error)) => println!("result Init({error})"),
            Err(error) => println!("result {error:?}"),
        }
        if scenario == "calls" {
            match slime_app::run(&mut app, &config()) {
                Err(RunError::Platform(PlatformError::SecondEventLoop)) => {
                    println!("second SecondEventLoop");
                }
                other => println!("second {other:?}"),
            }
        }
    }

    fn calls<'a>(lines: &[&'a str]) -> Vec<Vec<&'a str>> {
        lines
            .iter()
            .filter(|line| line.starts_with("call "))
            .map(|line| line.split_whitespace().skip(1).collect())
            .collect()
    }

    fn check_calls(lines: &[&str]) {
        let calls = calls(lines);
        let first = calls.first().expect("at least one call");
        assert_eq!(
            first[..3],
            ["init", "window=true", "gpu=true"],
            "init sees the window and the GPU: {first:?}"
        );
        assert!(
            first[3].starts_with("format=Some("),
            "init sees the surface format: {first:?}"
        );
        assert_eq!(
            calls.last().map(|call| call[..2].to_vec()),
            Some(vec!["shutdown", "exit_requested=false"]),
            "shutdown comes last, with no exit requested"
        );
        assert_eq!(
            calls.iter().filter(|call| call[0] == "init").count(),
            1,
            "init once"
        );
        assert_eq!(
            calls.iter().filter(|call| call[0] == "shutdown").count(),
            1,
            "shutdown once"
        );

        // Within a frame: fixed steps, then the update, then the render of the same frame, with no
        // event in between. Frames are counted by their updates.
        let mut frames = 0;
        let mut times = Vec::new();
        for (i, call) in calls.iter().enumerate() {
            match call[0] {
                "update" => {
                    assert_eq!(call[1], frames.to_string(), "frames are numbered in order");
                    frames += 1;
                    let micros: u64 = call[2]
                        .strip_prefix("t=")
                        .and_then(|t| t.parse().ok())
                        .expect("update time");
                    times.push(micros);
                    let next = calls.get(i + 1).map(|next| next[0]);
                    assert!(
                        matches!(
                            next,
                            Some("render" | "update" | "fixed_update" | "shutdown")
                        ),
                        "nothing but the next frame or the render follows an update: {next:?}"
                    );
                }
                "render" => {
                    let previous = &calls[i - 1];
                    assert_eq!(
                        (previous[0], previous[1]),
                        ("update", call[1]),
                        "render follows the update of the same frame: {previous:?}"
                    );
                }
                _ => {}
            }
        }
        assert_eq!(frames, FRAME_LIMIT, "every frame runs an update");
        let rendered = calls.iter().filter(|call| call[0] == "render").count();
        assert!(
            u64::try_from(rendered).expect("count") >= FRAME_LIMIT / 2,
            "most frames are drawn: {rendered}"
        );
        assert!(
            calls.iter().any(|call| call[0] == "event"),
            "the window reports events"
        );

        // The schedule paces the frames before the storm, the storm runs frames back to back, and the
        // first scheduled frame after the storm comes within one period, not after the storm's
        // worth of periods.
        let interval = |i: usize| (times[i] - times[i - 1]) / 1000;
        let storm_start = usize::try_from(STORM.start).expect("index");
        let storm_end = usize::try_from(STORM.end).expect("index");
        let paced = (2..storm_start).map(interval).sum::<u64>()
            / u64::try_from(storm_start - 2).expect("count");
        assert!(
            (8..40).contains(&paced),
            "frames before the storm are paced by the schedule: {paced} ms on average"
        );
        let stormy = (storm_start + 1..=storm_end).map(interval).sum::<u64>()
            / u64::try_from(storm_end - storm_start).expect("count");
        assert!(
            stormy < 10,
            "requested redraws run frames back to back: {stormy} ms on average"
        );
        let after = interval(storm_end + 1);
        assert!(
            after < 100,
            "the first scheduled frame after the storm comes within one period: {after} ms"
        );

        assert!(lines.contains(&"result Ok"), "the run succeeds: {lines:?}");
        assert!(
            lines.contains(&"second SecondEventLoop"),
            "a second windowed run is refused: {lines:?}"
        );
    }

    fn check_init_error(lines: &[&str]) {
        let calls = calls(lines);
        assert_eq!(calls.len(), 1, "init only: {calls:?}");
        assert_eq!(calls[0][0], "init");
        assert!(
            lines.contains(&"result Init(no save file)"),
            "the init error is returned: {lines:?}"
        );
    }

    fn check_exit_in_init(lines: &[&str]) {
        let calls = calls(lines);
        // The window's first events (its size) can still arrive before the loop stops.
        let names: Vec<&str> = calls
            .iter()
            .map(|call| call[0])
            .filter(|name| *name != "event")
            .collect();
        assert_eq!(
            names,
            ["init", "shutdown"],
            "an exit requested in init runs no frame: {calls:?}"
        );
        assert_eq!(calls.last().expect("shutdown")[1], "exit_requested=true");
        assert!(lines.contains(&"result Ok"), "{lines:?}");
    }
}
