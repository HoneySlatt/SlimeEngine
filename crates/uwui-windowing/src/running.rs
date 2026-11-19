//! The run of a platform's layer, which every layer shares: the handler called back on the
//! platform's window, and the order in which both are let go of. Part of the core, which knows no
//! platform: tested here on a fake one.
//!
//! The handler is dropped before the platform, so that what it made on the window's raw handles,
//! a GPU's surface, goes before the window, a panic's unwinding included; [`Handler::closing`] is
//! called once the window opened, however the run ends but by a panic.

use std::task::Waker;

use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use slime_input::CursorShape;
use uwui_time::Instant;

use crate::{Activation, Control, Drawn, Error, Handler, Happened, Window};

/// A platform's layer, its window open: what the loop reads and waits for, and the requests that
/// [`Window`] carries to it.
pub(crate) trait Platform {
    /// What came since the last turn.
    fn happened(&mut self) -> Result<Happened, Error>;

    /// Waits for the platform, a wake, or `deadline`, then reads what came.
    fn wait(&mut self, deadline: Option<Instant>) -> Result<(), Error>;

    /// The window's raw handles, live as long as the platform.
    fn raw_handles(&self) -> (RawDisplayHandle, RawWindowHandle);

    /// The host's clock now.
    fn now(&self) -> Instant;

    /// A waker that turns the loop from any thread.
    fn waker(&self) -> Waker;

    fn set_title(&mut self, title: &str);

    /// Whether an image must follow to apply it.
    fn set_min_size(&mut self, size: Option<[u32; 2]>) -> bool;

    fn set_fullscreen(&mut self, fullscreen: bool);

    fn activate(&mut self, activation: Activation);

    fn set_cursor(&mut self, shape: CursorShape);

    /// Asks the signal for the image presented next: its number.
    fn ask_signal(&mut self) -> u64;
}

/// Runs `handler` on the platform that `opened` gives, with how its window draws, until a turn
/// ends the run or the platform fails; hands `handler` back with how the run ended.
pub(crate) fn run<P: Platform, H: Handler>(
    opened: Result<(P, Drawn), Error>,
    handler: H,
) -> (H, Result<(), Error>) {
    let (platform, drawn) = match opened {
        Ok(opened) => opened,
        Err(error) => return (handler, Err(error)),
    };
    let mut running = Running { handler, platform };
    let ran = running.run(drawn);
    let Running { handler, platform } = running;
    drop(platform);
    (handler, ran)
}

/// The handler and the window that it draws into, in the order that they are dropped.
struct Running<H, P> {
    handler: H,
    platform: P,
}

impl<H: Handler, P: Platform> Running<H, P> {
    /// The loop, from the window's opening at `drawn` until a turn ends it or the platform fails;
    /// the handler told that the window closes in both cases.
    fn run(&mut self, drawn: Drawn) -> Result<(), Error> {
        let opened = self.handler.opened(
            &mut Window {
                platform: &mut self.platform,
            },
            drawn,
        );
        let ran = match opened {
            Ok(()) => self.turns(),
            Err(_) => Ok(()),
        };
        self.handler.closing();
        ran
    }

    fn turns(&mut self) -> Result<(), Error> {
        loop {
            let happened = self.platform.happened()?;
            let control = self.handler.turn(
                &mut Window {
                    platform: &mut self.platform,
                },
                happened,
            );
            match control {
                Control::Exit => return Ok(()),
                Control::Wait(deadline) => self.platform.wait(deadline)?,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::rc::Rc;

    use raw_window_handle::{WaylandDisplayHandle, WaylandWindowHandle};

    use super::*;
    use crate::Stopped;

    /// What happened, in order, to the handler and the platform.
    type Told = Rc<RefCell<Vec<&'static str>>>;

    /// A platform whose loop fails after `turns` turns: a run that should have ended before
    /// fails rather than turning without end, each turn's record growing.
    struct Fake {
        told: Told,
        turns: usize,
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            self.told.borrow_mut().push("platform dropped");
        }
    }

    impl Platform for Fake {
        fn happened(&mut self) -> Result<Happened, Error> {
            if self.turns == 0 {
                return Err(Error("the connection broke".to_owned()));
            }
            self.turns -= 1;
            Ok(Happened::default())
        }
        fn wait(&mut self, _: Option<Instant>) -> Result<(), Error> {
            Ok(())
        }
        fn raw_handles(&self) -> (RawDisplayHandle, RawWindowHandle) {
            let pointer = std::ptr::NonNull::<std::ffi::c_void>::dangling();
            (
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(pointer)),
                RawWindowHandle::Wayland(WaylandWindowHandle::new(pointer)),
            )
        }
        fn now(&self) -> Instant {
            Instant::from_nanos(0)
        }
        fn waker(&self) -> Waker {
            Waker::noop().clone()
        }
        fn set_title(&mut self, _: &str) {}
        fn set_min_size(&mut self, _: Option<[u32; 2]>) -> bool {
            false
        }
        fn set_fullscreen(&mut self, _: bool) {}
        fn activate(&mut self, _: Activation) {}
        fn set_cursor(&mut self, _: CursorShape) {}
        fn ask_signal(&mut self) -> u64 {
            0
        }
    }

    /// What the handler does: refuses to open, ends at its second turn, or panics in it.
    #[derive(Clone, Copy, PartialEq)]
    enum Does {
        Refuse,
        End,
        Panic,
        Run,
    }

    struct Fakes {
        told: Told,
        does: Does,
        turns: usize,
    }

    impl Drop for Fakes {
        fn drop(&mut self) {
            self.told.borrow_mut().push("handler dropped");
        }
    }

    impl Handler for Fakes {
        fn opened(&mut self, _: &mut Window<'_>, _: Drawn) -> Result<(), Stopped> {
            self.told.borrow_mut().push("opened");
            if self.does == Does::Refuse {
                return Err(Stopped);
            }
            Ok(())
        }
        fn turn(&mut self, _: &mut Window<'_>, _: Happened) -> Control {
            self.turns += 1;
            self.told.borrow_mut().push("turn");
            match (self.does, self.turns) {
                (Does::End, 2) => Control::Exit,
                (Does::Panic, 2) => panic!("the handler panics"),
                _ => Control::Wait(None),
            }
        }
        fn closing(&mut self) {
            self.told.borrow_mut().push("closing");
        }
    }

    /// Runs a handler that does `does` on a platform that fails after `turns` turns: what each
    /// told, in order, and the run's end, none for a panic.
    fn ran(does: Does, turns: usize) -> (Vec<&'static str>, Option<Result<(), String>>) {
        let told = Told::default();
        let platform = Fake {
            told: Rc::clone(&told),
            turns,
        };
        let handler = Fakes {
            told: Rc::clone(&told),
            does,
            turns: 0,
        };
        let drawn = Drawn {
            pixels: [1, 1],
            scale: 1.0,
        };
        let ended = catch_unwind(AssertUnwindSafe(|| {
            let (handler, ran) = run(Ok((platform, drawn)), handler);
            told.borrow_mut().push("handed back");
            drop(handler);
            ran.map_err(|error| error.to_string())
        }));
        let order = told.borrow().clone();
        (order, ended.ok())
    }

    #[test]
    fn the_handler_is_told_the_window_closes_before_the_platform_goes() {
        let (order, ended) = ran(Does::End, 100);
        assert_eq!(
            order,
            [
                "opened",
                "turn",
                "turn",
                "closing",
                "platform dropped",
                "handed back",
                "handler dropped"
            ]
        );
        assert_eq!(ended, Some(Ok(())));
    }

    #[test]
    fn a_platform_that_fails_closes_the_handler_first_and_says_why() {
        let (order, ended) = ran(Does::Run, 1);
        assert_eq!(
            order,
            [
                "opened",
                "turn",
                "closing",
                "platform dropped",
                "handed back",
                "handler dropped"
            ]
        );
        assert_eq!(ended, Some(Err("the connection broke".to_owned())));
    }

    #[test]
    fn a_handler_that_refuses_to_open_is_closed_without_a_turn() {
        let (order, ended) = ran(Does::Refuse, 100);
        assert_eq!(
            order,
            [
                "opened",
                "closing",
                "platform dropped",
                "handed back",
                "handler dropped"
            ]
        );
        assert_eq!(ended, Some(Ok(())));
    }

    #[test]
    fn a_panic_drops_the_handler_before_the_platform() {
        let (order, ended) = ran(Does::Panic, 100);
        assert_eq!(
            order,
            [
                "opened",
                "turn",
                "turn",
                "handler dropped",
                "platform dropped"
            ]
        );
        assert_eq!(ended, None);
    }

    #[test]
    fn a_platform_that_cannot_open_hands_the_handler_back_untouched() {
        let told = Told::default();
        let handler = Fakes {
            told: Rc::clone(&told),
            does: Does::Run,
            turns: 0,
        };
        let (handler, ran) = run::<Fake, _>(Err(Error("no display".to_owned())), handler);
        assert_eq!(
            ran.map_err(|error| error.to_string()),
            Err("no display".to_owned())
        );
        assert!(told.borrow().is_empty());
        drop(handler);
    }
}
