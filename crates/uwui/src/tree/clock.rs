//! The times that the host gives the interface (`docs/adr/0227-the-time-that-motion-reads.md`): each
//! frame's and each input event's on the host's clock, each root's clock, and the waker by which
//! any thread asks the host for a step; and what the interface asks back, the next frame due.

use std::task::Waker;
use std::time::Duration;

use super::{ElementId, Ui};
use crate::time::{EventTime, FrameShown, Instant, RootClock, RootTime};

/// The clock that a motion follows: the host's, or its root's ([`RootClock`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Clock {
    Host,
    Root(ElementId),
}

/// A time on the clock that a motion follows: when it started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Moment {
    Host(Instant),
    /// A time of a root's clock, and the time that the motion had already gone before it, kept
    /// when the root's clock went back ([`Ui::set_root_clock`]).
    Root {
        root: ElementId,
        at: RootTime,
        carried: Duration,
    },
}

impl Ui {
    /// Gives the interface the time at which its next [`Ui::frame`] shows, on the host's clock,
    /// with its source: its transitions go on to it, and a tooltip, a submenu or the end of a
    /// notification whose time came shows in that frame. A host gives it before each frame.
    ///
    /// The interface's time is the latest that the host gave, of a frame or of an event
    /// ([`Ui::set_event_time`]), and never goes back: a time earlier than one given before moves
    /// nothing, and the frame tells this one as given
    /// ([`FrameStats::shown`](super::FrameStats::shown)). Until a host gives a time, the
    /// interface's time is the clock's zero: a host gives one before it adds what waits, a
    /// notification or the pointer's rest, which would otherwise wait from zero and end at the
    /// first time given.
    pub fn set_frame_shown(&mut self, shown: FrameShown) {
        self.frame_shown = shown;
        self.now = self.now.max(shown.at);
    }

    /// Gives the interface the time at which the input event that the host gives next came, on
    /// the host's clock, with its source: the pointer's rest that it starts, before a tooltip shows
    /// or a submenu opens, starts then. Never back, as [`Ui::set_frame_shown`] says.
    pub fn set_event_time(&mut self, time: EventTime) {
        self.now = self.now.max(time.at);
    }

    /// Gives `root` its clock at the next frame: the time of the root's clock that the frame shows,
    /// and its speed. A root that is never given its clock follows the host's at speed 1; once
    /// given, it holds the last clock given until the next. The layers that open over it follow
    /// its clock too. Motions are placed by the time given; the speed says only whether the clock
    /// runs: while it is 0, the motions that follow the clock hold where they are, drawn as they
    /// were, and ask for no frame.
    ///
    /// A root's first clock, which replaces the host's, and a time earlier than its last, as when
    /// a game loads a save, start its motions again from the new time, each keeping how far it had
    /// gone. A later time moves them on, as a late frame does. Nothing happens for an id that names
    /// no root; a layer's clock is never read, since a layer follows the root that it opens over.
    ///
    /// What follows a root's clock, and what follows the host's, is [`RootClock`]'s.
    ///
    /// # Panics
    ///
    /// If the speed is not a finite number, 0 or above.
    #[track_caller]
    pub fn set_root_clock(&mut self, root: ElementId, clock: RootClock) {
        assert!(
            clock.speed.is_finite() && clock.speed >= 0.0,
            "a root's speed is a finite number, 0 or above, not {}",
            clock.speed
        );
        if self.node(root).is_none_or(|node| node.root.is_none()) {
            return;
        }
        let first = self.clocks.iter().all(|(each, _)| *each != root);
        let before = self.root_time(root);
        if first || clock.time < before {
            self.re_anchor(root, before, clock.time);
        }
        match self.clocks.iter_mut().find(|(each, _)| *each == root) {
            Some((_, given)) => *given = clock,
            None => self.clocks.push((root, clock)),
        }
    }

    /// The time on the host's clock of the next frame that the interface asks for, though nothing
    /// moves meanwhile: the pointer rests where a tooltip will show or a submenu open, or a
    /// notification waits for its time to end. A program that draws only when something changes
    /// asks for a frame then; none when nothing waits. What moves asks for every frame instead
    /// ([`Ui::needs_frame`]).
    pub fn next_frame_due(&self) -> Option<Instant> {
        self.rest_remaining()
            .into_iter()
            .chain(self.notice_remaining())
            .min()
            .map(|remaining| self.now + remaining)
    }

    /// Gives the interface the host's waker, which any thread may call to ask the host for a step:
    /// the events that came, then a frame. Clones of it ask the same host.
    pub fn set_waker(&mut self, waker: Waker) {
        self.waker = waker;
    }

    /// The host's waker ([`Ui::set_waker`]); one that wakes nothing until a host gives its own.
    pub fn waker(&self) -> &Waker {
        &self.waker
    }

    /// The time that `root`'s clock shows: the last that the host gave, or the host's own for a root
    /// that it gave none.
    pub(crate) fn root_time(&self, root: ElementId) -> RootTime {
        self.clocks
            .iter()
            .find(|(each, _)| *each == root)
            .map_or(RootTime::from_nanos(self.now.as_nanos()), |(_, clock)| {
                clock.time
            })
    }

    /// The clock of the root that `id` lies under, or of the root that its layer opens over, and so
    /// on to a root that is no layer: a menu of a paused game's interface holds with the game.
    pub(crate) fn root_clock_of(&self, id: ElementId) -> Clock {
        let mut at = self.root_of(id);
        while let Some(root) = at {
            match self.node(root).and_then(|node| node.layer.as_ref()) {
                Some(held) => {
                    let opener = self.root_of(held.layer.place.anchor());
                    if opener == Some(root) {
                        break;
                    }
                    at = opener;
                }
                None => return Clock::Root(root),
            }
        }
        at.map_or(Clock::Host, Clock::Root)
    }

    /// Whether `clock` goes on: the host's always, a root's unless it is paused.
    pub(crate) fn runs(&self, clock: Clock) -> bool {
        match clock {
            Clock::Host => true,
            Clock::Root(root) => self
                .clocks
                .iter()
                .find(|(each, _)| *each == root)
                .is_none_or(|(_, given)| given.speed > 0.0),
        }
    }

    /// The time that `clock` shows now.
    pub(crate) fn moment(&self, clock: Clock) -> Moment {
        match clock {
            Clock::Host => Moment::Host(self.now),
            Clock::Root(root) => Moment::Root {
                root,
                at: self.root_time(root),
                carried: Duration::ZERO,
            },
        }
    }

    /// The time from `since` to now, on its own clock.
    pub(crate) fn elapsed(&self, since: Moment) -> Duration {
        match since {
            Moment::Host(at) => self.now - at,
            Moment::Root { root, at, carried } => carried + (self.root_time(root) - at),
        }
    }

    /// Forgets the clock of `root`, which is removed.
    pub(crate) fn forget_clock(&mut self, root: ElementId) {
        self.clocks.retain(|(each, _)| *each != root);
    }
}

impl Moment {
    /// The clock that it is a time of.
    pub(crate) fn clock(self) -> Clock {
        match self {
            Self::Host(_) => Clock::Host,
            Self::Root { root, .. } => Clock::Root(root),
        }
    }
}
