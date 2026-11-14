//! The times that the host gives the interface (`docs/adr/0227-the-time-that-motion-reads.md`): each
//! target's frames' and each input event's on the host's clock, each root's clock, and the waker
//! by which any thread asks the host for a step; and what the interface asks back of each target,
//! the next frame due and whether it holds a change to draw
//! (`docs/adr/0258-each-target-its-frame-and-its-time-a-change-asks-for-its-targets-frame.md`).

use std::task::Waker;
use std::time::Duration;

use super::{ElementId, Ui};
use crate::host::TargetId;
use crate::time::{FrameShown, Instant, RootClock, RootTime};

/// What the interface keeps of a target's frames: the time given for the next, and whether the
/// target holds a change that a frame would draw. Kept apart from the target's input, so that a time
/// given before the target's first root is kept.
#[derive(Clone, Copy, Debug)]
pub(super) struct TargetFrame {
    target: TargetId,
    /// The time last given for its frame, as given; none until the host gives one.
    shown: Option<FrameShown>,
    /// The latest time given for its frames, which never goes back: what its motions read.
    at: Instant,
    /// Whether it holds a change that its next frame would draw.
    changed: bool,
}

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
    /// Gives the interface the time at which `target`'s next [`Ui::frame`] shows, on the host's
    /// clock, with its source: that target's transitions and waiting bars go on to it, and a
    /// tooltip, a submenu or the end of a notification whose time came shows in that frame. A host
    /// gives it before each frame of each target, a target's time kept even before its first root.
    ///
    /// A target's time never goes back: a time earlier than one given before for it moves nothing,
    /// and the frame tells this one as given ([`FrameStats::shown`](super::FrameStats::shown)). A
    /// target given no time yet reads the interface's time.
    ///
    /// The interface's time is the latest that the host gave, of any target's frame or of an event
    /// ([`Event::time`](crate::host::Event::time)), and never goes back: what waits, the pointer's
    /// rest and the notifications, counts on it. Until a host gives a time, it is the clock's zero:
    /// a host gives one before it adds what waits, which would otherwise wait from zero and end at
    /// the first time given.
    pub fn set_frame_shown(&mut self, target: TargetId, shown: FrameShown) {
        let record = self.frame_record(target);
        record.at = match record.shown {
            Some(_) => record.at.max(shown.at),
            None => shown.at,
        };
        record.shown = Some(shown);
        self.now = self.now.max(shown.at);
    }

    /// `target`'s record of its frames, made if it had none.
    fn frame_record(&mut self, target: TargetId) -> &mut TargetFrame {
        let at = match self.frames.iter().position(|each| each.target == target) {
            Some(at) => at,
            None => {
                self.frames.push(TargetFrame {
                    target,
                    shown: None,
                    at: Instant::ZERO,
                    changed: false,
                });
                self.frames.len() - 1
            }
        };
        &mut self.frames[at]
    }

    /// The time that `target`'s frames show, which its motions read: the latest given for it, or
    /// the interface's own for a target given none.
    pub(crate) fn frame_time(&self, target: TargetId) -> Instant {
        self.frames
            .iter()
            .find(|each| each.target == target && each.shown.is_some())
            .map_or(self.now, |each| each.at)
    }

    /// The time last given for `target`'s frame, as given; the default for a target given none.
    pub(crate) fn frame_shown(&self, target: TargetId) -> FrameShown {
        self.frames
            .iter()
            .find(|each| each.target == target)
            .and_then(|each| each.shown)
            .unwrap_or_default()
    }

    /// Notes that `target` holds a change that its next frame would draw.
    pub(crate) fn changed_on(&mut self, target: TargetId) {
        match self.frames.iter_mut().find(|each| each.target == target) {
            Some(record) => record.changed = true,
            None => self.frame_record(target).changed = true,
        }
    }

    /// Whether `target` holds a change that its next frame would draw.
    pub(crate) fn holds_change(&self, target: TargetId) -> bool {
        self.frames
            .iter()
            .any(|each| each.target == target && each.changed)
    }

    /// Notes that `target`'s frame drew what it held.
    pub(crate) fn drew(&mut self, target: TargetId) {
        if let Some(record) = self.frames.iter_mut().find(|each| each.target == target) {
            record.changed = false;
        }
    }

    /// Gives `root` its clock at the next frame: the time of the root's clock that the frame shows,
    /// and its speed. A root that is never given its clock follows the host's at speed 1, read at
    /// its target's frame's time ([`Ui::set_frame_shown`]); once
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

    /// The time on the host's clock of the next frame of `target` that the interface asks for,
    /// though nothing moves meanwhile: its pointer rests where a tooltip will show or a submenu
    /// open, or, on [`TargetId::MAIN`], a notification waits for its time to end. A program that
    /// draws only when something changes asks for a frame then; none when nothing waits. What moves
    /// asks for every frame instead ([`Ui::needs_frame`]).
    pub fn next_frame_due(&self, target: TargetId) -> Option<Instant> {
        let notices = if target == TargetId::MAIN {
            self.notice_remaining()
        } else {
            None
        };
        self.rest_remaining(target)
            .into_iter()
            .chain(notices)
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

    /// The time that `root`'s clock shows: the last that the host gave, or, for a root that it gave
    /// none, the time of its target's frames.
    pub(crate) fn root_time(&self, root: ElementId) -> RootTime {
        self.clocks
            .iter()
            .find(|(each, _)| *each == root)
            .map_or_else(
                || {
                    let target = self.target_of(root).unwrap_or(TargetId::MAIN);
                    RootTime::from_nanos(self.frame_time(target).as_nanos())
                },
                |(_, clock)| clock.time,
            )
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

    /// The time that `clock` shows now, for a motion of `target`'s: the host's clock read at that
    /// target's frame's time.
    pub(crate) fn moment(&self, clock: Clock, target: TargetId) -> Moment {
        match clock {
            Clock::Host => Moment::Host(self.frame_time(target)),
            Clock::Root(root) => Moment::Root {
                root,
                at: self.root_time(root),
                carried: Duration::ZERO,
            },
        }
    }

    /// The time from `since` to now, on its own clock, for a motion of `target`'s.
    pub(crate) fn elapsed(&self, since: Moment, target: TargetId) -> Duration {
        match since {
            Moment::Host(at) => self.frame_time(target) - at,
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
