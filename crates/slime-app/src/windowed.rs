//! The windowed driver: frames run from the callbacks of the platform's event loop, each time the
//! windowing system lets the window draw.

use std::time::{Duration, Instant};

use slime_gfx::{Gpu, PresentMode, Surface};
use slime_platform::{Event, EventHandler, LoopControl, Window, WindowConfig};

use crate::app::{App, AppConfig, Context, ExitReason, Frames};
use crate::error::RunError;

/// How many devices the run opens to replace lost ones within `RESTORE_WINDOW`: at the next loss
/// within it, it gives up, since a device lost again at once, or soon after each new one, will not
/// stay.
const MAX_RESTORES: usize = 3;
/// The time within which the run opens at most `MAX_RESTORES` devices.
const RESTORE_WINDOW: Duration = Duration::from_secs(5 * 60);

pub(crate) fn run_windowed(
    app: &mut impl App,
    config: &AppConfig,
    window: &WindowConfig,
) -> Result<(), RunError> {
    assert!(
        config.max_frame_rate > 0,
        "the maximum frame rate must not be zero"
    );
    let now = Instant::now();
    let mut driver = WindowedDriver {
        app,
        ctx: Context::default(),
        frames: Frames::new(config),
        surface: None,
        present_mode: config.present_mode,
        restore_lost_device: config.restore_lost_device,
        restores: Vec::new(),
        frame_period: Duration::from_secs(1) / config.max_frame_rate,
        last_frame: now,
        next_frame: now,
        started: false,
        stop: None,
        error: None,
    };
    tracing::debug!(?config, "running with a window");
    slime_platform::run_event_loop(window, &mut driver)?;
    driver.error.map_or(Ok(()), Err)
}

struct WindowedDriver<'a, A> {
    app: &'a mut A,
    ctx: Context,
    frames: Frames,
    /// The window's surface; the GPU that draws into it is in `ctx`.
    surface: Option<Surface>,
    present_mode: PresentMode,
    /// Whether a lost device is replaced (`AppConfig::restore_lost_device`); cleared when the run
    /// gives up replacing it.
    restore_lost_device: bool,
    /// When the run opened the devices that replaced lost ones, within `RESTORE_WINDOW`.
    restores: Vec<Instant>,
    /// The interval at which frames are scheduled, from `AppConfig::max_frame_rate`.
    frame_period: Duration,
    last_frame: Instant,
    /// When the next frame is due.
    next_frame: Instant,
    /// Whether `App::init` succeeded, which makes `App::shutdown` due at the end.
    started: bool,
    /// Why the loop is ending, once it has been decided.
    stop: Option<&'static str>,
    /// Why the run failed, if it did.
    error: Option<RunError>,
}

impl<A: App> WindowedDriver<'_, A> {
    /// Opens the GPU for the window, then initializes the application.
    fn start(&mut self, window: &Window) -> Result<(), RunError> {
        let size = window.inner_size();
        let (gpu, surface) =
            Gpu::new_for_window(window.clone(), size.width, size.height, self.present_mode)?;
        self.ctx.set_gpu(Some(gpu), Some(surface.format()));
        self.surface = Some(surface);
        self.app.init(&mut self.ctx).map_err(RunError::Init)
    }

    /// When the GPU device is lost and the application chose to replace it, lets the application
    /// drop its objects of the lost device (`App::gpu_lost`), releases the run's own, opens a new
    /// device for the window and lets the application create its objects again
    /// (`App::gpu_restored`), between frames. Does nothing when the run is stopping anyway, or the
    /// window's surface was lost for good. Gives up, leaving the loss to stop the run, once
    /// `MAX_RESTORES` devices were opened within `RESTORE_WINDOW`; and when no device opens, stops
    /// the run with that error.
    fn restore(&mut self) {
        if !self.restore_lost_device || self.stop.is_some() || self.ctx.exit_requested() {
            return;
        }
        let Some(Err(loss)) = self.ctx.gpu().map(Gpu::check) else {
            return;
        };
        if !self
            .surface
            .as_ref()
            .is_some_and(|surface| surface.check().is_ok())
        {
            return;
        }
        let now = Instant::now();
        self.restores
            .retain(|&opened| now.duration_since(opened) < RESTORE_WINDOW);
        if self.restores.len() >= MAX_RESTORES {
            tracing::error!(
                %loss,
                "the GPU device was lost again after {MAX_RESTORES} new ones within {} minutes: \
                 no new one replaces it",
                RESTORE_WINDOW.as_secs() / 60
            );
            self.restore_lost_device = false;
            return;
        }
        // Nothing may hold the lost device when the new one opens: Direct3D 12 would not open it on
        // the same adapter (ADR-0075). The application drops its objects, then the run its own.
        self.app.gpu_lost(&mut self.ctx);
        let lost = self.ctx.take_gpu().expect("the device that was found lost");
        let surface = self.surface.take().expect("checked above");
        match surface.restore(lost) {
            Ok((gpu, surface)) => {
                self.restores.push(now);
                tracing::warn!(%loss, "the GPU device was lost; a new one replaces it");
                self.ctx.set_gpu(Some(gpu), Some(surface.format()));
                self.surface = Some(surface);
                // A loss of the new device that it returns is handled as any other
                // (`Context::exit_with_error`).
                if let Err(error) = self.app.gpu_restored(&mut self.ctx) {
                    self.ctx.exit_with_error(error);
                }
            }
            Err(error) => {
                // The run has neither device nor surface left: it stops, with this error.
                tracing::error!(%error, "no new GPU device replaces the lost one");
                self.error.get_or_insert(RunError::Gfx(error));
                self.restore_lost_device = false;
                self.ctx.exit_because(ExitReason::DeviceLost);
            }
        }
    }
}

impl<A: App> EventHandler for WindowedDriver<'_, A> {
    fn window_created(&mut self, window: &Window) {
        self.ctx.set_window(Some(window.clone()));
        match self.start(window) {
            Ok(()) => {
                self.started = true;
                let now = Instant::now();
                self.last_frame = now;
                self.next_frame = now;
            }
            Err(error) => {
                self.error = Some(error);
                self.stop = Some("the run failed to start");
            }
        }
    }

    fn event(&mut self, event: Event) {
        if !self.started {
            return;
        }
        tracing::trace!(?event, "platform event");
        if let (Event::Resized(size), Some(surface)) = (&event, &mut self.surface) {
            surface.resize(size.width, size.height);
        }
        self.app.on_event(&mut self.ctx, &event);
        if event == Event::CloseRequested {
            self.ctx.exit_because(ExitReason::WindowClosed);
        }
    }

    fn redraw_requested(&mut self) {
        if self.started {
            self.restore();
        }
        if !self.started
            || self.stop.is_some()
            || self
                .frames
                .check_stop(
                    &mut self.ctx,
                    self.surface.as_ref(),
                    self.restore_lost_device,
                )
                .is_some()
        {
            return;
        }
        let now = Instant::now();
        self.frames.run_one(
            self.app,
            &mut self.ctx,
            now - self.last_frame,
            self.surface.as_mut(),
        );
        self.last_frame = now;
        self.next_frame = next_due(self.next_frame, now, self.frame_period);
    }

    fn about_to_wait(&mut self) -> LoopControl {
        if self.started {
            self.restore();
        }
        if self.stop.is_none() {
            self.stop = self.frames.check_stop(
                &mut self.ctx,
                self.surface.as_ref(),
                self.restore_lost_device,
            );
        }
        if self.stop.is_some() {
            return LoopControl::Exit;
        }
        let now = Instant::now();
        if now < self.next_frame {
            return LoopControl::WaitUntil(self.next_frame);
        }
        // The frame runs when the redraw arrives, which also wakes the loop. The deadline only
        // bounds the wait if it does not come, so that signals are still noticed.
        if let Some(window) = self.ctx.window() {
            window.request_redraw();
        }
        LoopControl::WaitUntil(now + self.frame_period)
    }

    fn exiting(&mut self) {
        if self.started {
            let reason = self.stop.unwrap_or("the event loop ended");
            tracing::info!(frames = self.frames.count(), "stopping: {reason}");
            self.app.shutdown(&mut self.ctx);
        }
        // The application's error comes first, before the error of a restore that failed; not
        // before the error of `init`, which `run` returns rather than any later one.
        if let Some(error) = self.ctx.take_error()
            && !matches!(self.error, Some(RunError::Init(_)))
        {
            self.error = Some(RunError::App(error));
        }
        if let Some(Err(error)) = self.ctx.gpu().map(Gpu::check) {
            self.error.get_or_insert(RunError::Gfx(error));
        }
        if let Some(Err(error)) = self.surface.as_ref().map(Surface::check) {
            self.error.get_or_insert(RunError::Gfx(error));
        }
        // The surface keeps the window alive: release it, the GPU and the window while the
        // connection to the windowing system is still open.
        self.surface = None;
        self.ctx.set_gpu(None, None);
        self.ctx.set_window(None);
    }
}

/// When the frame after one that was due at `due` and started at `started` is due.
///
/// Frames stay on a regular schedule, so that a frame starting a little late does not lower the
/// average rate. The schedule restarts from the frame that just started in two cases: after
/// falling a whole period behind, since catching up would run frames back to back; and after a
/// frame that ran before it was due, because the windowing system asked for it (during a resize)
/// or the application did. Counting such a frame as the scheduled one would push the schedule one
/// period further away at each of them, and the loop would then sleep until the schedule caught
/// up, without running frames or noticing signals.
fn next_due(due: Instant, started: Instant, period: Duration) -> Instant {
    let next = due + period;
    if started < due || next <= started {
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
    fn a_frame_that_ran_before_it_was_due_restarts_the_schedule() {
        let started = Instant::now();
        let due = started + Duration::from_millis(10);
        assert_eq!(next_due(due, started, PERIOD), started + PERIOD);
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
