# 0271. Frames on the platform's signal, drawn just in time, none at rest

- Status: Accepted; amends [0021](0021-frame-pacing.md) (frames at the display's rate, on the
  platform's signal, the schedule a cap), [0187](0187-a-hidden-window-at-rest.md) (the rule for
  uwui's desktop host and, from M6c.Q2f, the engine's windowed loop) and
  [0227](0227-the-time-that-motion-reads.md) (the time that the desktop host and the engine's
  windowed loop give their interfaces); superseded in part by
  [0309](0309-input-to-shown-measured-beside-input-to-present-in-motion-an-input-reaches-the-next-image.md) (frames drawn just in time, and the first frame after a rest)
- Date: 2026-11-15

## Context

[ADR-0021](0021-frame-pacing.md) paces the engine's frames by a schedule of its own, 60 a second
by default, presenting in Mailbox so that a hidden window never stalls the loop;
[ADR-0187](0187-a-hidden-window-at-rest.md) makes a window wait for the compositor's signal on
Wayland and hold its frames while hidden. Two debts follow: frames not in step with the display,
which judder at 144 and 165 Hz and show an image twice now and then at 60 Hz, and the editor at
60 frames a second on a display at 240 Hz, an input waiting up to 16.7 ms
([`debt.md`](../debt.md)). M6c.Q2f, which moves the editor to the display's rate, waits for this
decision; uwui's desktop host ([ADR-0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md))
needs it first.

The facts, read on 2026-11-14 and 2026-11-15:

- The platforms' signals: Wayland's frame callback, sent when the compositor can show a new image
  and not to a hidden surface (niri sends one a second); X11 Present's `PresentNotifyMSC` and
  `PresentCompleteNotify`, per CRTC; Windows' `DwmFlush`, `IDXGIOutput::WaitForVBlank` of the
  window's output, and Windows 11's compositor clock; macOS 14's `NSView.displayLink`, which
  follows the window across screens and stops for a hidden view. RandR and DWM give the display's
  period; Xvfb gives none.
- How the field paces: Chromium and Firefox a vsync thread on Windows, frame callbacks or
  `wp_presentation` on Wayland, a display link on macOS; GPUI frame callbacks, a `DwmFlush` thread,
  a display link, and a timer at RandR's rate on X11; Slint a timer at the monitor's rate on
  Windows and X11.
- Measured by ADR-0021: Fifo stalls a hidden window on niri and slows the windowed check four- to
  fourteen-fold; a free timer beats against the display. wgpu notes that Vulkan's Fifo does not
  promise to block (issue #9019). Metal offers Fifo and Immediate, no Mailbox.
- Input to present: sway's callbacks came every 4.4 ms at 240 Hz (M6c.Q2c). A frame drawn as soon
  as the callback comes leaves an input that arrives just after it waiting a whole refresh, then the
  frame's work: above one frame of 4.17 ms at the 99th percentile. The editor's look moves each
  element for 120 ms, so inputs 100 ms apart meet frames running. The engine's editor today, on
  the RX 6900 XT: 0.71 ms at the median for inputs apart, 13 ms at the 99th percentile.

## Decision

**Nothing at rest.** A host runs a frame only when its interface asks for one (`Ui::needs_frame`,
`Ui::next_frame_due`), when an event changes it, or when a wake changes what shows. Otherwise it
waits with no deadline but the interface's next. ADR-0187's holds stay: a window hidden, without
area or whose surface is remade draws nothing, and what asks from outside runs a frame after
100 ms.

**While frames run, one per refresh of the display that holds the window, on its platform's
signal:**

- Wayland: the compositor's frame callback.
- X11: Present's notices of the window's CRTC; where Present gives none, a timer at RandR's rate,
  60 a second where the display reports none, as under Xvfb.
- Windows: a thread that waits for DWM's signal and wakes the loop, which also keeps frames while
  the person drags or resizes the window.
- macOS: the view's display link.

A display of variable refresh paces at its highest rate. The rate and the signal are read again
when the window moves to another display, when its scale changes, and when the system reports a
change of displays.

**Drawn just in time.** A frame starts as late as its work allows before the compositor takes its
image: the signal's time, plus the period, less the frame's measured work and a margin. Input that
comes before the start reaches that image, in motion as at rest, and frames stay in step with the
display. The work is measured over recent frames; a frame that misses its deadline widens the
margin, and the host traces it. Where the platform gives no phase, the frame starts on the signal.
Built at M6d.7a2 on Wayland, then on each platform at its part.

**The first frame after a rest starts at once,** not on the next signal: an input at rest reaches
the screen within one frame's work.

**Presenting.** Mailbox where the surface offers it, Fifo elsewhere (ADR-0021); Metal presents in
Fifo, paced by the display link.

**The time given to the interface** is when the frame shows (ADR-0227): the platform's, predicted
from its presentations, where the host reads them (Wayland's `wp_presentation`, Present,
DXGI's statistics, the display link's `targetTimestamp`); else *on the grid*, the last signal's
time plus whole periods, as ADR-0227's fifth source, a new `FrameSource::OnGrid`.

**The engine follows at M6c.Q2f,** on winit: frames at the display's rate by default, on the
compositor's callback on Wayland and at the window's monitor's rate elsewhere;
`AppConfig::max_frame_rate` becomes a cap that a program may set, a game at 60 to save power; the
time on the grid. Until then, the engine keeps its schedule at 60 and gives each frame's start.

## Alternatives considered

- **Fifo everywhere, presenting pacing the frames:** every image shown in step, but `present`
  blocks the interface's thread, so input waits; it stalls a hidden window on niri (ADR-0021); and
  Vulkan's Fifo does not promise to block.
- **A timer at the display's rate everywhere,** as Slint on Windows and X11: simple, but no phase
  with the display, so an image is shown twice now and then and the next dropped (ADR-0021's
  measure), and on Windows the timer stops while the person drags the window.
- **Frames drawn as soon as the signal comes:** in step and simple, but an input in motion waits
  up to a refresh, which misses one frame at 240 Hz.
- **An input that draws its frame at once,** its image replacing the one waiting: near the frame's
  work, but frames leave the display's step at each input, what ADR-0187 rejected for the engine,
  more work for the GPU, and a block under Fifo.
- **Today's schedule at 60 a second:** judder at 144 and 165 Hz, and input waiting up to a frame of
  60.

## Consequences

- uwui's desktop host draws in step with each display, at its rate, and nothing at rest; its input
  in motion reaches the next image when its frame's work fits, measured at M6d.7a2 in a headless
  sway at 240 Hz.
- The debt "Frames are not synchronized with the display's refresh" closes for uwui's host part by
  part, and for the engine at M6c.Q2f, with "The editor draws at most 60 frames a second".
- A frame drawn just in time needs a good measure of its work: a heavy frame after a light one may
  miss its deadline once, shown a refresh late, until the margin widens.
- `FrameSource` gains `OnGrid`; the enum is `#[non_exhaustive]`.
