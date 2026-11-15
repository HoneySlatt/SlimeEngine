//! Input events' times anchored on the host's clock
//! (`docs/adr/0227-the-time-that-motion-reads.md`): part of the host's core.
//!
//! A platform that stamps its events in another clock, or from an undefined base, as Wayland's
//! milliseconds, gives their gaps but not where they lie on the host's clock. The host keeps their
//! gaps from their own times, and places the whole by the least gap between an event's reception
//! and its own time over a sliding window: the event that came the fastest. An event so placed is
//! never after its reception. Counters of 32 bits are unwrapped by their differences; the anchor
//! is reset when the system resumes, whose sleep moves the two clocks apart.

use std::collections::VecDeque;
use std::time::Duration;

use uwui::time::Instant;

/// How long a reception counts for the anchor.
const WINDOW: Duration = Duration::from_secs(10);

/// The most receptions that the window keeps; past it, the oldest leave.
const KEPT: usize = 256;

/// The anchor of one clock of the platform's events, given in milliseconds of 32 bits.
#[derive(Debug, Default)]
pub(crate) struct Anchor {
    /// The last time given, as the platform counts, and its milliseconds unwrapped.
    last: Option<(u32, i64)>,
    /// Receptions within the window, each the time it came on the host's clock and its gap to the
    /// event's own time, in nanoseconds, the gaps increasing from the front: a later reception with
    /// a smaller gap makes the earlier ones useless.
    window: VecDeque<(Instant, i128)>,
}

impl Anchor {
    /// The time on the host's clock of an event that the platform stamped `millis` and the host
    /// received at `received`.
    pub(crate) fn time(&mut self, millis: u32, received: Instant) -> Instant {
        let unwrapped = match self.last {
            // The difference read as signed: a counter that wrapped goes on, an event a little
            // older than the last one goes back.
            Some((last, unwrapped)) => {
                unwrapped + i64::from(millis.wrapping_sub(last).cast_signed())
            }
            None => i64::from(millis),
        };
        self.last = Some((millis, unwrapped));
        let event = i128::from(unwrapped) * 1_000_000;
        let gap = i128::from(received.as_nanos()) - event;
        while self
            .window
            .front()
            .is_some_and(|&(at, _)| received - at > WINDOW)
            || self.window.len() >= KEPT
        {
            self.window.pop_front();
        }
        while self.window.back().is_some_and(|&(_, kept)| kept >= gap) {
            self.window.pop_back();
        }
        self.window.push_back((received, gap));
        let least = self.window.front().map_or(gap, |&(_, least)| least);
        let placed = (event + least).clamp(0, i128::from(received.as_nanos()));
        Instant::from_nanos(u64::try_from(placed).unwrap_or(0))
    }

    /// Forgets the anchor: the system resumed, or the platform's clock may have changed.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(millis: u64) -> Instant {
        Instant::from_nanos(millis * 1_000_000)
    }

    #[test]
    fn the_gaps_are_kept_and_the_whole_placed_by_the_fastest_reception() {
        let mut anchor = Anchor::default();
        // The platform's clock is 5 s behind the host's; the first event came 3 ms late.
        assert_eq!(anchor.time(1_000, ms(6_003)), ms(6_003));
        // The second came 1 ms late: those that follow are placed on its anchor.
        assert_eq!(anchor.time(1_010, ms(6_011)), ms(6_011));
        assert_eq!(anchor.time(1_020, ms(6_028)), ms(6_021));
    }

    #[test]
    fn an_event_is_never_placed_after_its_reception() {
        let mut anchor = Anchor::default();
        anchor.time(1_000, ms(6_001));
        // A reception faster than any before: its own gap anchors it, at its reception.
        assert_eq!(anchor.time(1_010, ms(6_010)), ms(6_010));
    }

    #[test]
    fn a_counter_that_wraps_goes_on() {
        let mut anchor = Anchor::default();
        anchor.time(u32::MAX - 4, ms(10_000));
        // 10 ms later on the platform's clock, past its wrap.
        assert_eq!(anchor.time(5, ms(10_010)), ms(10_010));
        // An event a little older than the last goes back.
        assert_eq!(anchor.time(3, ms(10_012)), ms(10_008));
    }

    #[test]
    fn the_anchor_slides_past_its_window() {
        let mut anchor = Anchor::default();
        // A fast reception, then only slower ones: after the window, the fast one counts no more.
        anchor.time(0, ms(5_000));
        assert_eq!(anchor.time(1_000, ms(6_002)), ms(6_000));
        assert_eq!(anchor.time(11_000, ms(16_002)), ms(16_002));
    }

    #[test]
    fn a_reset_forgets_the_anchor() {
        let mut anchor = Anchor::default();
        anchor.time(1_000, ms(6_000));
        anchor.reset();
        // After a sleep of a minute the two clocks are a minute further apart.
        assert_eq!(anchor.time(2_000, ms(67_005)), ms(67_005));
        assert_eq!(anchor.time(2_010, ms(67_012)), ms(67_012));
    }
}
