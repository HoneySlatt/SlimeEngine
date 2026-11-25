//! Windows' signal of frames: the vertical blank of the display that holds the window, waited for
//! on a thread of the layer's own
//! (`docs/adr/0317-m6d7c-cut-in-three-windows-on-windows-sys-its-frames-on-the-displays-vertical-blank-direct3d-12-by-default.md`).
//!
//! The thread waits for the host's ask, one at a time, then for the next vertical blank through
//! `D3DKMTWaitForVerticalBlankEvent` on that display's output, found from the window's monitor and
//! found again when the window moves to another; it notes when the blank came on the host's clock,
//! and posts the window a message, which turns the loop, Win32's modal loop of a move or a resize
//! too. Unasked, it waits on its event alone, and never wakes.
//!
//! A wait that fails, returns sooner than half a period after the last refresh, or returns at its
//! call well before a period passed, as when the display is off, is no refresh: a timer at the
//! display's period takes its place, on the grid of the last refresh. The period is the display's
//! mode, else DWM's, else 60 a second, read again when the window moves to another display and when
//! Windows tells that the displays changed (`WM_DISPLAYCHANGE`). A monitor whose output does not
//! open, or whose wait fails, is left to the timer for a second before it is opened again.

#![expect(
    unsafe_code,
    reason = "Win32's and the display driver's calls, through windows-sys"
)]

use std::io;
use std::os::windows::io::{AsRawHandle, OwnedHandle};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use uwui_time::Instant;
use windows_sys::Wdk::Graphics::Direct3D::{
    D3DKMT_CLOSEADAPTER, D3DKMT_OPENADAPTERFROMHDC, D3DKMT_WAITFORVERTICALBLANKEVENT,
    D3DKMTCloseAdapter, D3DKMTOpenAdapterFromHdc, D3DKMTWaitForVerticalBlankEvent,
};
use windows_sys::Win32::Foundation::{STATUS_SUCCESS, WAIT_OBJECT_0};
use windows_sys::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmGetCompositionTimingInfo};
use windows_sys::Win32::Graphics::Gdi::{
    CreateDCW, DEVMODEW, DeleteDC, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW, GetMonitorInfoW,
    HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFOEXW, MonitorFromWindow,
};
use windows_sys::Win32::System::Threading::{INFINITE, WaitForMultipleObjects};

use super::timer::{self, Poster, Timer};
use crate::clock;

/// The period where neither the display's mode nor DWM gives one.
const SIXTY: Duration = Duration::from_nanos(16_666_667);

/// What a wait for the vertical blank tells, judged against the last refresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Judged {
    /// A refresh came.
    Refresh,
    /// No refresh: the timer gives the next one on the grid.
    Missed,
}

/// How long a wait lasts that returned at once rather than at a blank.
const AT_ONCE: Duration = Duration::from_micros(100);

/// How long a monitor whose output did not open, or whose wait failed, is left to the timer
/// before it is opened again.
const RETRY: Duration = Duration::from_secs(1);

/// How a wait for the vertical blank went.
#[derive(Clone, Copy, Debug)]
struct Wait {
    /// Whether the driver told a blank.
    succeeded: bool,
    /// When it returned.
    returned: Instant,
    /// Whether it returned at its call, within `AT_ONCE`.
    at_once: bool,
}

/// Judges `wait`, after the `last` refresh told, none before, at the display's `period`: a refresh
/// where it succeeded and came half a period or more after the last; but not where it returned at
/// once, as the wait before did, sooner than a period after the last, as with the display off,
/// whose waits all return at their call. A blank rarely comes just as the wait is called twice in a
/// row; a refresh so taken for missed is timed a period after the last, about when it came.
fn judge(wait: Wait, at_once_before: bool, last: Option<Instant>, period: Duration) -> Judged {
    let since = last.map(|last| wait.returned - last);
    let soon = since.is_some_and(|since| since < period / 2);
    let idle = wait.at_once && at_once_before && since.is_some_and(|since| since < period);
    if wait.succeeded && !soon && !idle {
        Judged::Refresh
    } else {
        Judged::Missed
    }
}

/// Whether the thread opens the output of `monitor` now, its output open on `open_on`, if any: where
/// none is open, or one of another monitor, or the displays `changed`; but not for a monitor whose
/// output `failed` less than `RETRY` before `now`, unless the displays changed since.
fn opens(
    open_on: Option<HMONITOR>,
    monitor: HMONITOR,
    changed: bool,
    failed: Option<(HMONITOR, Instant)>,
    now: Instant,
) -> bool {
    if changed {
        return true;
    }
    if open_on == Some(monitor) {
        return false;
    }
    failed.is_none_or(|(at, since)| at != monitor || now - since >= RETRY)
}

/// The next refresh on the grid of `last`, every `period`, after `now`; `now` itself without a
/// refresh before.
fn next_on_grid(now: Instant, last: Option<Instant>, period: Duration) -> Instant {
    let Some(last) = last else {
        return now;
    };
    let period = period.max(Duration::from_micros(100));
    let since = now - last;
    let periods = since.as_nanos() / period.as_nanos() + 1;
    let ahead = u64::try_from(periods * period.as_nanos()).unwrap_or(u64::MAX);
    Instant::from_nanos(last.as_nanos().saturating_add(ahead))
}

/// What the window's thread and the signal's share.
struct Shared {
    /// Set by each ask; a wait resets it.
    asked: OwnedHandle,
    stop: AtomicBool,
    /// When the last refresh came, on the host's clock, in nanoseconds.
    at: AtomicU64,
    /// The displays changed since the thread last opened an output: it reads it again.
    changed: AtomicBool,
}

/// The thread of the signal, which the window asks before each present.
pub(super) struct Signal {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Signal {
    /// Starts the thread, which posts the window `message` at each refresh asked.
    pub(super) fn start(window: Poster, message: u32) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            asked: timer::event()?,
            stop: AtomicBool::new(false),
            at: AtomicU64::new(0),
            changed: AtomicBool::new(false),
        });
        let timer = Timer::new()?;
        let thread = std::thread::Builder::new()
            .name("uwui signal".to_owned())
            .spawn({
                let shared = Arc::clone(&shared);
                move || run(&shared, &timer, window, message)
            })?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// The displays changed, or one's mode: the output and its period are read again at the next
    /// ask.
    pub(super) fn displays_changed(&self) {
        self.shared.changed.store(true, Ordering::Release);
    }

    /// Asks for the next refresh.
    pub(super) fn ask(&self) {
        timer::set(&self.shared.asked);
    }

    /// When the last refresh told came.
    pub(super) fn last(&self) -> Instant {
        Instant::from_nanos(self.shared.at.load(Ordering::Acquire))
    }
}

impl Drop for Signal {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        timer::set(&self.shared.asked);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The thread's loop: each ask answered by the next refresh.
fn run(shared: &Shared, timer: &Timer, window: Poster, message: u32) {
    let mut output: Option<Output> = None;
    // The monitor whose output last failed, and when: left to the timer until `RETRY` passed.
    let mut failed: Option<(HMONITOR, Instant)> = None;
    let mut last: Option<Instant> = None;
    let mut at_once_before = false;
    loop {
        let handles = [shared.asked.as_raw_handle()];
        // SAFETY: a live event, which this thread's `shared` holds.
        let woke = unsafe { WaitForMultipleObjects(1, handles.as_ptr(), 0, INFINITE) };
        if woke != WAIT_OBJECT_0 || shared.stop.load(Ordering::Acquire) {
            break;
        }
        let monitor = monitor(window);
        let changed = shared.changed.swap(false, Ordering::AcqRel);
        let open_on = output.as_ref().map(|output| output.monitor);
        if opens(open_on, monitor, changed, failed, clock::now()) {
            // The adapter of the output before closes first.
            drop(output.take());
            output = Output::open(monitor);
            failed = output.is_none().then(|| (monitor, clock::now()));
        }
        let period = output
            .as_ref()
            .map_or_else(dwm_period, |output| output.period);
        let called = clock::now();
        let succeeded = output.as_ref().is_some_and(Output::wait);
        let returned = clock::now();
        let wait = Wait {
            succeeded,
            returned,
            at_once: returned - called < AT_ONCE,
        };
        let judged = judge(wait, at_once_before, last, period);
        at_once_before = wait.at_once;
        let refresh = match judged {
            Judged::Refresh => returned,
            Judged::Missed => {
                if !succeeded && output.take().is_some() {
                    // Opened again after a while: the adapter may have gone, or been reset.
                    failed = Some((monitor, returned));
                }
                let next = next_on_grid(returned, last, period);
                timer.arm(Some(next - clock::now()));
                let handles = [timer.handle().as_raw_handle()];
                // SAFETY: a live timer, which this thread holds.
                unsafe { WaitForMultipleObjects(1, handles.as_ptr(), 0, INFINITE) };
                clock::now()
            }
        };
        last = Some(refresh);
        shared.at.store(refresh.as_nanos(), Ordering::Release);
        window.post(message, 0);
    }
}

/// The monitor that holds most of the window, or the nearest.
fn monitor(window: Poster) -> HMONITOR {
    // SAFETY: a window's handle, live while this thread runs; the call answers a monitor always.
    unsafe { MonitorFromWindow(window.0 as _, MONITOR_DEFAULTTONEAREST) }
}

/// DWM's period: its composition's, the fastest display's since Windows 11 22H2.
fn dwm_period() -> Duration {
    let mut info = DWM_TIMING_INFO {
        cbSize: u32::try_from(size_of::<DWM_TIMING_INFO>()).unwrap_or(u32::MAX),
        ..Default::default()
    };
    // SAFETY: a null window, which Windows 8.1 and later ask, and a live structure of its size.
    let read = unsafe { DwmGetCompositionTimingInfo(std::ptr::null_mut(), &raw mut info) };
    if read < 0 || info.rateRefresh.uiNumerator == 0 || info.rateRefresh.uiDenominator == 0 {
        return SIXTY;
    }
    let nanos = u64::from(info.rateRefresh.uiDenominator) * 1_000_000_000
        / u64::from(info.rateRefresh.uiNumerator);
    Duration::from_nanos(nanos)
}

/// A display's output, as the display's driver names it, and its period.
struct Output {
    monitor: HMONITOR,
    adapter: u32,
    source: u32,
    period: Duration,
}

impl Output {
    /// The output of `monitor`; none where its driver does not open it, as a display driver of
    /// another model than Windows Vista's.
    fn open(monitor: HMONITOR) -> Option<Self> {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = u32::try_from(size_of::<MONITORINFOEXW>()).unwrap_or(u32::MAX);
        // SAFETY: a monitor that Windows gave, and a live structure whose size its field says,
        // the extended one, whose device's name the call writes.
        let read = unsafe { GetMonitorInfoW(monitor, (&raw mut info).cast()) };
        if read == 0 {
            return None;
        }
        let device = info.szDevice.as_ptr();
        // SAFETY: the device's name that the call wrote, ended by a null within its array.
        let context =
            unsafe { CreateDCW(std::ptr::null(), device, std::ptr::null(), std::ptr::null()) };
        if context.is_null() {
            return None;
        }
        let mut open = D3DKMT_OPENADAPTERFROMHDC {
            hDc: context,
            ..Default::default()
        };
        // SAFETY: a live device context, and a live structure that the call fills.
        let opened = unsafe { D3DKMTOpenAdapterFromHdc(&raw mut open) };
        // SAFETY: the context made above, used by nothing else.
        unsafe { DeleteDC(context) };
        if opened != STATUS_SUCCESS {
            return None;
        }
        Some(Self {
            monitor,
            adapter: open.hAdapter,
            source: open.VidPnSourceId,
            period: mode_period(device).unwrap_or_else(dwm_period),
        })
    }

    /// Waits for the output's next vertical blank: whether the driver says that it came.
    fn wait(&self) -> bool {
        let wait = D3DKMT_WAITFORVERTICALBLANKEVENT {
            hAdapter: self.adapter,
            hDevice: 0,
            VidPnSourceId: self.source,
        };
        // SAFETY: an adapter that this value opened, and a live structure read during the call.
        unsafe { D3DKMTWaitForVerticalBlankEvent(&raw const wait) == STATUS_SUCCESS }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        let close = D3DKMT_CLOSEADAPTER {
            hAdapter: self.adapter,
        };
        // SAFETY: an adapter that this value opened, closed once.
        unsafe { D3DKMTCloseAdapter(&raw const close) };
    }
}

/// The period of the display mode of the device `device`, whose name ends with a null; none where
/// its mode says no rate, or 1, the hardware's default.
fn mode_period(device: *const u16) -> Option<Duration> {
    let mut mode = DEVMODEW {
        dmSize: u16::try_from(size_of::<DEVMODEW>()).unwrap_or(u16::MAX),
        ..Default::default()
    };
    // SAFETY: a device's name ended by a null, and a live structure whose size its field says.
    let read = unsafe { EnumDisplaySettingsW(device, ENUM_CURRENT_SETTINGS, &raw mut mode) };
    (read != 0 && mode.dmDisplayFrequency > 1)
        .then(|| Duration::from_nanos(1_000_000_000 / u64::from(mode.dmDisplayFrequency)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(millis: u64) -> Instant {
        Instant::from_nanos(millis * 1_000_000)
    }

    const PERIOD: Duration = Duration::from_millis(16);

    /// A wait that blocked, or returned at once, at `returned` milliseconds.
    fn blocked(returned: u64) -> Wait {
        Wait {
            succeeded: true,
            returned: ms(returned),
            at_once: false,
        }
    }

    fn at_once(returned: u64) -> Wait {
        Wait {
            at_once: true,
            ..blocked(returned)
        }
    }

    #[test]
    fn a_wait_is_a_refresh_unless_it_failed_or_came_too_soon() {
        assert_eq!(judge(blocked(100), false, None, PERIOD), Judged::Refresh);
        assert_eq!(
            judge(blocked(116), false, Some(ms(100)), PERIOD),
            Judged::Refresh
        );
        assert_eq!(
            judge(blocked(108), false, Some(ms(100)), PERIOD),
            Judged::Refresh
        );
        // Sooner than half a period.
        assert_eq!(
            judge(blocked(107), false, Some(ms(100)), PERIOD),
            Judged::Missed
        );
        let failed = Wait {
            succeeded: false,
            ..blocked(500)
        };
        assert_eq!(judge(failed, false, Some(ms(100)), PERIOD), Judged::Missed);
        assert_eq!(judge(failed, false, None, PERIOD), Judged::Missed);
    }

    #[test]
    fn waits_that_return_at_once_in_a_row_are_no_refresh_before_a_period() {
        // The display off: every wait returns at its call, a frame's work after the last refresh.
        assert_eq!(
            judge(at_once(110), true, Some(ms(100)), PERIOD),
            Judged::Missed
        );
        assert_eq!(
            judge(at_once(115), true, Some(ms(100)), PERIOD),
            Judged::Missed
        );
        // A period after the last, it is one.
        assert_eq!(
            judge(at_once(116), true, Some(ms(100)), PERIOD),
            Judged::Refresh
        );
        // A blank that comes just as the wait is called, once, after a refresh told late: a
        // refresh.
        assert_eq!(
            judge(at_once(113), false, Some(ms(100)), PERIOD),
            Judged::Refresh
        );
        assert_eq!(judge(at_once(100), true, None, PERIOD), Judged::Refresh);
    }

    #[test]
    fn an_output_opens_for_a_new_monitor_or_changed_displays_and_a_failed_one_waits() {
        let first = std::ptr::without_provenance_mut::<std::ffi::c_void>(1);
        let second = std::ptr::without_provenance_mut::<std::ffi::c_void>(2);
        let now = ms(10_000);
        let ago = |millis: u64| Some(ms(10_000 - millis));
        // None open, or one of another monitor: it opens.
        assert!(opens(None, first, false, None, now));
        assert!(opens(Some(second), first, false, None, now));
        // Open on this monitor: it stays, unless the displays changed, a mode's rate among them.
        assert!(!opens(Some(first), first, false, None, now));
        assert!(opens(Some(first), first, true, None, now));
        // This monitor's output failed half a second ago: left to the timer, unless the displays
        // changed; a second after, opened again; another monitor's failure does not hold it.
        let failed_ago = |millis| ago(millis).map(|at| (first, at));
        assert!(!opens(None, first, false, failed_ago(500), now));
        assert!(opens(None, first, true, failed_ago(500), now));
        assert!(opens(None, first, false, failed_ago(1_000), now));
        assert!(opens(None, second, false, failed_ago(500), now));
    }

    /// The display driver's own wait, on the primary display, which the machine that runs the
    /// tests has, CI's runner included: ten blanks in a row, each told by the driver, about a
    /// period of its mode apart.
    #[test]
    fn the_primary_displays_vertical_blank_comes_at_its_period() {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint};

        // SAFETY: a point, and a flag that answers the primary display always.
        let monitor = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
        let output = Output::open(monitor).expect("the display driver opens the primary display");
        assert!(output.wait(), "the driver tells no vertical blank");
        let mut blanks = vec![clock::now()];
        for _ in 0..10 {
            assert!(output.wait(), "the driver tells no vertical blank");
            blanks.push(clock::now());
        }
        let mut gaps: Vec<Duration> = blanks.windows(2).map(|pair| pair[1] - pair[0]).collect();
        gaps.sort();
        let median = gaps[gaps.len() / 2];
        println!(
            "vertical blank: every {median:?}, the mode's period {:?}",
            output.period
        );
        assert!(
            median >= output.period / 2 && median <= output.period * 3 / 2,
            "the blanks come every {median:?}, not every {:?}: {gaps:?}",
            output.period
        );
    }

    #[test]
    fn a_missed_refresh_is_timed_on_the_grid_of_the_last() {
        assert_eq!(next_on_grid(ms(101), Some(ms(100)), PERIOD), ms(116));
        assert_eq!(next_on_grid(ms(116), Some(ms(100)), PERIOD), ms(132));
        assert_eq!(next_on_grid(ms(150), Some(ms(100)), PERIOD), ms(164));
        assert_eq!(next_on_grid(ms(150), None, PERIOD), ms(150));
    }
}
