//! The signal of frames on X11, and when each image showed
//! (`docs/adr/0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md`): what the
//! layer reads of Present's notices and RandR's modes, knowing neither the connection nor the
//! clock, every time given to it.
//!
//! - *The signal*: Present's `NotifyMSC` asked for the next refresh of the window's CRTC as each
//!   image is presented, its notice the signal, at the refresh's time; where the server gives no
//!   Present, a timer of the layer's own at the display's rate, 60 a second where RandR tells none,
//!   on a grid set by the first signal asked.
//! - *The images shown*: Present's notices of the pixmaps that Vulkan's driver presents to the
//!   window, where it presents through Present, told in order, the first for the first image asked,
//!   as the driver presents each image once; a skipped one never showed. A driver that presents
//!   otherwise, as a software one that copies its images, tells none: the images asked wait for
//!   none past [`IN_FLIGHT`].
//! - *The display's period*: the mode of the CRTC that shows most of the window, from RandR.
//!
//! While the window is hidden, unmapped or covered whole, the signal waits until it shows again,
//! as a compositor's frame callback does on Wayland.

use std::collections::VecDeque;
use std::time::Duration;

use uwui_time::Instant;

use crate::Presentation;

/// The display's period where RandR tells none: 60 refreshes a second, as under Xvfb.
pub(super) const UNKNOWN_PERIOD: Duration = Duration::from_nanos(16_666_667);

/// How many images asked wait for their presentation at most: a driver that tells none for some
/// loses the oldest.
const IN_FLIGHT: usize = 8;

/// How a Present notice of a pixmap ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shown {
    /// The image showed at the time given.
    At(Instant),
    /// A later image replaced it before it showed.
    Skipped,
}

/// The signal, the images asked and their presentations, of one window.
#[derive(Debug)]
pub(super) struct Frames {
    /// Whether the server gives Present: else the layer's own timer signals.
    present: bool,
    /// The display's period, from RandR.
    period: Option<Duration>,
    /// Whether the window shows: else its signal waits.
    shown: bool,
    /// The images asked whose presentation is awaited, oldest first.
    asked: VecDeque<u64>,
    /// The latest signal not told yet.
    signal: Option<Instant>,
    /// The time of the timer's next signal, when one is asked and Present is missing.
    due: Option<Instant>,
    /// A time on the timer's grid: its first signal's.
    grid: Option<Instant>,
    /// The presentations not told yet.
    presentations: Vec<Presentation>,
}

impl Frames {
    pub(super) fn new(present: bool) -> Self {
        Self {
            present,
            period: None,
            shown: true,
            asked: VecDeque::new(),
            signal: None,
            due: None,
            grid: None,
            presentations: Vec::new(),
        }
    }

    /// The display's period now, as RandR tells it.
    pub(super) fn set_period(&mut self, period: Option<Duration>) {
        self.period = period;
    }

    /// The window shows, or it is hidden, unmapped or covered whole.
    pub(super) fn set_shown(&mut self, shown: bool) {
        self.shown = shown;
    }

    /// The image numbered `image` is presented next, at `now`: whether Present's `NotifyMSC` must
    /// be asked for it; else the layer's timer signals at the next tick of its grid.
    pub(super) fn asked(&mut self, image: u64, now: Instant) -> bool {
        if self.asked.len() == IN_FLIGHT {
            self.asked.pop_front();
        }
        self.asked.push_back(image);
        if self.present {
            return true;
        }
        let period = self.period.unwrap_or(UNKNOWN_PERIOD);
        let grid = *self.grid.get_or_insert(now);
        // The first tick after now: whole periods from the grid's time.
        let periods = (now - grid).as_nanos() / period.as_nanos().max(1) + 1;
        let ahead = period.saturating_mul(u32::try_from(periods).unwrap_or(u32::MAX));
        self.due = Some(grid + ahead);
        false
    }

    /// When the layer's timer must turn the loop, if it waits for a signal.
    pub(super) fn deadline(&self) -> Option<Instant> {
        self.due.filter(|_| self.shown)
    }

    /// Present's notice of the refresh asked by `NotifyMSC`, at `at`: the signal.
    pub(super) fn signalled(&mut self, at: Instant) {
        self.signal = Some(at);
    }

    /// Present's notice of the next pixmap that the driver presented: the oldest image asked.
    pub(super) fn presented(&mut self, shown: Shown) {
        let Some(image) = self.asked.pop_front() else {
            return;
        };
        self.presentations.push(match shown {
            Shown::At(at) => Presentation {
                image,
                shown: Some(at),
                refresh: self.period,
            },
            Shown::Skipped => Presentation {
                image,
                shown: None,
                refresh: None,
            },
        });
    }

    /// The signal that came by `now`, if the window shows, and the presentations told since the
    /// last call.
    pub(super) fn take(&mut self, now: Instant) -> (Option<Instant>, Vec<Presentation>) {
        if let Some(due) = self.due
            && due <= now
        {
            self.due = None;
            self.signal = Some(due);
        }
        let signal = if self.shown { self.signal.take() } else { None };
        (signal, std::mem::take(&mut self.presentations))
    }
}

/// A mode's refresh period, from its pixel clock in hertz and its totals; none for a mode that
/// gives none. A double scan draws each line twice; an interlaced mode draws half its lines at
/// each refresh.
pub(super) fn period(
    dot_clock: u32,
    htotal: u16,
    vtotal: u16,
    double: bool,
    interlaced: bool,
) -> Option<Duration> {
    let mut lines = u64::from(vtotal);
    if double {
        lines *= 2;
    }
    let dots = u64::from(htotal) * lines;
    if dot_clock == 0 || dots == 0 {
        return None;
    }
    // Nanoseconds per refresh: the dots of a refresh over the dots a second.
    let mut nanos = dots.saturating_mul(1_000_000_000) / u64::from(dot_clock);
    if interlaced {
        nanos /= 2;
    }
    (nanos > 0).then(|| Duration::from_nanos(nanos))
}

/// A rectangle of the screen: its left, top, width and height.
pub(super) type Rect = (i32, i32, u32, u32);

/// The period of the CRTC that shows most of `window`, among `crtcs`, each its rectangle and its
/// period; none where none shows any of it, or that one tells no period.
pub(super) fn window_period(window: Rect, crtcs: &[(Rect, Option<Duration>)]) -> Option<Duration> {
    crtcs
        .iter()
        .map(|(crtc, period)| (overlap(window, *crtc), *period))
        .filter(|(area, _)| *area > 0)
        // The first of the largest, so that a tie keeps the order RandR gives.
        .fold(
            None,
            |best: Option<(u64, Option<Duration>)>, (area, period)| match best {
                Some((largest, _)) if largest >= area => best,
                _ => Some((area, period)),
            },
        )
        .and_then(|(_, period)| period)
}

/// The area that two rectangles share.
fn overlap(a: Rect, b: Rect) -> u64 {
    let side = |start_a: i32, length_a: u32, start_b: i32, length_b: u32| {
        let end = (i64::from(start_a) + i64::from(length_a))
            .min(i64::from(start_b) + i64::from(length_b));
        let start = i64::from(start_a).max(i64::from(start_b));
        u64::try_from(end - start).unwrap_or(0)
    };
    side(a.0, a.2, b.0, b.2) * side(a.1, a.3, b.1, b.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(millis: u64) -> Instant {
        Instant::from_nanos(millis * 1_000_000)
    }

    #[test]
    fn a_modes_period_follows_its_clock_and_totals() {
        // 1920 × 1080 at 60 Hz, CEA-861: 148.5 MHz over 2200 × 1125.
        assert_eq!(
            period(148_500_000, 2200, 1125, false, false),
            Some(Duration::from_nanos(16_666_666))
        );
        // A double scan draws each line twice; an interlaced mode half of them.
        assert_eq!(
            period(148_500_000, 2200, 1125, true, false),
            Some(Duration::from_nanos(33_333_333))
        );
        assert_eq!(
            period(148_500_000, 2200, 1125, false, true),
            Some(Duration::from_nanos(8_333_333))
        );
        assert_eq!(period(0, 2200, 1125, false, false), None);
        assert_eq!(period(148_500_000, 0, 1125, false, false), None);
    }

    #[test]
    fn the_window_takes_the_period_of_the_crtc_that_shows_most_of_it() {
        let sixty = Some(Duration::from_micros(16_667));
        let two_forty = Some(Duration::from_micros(4_167));
        let crtcs = [
            ((0, 0, 1920, 1080), sixty),
            ((1920, 0, 2560, 1440), two_forty),
        ];
        // Mostly on the second.
        assert_eq!(window_period((1800, 100, 800, 600), &crtcs), two_forty);
        // Mostly on the first.
        assert_eq!(window_period((1000, 100, 800, 600), &crtcs), sixty);
        // On none, or on one that tells no period.
        assert_eq!(window_period((-900, -700, 800, 600), &crtcs), None);
        assert_eq!(
            window_period((10, 10, 80, 60), &[((0, 0, 100, 100), None)]),
            None
        );
        // A tie keeps RandR's order.
        assert_eq!(window_period((1820, 0, 200, 100), &crtcs), sixty);
    }

    #[test]
    fn present_signals_and_tells_each_pixmap_for_the_oldest_image_asked() {
        let mut frames = Frames::new(true);
        frames.set_period(Some(Duration::from_millis(4)));
        assert!(frames.asked(0, ms(1)));
        assert!(frames.asked(1, ms(2)));
        assert_eq!(frames.deadline(), None);
        frames.presented(Shown::Skipped);
        frames.presented(Shown::At(ms(5)));
        frames.signalled(ms(5));
        let (signal, presentations) = frames.take(ms(6));
        assert_eq!(signal, Some(ms(5)));
        assert_eq!(
            presentations,
            [
                Presentation {
                    image: 0,
                    shown: None,
                    refresh: None
                },
                Presentation {
                    image: 1,
                    shown: Some(ms(5)),
                    refresh: Some(Duration::from_millis(4))
                }
            ]
        );
        // A pixmap told for no image asked is no one's.
        frames.presented(Shown::At(ms(9)));
        assert_eq!(frames.take(ms(10)), (None, Vec::new()));
    }

    #[test]
    fn a_driver_that_tells_no_pixmap_keeps_only_the_latest_images_asked() {
        let mut frames = Frames::new(true);
        for image in 0..20 {
            frames.asked(image, ms(image));
        }
        frames.presented(Shown::At(ms(30)));
        let (_, presentations) = frames.take(ms(31));
        assert_eq!(presentations[0].image, 20 - IN_FLIGHT as u64);
    }

    #[test]
    fn without_present_the_timer_signals_on_its_grid_at_the_displays_rate() {
        let mut frames = Frames::new(false);
        frames.set_period(Some(Duration::from_millis(4)));
        assert!(!frames.asked(0, ms(10)));
        // The grid starts at the first ask: its next tick is a period later.
        assert_eq!(frames.deadline(), Some(ms(14)));
        assert_eq!(frames.take(ms(13)).0, None);
        assert_eq!(frames.take(ms(14)).0, Some(ms(14)));
        assert_eq!(frames.deadline(), None);
        // Asked between ticks: the next tick of the same grid.
        frames.asked(1, ms(15));
        assert_eq!(frames.deadline(), Some(ms(18)));
        // Asked on a tick: the one after.
        frames.asked(2, ms(22));
        assert_eq!(frames.deadline(), Some(ms(26)));
    }

    #[test]
    fn without_a_rate_the_timer_signals_sixty_times_a_second() {
        let mut frames = Frames::new(false);
        frames.asked(0, ms(0));
        assert_eq!(frames.deadline(), Some(Instant::from_nanos(16_666_667)));
    }

    #[test]
    fn a_hidden_window_keeps_its_signal_until_it_shows() {
        let mut frames = Frames::new(false);
        frames.asked(0, ms(0));
        frames.set_shown(false);
        assert_eq!(frames.deadline(), None);
        assert_eq!(frames.take(ms(100)).0, None);
        frames.set_shown(true);
        assert_eq!(
            frames.take(ms(200)).0,
            Some(Instant::from_nanos(16_666_667))
        );

        let mut frames = Frames::new(true);
        frames.set_shown(false);
        frames.signalled(ms(5));
        assert_eq!(frames.take(ms(6)).0, None);
        frames.set_shown(true);
        assert_eq!(frames.take(ms(7)).0, Some(ms(5)));
    }
}
