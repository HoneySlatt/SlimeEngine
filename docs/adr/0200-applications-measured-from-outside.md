# 0200. Applications measured from outside

- Status: Accepted; amends [0191](0191-input-in-sway-start-budgets-and-the-editors-rate.md) (a
  person's input given by one virtual pointer held, not by `wlrctl`) and
  [0198](0198-screens-of-reference-and-the-rivals-measured.md) (the tenth "Hello, World!", the
  sources of the ten in uwui's repository); amended by [0201](0201-the-rivals-narrowed.md) (how
  the rivals run while measured)
- Date: 2026-11-06

## Context

[ADR-0198](0198-screens-of-reference-and-the-rivals-measured.md) measures uwui's screens of
reference on the owner's programs and on their rivals. The owner's programs measure themselves
from inside: Slatty through a probe built into it, Opium through its scripts and a screenshot
mode. The rivals carry no probe, and most cannot be built with one: their frames, start, memory,
rest and power must be read from outside, the same way for all, before M6d.0c measures them.
[ADR-0199](0199-uwuis-targets-tools-and-references-narrowed.md) adds memory after a list is
scrolled and closed, the 95th percentile of frames and presentations missed.

M6d.0a found that `wlrctl`, which [ADR-0191](0191-input-in-sway-start-budgets-and-the-editors-rate.md)
chose to give a person's input in a headless sway, moved and clicked a pointer that never reached
Opium's window. Opium's own Wayland traffic, traced, shows why: sway without a screen has no
pointer of its own, and gives its seat one only while a virtual pointer exists. `wlrctl` makes one
for each command and destroys it at once; the seat gains the pointer and loses it within a
millisecond, before the window binds it, and the events go nowhere. The same race lost part of
the inputs that `scripts/input-to-present.sh` gave the editor and the game ([`debt.md`](../debt.md)).

The owner decided on 2026-11-06 where the "Hello, World!" programs live, which is the tenth, and
where frames are measured; the rest is the milestone's.

## Decision

**In a headless sway of its own,** as Slatty's and Opium's measures run: one output of
3840 × 2160 at 240 Hz and scale 1.5, the window alone on it, Xwayland on, drawn by sway's GL
renderer on the machine's GPU; the program without a session bus and behind a closed proxy.
[uwui's `scripts/measure.py`](https://github.com/HoneySlatt/uwui/blob/main/scripts/measure.py)
runs it, and speaks to the compositor through
[`scripts/wayland.py`](https://github.com/HoneySlatt/uwui/blob/main/scripts/wayland.py), a client
of the Wayland protocol written with Python's standard library alone.

**Frames are what the compositor shows.** `wayland.py` asks sway for copies of its output with
damage (wlr-screencopy): each copy waits until something on the output changed, and comes with
the time the compositor gives that frame. With the window alone on the output, each is a frame of
the program as a person sees it, Wayland's and Xwayland's alike. In motion a copy is one pixel, so
that the copies cost the compositor little; at a start, the whole output, its pixels hashed.

**A start has two figures:** from the launch to the program's first frame shown, and to its
first still frame, from which its pixels stay the same until no frame comes for 1.5 s and its
processes use at most two ticks of the processor in a second. What a probe calls the first useful
frame is the second, for a program whose last image is its useful one.

**Memory** is read from `/proc`: the program's resident size, the proportional size (PSS) summed
over its process and its children, which counts a program of several processes, as Electron and
WebKit are, without counting their shared libraries twice; and its video memory, from the GPU
clients of the kernel's DRM.

**A window of measure** reads over its seconds: the frames, the intervals between them at the
median, the 95th and 99th percentiles and the longest; the refreshes missed, an interval of n
refreshes missing n − 1, those of 100 ms or more counted as pauses; each thread's processor ticks
and every thread's wake-ups; the compositor's ticks, sway's and Xwayland's; the GPU's load and
power that amdgpu reports, read ten times a second. Rest is such a window, of 60 s.

**One virtual pointer is held** for the whole measure (wlr-virtual-pointer), moved to positions in
logical pixels: a person's pointer, buttons, wheel and touchpad's scroll, through the compositor.
`scripts/input-to-present.sh` keeps `wlrctl` until M6c.Q2f measures input to present again
([`debt.md`](../debt.md)).

**The GPU's power alone.** The processor's, from RAPL, only root reads.

**Checked against Slatty's probe,** within a margin written in
[uwui's `measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md).

**The frames in motion also on the owner's display,** once, Opium's table scrolled: Umbriel offers
the same protocols as sway, at the versions the method asks; that it copies with damage as sway
does is not yet tried. There the window stands beside others, and no start is measured.

**Ten "Hello, World!".** Beside the nine of ADR-0198, GPUI alone, as GPUI's documentation writes
it, which tells what GPUI costs apart from what gpui-kit adds. Their sources, their locks and how
each is built are in uwui's repository, `references/hello-world/`, each built in a shell of its own
that uwui's `flake.lock` pins; CI does not build them.

## Alternatives considered

- **Wayland's debug log in sway** (`WAYLAND_DEBUG=server`): every commit of every client, exact,
  but no pixels, and a compositor slowed by its own log, whose processor the measure reads.
- **A layer in the program's graphics API,** as MangoHud: frame times inside the process, of
  Vulkan and OpenGL alone, a dependency, and the time a frame is submitted, not shown.
- **Copies of the window alone** (ext-image-copy-capture): another window's changes out of the
  count, but the first frame known only once the window is listed, later than shown; sway and
  Umbriel both give output copies.
- **`wlrctl` kept, one command at a time:** events lost by a race.
- **A Wayland library for Python** (pywayland): less code to keep, but a dependency and generated
  bindings, for some ten requests.
- **The "Hello, World!" written in target directories only:** nothing to keep, but nothing frozen.
- **Frames only on the owner's display,** as uwui's quality bar wrote: the real path to the
  pixels, but the window beside others, the machine left to the measure each time.

## Consequences

- Any program is measured the same way, the market's as the owner's, without its source.
- A start counts what a person waits for, sway's own delays included: a figure larger than a
  probe's, which times a frame when it is drawn. A window that does not answer sway's first size
  at once waits for sway's transaction to time out, 200 ms; another compositor may not wait so.
- A program that never stands still, a sign of loading that turns, has no still frame.
- The copies cost the compositor: in motion at the display's rate, sway's processor and the GPU's
  load include them, measured in
  [uwui's `measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md);
  the program's own processor and the GPU's power do not move with them.
- With the window beside others, as on the owner's display, their changes are counted as frames.
- `scripts/input-to-present.sh`'s figures of M6c.Q2d counted the inputs that arrived; they are
  measured again with one pointer held, at M6c.Q2f.
- The processor's power is not measured, unless the owner gives a user its reading.
