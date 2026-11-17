//! The keys' repeat, which Wayland leaves to its clients
//! (`docs/adr/0273-m6d7a2-cut-in-three-waylands-keyboard-cursor-scale-and-activation.md`): the
//! last key pressed that repeats, at the rate and after the delay that the compositor's
//! `repeat_info` gives, from deadlines that the host's loop waits for. It knows no clock: every
//! time is given to it.

use std::time::Duration;

use uwui::time::Instant;

/// The rate and the delay that a compositor gives before it says, as the protocol's own example.
const DEFAULT: (u32, u32) = (25, 600);

/// The shortest gap between two repeats.
const SHORTEST_GAP: Duration = Duration::from_millis(1);

/// The most repeats given at once, when the loop comes late to its deadline: a machine that
/// stalls never floods the interface.
const MOST_AT_ONCE: usize = 2;

/// The repeat of one keyboard.
#[derive(Debug)]
pub(super) struct Repeat {
    /// The gap between repeats and the delay before the first; none when the compositor turns
    /// repeating off.
    pace: Option<(Duration, Duration)>,
    /// The key held, by its Linux code, and when it repeats next.
    held: Option<(u32, Instant)>,
}

impl Default for Repeat {
    fn default() -> Self {
        let mut repeat = Self {
            pace: None,
            held: None,
        };
        repeat.set_info(DEFAULT.0, DEFAULT.1);
        repeat
    }
}

impl Repeat {
    /// The compositor's `repeat_info`: `rate` repeats a second, the first after `delay`
    /// milliseconds; a rate of zero turns repeating off.
    pub(super) fn set_info(&mut self, rate: u32, delay: u32) {
        // A gap of a millisecond at least: a rate past a thousand a second, which the protocol
        // can carry, would otherwise give a gap of zero and a loop that never waits.
        self.pace = (rate > 0).then(|| {
            (
                (Duration::from_secs(1) / rate).max(SHORTEST_GAP),
                Duration::from_millis(u64::from(delay)),
            )
        });
        if self.pace.is_none() {
            self.held = None;
        }
    }

    /// The key of `code` went down at `at`; `repeats` when the keymap says it repeats. A key that
    /// does not repeat, as a modifier, leaves the one held repeating.
    pub(super) fn pressed(&mut self, code: u32, at: Instant, repeats: bool) {
        if !repeats {
            return;
        }
        self.held = self.pace.map(|(_, delay)| (code, at + delay));
    }

    /// The key of `code` went up: it repeats no more.
    pub(super) fn released(&mut self, code: u32) {
        if self.held.is_some_and(|(held, _)| held == code) {
            self.held = None;
        }
    }

    /// Nothing repeats: the focus left, or the keymap changed.
    pub(super) fn stop(&mut self) {
        self.held = None;
    }

    /// When the next repeat is due, if a key is held.
    pub(super) fn deadline(&self) -> Option<Instant> {
        self.held.map(|(_, next)| next)
    }

    /// The repeats due at `now`, each the key's code and its time; at most [`MOST_AT_ONCE`], the
    /// next one then a gap after `now`.
    pub(super) fn due(&mut self, now: Instant) -> Vec<(u32, Instant)> {
        let (Some((gap, _)), Some((code, mut next))) = (self.pace, self.held) else {
            return Vec::new();
        };
        let mut due = Vec::new();
        while next <= now {
            if due.len() == MOST_AT_ONCE {
                next = now + gap;
                break;
            }
            due.push((code, next));
            next += gap;
        }
        self.held = Some((code, next));
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(millis: u64) -> Instant {
        Instant::from_nanos(millis * 1_000_000)
    }

    #[test]
    fn a_key_held_repeats_after_the_delay_at_the_rate() {
        let mut repeat = Repeat::default();
        repeat.set_info(40, 300);
        repeat.pressed(30, at(1_000), true);
        assert_eq!(repeat.deadline(), Some(at(1_300)));
        assert!(repeat.due(at(1_299)).is_empty());
        assert_eq!(repeat.due(at(1_300)), [(30, at(1_300))]);
        assert_eq!(repeat.deadline(), Some(at(1_325)));
        assert_eq!(repeat.due(at(1_350)), [(30, at(1_325)), (30, at(1_350))]);
        repeat.released(30);
        assert_eq!(repeat.deadline(), None);
        assert!(repeat.due(at(2_000)).is_empty());
    }

    #[test]
    fn a_late_loop_gets_two_repeats_at_most() {
        let mut repeat = Repeat::default();
        repeat.set_info(40, 300);
        repeat.pressed(30, at(0), true);
        // A second late: two repeats, the next a gap after now.
        assert_eq!(repeat.due(at(1_300)), [(30, at(300)), (30, at(325))]);
        assert_eq!(repeat.deadline(), Some(at(1_325)));
    }

    #[test]
    fn only_the_last_key_that_repeats_is_held() {
        let mut repeat = Repeat::default();
        repeat.pressed(30, at(0), true);
        repeat.pressed(31, at(100), true);
        // The first released: the second repeats still.
        repeat.released(30);
        assert_eq!(repeat.deadline(), Some(at(700)));
        // A modifier pressed leaves it repeating.
        repeat.pressed(42, at(200), false);
        assert_eq!(repeat.deadline(), Some(at(700)));
        repeat.stop();
        assert_eq!(repeat.deadline(), None);
    }

    #[test]
    fn a_rate_of_zero_turns_repeating_off() {
        let mut repeat = Repeat::default();
        repeat.pressed(30, at(0), true);
        repeat.set_info(0, 300);
        assert_eq!(repeat.deadline(), None);
        repeat.pressed(30, at(10), true);
        assert_eq!(repeat.deadline(), None);
        // A rate that the protocol carries but no keyboard has: a gap of a millisecond.
        let mut repeat = Repeat::default();
        repeat.set_info(u32::MAX, 0);
        repeat.pressed(30, at(0), true);
        assert_eq!(repeat.due(at(0)), [(30, at(0))]);
        assert_eq!(repeat.deadline(), Some(at(1)));
        // The compositor's default, before it says.
        let repeat = Repeat::default();
        assert_eq!(
            repeat.pace,
            Some((Duration::from_millis(40), Duration::from_millis(600)))
        );
    }
}
