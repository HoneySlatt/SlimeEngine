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
//!   longest of the latest frames', from their turn's start to the end of their present; the margin
//!   starts at [`MARGIN`], widens by [`MARGIN_STEP`] at each frame drawn just in time that misses
//!   the refresh it aimed at, whose image's signal comes half a period or more after it, and
//!   narrows by as much after [`ON_TIME_TO_NARROW`] such frames on time in a row. A frame that did
//!   not wait for its start, or whose signal comes much later, as a hidden window's, counts
//!   neither way, nor does an image shown later than predicted: that is the prediction's. A frame
//!   drawn at its signal because its start had already passed is judged too, so that a margin as
//!   wide as a period narrows back. The pointers' moves are held until the frame starts. Where the
//!   platform tells nothing, the frame starts on the signal.
//! - *The first frame after a rest* starts at once when that time has passed, as it has after a
//!   rest longer than a period, so that an input at rest reaches the screen within one frame's
//!   work; after a shorter rest, it starts just in time, which reaches the same refresh with
//!   fresher input (`docs/adr/0309-input-to-shown-measured-beside-input-to-present-in-motion-an-input-reaches-the-next-image.md`).
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
//!   refresh after the frame's usual work, the median of the latest frames', on the grid of the
//!   last image shown and the display's period, plus the usual lag, in whole periods, between being
//!   ready and showing, of recent frames that came as this one: in step with the signal, or after a
//!   rest, which a compositor that composes at once when idle shows sooner;
//! - else *on the grid* ([`FrameSource::OnGrid`]): the last signal's time plus whole periods of the
//!   display, the first after the frame starts;
//! - the period is the one that the platform gives with its presentations, else measured from the
//!   signals' gaps while frames follow each other, as where a compositor tells a period of zero;
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

/// The longest lag that an image counts, in periods: an image shown later, as a hidden window's
/// shown again, says nothing of the next, and is left out of the lags and the misses.
const MAX_LAG: u32 = 3;

/// The margin that a frame drawn just in time keeps before the compositor composes: what the
/// compositor spends between composing and signalling, and the frames' jitter.
pub(crate) const MARGIN: Duration = Duration::from_millis(1);

/// How much a frame drawn just in time that misses its refresh widens the margin.
const MARGIN_STEP: Duration = Duration::from_micros(500);

/// How many frames drawn just in time and on time in a row narrow the margin by [`MARGIN_STEP`],
/// down to [`MARGIN`]: a second at 240 Hz.
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
    /// When the turn that may draw the next frame began: a frame's work counts from it.
    woke: Option<Instant>,
    /// The latest frames' work, from the turn's start to the end of their present, oldest first.
    works: VecDeque<Duration>,
    /// The latest images' lag, the whole periods from the first refresh after an image was ready
    /// to the one that showed it, oldest first: of the frames drawn in step with the signal, then
    /// of the frames drawn after a rest, which a compositor that composes at once when idle shows
    /// sooner.
    lags: [VecDeque<u32>; 2],
    /// The time given to the last frame drawn, which the next never goes before, and whether that
    /// frame is drawn in step with the signal.
    given: Option<(FrameShown, Cadence)>,
    /// When the frame wanted starts just in time, while the host waits for it.
    waiting_start: Option<Instant>,
    /// The refresh that the frame about to be drawn aims at, once it waited for its start.
    aiming: Option<Instant>,
    /// The image drawn just in time whose signal is awaited, and the refresh that it aimed at.
    aimed: Option<(u64, Instant)>,
    /// What the frames drawn just in time that missed their refresh added to [`MARGIN`].
    widened: Duration,
    /// The frames drawn just in time on time in a row since the margin last changed.
    on_time: u32,
}

/// How a frame comes after the signal of the image before.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cadence {
    /// Within a period of the signal, where the platform tells presentations: drawn just in time.
    InStep = 0,
    /// After a rest longer than a period, or without a signal: at once.
    AfterRest = 1,
}

/// An image presented, waiting for its presentation's time.
#[derive(Clone, Copy, Debug)]
struct InFlight {
    image: u64,
    /// When its present ended.
    ready: Instant,
    cadence: Cadence,
}

/// A frame drawn just in time that missed the refresh that it aimed at: its image's signal came
/// half a period or more after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Missed {
    /// The frame's image.
    pub(crate) image: u64,
    /// The refresh that it aimed at: the signal before it plus the period.
    pub(crate) aimed: Instant,
    /// When its image's signal came.
    pub(crate) signal: Instant,
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
    pub(crate) fn no_image(&mut self, now: Instant) {
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

    /// Whether a frame wanted waits, at `now`, for its signal or for its start just in time: the
    /// pointers' moves are held until it draws.
    pub(crate) fn holds_moves(&self, now: Instant) -> bool {
        self.awaiting.is_some() || self.waiting_start.is_some_and(|start| now < start)
    }

    /// The turn that may draw the next frame began at `now`: the frame's work counts from it.
    pub(crate) fn woke(&mut self, now: Instant) {
        self.woke = Some(now);
    }

    /// The platform's signal came, at `now`: the compositor can show a new image. The frame drawn
    /// just in time whose image it follows, if that frame missed the refresh that it aimed at.
    pub(crate) fn signalled(&mut self, now: Instant) -> Option<Missed> {
        let missed = self.judge_aim(now);
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
        missed
    }

    /// Whether the frame drawn just in time whose image's signal came at `now` made the refresh
    /// that it aimed at: the margin narrows after enough that did, widens for one that did not.
    fn judge_aim(&mut self, now: Instant) -> Option<Missed> {
        let ((image, aimed), period) = (self.aimed.take()?, self.period()?);
        let late = now.saturating_duration_since(aimed);
        if late > period * MAX_LAG {
            // A signal much later, as a hidden window's: nothing of the margin.
            return None;
        }
        if late < period / 2 {
            self.on_time += 1;
            if self.on_time >= ON_TIME_TO_NARROW {
                self.widened = self.widened.saturating_sub(MARGIN_STEP);
                self.on_time = 0;
            }
            return None;
        }
        self.widened = (self.widened + MARGIN_STEP).min(period);
        self.on_time = 0;
        Some(Missed {
            image,
            aimed,
            signal: now,
        })
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
        // Whether the frame is drawn just in time: the host waited for its start, or the start had
        // passed when the signal came and the frame is drawn at the signal's own turn, so that a
        // margin as wide as a period is still judged, and narrows.
        let waited = start.is_some()
            && (self.waiting_start == start
                || self.signal.is_some_and(|signal| {
                    !signal.waited && start.is_some_and(|start| start <= signal.at)
                }));
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
        let waits_for_start = wanted && start.is_some_and(|start| next == Next::WaitUntil(start));
        self.waiting_start = start.filter(|_| waits_for_start);
        self.aiming = match (next, self.signal, self.period()) {
            (Next::Frame, Some(signal), Some(period)) if waited => Some(signal.at + period),
            _ => None,
        };
        if next != Next::Frame
            && !waits_for_start
            && let Some(signal) = &mut self.signal
        {
            signal.waited = true;
        }
        next
    }

    /// When the frame after the last signal starts, drawn just in time: the signal's time plus the
    /// period, less the longest of the latest frames' work and the margin; none where the platform
    /// tells no presentation, or before a signal and a period.
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
    /// present ended at `ready`, with the platform's signal asked. Its work counts from the turn's
    /// start ([`woke`](Self::woke)), else from `now`.
    pub(crate) fn drew(&mut self, now: Instant, ready: Instant, image: u64) {
        let from = self.woke.take().map_or(now, |woke| woke.min(now));
        keep_recent(&mut self.works, ready.saturating_duration_since(from));
        self.aimed = self.aiming.take().map(|aimed| (image, aimed));
        if let Some((_, cadence)) = self.given {
            if self.in_flight.len() == IN_FLIGHT {
                self.in_flight.pop_front();
            }
            self.in_flight.push_back(InFlight {
                image,
                ready,
                cadence,
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
    /// did, the display's period then `refresh` where it gave one, a period of zero for none: the
    /// grid's anchor, the period and the lag of frames that came as this one. An image shown more
    /// than [`MAX_LAG`] periods after it was ready teaches nothing.
    pub(crate) fn presented(
        &mut self,
        image: u64,
        shown: Option<Instant>,
        refresh: Option<Duration>,
    ) {
        if let Some(refresh) = refresh.filter(|refresh| !refresh.is_zero()) {
            self.refresh = Some(refresh);
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
        let Some(shown) = shown else {
            return;
        };
        self.last_shown = Some(self.last_shown.map_or(shown, |last| last.max(shown)));
        let (Some(flight), Some(period)) = (flight, self.period()) else {
            return;
        };
        let lag =
            shown.saturating_duration_since(flight.ready).as_nanos() / period.as_nanos().max(1);
        if let Some(lag) = u32::try_from(lag).ok().filter(|lag| *lag <= MAX_LAG) {
            keep_recent(&mut self.lags[flight.cadence as usize], lag);
        }
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

    /// A frame's work, from its turn's start to the end of its present: the longest of the latest
    /// frames', which a frame drawn just in time leaves before the compositor composes; none before
    /// the first.
    pub(crate) fn work(&self) -> Duration {
        self.works.iter().max().copied().unwrap_or_default()
    }

    /// A frame's usual work: the median of the latest frames', which predicts when a frame is
    /// ready, so that one long frame does not move the times given.
    fn usual_work(&self) -> Duration {
        median(self.works.iter().copied()).unwrap_or_default()
    }

    /// How a frame starting at `now` comes after the last signal.
    fn cadence(&self, now: Instant) -> Cadence {
        match (self.awaiting, self.last_shown, self.signal, self.period()) {
            (None, Some(_), Some(signal), Some(period)) if now < signal.at + period => {
                Cadence::InStep
            }
            _ => Cadence::AfterRest,
        }
    }

    /// The time that a frame starting at `now` shows, which the frame then keeps: predicted from
    /// the presentations where the platform tells them; else on the grid of the last signal and
    /// the period, the first point after `now`; else the frame's start. Never before the time
    /// given to the frame before.
    pub(crate) fn shown(&mut self, now: Instant) -> FrameShown {
        let cadence = self.cadence(now);
        let shown = match (self.last_shown, self.signal, self.period()) {
            (Some(last), _, Some(period)) => {
                // The usual lag of frames that come so; of the others before any.
                let [own, other] = match cadence {
                    Cadence::InStep => [&self.lags[0], &self.lags[1]],
                    Cadence::AfterRest => [&self.lags[1], &self.lags[0]],
                };
                let lag = median(own.iter().copied())
                    .or_else(|| median(other.iter().copied()))
                    .unwrap_or(0);
                FrameShown {
                    at: grid_point_from(last, period, now + self.usual_work()) + period * lag,
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
            Some((given, _)) if given.at > shown.at => FrameShown {
                at: given.at,
                ..shown
            },
            _ => shown,
        };
        self.given = Some((shown, cadence));
        shown
    }
}

/// The median of `values`, the lower one of an even count, so that of two lags the shorter
/// counts; none of none.
fn median<T: Ord>(values: impl Iterator<Item = T>) -> Option<T> {
    let mut values: Vec<T> = values.collect();
    values.sort_unstable();
    let middle = values.len().saturating_sub(1) / 2;
    values.into_iter().nth(middle)
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

    const WORK: Duration = Duration::from_micros(300);

    /// The display of the tests: a refresh every period from 10 ms on.
    fn refresh(k: u64) -> Instant {
        micros(10_000) + PERIOD * u32::try_from(k).expect("a refresh of the tests")
    }

    /// The first refresh at or after `at`.
    fn refresh_from(at: Instant) -> Instant {
        let since = at.saturating_duration_since(refresh(0)).as_nanos();
        refresh(u64::try_from(since.div_ceil(PERIOD.as_nanos())).expect("a refresh"))
    }

    /// A compositor of the tests: it composes at each refresh an image ready before it, signals
    /// 0.1 ms after, and shows the image `lag` refreshes after it composes it.
    #[derive(Clone, Copy)]
    struct Compositor {
        lag: u32,
    }

    impl Compositor {
        /// When an image ready at `ready` shows, and the signal that follows its composition.
        fn takes(self, ready: Instant) -> (Instant, Instant) {
            let composed = refresh_from(ready);
            (
                composed + PERIOD * self.lag,
                composed + Duration::from_micros(100),
            )
        }
    }

    /// One frame wanted at `now`, drawn when the pacing says, with `work`, on `compositor`: the
    /// time given, when its image showed, the signal after it, and whether it missed.
    fn one_frame(
        pacing: &mut Pacing,
        now: Instant,
        work: Duration,
        image: u64,
        compositor: Compositor,
    ) -> (FrameShown, Instant, Instant, bool) {
        let start = match pacing.next(now, true, None) {
            Next::WaitUntil(start) => start,
            Next::Frame => now,
            Next::Wait => panic!("a frame is wanted"),
        };
        assert_eq!(pacing.next(start, true, None), Next::Frame);
        pacing.woke(start);
        let given = pacing.shown(start);
        pacing.drew(start, start + work, image);
        let (shown, signal) = compositor.takes(start + work);
        pacing.presented(image, Some(shown), Some(PERIOD));
        let missed = pacing.signalled(signal).is_some();
        (given, shown, signal, missed)
    }

    /// `count` frames in motion, each wanted at the signal of the one before, from `image`: the
    /// last signal, the next image's number and the frames that missed.
    fn in_motion(
        pacing: &mut Pacing,
        mut signal: Instant,
        mut image: u64,
        count: u32,
        compositor: Compositor,
    ) -> (Instant, u64, u32) {
        let mut misses = 0;
        for _ in 0..count {
            let (_, _, next, missed) = one_frame(pacing, signal, WORK, image, compositor);
            misses += u32::from(missed);
            signal = next;
            image += 1;
        }
        (signal, image, misses)
    }

    /// A pacing whose first frame, at 9.5 ms, showed at the first refresh, and whose frames then
    /// ran in motion for 20 refreshes on `compositor`: the last signal and the next image's number.
    fn running(compositor: Compositor) -> (Pacing, Instant, u64) {
        let mut pacing = Pacing::default();
        let (_, _, signal, _) = one_frame(&mut pacing, micros(9_500), WORK, 0, compositor);
        let (signal, image, _) = in_motion(&mut pacing, signal, 1, 20, compositor);
        (pacing, signal, image)
    }

    #[test]
    fn the_time_given_is_predicted_from_the_presentations() {
        let compositor = Compositor { lag: 0 };
        let mut pacing = Pacing::default();
        // The first frame: its start.
        let (given, shown, signal, _) = one_frame(&mut pacing, micros(9_500), WORK, 0, compositor);
        assert_eq!(given.source, FrameSource::Started);
        assert_eq!((shown, pacing.period()), (refresh(0), Some(PERIOD)));
        // The next, in step with the signal: the refresh after its work, on the grid of the first.
        let (given, shown, signal, missed) = one_frame(&mut pacing, signal, WORK, 1, compositor);
        assert_eq!(
            given,
            FrameShown {
                at: refresh(1),
                source: FrameSource::Predicted
            }
        );
        assert_eq!((shown, missed), (refresh(1), false));
        // A compositor that shows each image a refresh later than it composes it: its lag is
        // learned and the images show when predicted; no frame missed its refresh, nor does the
        // margin move.
        let late = Compositor { lag: 1 };
        let (signal, _, misses) = in_motion(&mut pacing, signal, 2, 20, late);
        assert_eq!((misses, pacing.margin(PERIOD)), (0, MARGIN));
        let (given, shown, _, missed) = one_frame(&mut pacing, signal, WORK, 22, late);
        assert_eq!((given.at, missed), (shown, false));
    }

    #[test]
    fn frames_in_motion_start_just_in_time() {
        let (mut pacing, signal, _) = running(Compositor { lag: 0 });
        // Wanted at the signal: the frame starts the period after it, less its work and the margin.
        assert_eq!(MARGIN, Duration::from_micros(1_000));
        let expected = Instant::from_nanos((signal + PERIOD).as_nanos() - 1_300_000);
        assert_eq!(pacing.just_in_time(), Some(expected));
        assert_eq!(pacing.next(signal, true, None), Next::WaitUntil(expected));
        // An input meanwhile waits for the same start, its moves held until then.
        let meanwhile = signal + Duration::from_micros(400);
        assert!(pacing.holds_moves(meanwhile));
        assert_eq!(
            pacing.next(meanwhile, true, None),
            Next::WaitUntil(expected)
        );
        assert!(!pacing.holds_moves(expected));
        assert_eq!(pacing.next(expected, true, None), Next::Frame);
        // Not wanted, it rests until something is, and holds no move.
        let (mut resting, signal, _) = running(Compositor { lag: 0 });
        assert_eq!(resting.next(signal, false, None), Next::Wait);
        assert!(!resting.holds_moves(signal));
    }

    #[test]
    fn after_a_rest_longer_than_a_period_the_frame_starts_at_once() {
        let (mut pacing, signal, _) = running(Compositor { lag: 0 });
        assert_eq!(pacing.next(signal, false, None), Next::Wait);
        // An input a period and more after the signal: the start just in time is past.
        let later = signal + PERIOD + Duration::from_micros(100);
        assert_eq!(pacing.next(later, true, None), Next::Frame);
        // A shorter rest waits for it, as the same refresh shows the frame either way.
        let (mut short, signal, _) = running(Compositor { lag: 0 });
        assert_eq!(short.next(signal, false, None), Next::Wait);
        let sooner = signal + Duration::from_millis(1);
        assert!(matches!(
            short.next(sooner, true, None),
            Next::WaitUntil(start) if start > sooner
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
    fn frames_drawn_just_in_time_keep_measuring_the_period_where_none_is_told() {
        // A compositor that tells a period of zero, as a headless sway: the period is measured
        // from the signals' gaps, which frames drawn just in time keep, and follows a display
        // whose period changes.
        let mut pacing = Pacing::default();
        pacing.shown(micros(0));
        pacing.drew(micros(0), micros(300), 0);
        pacing.presented(0, Some(micros(4_500)), Some(Duration::ZERO));
        let mut signal = micros(4_600);
        pacing.signalled(signal);
        let mut image = 1;
        let run = |pacing: &mut Pacing, signal: &mut Instant, image: &mut u64, gap, count| {
            for _ in 0..count {
                let start = match pacing.next(*signal, true, None) {
                    Next::WaitUntil(start) => start,
                    _ => *signal,
                };
                assert_eq!(pacing.next(start, true, None), Next::Frame);
                pacing.shown(start);
                pacing.drew(start, start + WORK, *image);
                pacing.presented(*image, Some(*signal + gap), Some(Duration::ZERO));
                *image += 1;
                *signal += gap;
                pacing.signalled(*signal);
            }
        };
        run(&mut pacing, &mut signal, &mut image, PERIOD, 12);
        assert_eq!(pacing.period(), Some(PERIOD));
        // Every gap counts, though the frames wait for their start just in time.
        assert_eq!(pacing.gaps.len(), 12);
        assert!(matches!(
            pacing.next(signal, true, None),
            Next::WaitUntil(_)
        ));
        let longer = Duration::from_micros(6_944);
        run(&mut pacing, &mut signal, &mut image, longer, 20);
        assert_eq!(pacing.period(), Some(longer));
    }

    #[test]
    fn a_frame_that_misses_its_refresh_widens_the_margin_and_a_second_on_time_undoes_it() {
        let compositor = Compositor { lag: 0 };
        let (mut pacing, signal, image) = running(compositor);
        assert_eq!(pacing.margin(PERIOD), MARGIN);
        // A frame drawn just in time whose work, longer than its margin, ends after the refresh
        // that it aimed at: composed a refresh later, its signal comes a period late.
        let (_, _, signal, missed) = one_frame(
            &mut pacing,
            signal,
            Duration::from_millis(2),
            image,
            compositor,
        );
        assert!(missed);
        assert_eq!(pacing.margin(PERIOD), MARGIN + MARGIN_STEP);
        // A second of images on time undoes it.
        let (signal, image, misses) = in_motion(
            &mut pacing,
            signal,
            image + 1,
            ON_TIME_TO_NARROW,
            Compositor { lag: 0 },
        );
        assert_eq!(misses, 0);
        assert_eq!(pacing.margin(PERIOD), MARGIN);
        // Never above the period.
        in_motion(&mut pacing, signal, image, 20, Compositor { lag: 3 });
        assert!(pacing.margin(PERIOD) <= PERIOD);
    }

    #[test]
    fn neither_a_hidden_windows_image_nor_a_frame_after_a_rest_moves_the_margin() {
        let (mut pacing, signal, image) = running(Compositor { lag: 0 });
        assert_eq!(pacing.next(signal, false, None), Next::Wait);
        // A caret's blink half a second later, the window hidden: the compositor tells its image
        // 10 s later, as the window shows again.
        let due = signal + Duration::from_millis(500);
        assert_eq!(pacing.next(due, false, Some(due)), Next::Frame);
        pacing.shown(due);
        pacing.drew(due, due + WORK, image);
        let lags = pacing.lags.clone();
        pacing.presented(image, Some(due + Duration::from_secs(10)), None);
        assert_eq!(pacing.signalled(due + Duration::from_secs(10)), None);
        assert_eq!((pacing.margin(PERIOD), &pacing.lags), (MARGIN, &lags));
        // An input 20 ms after a signal starts its frame at once, after its start just in time
        // too; each misses its refresh by a long work: neither was drawn just in time.
        let compositor = Compositor { lag: 0 };
        let (mut pacing, signal, image) = running(compositor);
        let long = Duration::from_millis(5);
        let now = signal + Duration::from_millis(20);
        let (_, _, _, missed) = one_frame(&mut pacing, now, long, image, compositor);
        assert!(!missed);
        assert_eq!(pacing.margin(PERIOD), MARGIN);
        let (mut pacing, signal, image) = running(compositor);
        assert_eq!(pacing.next(signal, false, None), Next::Wait);
        let after_start = pacing.just_in_time().expect("a start") + Duration::from_micros(200);
        let longer = Duration::from_micros(1_500);
        let (_, _, _, missed) = one_frame(&mut pacing, after_start, longer, image, compositor);
        assert!(!missed);
        assert_eq!(pacing.margin(PERIOD), MARGIN);
    }

    #[test]
    fn a_burst_of_misses_leaves_the_margin_wide_for_a_second_alone() {
        // Six frames whose signal comes a period late, as a compositor that drops compositions
        // under another program's load: the margin reaches the period, the starts fall before
        // the signal, and frames drawn at it still narrow it back.
        let compositor = Compositor { lag: 0 };
        let (mut pacing, mut signal, mut image) = running(compositor);
        for _ in 0..6 {
            let start = match pacing.next(signal, true, None) {
                Next::WaitUntil(start) => start,
                _ => signal,
            };
            assert_eq!(pacing.next(start, true, None), Next::Frame);
            pacing.woke(start);
            pacing.shown(start);
            pacing.drew(start, start + WORK, image);
            let (shown, on_time) = compositor.takes(start + WORK);
            pacing.presented(image, Some(shown), Some(PERIOD));
            image += 1;
            signal = on_time + PERIOD;
            assert!(pacing.signalled(signal).is_some());
        }
        assert_eq!(pacing.margin(PERIOD), MARGIN + 6 * MARGIN_STEP);
        assert!(pacing.just_in_time().is_some_and(|start| start <= signal));
        // A frame drawn after a short rest, though its start passed before its signal, is not
        // drawn at the signal: it is not judged.
        assert_eq!(pacing.next(signal, false, None), Next::Wait);
        let later = signal + Duration::from_millis(5);
        let (_, _, next, missed) = one_frame(&mut pacing, later, WORK, image, compositor);
        assert!(!missed);
        assert_eq!(pacing.margin(PERIOD), MARGIN + 6 * MARGIN_STEP);
        let (signal, image) = (next, image + 1);
        let (_, _, misses) = in_motion(
            &mut pacing,
            signal,
            image,
            6 * ON_TIME_TO_NARROW,
            compositor,
        );
        assert_eq!((misses, pacing.margin(PERIOD)), (0, MARGIN));
    }

    #[test]
    fn the_work_counts_from_the_turns_start() {
        let mut pacing = Pacing::default();
        // A turn that began 2 ms before its frame, the application's step between.
        pacing.woke(micros(10_000));
        pacing.shown(micros(12_000));
        pacing.drew(micros(12_000), micros(12_300), 0);
        pacing.presented(0, Some(micros(14_000)), Some(PERIOD));
        pacing.signalled(micros(14_100));
        // The start leaves the turn's 2.3 ms and the margin before the next refresh.
        assert_eq!(pacing.work(), Duration::from_micros(2_300));
        assert_eq!(
            pacing.just_in_time(),
            Some(Instant::from_nanos(
                (micros(14_100) + PERIOD).as_nanos() - 3_300_000
            ))
        );
    }

    #[test]
    fn a_hidden_windows_late_signal_counts_neither_way() {
        let compositor = Compositor { lag: 0 };
        let (mut pacing, signal, image) = running(compositor);
        // A frame drawn just in time, then the window hidden: its signal comes a second later.
        let start = match pacing.next(signal, true, None) {
            Next::WaitUntil(start) => start,
            next => panic!("{next:?}"),
        };
        assert_eq!(pacing.next(start, true, None), Next::Frame);
        pacing.shown(start);
        pacing.drew(start, start + WORK, image);
        assert_eq!(pacing.signalled(start + Duration::from_secs(1)), None);
        assert_eq!(pacing.margin(PERIOD), MARGIN);
    }

    #[test]
    fn frames_after_a_rest_and_frames_in_step_keep_lags_of_their_own() {
        // A compositor that shows an image composed while frames run a refresh later, and one
        // composed from idle at once, as wlroots and niri may.
        let busy = Compositor { lag: 1 };
        let idle = Compositor { lag: 0 };
        let (mut pacing, signal, image) = running(busy);
        // A rest, then a gesture: its first frame from idle, shown sooner than the frames after a
        // rest had shown so far, then frames in step, none of which misses.
        let now = signal + Duration::from_millis(50);
        let (_, _, signal, _) = one_frame(&mut pacing, now, WORK, image, idle);
        let (signal, image, misses) = in_motion(&mut pacing, signal, image + 1, 16, busy);
        assert_eq!(misses, 0);
        // A second gesture, both lags learned: every time given is when its image shows.
        let now = signal + Duration::from_millis(50);
        let (given, shown, signal, _) = one_frame(&mut pacing, now, WORK, image, idle);
        assert_eq!(given.at, shown);
        let (given, shown, _, _) = one_frame(&mut pacing, signal, WORK, image + 1, busy);
        assert_eq!(given.at, shown);
    }

    #[test]
    fn one_long_frame_neither_moves_the_times_given_ahead_nor_holds_them() {
        let compositor = Compositor { lag: 0 };
        let (mut pacing, signal, image) = running(compositor);
        // One frame of 50 ms, as a dialog's first or a device made again.
        let long = Duration::from_millis(50);
        let (_, _, mut signal, _) = one_frame(&mut pacing, signal, long, image, compositor);
        let mut last = None;
        for image in image + 1..image + 40 {
            let (given, shown, next, missed) =
                one_frame(&mut pacing, signal, WORK, image, compositor);
            assert_eq!((given.at, missed), (shown, false), "image {image}");
            assert!(last.is_none_or(|last| given.at > last), "image {image}");
            last = Some(given.at);
            signal = next;
        }
    }

    #[test]
    fn the_time_given_never_goes_back() {
        let (mut pacing, signal, _) = running(Compositor { lag: 1 });
        let given = pacing.shown(signal);
        // The lags forgotten, as after a change of compositor's pace: a frame drawn again at once
        // would be predicted a refresh sooner, and is given the time before.
        pacing.lags = [VecDeque::new(), VecDeque::new()];
        assert_eq!(pacing.shown(signal), given);
    }

    #[test]
    fn a_period_of_zero_is_none() {
        let mut pacing = Pacing::default();
        pacing.shown(at(0));
        pacing.drew(at(0), micros(300), 0);
        pacing.presented(0, Some(micros(4_500)), Some(Duration::ZERO));
        assert_eq!(pacing.period(), None);
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
        // An image shown much later, as a hidden window's, teaches no lag.
        pacing.presented(16, Some(at(10_000)), None);
        assert!(pacing.lags.iter().all(VecDeque::is_empty));
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
        pacing.no_image(at(0));
        assert_eq!(pacing.next(at(50), true, None), Next::WaitUntil(at(100)));
        assert_eq!(pacing.next(at(100), true, None), Next::Frame);
        pacing.no_image(at(100));
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
