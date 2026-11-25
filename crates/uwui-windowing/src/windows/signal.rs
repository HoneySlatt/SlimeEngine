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
//! A wait that fails, or returns sooner than half a period after the last refresh, as when the
//! display is off, is no refresh: a timer at the display's period takes its place, on the grid of
//! the last refresh. The period is the display's mode, else DWM's, else 60 a second.

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

/// Judges a wait that `succeeded` and returned at `returned`: a refresh where it succeeded and
/// came half a `period` or more after the `last` refresh told, or with none told before.
fn judge(succeeded: bool, returned: Instant, last: Option<Instant>, period: Duration) -> Judged {
    let soon = last.is_some_and(|last| returned - last < period / 2);
    if succeeded && !soon {
        Judged::Refresh
    } else {
        Judged::Missed
    }
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
    let mut last: Option<Instant> = None;
    loop {
        let handles = [shared.asked.as_raw_handle()];
        // SAFETY: a live event, which this thread's `shared` holds.
        let woke = unsafe { WaitForMultipleObjects(1, handles.as_ptr(), 0, INFINITE) };
        if woke != WAIT_OBJECT_0 || shared.stop.load(Ordering::Acquire) {
            break;
        }
        let monitor = monitor(window);
        if output
            .as_ref()
            .is_none_or(|output| output.monitor != monitor)
        {
            output = Output::open(monitor);
        }
        let period = output
            .as_ref()
            .map_or_else(dwm_period, |output| output.period);
        let succeeded = output.as_ref().is_some_and(Output::wait);
        let returned = clock::now();
        let refresh = match judge(succeeded, returned, last, period) {
            Judged::Refresh => returned,
            Judged::Missed => {
                if !succeeded {
                    // Opened again at the next ask: the adapter may have gone, or been reset.
                    output = None;
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

    #[test]
    fn a_wait_is_a_refresh_unless_it_failed_or_came_too_soon() {
        assert_eq!(judge(true, ms(100), None, PERIOD), Judged::Refresh);
        assert_eq!(judge(true, ms(116), Some(ms(100)), PERIOD), Judged::Refresh);
        assert_eq!(judge(true, ms(108), Some(ms(100)), PERIOD), Judged::Refresh);
        // Sooner than half a period: the display off, its wait at once.
        assert_eq!(judge(true, ms(107), Some(ms(100)), PERIOD), Judged::Missed);
        assert_eq!(judge(false, ms(500), Some(ms(100)), PERIOD), Judged::Missed);
        assert_eq!(judge(false, ms(100), None, PERIOD), Judged::Missed);
    }

    #[test]
    fn a_missed_refresh_is_timed_on_the_grid_of_the_last() {
        assert_eq!(next_on_grid(ms(101), Some(ms(100)), PERIOD), ms(116));
        assert_eq!(next_on_grid(ms(116), Some(ms(100)), PERIOD), ms(132));
        assert_eq!(next_on_grid(ms(150), Some(ms(100)), PERIOD), ms(164));
        assert_eq!(next_on_grid(ms(150), None, PERIOD), ms(150));
    }
}
