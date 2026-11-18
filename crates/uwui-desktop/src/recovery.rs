//! How often the host makes its device and its window's surface again, and when: part of the
//! host's core, which knows no platform and reads no clock, every time given to it
//! (`docs/adr/0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md`,
//! `docs/adr/0305-uwuis-desktop-host-makes-its-device-and-surface-again-a-programs-images-made-again-from-their-source.md`).
//!
//! - A lost device is replaced at once, three times within five minutes: a device lost again at
//!   once, or soon after each new one, will not stay, and the fourth loss ends the run.
//! - A surface given up is made again a second later, or at once at an event that may change it,
//!   the window resized or its scale changed; after an attempt that failed, only after a wait that
//!   doubles, which no event hastens. The fourth surface given up or attempt failed within five
//!   minutes ends the run.
//!
//! Adapted from the engine's windowed loop (`crates/slime-app/src/windowed.rs`, its `Budget` and
//! `Remake`), under uwui's licence, its owner the author of both (ADR-0270).

use std::time::Duration;

use uwui::time::Instant;

/// How many of one kind of loss the run recovers from within [`WINDOW`]: the one past it ends the
/// run.
pub(crate) const MAX_RECOVERIES: usize = 3;

/// The time within which the run recovers from [`MAX_RECOVERIES`] losses of one kind.
pub(crate) const WINDOW: Duration = Duration::from_secs(5 * 60);

/// The first wait before a surface given up is made again; each attempt that fails doubles it.
const FIRST_REMAKE_WAIT: Duration = Duration::from_secs(1);

/// The losses of one kind that the run recovered from, within [`WINDOW`].
#[derive(Debug, Default)]
pub(crate) struct Budget {
    /// When each was spent, within `WINDOW`.
    spent: Vec<Instant>,
}

impl Budget {
    /// Spends one at `now`: whether the run may recover from it.
    pub(crate) fn spend(&mut self, now: Instant) -> bool {
        self.spent.retain(|&spent| now - spent < WINDOW);
        let left = self.spent.len() < MAX_RECOVERIES;
        self.spent.push(now);
        left
    }
}

/// What the host does about a surface given up: when it makes it again, and whether the run goes
/// on.
#[derive(Debug, Default)]
pub(crate) struct Remake {
    /// The surfaces given up and the attempts that failed.
    budget: Budget,
    /// When the surface given up is made again next; none while the surface shows images.
    next: Option<Instant>,
    /// The wait after the last attempt.
    wait: Duration,
    /// Whether an attempt failed since the surface was given up: events no longer hasten the next.
    failed: bool,
}

impl Remake {
    /// The surface was found given up at `now`: whether the run goes on. The first attempt to make
    /// it again comes [`FIRST_REMAKE_WAIT`] later.
    pub(crate) fn given_up(&mut self, now: Instant) -> bool {
        self.wait = FIRST_REMAKE_WAIT;
        self.next = Some(now + self.wait);
        self.failed = false;
        self.budget.spend(now)
    }

    /// An attempt at `now` failed: whether the run goes on. The next waits twice as long.
    pub(crate) fn failed(&mut self, now: Instant) -> bool {
        self.wait *= 2;
        self.next = Some(now + self.wait);
        self.failed = true;
        self.budget.spend(now)
    }

    /// The surface shows images again.
    pub(crate) fn made(&mut self) {
        self.next = None;
    }

    /// An event at `now` that may change the surface: the next attempt comes at once, unless one
    /// failed since the surface was given up.
    pub(crate) fn soon(&mut self, now: Instant) {
        if let Some(next) = &mut self.next
            && !self.failed
        {
            *next = (*next).min(now);
        }
    }

    /// When the surface given up is made again next; none while it shows images.
    pub(crate) fn next(&self) -> Option<Instant> {
        self.next
    }

    /// Whether an attempt is due at `now`.
    pub(crate) fn due(&self, now: Instant) -> bool {
        self.next.is_some_and(|next| next <= now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> Instant {
        Instant::from_nanos(seconds * 1_000_000_000)
    }

    #[test]
    fn the_fourth_loss_within_five_minutes_ends_the_run_and_older_ones_are_forgotten() {
        let mut budget = Budget::default();
        for second in 0..3 {
            assert!(budget.spend(at(second)), "loss {second}");
        }
        assert!(!budget.spend(at(3)), "the fourth within five minutes");
        // Five minutes after the first three, they no longer count; the fourth still does.
        let later = at(3) + (WINDOW - Duration::from_secs(1));
        assert!(budget.spend(later));
        assert!(budget.spend(later));
        assert!(!budget.spend(later), "the fourth and two since count");
    }

    #[test]
    fn a_surface_given_up_is_made_again_after_a_wait_that_an_event_hastens() {
        let mut remake = Remake::default();
        assert_eq!(remake.next(), None);
        assert!(remake.given_up(at(10)));
        assert!(!remake.due(at(10)));
        assert_eq!(remake.next(), Some(at(11)));
        assert!(remake.due(at(11)));
        // The window resized: at once.
        remake.soon(Instant::from_nanos(10_200_000_000));
        assert!(remake.due(Instant::from_nanos(10_200_000_000)));
        remake.made();
        assert_eq!(remake.next(), None);
        assert!(!remake.due(at(20)));
        // An event while it shows images changes nothing.
        remake.soon(at(20));
        assert_eq!(remake.next(), None);
    }

    #[test]
    fn after_an_attempt_that_failed_the_wait_doubles_and_no_event_hastens_it() {
        let mut remake = Remake::default();
        assert!(remake.given_up(at(0)));
        assert!(remake.failed(at(1)));
        assert_eq!(remake.next(), Some(at(3)));
        remake.soon(at(2));
        assert!(!remake.due(at(2)), "an event does not hasten it");
        assert!(remake.failed(at(3)));
        assert_eq!(remake.next(), Some(at(7)));
        assert!(!remake.failed(at(7)), "the fourth spent ends the run");
    }
}
