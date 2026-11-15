//! When the host draws a frame, and the time that it gives the interface
//! (`docs/adr/0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md`,
//! `docs/adr/0187-a-hidden-window-at-rest.md`): part of the host's core, which knows no platform
//! and reads no clock, every time given to it.
//!
//! - *At rest*, nothing runs: the host waits with no deadline but the interface's next
//!   ([`Ui::next_frame_due`](uwui::tree::Ui::next_frame_due)).
//! - *The first frame after a rest* starts at once, so that an input at rest reaches the screen
//!   within one frame's work.
//! - *While frames run*, one per refresh, on the platform's signal: each image is presented with a
//!   signal asked, the compositor's frame callback on Wayland, and the next frame waits for it.
//! - *A window that holds its frames*, whose signal does not come, as a hidden one's on Wayland,
//!   draws nothing for the interface's own changes. What asks from outside, an event of the window
//!   or a wake of another thread, still runs a frame once the signal is [`SIGNAL_WAIT`] late; and
//!   so does the first
//!   change after each signal that is that late, whatever asks, which finds an image presented to
//!   a device lost before the host knew it, whose signal would never come.
//!
//! The time given to the interface is *on the grid* ([`FrameSource::OnGrid`]): the last signal's
//! time plus whole periods of the display, the first after the frame starts. The period is
//! measured from the signals' gaps while frames follow each other; until then, and for the first
//! frame of all, which has no signal before it, the frame's start ([`FrameSource::Started`]).

use std::time::Duration;

use uwui::time::{FrameShown, FrameSource, Instant};

/// The longest that a frame wanted from outside waits for the platform's signal
/// (`docs/adr/0187-a-hidden-window-at-rest.md`): longer than a refresh of any display, so that a
/// window shown keeps its frames in step with it, and short enough that a request to a hidden
/// window is answered at once.
pub(crate) const SIGNAL_WAIT: Duration = Duration::from_millis(100);

/// How many gaps between signals the period is measured on.
const GAPS: usize = 15;

/// The shortest and the longest gap between two signals that counts as a period: shorter, two
/// signals of one refresh; longer, a window that the compositor signals rarely, as niri a hidden
/// one, once a second.
const GAP_RANGE: (Duration, Duration) = (Duration::from_micros(500), Duration::from_millis(250));

/// What the host does next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Next {
    /// Draws a frame now.
    Frame,
    /// Waits for an event, a wake or the signal, or until this time.
    WaitUntil(Instant),
    /// Waits for an event, a wake or the signal, with no deadline: at rest.
    Wait,
}

/// The pacing of one window's frames.
#[derive(Debug, Default)]
pub(crate) struct Pacing {
    /// When the host asked for the platform's signal with its last image, while it has not come.
    awaiting: Option<Instant>,
    /// Whether a frame ran while the signal was late, since it last came: the one frame that checks
    /// each hold.
    checked: bool,
    /// Whether something outside asked for a frame since the last one: an event of the window or a
    /// wake of another thread. Forgotten once a step changed nothing.
    outside: bool,
    /// A change of the host's own that the next frame carries.
    pending: bool,
    /// The last signal, and whether a frame was drawn at it, so that the gap to the next one is a
    /// refresh.
    signal: Option<Signal>,
    /// The latest gaps between signals that followed each other, oldest first.
    gaps: Vec<Duration>,
}

#[derive(Clone, Copy, Debug)]
struct Signal {
    at: Instant,
    /// Whether a frame was drawn at this signal, before the loop waited again.
    drawn_at: bool,
    /// Whether the loop has waited since this signal.
    waited: bool,
}

impl Pacing {
    /// Something outside asked for a frame: an event of the window or a wake of another thread. It
    /// runs one even while the window holds its frames, once their signal is late.
    pub(crate) fn asked_from_outside(&mut self) {
        self.outside = true;
    }

    /// A change of the host's own, a configure acknowledged or a request of the window, which the
    /// next frame's commit must carry: a frame is wanted until one is drawn.
    pub(crate) fn host_changed(&mut self) {
        self.pending = true;
    }

    /// The host had no image to draw the frame started at `now` into, and presented nothing: it
    /// tries again once a signal would be late, as a frame whose signal does not come, rather than
    /// at once.
    pub(crate) fn missed(&mut self, now: Instant) {
        self.awaiting = Some(now);
        self.checked = false;
    }

    /// Whether the last image presented waits for its signal.
    pub(crate) fn awaiting(&self) -> bool {
        self.awaiting.is_some()
    }

    /// The platform's signal came, at `now`: the compositor can show a new image.
    pub(crate) fn signalled(&mut self, now: Instant) {
        if let Some(last) = self.signal
            && last.drawn_at
        {
            let gap = now - last.at;
            if (GAP_RANGE.0..=GAP_RANGE.1).contains(&gap) {
                if self.gaps.len() == GAPS {
                    self.gaps.remove(0);
                }
                self.gaps.push(gap);
            }
        }
        self.signal = Some(Signal {
            at: now,
            drawn_at: false,
            waited: false,
        });
        self.awaiting = None;
        self.checked = false;
    }

    /// What the host does next, at `now`: `wanted` when the interface or the program changed what
    /// shows, `due` the interface's next frame asked though nothing changes.
    pub(crate) fn next(&mut self, now: Instant, wanted: bool, due: Option<Instant>) -> Next {
        let wanted = wanted || self.pending || due.is_some_and(|due| due <= now);
        if !wanted {
            // What asked from outside was served by a step that changed nothing.
            self.outside = false;
        }
        let next = match self.awaiting {
            None if wanted => Next::Frame,
            None => due.map_or(Next::Wait, Next::WaitUntil),
            Some(asked) => {
                let late = asked + SIGNAL_WAIT;
                let may = self.outside || !self.checked;
                match (wanted, may) {
                    (true, true) if now >= late => Next::Frame,
                    (true, true) => Next::WaitUntil(late),
                    // Before its check, the time that the interface waits for wakes the window
                    // once, no sooner than the signal is late.
                    (false, true) => due.map_or(Next::Wait, |due| Next::WaitUntil(due.max(late))),
                    // The window holds its frames: the signal, an event or a wake runs this
                    // again. A hidden window wakes for nothing that it waits for (ADR-0227).
                    (_, false) => Next::Wait,
                }
            }
        };
        if next != Next::Frame
            && let Some(signal) = &mut self.signal
        {
            signal.waited = true;
        }
        next
    }

    /// The host drew a frame at `now` and presented its image, with the platform's signal asked.
    pub(crate) fn drew(&mut self, now: Instant) {
        if self.awaiting.is_some() {
            // A frame while the signal was late: the hold is checked until the signal comes.
            self.checked = true;
        }
        if let Some(signal) = &mut self.signal
            && !signal.waited
        {
            signal.drawn_at = true;
        }
        self.awaiting = Some(now);
        self.outside = false;
        self.pending = false;
    }

    /// The display's period, measured from the signals' gaps: their median; none before one gap.
    pub(crate) fn period(&self) -> Option<Duration> {
        let mut gaps = self.gaps.clone();
        gaps.sort_unstable();
        gaps.get(gaps.len() / 2).copied()
    }

    /// The time that a frame starting at `now` shows: on the grid of the last signal and the
    /// period, the first point after `now`; the frame's start without a signal or a period.
    pub(crate) fn shown(&self, now: Instant) -> FrameShown {
        let (Some(signal), Some(period)) = (self.signal, self.period()) else {
            return FrameShown {
                at: now,
                source: FrameSource::Started,
            };
        };
        let since = now - signal.at;
        let periods = since.as_nanos() / period.as_nanos() + 1;
        let ahead = period
            .checked_mul(u32::try_from(periods).unwrap_or(u32::MAX))
            .unwrap_or(Duration::MAX);
        FrameShown {
            at: signal.at + ahead,
            source: FrameSource::OnGrid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn at(millis: u64) -> Instant {
        Instant::from_nanos(millis * 1_000_000)
    }

    #[test]
    fn at_rest_it_waits_with_no_deadline_but_the_interfaces() {
        let mut pacing = Pacing::default();
        assert_eq!(pacing.next(at(0), false, None), Next::Wait);
        assert_eq!(
            pacing.next(at(0), false, Some(at(500))),
            Next::WaitUntil(at(500))
        );
        // A frame due runs once its time comes.
        assert_eq!(pacing.next(at(500), false, Some(at(500))), Next::Frame);
    }

    #[test]
    fn the_first_frame_after_a_rest_starts_at_once() {
        let mut pacing = Pacing::default();
        assert_eq!(pacing.next(at(0), true, None), Next::Frame);
        pacing.drew(at(0));
        pacing.signalled(at(4));
        // At rest after the signal, then an input: drawn at once, not on a next signal.
        assert_eq!(pacing.next(at(4), false, None), Next::Wait);
        assert_eq!(pacing.next(at(900), true, None), Next::Frame);
    }

    #[test]
    fn while_frames_run_each_waits_for_the_signal() {
        let mut pacing = Pacing::default();
        assert_eq!(pacing.next(at(0), true, None), Next::Frame);
        pacing.drew(at(0));
        // A change before the signal waits for it, at most until it is late.
        assert_eq!(pacing.next(at(1), true, None), Next::WaitUntil(at(100)));
        pacing.signalled(at(4));
        assert_eq!(pacing.next(at(4), true, None), Next::Frame);
    }

    #[test]
    fn a_window_that_holds_its_frames_checks_the_hold_once_then_waits() {
        let mut pacing = Pacing::default();
        pacing.next(at(0), true, None);
        pacing.drew(at(0));
        // The signal does not come: the first change once it is late runs one frame, which finds a
        // lost device if there is one.
        assert_eq!(pacing.next(at(150), true, None), Next::Frame);
        pacing.drew(at(150));
        // Then the interface's own changes wait: nothing runs while the window is hidden.
        assert_eq!(pacing.next(at(400), true, None), Next::Wait);
        assert_eq!(pacing.next(at(400), true, Some(at(300))), Next::Wait);
        // A wake from outside runs a frame once the signal is late again.
        pacing.asked_from_outside();
        assert_eq!(pacing.next(at(400), true, None), Next::Frame);
        pacing.drew(at(400));
        pacing.asked_from_outside();
        assert_eq!(pacing.next(at(410), true, None), Next::WaitUntil(at(500)));
        // The signal that comes ends the hold.
        pacing.signalled(at(600));
        assert_eq!(pacing.next(at(600), true, None), Next::Frame);
    }

    #[test]
    fn nothing_wanted_while_the_signal_is_late_rests() {
        let mut pacing = Pacing::default();
        pacing.next(at(0), true, None);
        pacing.drew(at(0));
        assert_eq!(pacing.next(at(300), false, None), Next::Wait);
    }

    #[test]
    fn the_time_given_is_on_the_grid_of_the_signals() {
        let mut pacing = Pacing::default();
        // The first frame has no signal before it: its start.
        assert_eq!(
            pacing.shown(at(0)),
            FrameShown {
                at: at(0),
                source: FrameSource::Started
            }
        );
        let period = Duration::from_micros(4_167);
        let mut now = at(0);
        for _ in 0..5 {
            assert_eq!(pacing.next(now, true, None), Next::Frame);
            pacing.drew(now);
            now += period;
            pacing.signalled(now);
        }
        assert_eq!(pacing.period(), Some(period));
        // A frame drawn at the signal shows a period later.
        let shown = pacing.shown(now);
        assert_eq!(
            (shown.at, shown.source),
            (now + period, FrameSource::OnGrid)
        );
        // After a rest of 1.001 s, on the grid still: the first point after the frame's start.
        let later = now + Duration::from_millis(1001);
        let shown = pacing.shown(later);
        assert!(shown.at > later && shown.at <= later + period);
        assert_eq!((shown.at - now).as_nanos() % period.as_nanos(), 0);
    }

    #[test]
    fn the_period_counts_only_the_gaps_of_frames_drawn_at_their_signal() {
        let mut pacing = Pacing::default();
        pacing.next(at(0), true, None);
        pacing.drew(at(0));
        pacing.signalled(at(16));
        // At rest after the signal, then a frame 200 ms later: the gap to its signal is no period.
        assert_eq!(pacing.next(at(16), false, None), Next::Wait);
        assert_eq!(pacing.next(at(216), true, None), Next::Frame);
        pacing.drew(at(216));
        pacing.signalled(at(232));
        assert_eq!(pacing.period(), None);
        // A frame drawn at that signal: its gap counts.
        assert_eq!(pacing.next(at(232), true, None), Next::Frame);
        pacing.drew(at(232));
        pacing.signalled(at(248));
        assert_eq!(pacing.period(), Some(16 * MS));
    }

    #[test]
    fn the_period_is_the_median_of_the_latest_gaps() {
        let mut pacing = Pacing::default();
        let mut now = at(0);
        // One early signal and one late among regular ones change nothing; a hidden window's
        // signal, a second later, is no period.
        for gap in [16, 16, 2, 16, 40, 16, 1000, 16] {
            pacing.next(now, true, None);
            pacing.drew(now);
            now += gap * MS;
            pacing.signalled(now);
        }
        assert_eq!(pacing.period(), Some(16 * MS));
        assert_eq!(pacing.gaps.len(), 6);
    }
    #[test]
    fn a_change_of_the_hosts_own_waits_for_the_signal_then_draws() {
        let mut pacing = Pacing::default();
        pacing.next(at(0), true, None);
        pacing.drew(at(0));
        // A configure acknowledged while the callback is awaited: the frame must carry it.
        pacing.host_changed();
        assert_eq!(pacing.next(at(1), false, None), Next::WaitUntil(at(100)));
        pacing.signalled(at(4));
        assert_eq!(pacing.next(at(4), false, None), Next::Frame);
        pacing.drew(at(4));
        pacing.signalled(at(8));
        assert_eq!(pacing.next(at(8), false, None), Next::Wait);
    }

    #[test]
    fn a_window_that_holds_its_frames_wakes_for_no_due_time_once_checked() {
        let mut pacing = Pacing::default();
        pacing.next(at(0), true, None);
        pacing.drew(at(0));
        // Before its check, the due time wakes it once, no sooner than the signal is late.
        assert_eq!(
            pacing.next(at(10), false, Some(at(50))),
            Next::WaitUntil(at(100))
        );
        assert_eq!(pacing.next(at(100), false, Some(at(50))), Next::Frame);
        pacing.drew(at(100));
        // Checked, the hidden window waits for nothing but the signal, an event or a wake.
        assert_eq!(pacing.next(at(120), false, Some(at(300))), Next::Wait);
    }

    #[test]
    fn a_wake_that_changes_nothing_asks_no_frame_of_a_hidden_window_later() {
        let mut pacing = Pacing::default();
        pacing.next(at(0), true, None);
        pacing.drew(at(0));
        pacing.next(at(150), true, None);
        pacing.drew(at(150));
        // A wake whose step changed nothing.
        pacing.asked_from_outside();
        assert_eq!(pacing.next(at(300), false, None), Next::Wait);
        // The interface's own change, later, still waits while the window is hidden.
        assert_eq!(pacing.next(at(400), true, None), Next::Wait);
    }

    #[test]
    fn a_frame_without_an_image_tries_again_once_the_signal_would_be_late() {
        let mut pacing = Pacing::default();
        // No image twice: each time, one more try a signal's wait later, not a hold.
        pacing.next(at(0), true, None);
        pacing.missed(at(0));
        assert_eq!(pacing.next(at(50), true, None), Next::WaitUntil(at(100)));
        assert_eq!(pacing.next(at(100), true, None), Next::Frame);
        pacing.missed(at(100));
        assert_eq!(pacing.next(at(200), true, None), Next::Frame);
    }
}
