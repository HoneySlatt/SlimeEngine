# 0317. M6d.7c cut in three; Windows on windows-sys, its frames on the display's vertical blank, Direct3D 12 by default

- Status: Accepted; supersedes in part
  [0271](0271-frames-on-the-platforms-signal-drawn-just-in-time-none-at-rest.md) (Windows'
  signal of frames)
- Date: 2026-11-25

## Context

M6d.7c, "Windows" ([`stage-6d.md`](../stage-6d.md#m6d7c--windows)), is the fourth part of M6d.7
([ADR-0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)).
Its card holds a Win32 window, its messages and keyboard, DWM's signal on a thread of its own,
DXGI's statistics, the resume's notice and a covered window, its tests in CI on Windows. ADR-0271
gave Windows "a thread that waits for DWM's signal and wakes the loop, which also keeps frames while
the person drags or resizes the window".

The facts, read in the repository and in current sources on 2026-11-25:

- Wayland's layer took four sessions, X11's three
  ([ADR-0311](0311-m6d7b-cut-in-three-x11s-window-frames-and-pointer-first-on-x11rb.md)). Windows'
  shares the core (`running`, `anchor`, `fingers`, `wake`) and the host's pacing; its window, loop,
  clock, signal, pointer, keyboard and touch are each written anew. The host's tests at rest read
  Linux's `/proc` and its signals: Windows needs its own way to watch the threads and end the run.
- Win32 runs a modal loop of its own while the person moves or resizes a window, and calls the
  window's procedure from inside it; the layer must turn the host from there, which
  [ADR-0302](0302-m6d7a2b-cut-in-three-the-platform-layer-runs-the-loop-waylands-mouse-and-touch.md)'s
  seam assumed.
- `windows-sys` 0.61.2 (MIT OR Apache-2.0) is a dependency of the workspace already, for the
  engine's clock; its features add no crate to the lockfile. It declares Win32's functions,
  `dwmapi`'s and `gdi32`'s `D3DKMT` calls, not COM's interfaces. `windows` 0.62.2, which wraps COM,
  is in the lockfile through `wgpu-hal`, whose Direct3D 12 surface gives its swap chain
  (`IDXGISwapChain3`).
- How the field paces on Windows: Zed's GPUI on `DwmFlush`, on one thread; Chromium and Firefox on
  DXGI's `IDXGIOutput::WaitForVBlank` of the primary display; Chromium formerly on
  `D3DKMTWaitForVerticalBlankEvent` of the window's display, re-found as the window moved.
  Windows 11's compositor clock (`DCompositionWaitForCompositorClock`, build 22000) was backed out
  of Zed for its faults and is off by default in Chromium. Since Windows 11 22H2, DWM runs at the
  rate of the fastest display, and `DwmGetCompositionTimingInfo` with it: a window on a display at
  144 Hz beside one at 240 Hz, paced on DWM, draws at 240 (Zed's measures). Each of these waits
  returns early when the display is off.
- Under the flip model that wgpu uses, `DXGI_STATUS_OCCLUDED` never comes (Microsoft's
  documentation, and measured on Windows 11 25H2): a covered window is told by nothing but its
  geometry, which Chromium computes on a thread of its own.
- The engine opens Direct3D 12 on Windows by default (`slime-gfx`); `uwui-desktop` asks for
  `Backends::PRIMARY`, which prefers Vulkan where a driver gives it. DXGI's statistics exist only on
  a DXGI swap chain.
- GitHub's `windows-2025` runner (Windows Server 2025, build 26100) runs its jobs in an interactive
  console session at 1024 × 768, without a GPU: other projects there show a window composed by DWM
  and presented by WARP's Direct3D 12, and injected input reaching it. GitHub documents none of it.

## Decision

The owner's decisions of 2026-11-25, one at a time.

**M6d.7c is cut in three,** each part in a session of its own:

- **M6d.7c1 — Windows' window, frames and pointer.** The layer on Win32: the window and its
  requests, its scale per display, the loop and its wait, the modal loop of a move or a resize
  turning the host, the host's clock, frames on the display's vertical blank, the mouse and its
  wheel with their times, the cursor's shapes, its closing and the termination signals; the
  example at rest in CI on Windows.
- **M6d.7c2 — Windows' keyboard, focus and touch.** Its keys, layouts, dead keys and modifiers, the
  focus, touch and the pen; a test that types in the example in CI on Windows.
- **M6d.7c3 — The images' times, a covered window, and the resume.** DXGI's statistics, a covered
  window, Windows' notice of a resume through
  [ADR-0315](0315-a-resume-told-by-the-kernels-own-notice-and-given-to-the-program-as-an-action.md)'s
  door; each source put to the owner at its start.

M6d.7c's "Done when" is met at M6d.7c3.

**`windows-sys` speaks Win32,** as ADR-0270 chose, at the workspace's 0.61.2, with features that
add no crate; for `uwui-windowing`, and for `uwui-desktop`'s tests on Windows, which watch the
example's threads. Each call is in an `unsafe` block with its `// SAFETY:`.

**Windows' signal of frames is the vertical blank of the display that holds the window,** which
supersedes ADR-0271's DWM signal for Windows: a thread of the layer's own waits in
`D3DKMTWaitForVerticalBlankEvent` on that display's output, found from the window's monitor and
found again when the window moves to another, and wakes the loop. It waits only while the host
asks for the signal, so that nothing wakes at rest. Where the wait fails, or returns too soon to be
a refresh, as with the display off, a timer at the display's period takes its place for that
refresh: the period of the display's mode, else DWM's. The rest of ADR-0271's rule for Windows
holds: the signal keeps the frames while the person moves or resizes the window, and the rate is
read again when the window moves to another display and when the system reports a change of
displays.

**`uwui-desktop` opens Direct3D 12 on Windows by default,** as the engine does; `WGPU_BACKEND`
still chooses another. Its shaders compile with the DXC that lies beside the program, else FXC, as
[ADR-0066](0066-shipping-dxc-beside-the-programs.md) has the engine's do: the engine's code of that
choice passes under uwui's licence, MIT OR Apache-2.0, the owner's go of 2026-11-25, beside the code
that ADR-0270 named.

## Alternatives considered

On the cut:

- **Two parts, the keyboard with the window:** one session less, but a first part as large as
  Wayland's first two together, at risk of stopping midway.
- **Four parts, the resume alone:** a lighter third part, but a session for a window's message and
  the clocks' measure already written.

On the binding:

- **`windows`:** COM's interfaces and safer wrappers in one crate, but a heavier build, an API that
  changes at each release, and against ADR-0270's choice.
- **Declarations of the project's own:** no dependency, but layouts and calling conventions kept by
  hand, which a crate generated from Microsoft's metadata spares.

On the signal:

- **`DwmFlush`,** as Zed: the simplest, but paced at the fastest display's rate, jittery by others'
  measures, and undocumented when nothing changes on screen.
- **DXGI's `WaitForVBlank`** of the window's display: what Chromium and Firefox use today, on the
  primary display; but a COM interface, so `windows` beside `windows-sys` in the same crate.
- **Windows 11's compositor clock, `DwmFlush` on Windows 10:** follows the compositor's own rate
  and its boosts, but faults reported at a resume and when an adapter goes, off by default in
  Chromium, and paced at the fastest display too.

On the GPU's backend:

- **`Backends::PRIMARY` kept:** no change, but Vulkan on most machines with a GPU, against the
  engine's choice, without DXGI's statistics, and not what CI's WARP tests.

## Consequences

- M6d.7c1, M6d.7c2 and M6d.7c3 replace M6d.7c in [`stage-6d.md`](../stage-6d.md).
- `uwui-windowing` and `uwui-desktop`'s tests depend on `windows-sys` on Windows;
  [`dependencies.md`](../dependencies.md) records it.
- A window paces at its own display's rate on a machine of displays of several rates.
- `uwui-desktop` on Windows needs DXC beside the program, as the engine's programs do
  ([ADR-0066](0066-shipping-dxc-beside-the-programs.md)).
- Until M6d.7c2, a window on Windows takes no keys; until M6d.7c3, it tells no image's time, holds
  its frames only while minimized, and tells no resume.

## Validation and reconsideration

Read: Microsoft's documentation of `DwmFlush`, `DwmGetCompositionTimingInfo`,
`D3DKMTWaitForVerticalBlankEvent`, `DCompositionWaitForCompositorClock` and `DXGI_STATUS`; the
current sources of Zed, Chromium, Firefox, SDL and WinUI; reports of other projects on GitHub's
runners. Assumed until M6d.7c1 checks it in CI: that the runner shows the window and its vertical
blank, as others report. A machine where the vertical blank cannot be read, or a pacing measured
worse than DWM's, would justify reading DXGI's `WaitForVBlank` instead.
