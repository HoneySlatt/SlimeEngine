//! Waitable timers of a high resolution, and the thread that turns the loop at its deadline.
//!
//! The loop's deadline reaches the window as a message, not as a handle that the loop waits on, so
//! that it ends the wait of Win32's own modal loop too, while the person moves or resizes the
//! window, a loop that waits only for messages. The thread waits on the timer and on its own end,
//! and posts the window one message each time the timer comes due: disarmed, it never wakes.

#![expect(unsafe_code, reason = "Win32's calls, through windows-sys")]

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WPARAM};
use windows_sys::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CancelWaitableTimer, CreateEventW,
    CreateWaitableTimerExW, INFINITE, SetEvent, SetWaitableTimer, TIMER_ALL_ACCESS,
    WaitForMultipleObjects,
};
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

/// A waitable timer that resets as a wait takes it, of a high resolution where Windows gives one,
/// from Windows 10's version 1803, else of the system's tick.
pub(super) struct Timer(OwnedHandle);

impl Timer {
    pub(super) fn new() -> io::Result<Self> {
        let create = |flags| {
            // SAFETY: no name and no attributes; the call returns a new handle, or null.
            let handle = unsafe {
                CreateWaitableTimerExW(std::ptr::null(), std::ptr::null(), flags, TIMER_ALL_ACCESS)
            };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: the handle was just made, and nothing else owns it.
            Ok(Self(unsafe { OwnedHandle::from_raw_handle(handle) }))
        };
        create(CREATE_WAITABLE_TIMER_HIGH_RESOLUTION).or_else(|_| create(0))
    }

    /// Arms the timer `after` from now, none to disarm it.
    pub(super) fn arm(&self, after: Option<Duration>) {
        let handle = self.0.as_raw_handle();
        let Some(after) = after else {
            // SAFETY: the handle is a live timer of this value's.
            unsafe { CancelWaitableTimer(handle) };
            return;
        };
        // A due time below zero counts from now, in units of 100 ns; zero is due at once.
        let hundreds = i64::try_from(after.as_nanos() / 100).unwrap_or(i64::MAX);
        let due = -hundreds.max(1);
        // SAFETY: the handle is a live timer, the due time a live `i64` read during the call; no
        // period and no completion routine.
        unsafe { SetWaitableTimer(handle, &raw const due, 0, None, std::ptr::null(), 0) };
    }

    pub(super) fn handle(&self) -> &OwnedHandle {
        &self.0
    }
}

/// An event of Win32 that a wait resets as it takes it.
pub(super) fn event() -> io::Result<OwnedHandle> {
    // SAFETY: no name and no attributes; auto-reset, unset; the call returns a new handle, or null.
    let handle = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the handle was just made, and nothing else owns it.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}

/// Sets `event`, from any thread.
pub(super) fn set(event: &OwnedHandle) {
    // SAFETY: the handle is a live event.
    unsafe { SetEvent(event.as_raw_handle()) };
}

/// The window that a thread posts its messages to: its handle as a number, which any thread may
/// post to, and which outlives every thread that posts, each ended before the window goes.
#[derive(Clone, Copy)]
pub(super) struct Poster(pub(super) isize);

impl Poster {
    /// Posts the window `message`, without waiting for it.
    pub(super) fn post(self, message: u32, wparam: WPARAM) {
        // SAFETY: a window's handle, live while the thread that posts runs; a message of the
        // layer's own, which carries no pointer. A full queue loses it, as any message.
        unsafe { PostMessageW(self.0 as _, message, wparam, 0) };
    }
}

/// The thread that posts the window `message` each time its timer comes due.
pub(super) struct Deadlines {
    timer: Arc<Timer>,
    stop: Arc<OwnedHandle>,
    thread: Option<JoinHandle<()>>,
}

impl Deadlines {
    pub(super) fn start(window: Poster, message: u32) -> io::Result<Self> {
        let timer = Arc::new(Timer::new()?);
        let stop = Arc::new(event()?);
        let thread = std::thread::Builder::new()
            .name("uwui deadlines".to_owned())
            .spawn({
                let timer = Arc::clone(&timer);
                let stop = Arc::clone(&stop);
                move || {
                    let handles = [stop.as_raw_handle(), timer.handle().as_raw_handle()];
                    loop {
                        // SAFETY: two live handles, which this thread holds until it ends.
                        let woke =
                            unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) };
                        if woke != WAIT_OBJECT_0 + 1 {
                            // Its end asked, or a wait that fails, which only a handle closed
                            // under it would make.
                            return;
                        }
                        window.post(message, 0);
                    }
                }
            })?;
        Ok(Self {
            timer,
            stop,
            thread: Some(thread),
        })
    }

    /// Arms the deadline `after` from now, none to disarm it.
    pub(super) fn arm(&self, after: Option<Duration>) {
        self.timer.arm(after);
    }
}

impl Drop for Deadlines {
    fn drop(&mut self) {
        set(&self.stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
