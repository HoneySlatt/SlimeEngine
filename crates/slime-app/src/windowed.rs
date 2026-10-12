//! The windowed driver: frames run from the callbacks of the platform's event loop.

use std::time::{Duration, Instant};

use slime_platform::{Event, EventHandler, LoopControl, PlatformError, Window, WindowConfig};

use crate::app::{App, AppConfig, Context, ExitReason, Frames};

pub(crate) fn run_windowed(
    app: &mut impl App,
    config: &AppConfig,
    window: &WindowConfig,
) -> Result<(), PlatformError> {
    assert!(
        config.max_frame_rate > 0,
        "the maximum frame rate must not be zero"
    );
    let now = Instant::now();
    let mut driver = WindowedDriver {
        app,
        ctx: Context::default(),
        frames: Frames::new(config),
        frame_period: Duration::from_secs(1) / config.max_frame_rate,
        last_frame: now,
        next_frame: now,
        stop: None,
    };
    tracing::debug!(?config, "running with a window");
    slime_platform::run_event_loop(window, &mut driver)
}

struct WindowedDriver<'a, A> {
    app: &'a mut A,
    ctx: Context,
    frames: Frames,
    /// The interval at which frames are scheduled, from `AppConfig::max_frame_rate`.
    frame_period: Duration,
    last_frame: Instant,
    /// When the next frame is due.
    next_frame: Instant,
    /// Why the loop is ending, once it has been decided.
    stop: Option<&'static str>,
}

impl<A: App> EventHandler for WindowedDriver<'_, A> {
    fn window_created(&mut self, window: &Window) {
        self.ctx.set_window(Some(window.clone()));
        self.app.init(&mut self.ctx);
        let now = Instant::now();
        self.last_frame = now;
        self.next_frame = now;
    }

    fn event(&mut self, event: Event) {
        tracing::trace!(?event, "platform event");
        self.app.on_event(&mut self.ctx, &event);
        if event == Event::CloseRequested {
            self.ctx.exit_because(ExitReason::WindowClosed);
        }
    }

    fn about_to_wait(&mut self) -> LoopControl {
        if self.stop.is_none() && self.frames.check_stop(&mut self.ctx).is_none() {
            let now = Instant::now();
            if now >= self.next_frame {
                self.frames
                    .run_one(self.app, &mut self.ctx, now - self.last_frame);
                self.last_frame = now;
                self.next_frame = next_due(self.next_frame, now, self.frame_period);
            }
        }
        if self.stop.is_none() {
            self.stop = self.frames.check_stop(&mut self.ctx);
        }
        match self.stop {
            Some(_) => LoopControl::Exit,
            None => LoopControl::WaitUntil(self.next_frame),
        }
    }

    fn exiting(&mut self) {
        let reason = self.stop.unwrap_or("the event loop ended");
        tracing::info!(frames = self.frames.count(), "stopping: {reason}");
        self.app.shutdown(&mut self.ctx);
        self.ctx.set_window(None);
    }
}

/// When the frame after one that was due at `due` and started at `started` is due.
///
/// Frames stay on a regular schedule, so that a frame starting a little late does not lower the
/// average rate. After falling a whole period behind, the schedule restarts from the frame that
/// just started: catching up would run frames back to back.
fn next_due(due: Instant, started: Instant, period: Duration) -> Instant {
    let next = due + period;
    if next <= started {
        started + period
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERIOD: Duration = Duration::from_millis(16);

    #[test]
    fn a_frame_on_time_schedules_the_next_one_period_later() {
        let due = Instant::now();
        assert_eq!(next_due(due, due, PERIOD), due + PERIOD);
    }

    #[test]
    fn a_slightly_late_frame_keeps_the_schedule() {
        let due = Instant::now();
        let started = due + Duration::from_millis(5);
        assert_eq!(next_due(due, started, PERIOD), due + PERIOD);
    }

    #[test]
    fn a_frame_late_by_a_period_or_more_restarts_the_schedule() {
        let due = Instant::now();
        for late in [PERIOD, PERIOD * 3 + Duration::from_millis(2)] {
            let started = due + late;
            assert_eq!(next_due(due, started, PERIOD), started + PERIOD);
        }
    }
}
