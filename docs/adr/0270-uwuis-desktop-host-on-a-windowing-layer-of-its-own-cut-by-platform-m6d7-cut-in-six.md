# 0270. uwui's desktop host on a windowing layer of its own, its application's side, its crashes; M6d.7 cut in six

- Status: Accepted; amends [0181](0181-uwuis-scope.md) (uwui's desktop host has a windowing layer
  of its own, not winit), [0227](0227-the-time-that-motion-reads.md) (the desktop host reads each
  event's time from its platform; no pull request to winit for uwui) and
  [0249](0249-the-hosts-api-its-items-and-their-table.md) (the desktop host reads what its table
  names under each platform, not winit's column); amended by
  [0279](0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md) (one windowing layer: a seam in the host now, the engine
  moved onto it at a port) and
  [0273](0273-m6d7a2-cut-in-three-waylands-keyboard-cursor-scale-and-activation.md) (M6d.7a2 cut in
  three) and
  [0306](0306-uwuis-crash-reports-a-marker-held-by-a-lock-hours-counted-a-panic-ends-with-101.md)
  (no lines of the log in a crash report while the host keeps no log)
- Date: 2026-11-15

## Context

M6d.7 builds uwui's first desktop host: an application on uwui alone, its window, frames at the
display's rate, none at rest, on every compositor of the series
([ADR-0195](0195-a-series-of-headless-compositors.md)). Its card also asks for the window's
requests, decorations, the platforms' times of frames and of input, the system's notice of a
resume, a waker, an example, [ADR-0226](0226-an-application-started-in-one-call-each-root-given-its-look.md)'s
check both ways, a CI job on macOS ([ADR-0173](0173-the-platforms-of-applications.md)), a blank
window among the programs measured ([ADR-0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md))
and crash reports kept on the machine ([ADR-0215](0215-the-engine-measured-against-the-market.md)).
The frames' rate is [ADR-0271](0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md)'s.

The facts, read on 2026-11-14 and 2026-11-15 in the repository and in current sources:

- uwui's host API ([ADR-0249](0249-the-hosts-api-its-items-and-their-table.md)) is complete
  enough for a host: events with their times, questions, outputs per target, each target's size,
  its frame's time, `Ui::frame`, `needs_frame`, `next_frame_due` and the waker. `uwui-render`
  opens no device: a host gives it one.
- winit 0.30.13 is the latest release; 0.31 has been in beta for a year (beta.4, 2026-10-24), a
  large break. No version gives an event's time (pull request #4562, open since 2026-04-25, has
  no maintainer's decision), a frame's time (#2412, open since 2022), a resume on the desktop, or
  a covered window on Windows. On Wayland, its redraws wait without bound for the compositor's
  callbacks (#4668).
- wgpu 30 gives no presentation's time; it gives the raw swapchains through `as_hal`. Mesa's
  Vulkan driver makes the surface's commits on Wayland, and reads `wp_presentation` itself, which
  it gives programs as `VK_EXT_present_timing` from Mesa 26.1; wgpu does not enable it.
- The best hosts of the field have their own layer for each platform: Zed's GPUI (Wayland through
  `wayland-client`, X11 through `x11rb`, Win32, AppKit), Chromium, Firefox. Slint is on winit.
- [ADR-0266](0266-uwui-targets-the-owners-shell-its-lock-and-login-screens-and-what-yuna-draws.md)
  gives uwui hosts that winit cannot serve: a layer shell, a session lock.
- Weston, Mutter, Muffin and GameScope give no `xdg-decoration`: a window there has the bar that it
  draws.
- uwui's authoring API, what a view is, is uwui's first stage's; uwui has one look, the editor's.
- The engine's `HostClock`, its trace and its windowed loop's holds are under the engine's licence
  ([ADR-0214](0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md)); uwui is under MIT OR
  Apache-2.0. The owner is the only author of both.
- No program of the engine writes a crash report yet: one panic hook logs
  ([ADR-0176](0176-fuzzing-and-crash-reports.md) planned the report for M6c.Q4).

## Decision

**A windowing layer of uwui's own,** the owner's choice of 2026-11-15. uwui's desktop host speaks
each platform itself: Wayland through `wayland-client`, X11 through `x11rb`, Win32 through
`windows-sys`, AppKit through `objc2`. It reads every time and every protocol the platforms give,
with nothing between, and is the base of ADR-0266's hosts. The engine keeps `slime-platform` on
winit ([ADR-0011](0011-windowing-with-winit.md)) until M6d.11, when the two part.

- No type of a platform's library appears in the host's public API.
- The host's *core* holds what is not a platform's: the pacing of frames, the rest, the clock
  given to it, wakes, termination and the application's side. It is tested without a window.
- The crate is `uwui-desktop`, a name kept until M6d.10 names uwui's crates. The facade that picks
  the host by target ([ADR-0226](0226-an-application-started-in-one-call-each-root-given-its-look.md))
  waits for a second host.

**The Wayland layer on `wayland-client` itself,** not on smithay-client-toolkit, so that the host
orders each request: the frame callback and the presentation's feedback asked just before the
commit that the Vulkan driver makes. Its loop is its own, on `rustix`'s `poll`, an `eventfd` for
the waker and a `timerfd` for deadlines, rather than `calloop`. The keyboard's maps come from
libxkbcommon, loaded at run time through `xkbcommon-dl`. Each dependency is in the lockfile at the
version that winit pulls: `wayland-client` 0.31.15, `wayland-backend` 0.3.17, `wayland-protocols`
0.32.13 (MIT), `xkbcommon-dl` 0.4.2 (MIT); with `rustix`, `ctrlc` and `pollster`, which the engine
uses. X11's, Windows' and macOS's are put to the owner at their parts.

**The application's side:** `run(look, window, app)`, which never returns. The look comes first
(ADR-0226), a value of fonts, a sheet and themes. `App` has four calls: `start`, which builds the
roots and may fail, its error reported and its exit code named; `step`, after the events and
before each frame, where the program reads and serves its actions; `close_requested`, which keeps
the window open or lets it close, as an editor with unsaved work does; and `stop`. The window
carries the requests: its title, its identity, its minimum size, fullscreen without borders,
brought to the front, attention, closing. The host ends on SIGTERM, SIGINT and SIGHUP through
`close_requested`. `App` becomes the host's side of the facade, kept when the authoring API comes.

**Each event's time from its platform:** Wayland's events' milliseconds, `zwp_input_timestamps_v1`
where given, X11's, Windows' message time and `PerformanceCount`, AppKit's `timestamp`, converted
to the host's clock as ADR-0227 says. uwui opens no pull request to winit; the engine's need of
#4562 is the engine's, at M6c.Q2f.

**Code taken from the engine,** the owner's go: the clock that counts a suspend, the trace that
`scripts/frame-pacing.py` reads, and the windowed loop's holds pass under MIT OR Apache-2.0 in the
host, each named in its commit, adapted to the host (its clock given, tested without a window).
The engine keeps its own; the two part at M6d.11.

**Crash reports: a text and a session's marker.** A panic writes a text file in the user's state
directory: its message and backtrace, the program's version, the system, the GPU and its driver,
the last lines of the log, the home directory written `~`. A *session's marker*, a file written at
start and removed at a clean end, is found by the next start when the run did not end cleanly: a
crash before the first frame, an abort, the system's killer of memory, a power cut. It counts the
hours without a crash that ADR-0215 makes a budget, and points to the system's own report of a
native crash where one is kept. Nothing leaves the machine. Minidumps written by a second process
are decided later, on figures: a watcher's wakes against rest, its memory against the blank
window, symbols kept for each build.

**M6d.7 cut in six,** by platform, each part running the example on one platform more:

- **M6d.7a1 — The host's core and its Wayland window.** The crate; its core; the Wayland layer's
  window and its requests, the compositor's decorations, frames on its signal, the pointer; the
  example; rest on Weston, sway, KWin and Mutter; ADR-0226's check both ways, and a check that
  `uwui` pulls no windowing library nor wgpu.
- **M6d.7a2 — Wayland whole, and the host measured.** The keyboard through libxkbcommon, the
  cursor, the fractional scale, activation; the presentations' and the events' times; frames drawn
  just in time (ADR-0271); the blank window; a tool that measures both in one run; input to present
  in a headless sway at 240 Hz; the crash report and the session's marker.
- **M6d.7b — X11.** `x11rb`, XKB, XInput2, Present's notices, RandR's rate; rest under Xvfb;
  Linux's notice of a resume (logind's `PrepareForSleep`).
- **M6d.7c — Windows.** A Win32 window, its messages and keyboard; DWM's signal on a thread of its
  own; DXGI's statistics; the resume's notice; a covered window, which `DXGI_STATUS_OCCLUDED`
  does not tell under the flip model that wgpu uses; checked in CI and in the Windows virtual
  machine.
- **M6d.7d — macOS.** An AppKit window, the view's display link, `mach_continuous_time`, the
  wake's notice; the CI job on macOS, its runner and scope designed at its start.
- **M6d.7e — Decorations drawn by uwui, and several windows.** A bar, its title and buttons, its
  edges where the compositor draws none, and Windows' and macOS's bars given a place in uwui's
  layout; several windows, a target removed, a layer's own target.

The card's "Done when" is met at M6d.7d. M6d.8, the clipboard, input methods and accessibility, is
cut by platform in the same way at its start.

## Alternatives considered

- **winit 0.30, with the host's own code beside it** for times, resumes and the display link: the
  lead's recommendation, the least work now. Rejected by the owner: winit's gaps worked around
  rather than closed, and ADR-0266's hosts need a layer of uwui's own anyway.
- **winit 0.31's beta:** popups, coalesced wakes, insets; still no time of an event or a frame, a
  beta, and AccessKit's adapter binds 0.30.
- **smithay-client-toolkit for Wayland:** less code; its design around `calloop` hides the order of
  requests that frames and times need, and half of winit would be made again.
- **A provisional trait replaced later, closures, or a `Host` that the program drives:** closures
  read worse once closing and stopping come; a program that writes its loop goes against
  ADR-0226's one call.
- **The engine's code written anew:** a clear provenance, but more work for the same result, and
  bugs the engine fixed come back.
- **Minidumps from a second process now** (`crash-handler`, `minidumper`, `minidump-writer`, as
  Zed): the only way to read a driver's crash, but about ten crates, a second process's memory, a
  watcher that may wake against rest, symbols for each build, and nothing before it starts.
  **Panics alone:** blind to an abort and to a driver's crash, and no hours counted.
- **M6d.7 whole, or cut in four or five around winit:** the cut followed the choice of the layer.

## Consequences

- Stage 6d grows by several sessions: M6d.7 and M6d.8 write per platform what winit gave. M6d.11
  waits for them.
- The repository holds two windowing layers until M6d.11: the engine's on winit, uwui's own.
- The desktop host reads the times that ADR-0227 asks for on each platform, and closes the debt of
  stamps taken at reception for uwui; the engine keeps it until M6c.Q2f.
- A window under Weston or Mutter has no bar until M6d.7e.
- ROADMAP's rows on windowing and on hosts of applications name uwui's own layer.
- Each part puts its own dependencies to the owner.
