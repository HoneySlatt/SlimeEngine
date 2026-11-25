//! uwui's platform layer on Windows, on Win32 through `windows-sys`
//! (`docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md`,
//! `docs/adr/0317-m6d7c-cut-in-three-windows-on-windows-sys-its-frames-on-the-displays-vertical-blank-direct3d-12-by-default.md`).
//!
//! - *The window*: a top-level window of its own class, aware of each display's scale (per-monitor
//!   awareness, its second version), opened at its logical size times its display's scale; its
//!   title, its smallest size (`WM_GETMINMAXINFO`), filling its display without borders, brought to
//!   the front or flashed for attention; its closing (`WM_CLOSE`) told to the host rather than
//!   done. Its scale follows the display that holds it (`WM_DPICHANGED`), at the size that Windows
//!   suggests. The system groups the windows by their program: the application's identity is not
//!   used. A minimized window holds its signal until it shows again.
//! - *The loop*: Win32's messages, waited for with `MsgWaitForMultipleObjectsEx`; a wake, the
//!   deadline and the signal each reach the window as a message of the layer's own, so that they
//!   turn Win32's own modal loop too, while the person moves or resizes the window, or opens its
//!   menu. From that loop the layer turns the host itself, through the core's
//!   [`Turn`](crate::running::Turn): frames go on during a drag. A panic in the window's procedure
//!   is held, the modal loop ended, and the panic resumed once the layer's wait has it back.
//! - *Its frames' signal*: the vertical blank of the display that holds the window (`signal`).
//!   Windows tells no image's time to this layer yet: the host measures the period from the
//!   signals.
//! - *The pointer*: the mouse's moves, buttons and wheel (`pointer`), each with its message's time,
//!   milliseconds of the system's tick, anchored on the host's clock (`crate::anchor`); its leaving
//!   told through `TrackMouseEvent`; its shapes, the system's cursors.
//! - *The clock*: the performance counter, which counts sleep (`crate::clock`); a resume is told at
//!   the first turn after it, measured against the unbiased interrupt time.
//!
//! Every value of the window lives in [`State`], which the window's procedure reaches through the
//! window's own data, and which changes only through cells: the procedure runs within the layer's
//! calls of Win32, and within the host's own (a title set, a swap chain resized), so that no
//! borrow is held across a call that may send a message.

#![expect(unsafe_code, reason = "Win32's calls, through windows-sys")]

mod pointer;
mod signal;
mod timer;

use std::any::Any;
use std::cell::{Cell, OnceCell, RefCell};
use std::num::NonZeroIsize;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::ptr::NonNull;
use std::sync::Arc;
use std::task::Waker;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use slime_input::CursorShape;
use uwui_time::{EventTime, Instant};
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, ValidateRect,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
    SetProcessDpiAwarenessContext,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    EndMenu, FLASHW_TIMERNOFG, FLASHW_TRAY, FLASHWINFO, FlashWindowEx, GWL_EXSTYLE, GWL_STYLE,
    GWLP_USERDATA, GetClientRect, GetMessageTime, GetWindowLongPtrW, GetWindowPlacement, HCURSOR,
    HTCLIENT, HWND_TOP, IDC_ARROW, IDC_IBEAM, IDC_SIZENS, IDC_SIZEWE, LoadCursorW, MINMAXINFO, MSG,
    MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, QS_ALLINPUT,
    RegisterClassExW, SIZE_MINIMIZED, SW_SHOWDEFAULT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetCursor, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPlacement, SetWindowPos, SetWindowTextW, ShowWindow,
    TranslateMessage, UnregisterClassW, WINDOWPLACEMENT, WM_APP, WM_CANCELMODE, WM_CAPTURECHANGED,
    WM_CLOSE, WM_DPICHANGED, WM_ENTERMENULOOP, WM_ENTERSIZEMOVE, WM_ERASEBKGND, WM_EXITMENULOOP,
    WM_EXITSIZEMOVE, WM_GETMINMAXINFO, WM_NCCREATE, WM_PAINT, WM_SETCURSOR, WM_SIZE, WNDCLASSEXW,
    WS_OVERLAPPEDWINDOW, WS_POPUP,
};

use self::pointer::Held;
use self::signal::Signal;
use self::timer::{Deadlines, Poster};
use crate::anchor::Anchor;
use crate::clock::{self, Suspends};
use crate::running::{self, Turn, Waited};
use crate::wake::{self, Asks};
use crate::{Activation, Configured, Control, Drawn, Error, Happened, Input, Opening, Window};

/// The layer's own messages: another thread woke the loop, the signal came, the deadline passed.
const WAKE: u32 = WM_APP;
const SIGNAL: u32 = WM_APP + 1;
const DEADLINE: u32 = WM_APP + 2;

/// The mouse left the window, as `TrackMouseEvent` asked: `windows-sys` declares it among the
/// common controls, a feature the layer needs nothing else of.
const WM_MOUSELEAVE: u32 = 0x02A3;

/// The scale of a window at 96 dots an inch, Windows' scale of 1.
const DPI_OF_ONE: f64 = 96.0;

/// The window class's name, which the process registers once.
const CLASS: &str = "uwui window";

/// `text` as Win32's wide string, ended by a null; a null within it ends it there.
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16()
        .take_while(|unit| *unit != 0)
        .chain(std::iter::once(0))
        .collect()
}

/// The last error of the calling thread, as the layer's error.
fn failed(what: &str) -> Error {
    Error::new(what, std::io::Error::last_os_error())
}

/// What came since the host's last turn, which the window's procedure gathers.
#[derive(Default)]
struct Told {
    inputs: Vec<(EventTime, Input)>,
    configured: Option<Configured>,
    drawn: Option<Drawn>,
    close: bool,
}

/// What a window's filling its display without borders put aside, to bring it back.
#[derive(Clone, Copy)]
struct Restore {
    style: isize,
    placement: WINDOWPLACEMENT,
}

/// The threads that post the window their messages.
struct Threads {
    signal: Signal,
    deadlines: Deadlines,
}

/// The window and all that its procedure reaches.
struct State {
    window: Cell<HWND>,
    instance: HINSTANCE,
    told: RefCell<Told>,
    /// Whether something came that the host must be told: the wait ends.
    pending: Cell<bool>,
    anchor: RefCell<Anchor>,
    suspends: RefCell<Suspends>,
    asks: OnceCell<Arc<Asks>>,
    threads: OnceCell<Threads>,
    /// The display's dots an inch, 96 at a scale of 1.
    dpi: Cell<u32>,
    /// How the window's images were last told drawn.
    drawn: Cell<Drawn>,
    minimized: Cell<bool>,
    /// The smallest size asked, in logical pixels.
    min_size: Cell<Option<[u32; 2]>>,
    /// Set while the window fills its display.
    restore: Cell<Option<Restore>>,
    cursor: Cell<HCURSOR>,
    /// Whether the mouse lies in the window, its leaving followed.
    inside: Cell<bool>,
    held: RefCell<Held>,
    /// The signal asked while minimized, asked again as the window shows.
    signal_held: Cell<bool>,
    images: Cell<u64>,
    /// When the signal last came, not yet told.
    signalled: Cell<Option<Instant>>,
    deadline: Cell<Option<Instant>>,
    /// The host, lent by the layer's wait while it lasts, which a modal loop turns.
    host: Cell<Option<NonNull<dyn Turn>>>,
    /// How deep Win32's modal loops run: a move, a resize, a menu.
    modal: Cell<u32>,
    /// Whether the host's turn runs, which the procedure must not turn again.
    turning: Cell<bool>,
    /// A turn from within a modal loop ended the run.
    ended: Cell<bool>,
    /// A panic in the procedure, resumed once the layer has its thread back.
    panic: Cell<Option<Box<dyn Any + Send>>>,
}

impl State {
    fn hwnd(&self) -> HWND {
        self.window.get()
    }

    fn scale(&self) -> f64 {
        f64::from(self.dpi.get()) / DPI_OF_ONE
    }

    fn threads(&self) -> Option<&Threads> {
        self.threads.get()
    }

    /// Resumes a panic that the procedure held.
    fn resume_panic(&self) {
        if let Some(payload) = self.panic.take() {
            resume_unwind(payload);
        }
    }

    /// What came since the last turn.
    fn happened(&self) -> Happened {
        self.resume_panic();
        // Before the events read from here on are timed: their anchor is made again after a
        // suspend.
        let resumed = self.suspends.borrow_mut().resumed();
        if resumed.is_some() {
            self.anchor.borrow_mut().reset();
        }
        self.pending.set(false);
        let told = std::mem::take(&mut *self.told.borrow_mut());
        let asks = self.asks.get();
        Happened {
            inputs: told.inputs,
            configured: told.configured,
            drawn: told.drawn,
            close_asked: told.close,
            terminated: asks.is_some_and(|asks| asks.take_terminated()),
            woken: asks.is_some_and(|asks| asks.take_woken()),
            signal: self.signalled.take(),
            presentations: Vec::new(),
            exposed: false,
            resumed,
        }
    }

    /// Arms the deadline, none to disarm it.
    fn arm(&self, deadline: Option<Instant>) {
        if self.deadline.get() == deadline {
            return;
        }
        self.deadline.set(deadline);
        if let Some(threads) = self.threads() {
            threads
                .deadlines
                .arm(deadline.map(|deadline| deadline - clock::now()));
        }
    }

    fn due(&self) -> bool {
        self.deadline
            .get()
            .is_some_and(|deadline| clock::now() >= deadline)
    }

    /// The window's client area, in physical pixels.
    fn client_pixels(&self) -> [u32; 2] {
        let mut rect = RECT::default();
        // SAFETY: the window is live while the state is, and the rectangle a live one.
        unsafe { GetClientRect(self.hwnd(), &raw mut rect) };
        [rect.right - rect.left, rect.bottom - rect.top]
            .map(|side| u32::try_from(side).unwrap_or(0))
    }

    /// The window's outer size that gives a client area of `pixels`, at its display's scale.
    fn outer(&self, pixels: [u32; 2]) -> [i32; 2] {
        let [width, height] = pixels.map(|side| i32::try_from(side).unwrap_or(i32::MAX));
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        // SAFETY: the window is live; the rectangle a live one, which the call grows.
        unsafe {
            let style = GetWindowLongPtrW(self.hwnd(), GWL_STYLE) as u32;
            let extended = GetWindowLongPtrW(self.hwnd(), GWL_EXSTYLE) as u32;
            AdjustWindowRectExForDpi(&raw mut rect, style, 0, extended, self.dpi.get());
        }
        [rect.right - rect.left, rect.bottom - rect.top]
    }

    /// Logical pixels at the window's scale, in physical ones.
    fn physical(&self, logical: [u32; 2]) -> [u32; 2] {
        let scale = self.scale();
        logical.map(|side| (f64::from(side) * scale).round() as u32)
    }

    /// Tells the host the window's size, and how its images are drawn where that changed.
    fn tell_size(&self) {
        if self.minimized.get() {
            return;
        }
        let pixels = self.client_pixels().map(|side| side.max(1));
        let scale = self.scale();
        let size = pixels.map(|side| ((f64::from(side) / scale).round() as u32).max(1));
        let drawn = Drawn { pixels, scale };
        let mut told = self.told.borrow_mut();
        told.configured = Some(Configured {
            size,
            fullscreen: self.restore.get().is_some(),
        });
        if self.drawn.get() != drawn {
            self.drawn.set(drawn);
            told.drawn = Some(drawn);
        }
        self.pending.set(true);
    }

    /// Gives the host `input`, timed by its message.
    fn input(&self, input: Input) {
        // SAFETY: the call reads the calling thread's last message, the one handled.
        let millis = unsafe { GetMessageTime() } as u32;
        let at = self.anchor.borrow_mut().time(millis, clock::now());
        self.told
            .borrow_mut()
            .inputs
            .push((EventTime::anchored(at), input));
        self.pending.set(true);
    }

    /// Follows the mouse's leaving the window: told at once where it lies outside already.
    fn track_leaving(&self) {
        let mut track = TRACKMOUSEEVENT {
            cbSize: u32::try_from(size_of::<TRACKMOUSEEVENT>()).unwrap_or(u32::MAX),
            dwFlags: TME_LEAVE,
            hwndTrack: self.hwnd(),
            dwHoverTime: 0,
        };
        // SAFETY: a live structure whose size its field says, of the live window.
        unsafe { TrackMouseEvent(&raw mut track) };
    }

    /// Handles `message`; none to leave it to Windows' own handling.
    fn message(&self, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        if let Some(input) = pointer::translate(message, wparam, lparam) {
            if matches!(input, Input::PointerMoved { .. }) && !self.inside.replace(true) {
                self.track_leaving();
            }
            let capture = self.held.borrow_mut().follow(&input);
            self.input(input);
            match capture {
                // SAFETY: the live window, on its own thread.
                Some(true) => unsafe {
                    SetCapture(self.hwnd());
                },
                Some(false) => {
                    // SAFETY: a call of the window's own thread.
                    unsafe { ReleaseCapture() };
                    // A drag released outside the window is told to have left it.
                    self.track_leaving();
                }
                None => {}
            }
            // The side buttons' messages ask for TRUE.
            return Some(1);
        }
        match message {
            WM_MOUSELEAVE => {
                self.inside.set(false);
                if !self.held.borrow().any() {
                    self.input(Input::PointerLeft {
                        pointer: slime_input::Pointer::MOUSE,
                    });
                }
                Some(0)
            }
            WM_CAPTURECHANGED => {
                // Another window took the capture, as a dialog of the system: what was held is
                // let go of.
                if lparam as HWND != self.hwnd() {
                    let released = self.held.borrow_mut().release_all();
                    for input in released {
                        self.input(input);
                    }
                }
                Some(0)
            }
            WM_SETCURSOR => {
                if u32::from((lparam & 0xFFFF) as u16) == HTCLIENT {
                    // SAFETY: a cursor that the system loaded, which lives as the process does.
                    unsafe { SetCursor(self.cursor.get()) };
                    return Some(1);
                }
                None
            }
            WM_CLOSE => {
                self.told.borrow_mut().close = true;
                self.pending.set(true);
                Some(0)
            }
            WM_SIZE => {
                let minimized = wparam == SIZE_MINIMIZED as usize;
                let was = self.minimized.replace(minimized);
                if !minimized {
                    if was
                        && self.signal_held.take()
                        && let Some(threads) = self.threads()
                    {
                        threads.signal.ask();
                    }
                    self.tell_size();
                }
                Some(0)
            }
            WM_DPICHANGED => {
                self.dpi.set(u32::from((wparam >> 16) as u16));
                // SAFETY: Windows gives the rectangle that it suggests, live during the message.
                let suggested = unsafe { *(lparam as *const RECT) };
                // SAFETY: the live window, which the call moves and sizes, sending its messages.
                unsafe {
                    SetWindowPos(
                        self.hwnd(),
                        std::ptr::null_mut(),
                        suggested.left,
                        suggested.top,
                        suggested.right - suggested.left,
                        suggested.bottom - suggested.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                // Told even where the size in pixels stays the same.
                self.tell_size();
                Some(0)
            }
            WM_GETMINMAXINFO => {
                if let Some(min) = self.min_size.get() {
                    let [width, height] = self.outer(self.physical(min));
                    // SAFETY: Windows gives the structure to fill, live during the message.
                    let info = unsafe { &mut *(lparam as *mut MINMAXINFO) };
                    info.ptMinTrackSize = POINT {
                        x: width,
                        y: height,
                    };
                }
                Some(0)
            }
            WM_ENTERSIZEMOVE | WM_ENTERMENULOOP => {
                self.modal.set(self.modal.get() + 1);
                None
            }
            WM_EXITSIZEMOVE | WM_EXITMENULOOP => {
                self.modal.set(self.modal.get().saturating_sub(1));
                None
            }
            // The swap chain draws all of the window: nothing to erase or paint.
            WM_ERASEBKGND => Some(1),
            WM_PAINT => {
                // SAFETY: the live window, all of which is drawn.
                unsafe { ValidateRect(self.hwnd(), std::ptr::null()) };
                Some(0)
            }
            WAKE => {
                self.pending.set(true);
                Some(0)
            }
            SIGNAL => {
                if let Some(threads) = self.threads() {
                    self.signalled.set(Some(threads.signal.last()));
                    self.pending.set(true);
                }
                Some(0)
            }
            // The wait, or the modal loop's turn, reads the deadline.
            DEADLINE => Some(0),
            _ => None,
        }
    }

    /// Turns the host from within Win32's modal loop, where something came or the deadline passed.
    fn turn_within(&self) {
        if self.modal.get() == 0 || self.turning.get() || self.ended.get() {
            return;
        }
        let Some(mut host) = self.host.get() else {
            return;
        };
        if !self.pending.get() && !self.due() {
            return;
        }
        let happened = self.happened();
        self.turning.set(true);
        let mut view = View(self);
        // SAFETY: the host is lent by the layer's wait, which this procedure runs within, and
        // which uses it not while it lends it; no turn runs now to hold it.
        let control = unsafe { host.as_mut() }.turn(
            &mut Window {
                platform: &mut view,
            },
            happened,
        );
        self.turning.set(false);
        match control {
            Control::Wait(deadline) => self.arm(deadline),
            Control::Exit => {
                self.ended.set(true);
                self.end_modal();
            }
        }
    }

    /// Ends Win32's modal loop, a move, a resize or a menu.
    fn end_modal(&self) {
        // SAFETY: calls of the window's own thread, on the live window.
        unsafe {
            EndMenu();
            SendMessageW(self.hwnd(), WM_CANCELMODE, 0, 0);
        }
    }

    /// Fills or leaves the window's display.
    fn set_fullscreen(&self, fullscreen: bool) {
        let hwnd = self.hwnd();
        match (fullscreen, self.restore.get()) {
            (true, None) => {
                let mut placement = WINDOWPLACEMENT {
                    length: u32::try_from(size_of::<WINDOWPLACEMENT>()).unwrap_or(u32::MAX),
                    ..Default::default()
                };
                let mut monitor = MONITORINFO {
                    cbSize: u32::try_from(size_of::<MONITORINFO>()).unwrap_or(u32::MAX),
                    ..Default::default()
                };
                // SAFETY: the live window, and live structures whose sizes their fields say.
                let style = unsafe {
                    GetWindowPlacement(hwnd, &raw mut placement);
                    GetMonitorInfoW(
                        MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                        &raw mut monitor,
                    );
                    GetWindowLongPtrW(hwnd, GWL_STYLE)
                };
                // Set before the window moves, whose size it then tells filling.
                self.restore.set(Some(Restore { style, placement }));
                let area = monitor.rcMonitor;
                // SAFETY: the live window, whose style and place the calls change.
                unsafe {
                    SetWindowLongPtrW(
                        hwnd,
                        GWL_STYLE,
                        (style & !(WS_OVERLAPPEDWINDOW as isize)) | WS_POPUP as isize,
                    );
                    SetWindowPos(
                        hwnd,
                        HWND_TOP,
                        area.left,
                        area.top,
                        area.right - area.left,
                        area.bottom - area.top,
                        SWP_FRAMECHANGED | SWP_NOOWNERZORDER,
                    );
                }
            }
            (false, Some(restore)) => {
                self.restore.set(None);
                // SAFETY: the live window, given back its style and place.
                unsafe {
                    SetWindowLongPtrW(hwnd, GWL_STYLE, restore.style);
                    SetWindowPlacement(hwnd, &raw const restore.placement);
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE
                            | SWP_NOSIZE
                            | SWP_NOZORDER
                            | SWP_NOOWNERZORDER
                            | SWP_FRAMECHANGED,
                    );
                }
            }
            _ => return,
        }
        // Told even where the size stays the same.
        self.tell_size();
    }
}

/// The window's procedure: each message handled by the window's state, a panic held for the
/// layer to resume, the host turned from within a modal loop.
unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        // SAFETY: Windows gives the window's creation, whose parameter is the state that `open`
        // made and keeps live until the window goes.
        unsafe {
            let creation = &*(lparam as *const CREATESTRUCTW);
            let state = creation.lpCreateParams.cast::<State>();
            (*state).window.set(hwnd);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        }
    }
    // SAFETY: the window's data is null, or the state that `open` set, live until the window goes.
    let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State };
    // SAFETY: as above.
    let Some(state) = (unsafe { state.as_ref() }) else {
        // SAFETY: Windows' own handling of a message of the live window.
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    };
    let handled = catch_unwind(AssertUnwindSafe(|| {
        let answer = state.message(message, wparam, lparam);
        state.turn_within();
        answer
    }));
    match handled {
        Ok(Some(answer)) => answer,
        // SAFETY: Windows' own handling of a message of the live window.
        Ok(None) => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        Err(payload) => {
            if state.panic.replace(Some(payload)).is_none() {
                state.end_modal();
            }
            0
        }
    }
}

/// The window's requests and reads, on its state: the platform's own, and the view that a turn
/// from within a modal loop is given.
struct View<'a>(&'a State);

impl running::Platform for View<'_> {
    fn happened(&mut self) -> Result<Happened, Error> {
        Ok(self.0.happened())
    }

    /// Never called: the layer's own wait serves the loop, which a turn from within it never
    /// reaches.
    fn wait(&mut self, _: Option<Instant>, _: &mut dyn Turn) -> Result<Waited, Error> {
        Ok(Waited::Ready)
    }

    fn raw_handles(&self) -> (RawDisplayHandle, RawWindowHandle) {
        let state = self.0;
        let mut window = Win32WindowHandle::new(
            NonZeroIsize::new(state.hwnd() as isize)
                .expect("the window is made before its state is reached"),
        );
        window.hinstance = NonZeroIsize::new(state.instance as isize);
        (
            RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
            RawWindowHandle::Win32(window),
        )
    }

    fn now(&self) -> Instant {
        clock::now()
    }

    fn waker(&self) -> Waker {
        self.0
            .asks
            .get()
            .map_or_else(|| Waker::noop().clone(), Asks::waker)
    }

    fn set_title(&mut self, title: &str) {
        let title = wide(title);
        // SAFETY: the live window, and a wide string ended by a null, read during the call.
        unsafe { SetWindowTextW(self.0.hwnd(), title.as_ptr()) };
    }

    fn set_min_size(&mut self, size: Option<[u32; 2]>) -> bool {
        let state = self.0;
        state.min_size.set(size);
        if let Some(min) = size {
            let [width, height] = state.client_pixels();
            let [least_width, least_height] = state.physical(min);
            if width < least_width || height < least_height {
                let [outer_width, outer_height] =
                    state.outer([width.max(least_width), height.max(least_height)]);
                // SAFETY: the live window, which the call sizes, sending its messages.
                unsafe {
                    SetWindowPos(
                        state.hwnd(),
                        std::ptr::null_mut(),
                        0,
                        0,
                        outer_width,
                        outer_height,
                        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
        }
        false
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.0.set_fullscreen(fullscreen);
    }

    fn activate(&mut self, activation: Activation) {
        let hwnd = self.0.hwnd();
        // Windows may refuse the front to a program that the person did not last use: it asks
        // for attention then.
        // SAFETY: the live window.
        if activation == Activation::Front && unsafe { SetForegroundWindow(hwnd) } != 0 {
            return;
        }
        let flash = FLASHWINFO {
            cbSize: u32::try_from(size_of::<FLASHWINFO>()).unwrap_or(u32::MAX),
            hwnd,
            dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
            uCount: 0,
            dwTimeout: 0,
        };
        // SAFETY: a live structure whose size its field says, of the live window.
        unsafe { FlashWindowEx(&raw const flash) };
    }

    fn set_cursor(&mut self, shape: CursorShape) {
        let name = match shape {
            CursorShape::Text => IDC_IBEAM,
            CursorShape::ResizeHorizontal => IDC_SIZEWE,
            CursorShape::ResizeVertical => IDC_SIZENS,
            _ => IDC_ARROW,
        };
        // SAFETY: one of the system's cursors, which the call loads once and shares.
        let cursor = unsafe { LoadCursorW(std::ptr::null_mut(), name) };
        if cursor.is_null() {
            return;
        }
        self.0.cursor.set(cursor);
        if self.0.inside.get() {
            // SAFETY: a cursor of the system's, which lives as the process does.
            unsafe { SetCursor(cursor) };
        }
    }

    fn ask_signal(&mut self) -> u64 {
        let state = self.0;
        let image = state.images.get();
        state.images.set(image + 1);
        if state.minimized.get() {
            state.signal_held.set(true);
        } else if let Some(threads) = state.threads() {
            threads.signal.ask();
        }
        image
    }
}

/// The platform: the window's state, boxed where its procedure finds it.
pub(crate) struct Platform {
    state: Box<State>,
}

impl Platform {
    /// Opens the window as `opening` says, and how its images are drawn.
    pub(crate) fn open(opening: Opening) -> Result<(Self, Drawn), Error> {
        // Each display's scale, its second version; a manifest or the host may have set it first,
        // which the call then refuses, harmlessly.
        // SAFETY: a constant context that Windows defines.
        unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        // SAFETY: null names the program's own module, which lives as the process does.
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let class = wide(CLASS);
        let description = WNDCLASSEXW {
            cbSize: u32::try_from(size_of::<WNDCLASSEXW>()).unwrap_or(u32::MAX),
            lpfnWndProc: Some(procedure),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            ..Default::default()
        };
        // SAFETY: a live description whose class name lives during the call. A class registered
        // already by an earlier run in the process answers zero, and serves.
        unsafe { RegisterClassExW(&raw const description) };
        // SAFETY: the system's arrow, loaded once and shared.
        let arrow = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
        let state = Box::new(State {
            window: Cell::new(std::ptr::null_mut()),
            instance,
            told: RefCell::default(),
            pending: Cell::new(false),
            anchor: RefCell::default(),
            suspends: RefCell::new(Suspends::new()),
            asks: OnceCell::new(),
            threads: OnceCell::new(),
            dpi: Cell::new(96),
            drawn: Cell::new(Drawn {
                pixels: [1, 1],
                scale: 1.0,
            }),
            minimized: Cell::new(false),
            min_size: Cell::new(opening.min_size),
            restore: Cell::new(None),
            cursor: Cell::new(arrow),
            inside: Cell::new(false),
            held: RefCell::default(),
            signal_held: Cell::new(false),
            images: Cell::new(0),
            signalled: Cell::new(None),
            deadline: Cell::new(None),
            host: Cell::new(None),
            modal: Cell::new(0),
            turning: Cell::new(false),
            ended: Cell::new(false),
            panic: Cell::new(None),
        });
        let title = wide(&opening.title);
        // SAFETY: the class registered above, a title ended by a null, and the state, which the
        // procedure reaches from `WM_NCCREATE` on, and which outlives the window.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                (&raw const *state).cast(),
            )
        };
        state.resume_panic();
        if hwnd.is_null() {
            return Err(failed("the window could not open"));
        }
        let platform = Self { state };
        let state = &*platform.state;
        // SAFETY: the live window.
        state.dpi.set(unsafe { GetDpiForWindow(hwnd) }.max(1));
        let [width, height] = state.outer(state.physical(opening.size));
        // SAFETY: the live window, which the call sizes, sending its messages.
        unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        let window = Poster(hwnd as isize);
        let asks = Asks::new(move || window.post(WAKE, 0));
        wake::on_termination(&asks);
        let _ = state.asks.set(asks);
        let threads = Threads {
            signal: Signal::start(window, SIGNAL)
                .map_err(|cause| Error::new("the signal's thread", cause))?,
            deadlines: Deadlines::start(window, DEADLINE)
                .map_err(|cause| Error::new("the deadlines' thread", cause))?,
        };
        let _ = state.threads.set(threads);
        if opening.fullscreen {
            state.set_fullscreen(true);
        }
        // SAFETY: the live window, shown as the program's start asks.
        unsafe { ShowWindow(hwnd, SW_SHOWDEFAULT) };
        state.resume_panic();
        state.tell_size();
        let drawn = state.drawn.get();
        Ok((platform, drawn))
    }
}

impl running::Platform for Platform {
    fn happened(&mut self) -> Result<Happened, Error> {
        View(&self.state).happened()
    }

    fn wait(&mut self, deadline: Option<Instant>, host: &mut dyn Turn) -> Result<Waited, Error> {
        let state = &*self.state;
        state.arm(deadline);
        // SAFETY: the lifetime of the host is erased while it is lent; the guard takes it back
        // before this wait returns, a panic's unwinding included, and the procedure reaches it
        // only within the wait.
        let lent: NonNull<dyn Turn> = unsafe {
            std::mem::transmute::<NonNull<dyn Turn + '_>, NonNull<dyn Turn>>(NonNull::from(host))
        };
        state.host.set(Some(lent));
        let _lent = Lent(state);
        let mut message = MSG::default();
        loop {
            // SAFETY: a live message, which the calls fill, then read, of this thread's queue.
            while unsafe { PeekMessageW(&raw mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) }
                != 0
            {
                // SAFETY: the message just taken.
                unsafe {
                    TranslateMessage(&raw const message);
                    DispatchMessageW(&raw const message);
                }
                state.resume_panic();
                if state.ended.get() {
                    return Ok(Waited::Ended);
                }
            }
            if state.pending.get() || state.due() {
                return Ok(Waited::Ready);
            }
            // SAFETY: no handle: the wait ends with a message in the queue, seen or not.
            unsafe {
                MsgWaitForMultipleObjectsEx(
                    0,
                    std::ptr::null(),
                    windows_sys::Win32::System::Threading::INFINITE,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
            }
        }
    }

    fn raw_handles(&self) -> (RawDisplayHandle, RawWindowHandle) {
        View(&self.state).raw_handles()
    }

    fn now(&self) -> Instant {
        clock::now()
    }

    fn waker(&self) -> Waker {
        View(&self.state).waker()
    }

    fn set_title(&mut self, title: &str) {
        View(&self.state).set_title(title);
    }

    fn set_min_size(&mut self, size: Option<[u32; 2]>) -> bool {
        View(&self.state).set_min_size(size)
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.state.set_fullscreen(fullscreen);
    }

    fn activate(&mut self, activation: Activation) {
        View(&self.state).activate(activation);
    }

    fn set_cursor(&mut self, shape: CursorShape) {
        View(&self.state).set_cursor(shape);
    }

    fn ask_signal(&mut self) -> u64 {
        View(&self.state).ask_signal()
    }
}

/// The host lent to the window's procedure, taken back as the wait ends.
struct Lent<'a>(&'a State);

impl Drop for Lent<'_> {
    fn drop(&mut self) {
        self.0.host.set(None);
    }
}

impl Drop for Platform {
    fn drop(&mut self) {
        // The threads end first: none posts to a window that went.
        drop(self.state.threads.take());
        let hwnd = self.state.hwnd();
        let class = wide(CLASS);
        // SAFETY: the live window, which the call destroys, its procedure reaching the state,
        // live until this returns; then the class, registered by `open`.
        unsafe {
            DestroyWindow(hwnd);
            UnregisterClassW(class.as_ptr(), self.state.instance);
        }
        // A panic in the procedure during the window's end is dropped: the layer is closing.
        drop(self.state.panic.take());
    }
}
