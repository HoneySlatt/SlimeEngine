//! When the host draws a frame, and the time that it gives the interface
//! (`docs/adr/0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md`,
//! `docs/adr/0187-a-hidden-window-at-rest.md`): part of the host's core, which knows no platform
//! and reads no clock, every time given to it.
//!
//! - *At rest*, nothing runs: the host waits with no deadline but the interface's next
//!   ([`Ui::next_frame_due`](uwui::tree::Ui::next_frame_due)).
//! - *While frames run*, one per refresh, on the platform's signal: each image is presented with a
//!   signal asked, the compositor's frame callback on Wayland, and the next frame waits for it.
//! - *Drawn just in time*, where the platform tells when images show: a frame starts at the
//!   signal's time plus the display's period, less the frame's work and a margin, so that input
//!   that comes before it reaches the refresh that the compositor composes next. The work is the
//!   longest of the latest frames'; the margin starts at [`MARGIN`], widens by [`MARGIN_STEP`] at
//!   each image that shows a refresh late, and narrows by a tenth of that after
//!   [`ON_TIME_TO_NARROW`] images on time in a row. Where the platform tells nothing, the frame
//!   starts on the signal.
//! - *The first frame after a rest* starts at once when that time has passed, as it has after a
//!   rest longer than a period, so that an input at rest reaches the screen within one frame's
//!   work; after a shorter rest, it starts just in time, which reaches the same refresh with fresher
//!   input (`docs/adr/0309-input-to-shown-measured-beside-input-to-present-in-motion-an-input-reaches-the-next-image.md`).
//! - *A window that holds its frames*, whose signal does not come, as a hidden one's on Wayland,
//!   draws nothing for the interface's own changes. What asks from outside, an event of the window
//!   or a wake of another thread, still runs a frame once the signal is [`SIGNAL_WAIT`] late; and
//!   so does the first
//!   change after each signal that is that late, whatever asks, which finds an image presented to
//!   a device lost before the host knew it, whose signal would never come.
//!
//! The time given to the interface is when the frame shows
//! (`docs/adr/0227-the-time-that-motion-reads.md`):
//!
//! - *predicted* ([`FrameSource::Predicted`]) where the platform tells when images show: the first
//!   refresh after the frame's work, on the grid of the last image shown and the display's period
//!   that the platform gave with it, plus the lag that recent images showed, in whole periods,
//!   between being ready and showing;
//! - else *on the grid* ([`FrameSource::OnGrid`]): the last signal's time plus whole periods of the
//!   display, the first after the frame starts, the period measured from the signals' gaps while
//!   frames follow each other;
//! - until either, and for the first frame of all, the frame's start ([`FrameSource::Started`]).
//!
//! The time given never goes back.

use std::collections::VecDeque;
use std::time::Duration;

use uwui::time::{FrameShown, FrameSource, Instant};

/// The longest that a frame wanted from outside waits for the platform's signal
/// (`docs/adr/0187-a-hidden-window-at-rest.md`): longer than a refresh of any display, so that a
/// window shown keeps its frames in step with it, and short enough that a request to a hidden
/// window is answered at once.
pub(crate) const SIGNAL_WAIT: Duration = Duration::from_millis(100);

/// The longest wait before a frame drawn in part is drawn again: its wait doubles from
/// [`SIGNAL_WAIT`] up to it, so that a window whose GPU's memory comes back draws within two
/// seconds, and one whose memory never does wakes less than once a second.
const LONGEST_REDRAW_WAIT: Duration = Duration::from_millis(1600);

/// How many gaps between signals the period is measured on.
const GAPS: usize = 15;

/// The shortest and the longest gap between two signals that counts as a period: shorter, two
/// signals of one refresh; longer, a window that the compositor signals rarely, as niri a hidden
/// one, once a second.
const GAP_RANGE: (Duration, Duration) = (Duration::from_micros(500), Duration::from_millis(250));

/// How many recent frames a frame's work, and the images' lag, are read over.
const RECENT: usize = 16;

/// How many images presented wait for their presentation's time at most: a platform that tells
/// none for some loses the oldest.
const IN_FLIGHT: usize = 8;

/// The longest lag that an image counts, in periods: an image shown much later, as a hidden
/// window's, says nothing of the next.
const MAX_LAG: u32 = 3;

/// The margin that a frame drawn just in time keeps before the compositor composes: what the
/// compositor spends between composing and signalling, and the frames' jitter.
pub(crate) const MARGIN: Duration = Duration::from_millis(1);

/// How much an image shown a refresh late widens the margin.
const MARGIN_STEP: Duration = Duration::from_micros(500);

/// How many images shown on time in a row narrow the margin by a tenth of [`MARGIN_STEP`], down
/// to [`MARGIN`]: a second at 240 Hz.
const ON_TIME_TO_NARROW: u32 = 240;

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
    /// When a frame drawn in part, and not presented, is drawn again, and the wait that led there;
    /// none since a frame was presented.
    redraw: Option<(Instant, Duration)>,
    /// The display's period that the platform gave with its last presentation.
    refresh: Option<Duration>,
    /// When the last image shown showed: the grid's anchor.
    last_shown: Option<Instant>,
    /// The images presented whose presentation was not told yet, oldest first.
    in_flight: VecDeque<InFlight>,
    /// The latest frames' work, from their start to the end of their present, oldest first.
    works: VecDeque<Duration>,
    /// The latest images' lag: the whole periods from the first refresh after an image was ready
    /// to the one that showed it, oldest first.
    lags: VecDeque<u32>,
    /// The time given to the last frame drawn, which the next never goes before.
    given: Option<FrameShown>,
    /// What the images shown late added to [`MARGIN`].
    widened: Duration,
    /// The images shown on time in a row since the margin last changed.
    on_time: u32,
}

/// An image presented, waiting for its presentation's time.
#[derive(Clone, Copy, Debug)]
struct InFlight {
    image: u64,
    /// When its present ended.
    ready: Instant,
    /// The time given to its frame.
    given: FrameShown,
}

/// An image that showed later than the time given to its frame by half a period or more: its frame
/// missed the refresh that it was drawn for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Missed {
    /// The time given to its frame.
    pub(crate) given: Instant,
    /// When it showed.
    pub(crate) shown: Instant,
}

/// Pushes `value` at the back of `recent`, the oldest let go of past `RECENT`.
fn keep_recent<T>(recent: &mut VecDeque<T>, value: T) {
    if recent.len() == RECENT {
        recent.pop_front();
    }
    recent.push_back(value);
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

    /// The frame started at `now` was drawn in part and its image not presented, the GPU's memory
    /// short: the frame is drawn again after a wait that doubles at each such frame, from
    /// [`SIGNAL_WAIT`] to [`LONGEST_REDRAW_WAIT`], and none sooner, so that the window neither
    /// shows a broken image nor rests on one, nor spins.
    pub(crate) fn not_presented(&mut self, now: Instant) {
        let wait = self
            .redraw
            .map_or(SIGNAL_WAIT, |(_, wait)| (wait * 2).min(LONGEST_REDRAW_WAIT));
        self.redraw = Some((now + wait, wait));
        self.pending = true;
    }

    /// Whether a frame is owed whatever the interface wants: a change of the host's own, or a
    /// frame drawn in part to draw again.
    pub(crate) fn owes_frame(&self) -> bool {
        self.pending || self.redraw.is_some()
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
        let start = self.just_in_time();
        let next = match (self.awaiting, self.redraw) {
            // A frame drawn in part is drawn again after its wait, none sooner.
            (_, Some((redraw, _))) if now < redraw => Next::WaitUntil(redraw),
            (None, _) if wanted => match start {
                Some(start) if now < start => Next::WaitUntil(start),
                _ => Next::Frame,
            },
            (None, _) => due.map_or(Next::Wait, Next::WaitUntil),
            (Some(asked), _) => {
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
        // A wait for the frame's start just in time is no rest: the frame is drawn at this signal.
        if next != Next::Frame
            && !(wanted && start.is_some_and(|start| next == Next::WaitUntil(start)))
            && let Some(signal) = &mut self.signal
        {
            signal.waited = true;
        }
        next
    }

    /// When the frame after the last signal starts, drawn just in time: the signal's time plus the
    /// period, less the frame's work and the margin; none where the platform tells no presentation,
    /// or before a signal and a period.
    pub(crate) fn just_in_time(&self) -> Option<Instant> {
        self.last_shown?;
        let (signal, period) = (self.signal?, self.period()?);
        let lead = self.work() + self.margin(period);
        let lead = u64::try_from(lead.as_nanos()).unwrap_or(u64::MAX);
        Some(Instant::from_nanos(
            (signal.at + period).as_nanos().saturating_sub(lead),
        ))
    }

    /// The margin that a frame drawn just in time keeps, never above the display's `period`.
    pub(crate) fn margin(&self, period: Duration) -> Duration {
        (MARGIN + self.widened).min(period)
    }

    /// The host drew a frame started at `now`, and presented its image numbered `image`, its
    /// present ended at `ready`, with the platform's signal asked.
    pub(crate) fn drew(&mut self, now: Instant, ready: Instant, image: u64) {
        keep_recent(&mut self.works, ready.saturating_duration_since(now));
        if let Some(given) = self.given {
            if self.in_flight.len() == IN_FLIGHT {
                self.in_flight.pop_front();
            }
            self.in_flight.push_back(InFlight {
                image,
                ready,
                given,
            });
        }
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
        self.redraw = None;
    }

    /// The platform told that the image numbered `image` showed at `shown`, none if it never
    /// did, the display's period then `refresh` where it gave one: the image's frame, if it missed
    /// the refresh that its time was predicted for.
    pub(crate) fn presented(
        &mut self,
        image: u64,
        shown: Option<Instant>,
        refresh: Option<Duration>,
    ) -> Option<Missed> {
        if refresh.is_some() {
            self.refresh = refresh;
        }
        // Images whose presentation the platform did not tell are let go of.
        while self
            .in_flight
            .front()
            .is_some_and(|flight| flight.image < image)
        {
            self.in_flight.pop_front();
        }
        let flight = self
            .in_flight
            .front()
            .filter(|flight| flight.image == image)
            .copied();
        if flight.is_some() {
            self.in_flight.pop_front();
        }
        let shown = shown?;
        self.last_shown = Some(self.last_shown.map_or(shown, |last| last.max(shown)));
        let (flight, period) = (flight?, self.period()?);
        let lag = shown.saturating_duration_since(flight.ready).as_nanos() / period.as_nanos();
        keep_recent(
            &mut self.lags,
            u32::try_from(lag).map_or(MAX_LAG, |lag| lag.min(MAX_LAG)),
        );
        if flight.given.source != FrameSource::Predicted {
            return None;
        }
        if shown < flight.given.at + period / 2 {
            self.on_time += 1;
            if self.on_time >= ON_TIME_TO_NARROW {
                self.widened = self.widened.saturating_sub(MARGIN_STEP / 10);
                self.on_time = 0;
            }
            return None;
        }
        self.widened = (self.widened + MARGIN_STEP).min(period);
        self.on_time = 0;
        Some(Missed {
            given: flight.given.at,
            shown,
        })
    }

    /// The display's period: the one that the platform gave with its last presentation, else the
    /// median of the signals' gaps; none before either.
    pub(crate) fn period(&self) -> Option<Duration> {
        if self.refresh.is_some() {
            return self.refresh;
        }
        let mut gaps = self.gaps.clone();
        gaps.sort_unstable();
        gaps.get(gaps.len() / 2).copied()
    }

    /// A frame's work, from its start to the end of its present: the longest of the latest
    /// frames', none before the first.
    pub(crate) fn work(&self) -> Duration {
        self.works.iter().max().copied().unwrap_or_default()
    }

    /// The time that a frame starting at `now` shows, which the frame then keeps: predicted from
    /// the presentations where the platform tells them; else on the grid of the last signal and
    /// the period, the first point after `now`; else the frame's start. Never before the time
    /// given to the frame before.
    pub(crate) fn shown(&mut self, now: Instant) -> FrameShown {
        let shown = match (self.last_shown, self.signal, self.period()) {
            (Some(last), _, Some(period)) => {
                let lag = self.lags.iter().min().copied().unwrap_or(0);
                FrameShown {
                    at: grid_point_from(last, period, now + self.work()) + period * lag,
                    source: FrameSource::Predicted,
                }
            }
            (None, Some(signal), Some(period)) => FrameShown {
                // Strictly after the frame's start: a frame at the signal shows a period later.
                at: grid_point_from(signal.at, period, now + Duration::from_nanos(1)),
                source: FrameSource::OnGrid,
            },
            _ => FrameShown {
                at: now,
                source: FrameSource::Started,
            },
        };
        let shown = match self.given {
            Some(given) if given.at > shown.at => FrameShown {
                at: given.at,
                ..shown
            },
            _ => shown,
        };
        self.given = Some(shown);
        shown
    }
}

/// The first point at or after `at` on the grid of `anchor` and `period`; `anchor` itself if `at`
/// comes before it.
fn grid_point_from(anchor: Instant, period: Duration, at: Instant) -> Instant {
    let since = at.saturating_duration_since(anchor).as_nanos();
    let periods = since.div_ceil(period.as_nanos().max(1));
    anchor
        + period
            .checked_mul(u32::try_from(periods).unwrap_or(u32::MAX))
            .unwrap_or(Duration::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn at(millis: u64) -> Instant {
        Instant::from_nanos(millis * 1_000_000)
    }

    const PERIOD: Duration = Duration::from_micros(4_167);

    fn micros(micros: u64) -> Instant {
        Instant::from_nanos(micros * 1_000)
    }

    #[test]
    fn the_time_given_is_predicted_from_the_presentations() {
        let mut pacing = Pacing::default();
        // The first frame: its start.
        assert_eq!(pacing.shown(at(0)).source, FrameSource::Started);
        pacing.drew(at(0), micros(300), 0);
        // Ready at 0.3 ms, it showed at 8.5 ms: a refresh after the first one after it was ready,
        // as a compositor that shows an image a refresh after it composes it.
        assert_eq!(pacing.presented(0, Some(micros(8_500)), Some(PERIOD)), None);
        assert_eq!(pacing.period(), Some(PERIOD));
        assert_eq!(pacing.work(), Duration::from_micros(300));
        // A frame at 10 ms, ready by 10.3 ms: the first refresh after it on the grid of 8.5 ms,
        // 12.667 ms, then the lag of a refresh.
        let shown = pacing.shown(micros(10_000));
        assert_eq!(
            shown,
            FrameShown {
                at: micros(8_500 + 2 * 4_167),
                source: FrameSource::Predicted
            }
        );
        pacing.drew(micros(10_000), micros(10_300), 1);
        assert_eq!(pacing.presented(1, Some(shown.at), None), None);
        // A frame that showed a refresh later than predicted missed it.
        let shown = pacing.shown(micros(20_000));
        assert_eq!(shown.at, micros(16_834 + 2 * 4_167));
        pacing.drew(micros(20_000), micros(20_300), 2);
        let late = shown.at + PERIOD;
        assert_eq!(
            pacing.presented(2, Some(late), None),
            Some(Missed {
                given: shown.at,
                shown: late
            })
        );
        // Its lag, longer, does not move the next prediction: the shortest of the latest counts.
        assert_eq!(pacing.lags.iter().min(), Some(&1));
    }

    /// A pacing whose first frame, at 0 and 0.3 ms of work, showed at 4.5 ms, the platform giving
    /// `refresh`, and whose signal came at 4.6 ms.
    fn told_presentations(refresh: Option<Duration>) -> Pacing {
        let mut pacing = Pacing::default();
        assert_eq!(pacing.next(at(0), true, None), Next::Frame);
        pacing.shown(at(0));
        pacing.drew(at(0), micros(300), 0);
        pacing.presented(0, Some(micros(4_500)), refresh);
        pacing.signalled(micros(4_600));
        pacing
    }

    #[test]
    fn frames_in_motion_start_just_in_time() {
        let mut pacing = told_presentations(Some(PERIOD));
        // Wanted at the signal: the frame starts the period after it, less its work and the margin.
        let start = micros(4_600 + 4_167 - 300 - 1_000);
        assert_eq!(MARGIN, Duration::from_micros(1_000));
        assert_eq!(pacing.just_in_time(), Some(start));
        assert_eq!(
            pacing.next(micros(4_600), true, None),
            Next::WaitUntil(start)
        );
        // An input meanwhile changes nothing: it waits for the same start, and reaches its frame.
        assert_eq!(
            pacing.next(micros(5_000), true, None),
            Next::WaitUntil(start)
        );
        assert_eq!(pacing.next(start, true, None), Next::Frame);
        // Not wanted, it rests until something is.
        let mut resting = told_presentations(Some(PERIOD));
        assert_eq!(resting.next(micros(4_600), false, None), Next::Wait);
    }

    #[test]
    fn after_a_rest_longer_than_a_period_the_frame_starts_at_once() {
        let mut pacing = told_presentations(Some(PERIOD));
        assert_eq!(pacing.next(micros(4_600), false, None), Next::Wait);
        // An input a period and more after the signal: the start just in time is past.
        assert_eq!(pacing.next(micros(9_000), true, None), Next::Frame);
        // A shorter rest waits for it, as the same refresh shows the frame either way.
        let mut short = told_presentations(Some(PERIOD));
        assert_eq!(short.next(micros(4_600), false, None), Next::Wait);
        assert!(matches!(
            short.next(micros(5_600), true, None),
            Next::WaitUntil(start) if start > micros(5_600)
        ));
    }

    #[test]
    fn without_presentations_frames_start_on_the_signal() {
        let mut pacing = Pacing::default();
        pacing.next(at(0), true, None);
        pacing.drew(at(0), micros(300), 0);
        pacing.signalled(micros(4_600));
        assert_eq!(pacing.just_in_time(), None);
        assert_eq!(pacing.next(micros(4_600), true, None), Next::Frame);
    }

    #[test]
    fn a_wait_just_in_time_keeps_the_signals_gap_a_period() {
        // No period given with the presentations: it is measured from the signals' gaps, which a
        // frame drawn just in time at each signal keeps.
        let mut pacing = told_presentations(None);
        let mut signal = micros(4_600);
        for image in 1..4 {
            // The first gaps measure the period; then the frame waits for its start.
            let next = pacing.next(signal, true, None);
            let start = match next {
                Next::WaitUntil(start) => start,
                _ => signal,
            };
            assert_eq!(pacing.next(start, true, None), Next::Frame, "image {image}");
            pacing.shown(start);
            pacing.drew(start, start + Duration::from_micros(300), image);
            signal += PERIOD;
            pacing.signalled(signal);
        }
        assert_eq!(pacing.period(), Some(PERIOD));
        assert!(matches!(
            pacing.next(signal, true, None),
            Next::WaitUntil(_)
        ));
    }

    #[test]
    fn an_image_shown_late_widens_the_margin_and_images_on_time_narrow_it() {
        let mut pacing = told_presentations(Some(PERIOD));
        assert_eq!(pacing.margin(PERIOD), MARGIN);
        let mut now = micros(10_000);
        let mut image = 1;
        let mut show = |pacing: &mut Pacing, late: bool| {
            let given = pacing.shown(now);
            pacing.drew(now, now + Duration::from_micros(300), image);
            let shown = if late { given.at + PERIOD } else { given.at };
            let missed = pacing.presented(image, Some(shown), None);
            assert_eq!(missed.is_some(), late);
            image += 1;
            now = shown + Duration::from_micros(100);
        };
        show(&mut pacing, true);
        show(&mut pacing, true);
        assert_eq!(pacing.margin(PERIOD), MARGIN + 2 * MARGIN_STEP);
        // A second of images on time narrows it by a tenth of a step.
        for _ in 0..ON_TIME_TO_NARROW {
            show(&mut pacing, false);
        }
        assert_eq!(
            pacing.margin(PERIOD),
            MARGIN + 2 * MARGIN_STEP - MARGIN_STEP / 10
        );
        // Never above the period.
        for _ in 0..20 {
            show(&mut pacing, true);
        }
        assert_eq!(pacing.margin(PERIOD), PERIOD);
    }

    #[test]
    fn the_time_given_never_goes_back() {
        let mut pacing = Pacing::default();
        pacing.drew(at(0), micros(100), 0);
        pacing.presented(0, Some(micros(4_000)), Some(PERIOD));
        let first = pacing.shown(micros(5_000));
        // A later presentation on an earlier grid, as a compositor's clock read again: the next
        // frame's time is not before the last given.
        pacing.presented(7, Some(micros(4_001)), None);
        assert!(pacing.shown(micros(5_000)).at >= first.at);
    }

    #[test]
    fn images_whose_presentation_never_comes_are_let_go_of() {
        let mut pacing = Pacing::default();
        for image in 0..20 {
            pacing.shown(at(image));
            pacing.drew(at(image), at(image), image);
        }
        assert_eq!(pacing.in_flight.len(), IN_FLIGHT);
        // The platform tells the 15th: those before it are let go of; a discarded image teaches
        // nothing.
        pacing.presented(15, None, Some(PERIOD));
        assert_eq!(
            pacing.in_flight.front().map(|flight| flight.image),
            Some(16)
        );
        assert_eq!(pacing.last_shown, None);
        // An image shown much later, as a hidden window's, counts as a lag of three periods.
        pacing.presented(16, Some(at(10_000)), None);
        assert_eq!(pacing.lags.back(), Some(&MAX_LAG));
    }

    #[test]
    fn a_frame_is_owed_for_a_change_of_the_host_or_a_redraw_until_one_is_drawn() {
        let mut pacing = Pacing::default();
        assert!(!pacing.owes_frame());
        pacing.host_changed();
        assert!(pacing.owes_frame());
        pacing.drew(at(0), at(0), 0);
        assert!(!pacing.owes_frame());
        pacing.not_presented(at(10));
        assert!(pacing.owes_frame());
        pacing.drew(at(200), at(200), 0);
        assert!(!pacing.owes_frame());
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
        pacing.drew(at(0), at(0), 0);
        pacing.signalled(at(4));
        // At rest after the signal, then an input: drawn at once, not on a next signal.
        assert_eq!(pacing.next(at(4), false, None), Next::Wait);
        assert_eq!(pacing.next(at(900), true, None), Next::Frame);
    }

    #[test]
    fn while_frames_run_each_waits_for_the_signal() {
        let mut pacing = Pacing::default();
        assert_eq!(pacing.next(at(0), true, None), Next::Frame);
        pacing.drew(at(0), at(0), 0);
        // A change before the signal waits for it, at most until it is late.
        assert_eq!(pacing.next(at(1), true, None), Next::WaitUntil(at(100)));
        pacing.signalled(at(4));
        assert_eq!(pacing.next(at(4), true, None), Next::Frame);
    }

    #[test]
    fn a_window_that_holds_its_frames_checks_the_hold_once_then_waits() {
        let mut pacing = Pacing::default();
        pacing.next(at(0), true, None);
        pacing.drew(at(0), at(0), 0);
        // The signal does not come: the first change once it is late runs one frame, which finds a
        // lost device if there is one.
        assert_eq!(pacing.next(at(150), true, None), Next::Frame);
        pacing.drew(at(150), at(150), 0);
        // Then the interface's own changes wait: nothing runs while the window is hidden.
        assert_eq!(pacing.next(at(400), true, None), Next::Wait);
        assert_eq!(pacing.next(at(400), true, Some(at(300))), Next::Wait);
        // A wake from outside runs a frame once the signal is late again.
        pacing.asked_from_outside();
        assert_eq!(pacing.next(at(400), true, None), Next::Frame);
        pacing.drew(at(400), at(400), 0);
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
        pacing.drew(at(0), at(0), 0);
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
            pacing.drew(now, now, 0);
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
        pacing.drew(at(0), at(0), 0);
        pacing.signalled(at(16));
        // At rest after the signal, then a frame 200 ms later: the gap to its signal is no period.
        assert_eq!(pacing.next(at(16), false, None), Next::Wait);
        assert_eq!(pacing.next(at(216), true, None), Next::Frame);
        pacing.drew(at(216), at(216), 0);
        pacing.signalled(at(232));
        assert_eq!(pacing.period(), None);
        // A frame drawn at that signal: its gap counts.
        assert_eq!(pacing.next(at(232), true, None), Next::Frame);
        pacing.drew(at(232), at(232), 0);
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
            pacing.drew(now, now, 0);
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
        pacing.drew(at(0), at(0), 0);
        // A configure acknowledged while the callback is awaited: the frame must carry it.
        pacing.host_changed();
        assert_eq!(pacing.next(at(1), false, None), Next::WaitUntil(at(100)));
        pacing.signalled(at(4));
        assert_eq!(pacing.next(at(4), false, None), Next::Frame);
        pacing.drew(at(4), at(4), 0);
        pacing.signalled(at(8));
        assert_eq!(pacing.next(at(8), false, None), Next::Wait);
    }

    #[test]
    fn a_window_that_holds_its_frames_wakes_for_no_due_time_once_checked() {
        let mut pacing = Pacing::default();
        pacing.next(at(0), true, None);
        pacing.drew(at(0), at(0), 0);
        // Before its check, the due time wakes it once, no sooner than the signal is late.
        assert_eq!(
            pacing.next(at(10), false, Some(at(50))),
            Next::WaitUntil(at(100))
        );
        assert_eq!(pacing.next(at(100), false, Some(at(50))), Next::Frame);
        pacing.drew(at(100), at(100), 0);
        // Checked, the hidden window waits for nothing but the signal, an event or a wake.
        assert_eq!(pacing.next(at(120), false, Some(at(300))), Next::Wait);
    }

    #[test]
    fn a_wake_that_changes_nothing_asks_no_frame_of_a_hidden_window_later() {
        let mut pacing = Pacing::default();
        pacing.next(at(0), true, None);
        pacing.drew(at(0), at(0), 0);
        pacing.next(at(150), true, None);
        pacing.drew(at(150), at(150), 0);
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

    #[test]
    fn a_frame_drawn_in_part_is_drawn_again_after_a_wait_that_doubles() {
        let mut pacing = Pacing::default();
        assert_eq!(pacing.next(at(0), false, None), Next::Wait);
        // The interface changed; its frame, drawn in part, is not presented.
        assert_eq!(pacing.next(at(0), true, None), Next::Frame);
        pacing.not_presented(at(0));
        // Wanted again though the interface asks nothing more: its changes are spent.
        assert_eq!(pacing.next(at(10), false, None), Next::WaitUntil(at(100)));
        // An input meanwhile does not hasten it.
        pacing.asked_from_outside();
        assert_eq!(pacing.next(at(20), true, None), Next::WaitUntil(at(100)));
        assert_eq!(pacing.next(at(100), false, None), Next::Frame);
        let mut now = at(100);
        for wait in [200, 400, 800, 1600, 1600] {
            pacing.not_presented(now);
            assert_eq!(
                pacing.next(now, false, None),
                Next::WaitUntil(now + wait * MS),
                "after {now:?}"
            );
            now += wait * MS;
            assert_eq!(pacing.next(now, false, None), Next::Frame);
        }
        // A frame presented forgets the waits: the next one drawn in part waits the first again.
        pacing.drew(now, now, 0);
        pacing.signalled(now + 4 * MS);
        now += 4 * MS;
        assert_eq!(pacing.next(now, false, None), Next::Wait);
        pacing.not_presented(now);
        assert_eq!(
            pacing.next(now, false, None),
            Next::WaitUntil(now + SIGNAL_WAIT)
        );
    }
}
