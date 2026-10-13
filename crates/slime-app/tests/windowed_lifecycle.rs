//! The lifecycle of a windowed run, checked in a Wayland compositor: what the application sees in
//! `init`, the order of the calls, the stop conditions, the frame schedule after a burst of
//! redraws, the time each frame represents, the profiling spans of each frame, the memory of a run
//! whose frames draw nothing, a run whose GPU cannot open, and a window larger than the GPU can
//! draw into. `scripts/check.sh` runs it in a headless Weston with lavapipe (ADR-0024). It is not
//! in the default run of `cargo test` (`test = false` in Cargo.toml), since it needs a compositor,
//! and it has no harness, since the windowing library needs the main thread.
//!
//! A process can run one windowed application, so each scenario runs in a child process: this
//! program runs itself again with `SLIME_APP_TEST_SCENARIO` set, and checks what the child prints.
//!
//! Linux only, and compiled out elsewhere (ADR-0015): the check needs a Wayland compositor, and
//! `main` does nothing on other platforms. On Windows, the windowed lifecycle is checked by hand
//! in the virtual machine, with the `window` and `scene` examples.

#[cfg(target_os = "linux")]
mod common;

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
    const SCENARIOS: [&str; 7] = [
        "calls",
        "spans",
        "init-error",
        "exit-in-init",
        "draws-nothing",
        "no-gpu",
        "oversize",
    ];

    /// Frames of the `calls` and `spans` scenarios, and those in which the application asks for an
    /// extra redraw.
    const FRAME_LIMIT: u64 = 50;
    const STORM: std::ops::Range<u64> = 10..40;

    /// How far the time a frame represents can be from the time the application measures between
    /// its updates. The two measurements are a few microseconds apart, more when the process is
    /// preempted between them; a frame whose time did not come from the clock would be off by
    /// several milliseconds during the storm, where frames come less than 10 ms apart.
    const DELTA_TOLERANCE_MICROS: u64 = 5_000;

    /// Frames of the `draws-nothing` scenario, whose memory is measured once the run has settled,
    /// after `SETTLED` frames, then at its last frame. Presenting images that nothing drew into
    /// once made a run grow by about 173 KiB per frame: 121 320 KiB between the two measurements
    /// on 2026-10-13, against none with the fix (`SurfaceFrame::present`). The test fails beyond
    /// a quarter of that growth.
    const UNDRAWN_FRAMES: u64 = 1000;
    const SETTLED: u64 = 300;
    const MAX_GROWTH_KIB: u64 = 30_000;

    /// Prints every call it receives, with the time of each frame.
    struct Recorder {
        scenario: &'static str,
        /// When `init` returned, which the times of the frames count from.
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
            self.start = Some(Instant::now());
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
            let start = self.start.expect("init returned before the first frame");
            self.last_update = frame.index();
            println!(
                "call update {} t={} delta={}",
                frame.index(),
                (now - start).as_micros(),
                frame.delta().as_micros()
            );
            if STORM.contains(&frame.index()) {
                ctx.window().expect("windowed run").request_redraw();
            }
            if self.scenario == "draws-nothing"
                && [SETTLED, UNDRAWN_FRAMES - 1].contains(&frame.index())
            {
                println!("memory {} {}", frame.index(), resident_kib());
            }
        }

        fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
            // In `draws-nothing`, the image is presented without anything drawn into it, as with
            // the default `render`.
            if self.scenario != "draws-nothing" {
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
            }
            if self.scenario == "oversize" {
                let window = ctx.window().expect("windowed run").inner_size();
                println!(
                    "image {} {} window {} {}",
                    target.width(),
                    target.height(),
                    window.width,
                    window.height
                );
            }
            println!("call render {}", self.last_update);
        }

        fn shutdown(&mut self, ctx: &mut Context) {
            println!("call shutdown exit_requested={}", ctx.exit_requested());
        }
    }

    fn config(scenario: &str) -> AppConfig {
        let mut config = AppConfig {
            window: Some(WindowConfig {
                title: "windowed lifecycle".to_owned(),
                width: 320,
                height: 240,
            }),
            frame_limit: Some(FRAME_LIMIT),
            ..AppConfig::default()
        };
        match scenario {
            "draws-nothing" => {
                // Frames as fast as they come, so that many of them take a few seconds.
                config.frame_limit = Some(UNDRAWN_FRAMES);
                config.max_frame_rate = 1000;
            }
            "oversize" => {
                // Wider than the largest image the device accepts (8192 pixels with the default
                // limits); low, so that its images stay small for lavapipe.
                config.window = Some(WindowConfig {
                    title: "windowed lifecycle".to_owned(),
                    width: 9000,
                    height: 64,
                });
                config.frame_limit = Some(10);
            }
            _ => {}
        }
        config
    }

    /// The memory the process occupies, in KiB (`VmRSS`).
    fn resident_kib() -> u64 {
        std::fs::read_to_string("/proc/self/status")
            .expect("the process's status")
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))
            .and_then(|value| value.trim().strip_suffix(" kB"))
            .and_then(|value| value.parse().ok())
            .expect("VmRSS, in kB")
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
            let mut command = Command::new(std::env::current_exe().expect("test binary path"));
            command
                .env(SCENARIO_ENV, scenario)
                .env_remove(slime_app::LOG_ENV_VAR);
            if scenario == "no-gpu" {
                // A graphics API this build does not have: no adapter at all, whatever the drivers.
                command.env("WGPU_BACKEND", "gl");
            }
            let output = command.output().expect("child test process runs");
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
                "spans" => check_spans(&lines),
                "init-error" => check_init_error(&lines),
                "exit-in-init" => check_exit_in_init(&lines),
                "draws-nothing" => check_draws_nothing(&lines),
                "no-gpu" => check_no_gpu(&lines),
                "oversize" => check_oversize(&lines),
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
        let result = if scenario == "spans" {
            // The event loop, and every frame with it, runs on this thread.
            let subscriber = crate::common::span_log(|line| println!("span {line}"));
            tracing::subscriber::with_default(subscriber, || {
                slime_app::run(&mut app, &config(scenario))
            })
        } else {
            slime_app::run(&mut app, &config(scenario))
        };
        match result {
            Ok(()) => println!("result Ok"),
            Err(RunError::Init(error)) => println!("result Init({error})"),
            Err(error) => println!("result {error:?}"),
        }
        if scenario == "calls" {
            match slime_app::run(&mut app, &config(scenario)) {
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
        let mut deltas = Vec::new();
        for (i, call) in calls.iter().enumerate() {
            match call[0] {
                "update" => {
                    assert_eq!(call[1], frames.to_string(), "frames are numbered in order");
                    frames += 1;
                    let micros = |field: &str, name: &str| -> u64 {
                        field
                            .strip_prefix(name)
                            .and_then(|value| value.parse().ok())
                            .unwrap_or_else(|| panic!("{name} in microseconds: {call:?}"))
                    };
                    times.push(micros(call[2], "t="));
                    deltas.push(micros(call[3], "delta="));
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

        // Each frame represents the time since the previous one, which the engine measures just
        // before the frame's fixed steps, and the application here at the start of `update`: the
        // two agree up to what runs in between. The first frame counts from the end of `init`, so
        // it cannot represent more than what the application measured since; counting the opening
        // of the window and the GPU as well would make it larger.
        assert!(
            deltas[0] <= times[0],
            "the first frame represents the time since init returned: {} µs, {} µs measured",
            deltas[0],
            times[0]
        );
        let max_delta = config("calls").max_frame_delta.as_micros();
        let max_delta = u64::try_from(max_delta).expect("microseconds");
        for i in 1..times.len() {
            let measured = (times[i] - times[i - 1]).min(max_delta);
            assert!(
                deltas[i].abs_diff(measured) <= DELTA_TOLERANCE_MICROS,
                "frame {i} represents the time since the previous frame: {} µs, {measured} µs \
                 measured",
                deltas[i]
            );
        }

        assert!(lines.contains(&"result Ok"), "the run succeeds: {lines:?}");
        assert!(
            lines.contains(&"second SecondEventLoop"),
            "a second windowed run is refused: {lines:?}"
        );
    }

    /// Each frame is a `frame` span containing, in order, a `fixed_update` span per fixed step, an
    /// `update` span, an `acquire` span and, when the frame is drawn, a `render` span, which
    /// contains `submit`, and a `present` span. A span that ends too early is exited before the
    /// call it should contain, and a span moved out of `frame` loses `frame/` from its path. Nothing
    /// of the application runs in `acquire`, `submit` or `present`, so for these only the place is
    /// checked.
    fn check_spans(lines: &[&str]) {
        const FIXED_STEP: [&str; 3] = [
            "span enter frame/fixed_update",
            "call fixed_update",
            "span exit frame/fixed_update",
        ];
        const UPDATE_AND_ACQUIRE: [&str; 5] = [
            "span enter frame/update",
            "call update",
            "span exit frame/update",
            "span enter frame/acquire",
            "span exit frame/acquire",
        ];
        const DRAWING: [&str; 7] = [
            "span enter frame/render",
            "span enter frame/render/submit",
            "span exit frame/render/submit",
            "call render",
            "span exit frame/render",
            "span enter frame/present",
            "span exit frame/present",
        ];
        assert!(lines.contains(&"result Ok"), "the run succeeds: {lines:?}");

        // The spans and the calls in the order they came, a call without its arguments:
        // `call update 3 t=…` becomes `call update`.
        let mut events = lines
            .iter()
            .filter(|line| line.starts_with("span ") || line.starts_with("call "))
            .map(|line| match line.match_indices(' ').nth(1) {
                Some((end, _)) if line.starts_with("call ") => &line[..end],
                _ => line,
            });
        let mut frames = 0;
        let mut drawn = 0;
        while let Some(event) = events.next() {
            if event.starts_with("call ") {
                // Between frames: init, an event, shutdown.
                continue;
            }
            assert_eq!(
                event, "span enter frame",
                "spans are entered within frames only"
            );
            let frame: Vec<&str> = events
                .by_ref()
                .take_while(|event| *event != "span exit frame")
                .collect();
            let mut rest = &frame[..];
            while let Some(after) = rest.strip_prefix(&FIXED_STEP) {
                rest = after;
            }
            let rest = rest
                .strip_prefix(&UPDATE_AND_ACQUIRE)
                .unwrap_or_else(|| panic!("frame {frames}: the update, then acquire: {frame:?}"));
            match rest {
                [] => {}
                drawing if drawing == DRAWING => drawn += 1,
                _ => panic!("frame {frames}: render with submit, then present: {frame:?}"),
            }
            frames += 1;
        }
        assert_eq!(frames, FRAME_LIMIT, "every frame is a span");
        assert!(drawn >= FRAME_LIMIT / 2, "most frames are drawn: {drawn}");
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

    fn check_draws_nothing(lines: &[&str]) {
        // wgpu's errors panic, so the run succeeds only if presenting undrawn images is valid.
        assert!(lines.contains(&"result Ok"), "the run succeeds: {lines:?}");
        let presented = calls(lines)
            .iter()
            .filter(|call| call[0] == "render")
            .count();
        assert!(
            u64::try_from(presented).expect("count") >= UNDRAWN_FRAMES / 2,
            "most frames are presented: {presented}"
        );
        let memory: Vec<u64> = lines
            .iter()
            .filter_map(|line| line.strip_prefix("memory "))
            .map(|line| {
                let kib = line.split_whitespace().nth(1).expect("memory in KiB");
                kib.parse().expect("memory in KiB")
            })
            .collect();
        let [settled, last] = memory[..] else {
            panic!("the memory is measured twice: {lines:?}");
        };
        let growth = last.saturating_sub(settled);
        assert!(
            growth < MAX_GROWTH_KIB,
            "memory stays flat when frames draw nothing: {growth} KiB more over the last {} frames",
            UNDRAWN_FRAMES - 1 - SETTLED
        );
    }

    fn check_no_gpu(lines: &[&str]) {
        // Without any graphics API, wgpu cannot even create the window's surface; the engine then
        // names the missing adapter rather than the surface.
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("result Gfx(NoAdapter(")),
            "a run that cannot open a GPU returns why: {lines:?}"
        );
        let calls = calls(lines);
        assert!(
            calls.is_empty(),
            "a run that cannot open a GPU calls neither init nor anything after: {calls:?}"
        );
    }

    fn check_oversize(lines: &[&str]) {
        // wgpu rejects a surface larger than the device's largest image, and its errors panic.
        assert!(lines.contains(&"result Ok"), "the run succeeds: {lines:?}");
        let sizes: Vec<Vec<u32>> = lines
            .iter()
            .filter_map(|line| line.strip_prefix("image "))
            .map(|line| {
                line.split_whitespace()
                    .filter_map(|word| word.parse().ok())
                    .collect()
            })
            .collect();
        assert!(!sizes.is_empty(), "frames are drawn: {lines:?}");
        for size in sizes {
            let [width, height, window_width, window_height] = size[..] else {
                panic!("an image's size and its window's: {size:?}");
            };
            assert!(
                window_width > 8192,
                "the compositor keeps the window wider than the device's largest image: {size:?}"
            );
            assert_eq!(
                (width, height),
                (8192, window_height),
                "the image is as wide as the device allows, and as high as the window"
            );
        }
    }
}
