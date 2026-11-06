# 0193. First measures of resources

- Status: Accepted; amends [0175](0175-the-quality-bar.md) (the budgets of the editor's memory
  and video memory, and of a game's power at 60 frames a second; how memory and power are
  measured)
- Date: 2026-11-06

## Context

[ADR-0175](0175-the-quality-bar.md) set two rules that nothing measured: the editor at rest adds
no power to the GPU's beyond the desktop's; memory does not grow over 100 undos and redos and 10
runs of Play. The editor's memory and video memory on the sample's zone, and the power of a game
capped at 60 frames a second, were to be measured first, their figures proposed after.
[ADR-0189](0189-first-measures-of-time.md) put these measures in M6c.Q2e, with `heaptrack`.

At the milestone's start, nothing read a program's memory or the GPU's power. The resident size
that the system counts holds what the allocator keeps after the program freed it, and the program
and its libraries mapped from their files; it cannot tell a leak from memory kept. Video memory
is the GPU's driver's to count.

Measured on 2026-11-06 on the RX 6900 XT, in a headless Weston at 3840 × 2160 through its GL
renderer ([`measurements.md`](../measurements.md#m6cq2e-ryzen-7-5800x-and-rx-6900-xt)): the editor
on the zone at rest, 124 to 173 MiB resident, 986 MiB of video memory asked of the driver; no
growth over 100 undos and redos nor over 10 runs of Play, once wgpu's pool of command buffers has
grown to its most; *Hunna Wood* at 60 frames a second, the desktop at rest, 37.6 W above the
desktop's 34.0 W.

The owner decided on 2026-11-06, one question at a time, the three figures.

## Decision

**Memory measured through the MCP server.** `crates/slime-mcp/tests/resources.rs`, a measure
without a harness that no check runs, drives `hunnawood-editor` as an agent does, in a window, on
a copy of the zone. It reads at each phase, once the editor is at rest
([ADR-0185](0185-rest-checked-in-ci.md)'s helper):

- the resident size and its parts, anonymous, of files and shared, from `/proc/<pid>/status`;
- the video memory, from `/proc/<pid>/fdinfo`, as the kernel's DRM counts it for each client of a
  device: what the process asked of the driver (`drm-total-vram`), what of it is resident, and the
  system's memory that the GPU maps (`drm-resident-gtt`).

It reads them with the zone drawn whole; after a step and 300 undos and redos, which let wgpu's
pool of command buffers grow to its most, or nearly; after 100 more; after a Play, which builds the game;
after 10 more. Then it starts *Hunna Wood* at its 60 frames a second, and reads its own.

**`heaptrack` tells the heap apart.** `scripts/resources.sh --heaptrack` runs the editor under
`heaptrack`, and prints its heap at the end of each phase. Memory that the allocator keeps, which
the resident size counts, is not the program's.

**What *grows* means.** The heap grows when it rises with the count of undos and redos or of
Plays: after the budget's series, it stands above its figure before them by more than it varies
from one phase to the next, and a longer series, `--undos 1000` or `--plays 100`, rises further.
A pool that fills once and holds, as wgpu's command buffers, does not grow.

**Power read from the GPU's driver.** The amdgpu driver's sensor of the GPU's power, its average
over the last second (`power1_average`, in `/sys/class/hwmon`), read every 250 ms for 30 s, without
privilege: before anything starts, with the editor at rest, with the game running, and after both
ended. A phase's power is compared with the desktop's at rest, before and after.

**Where it runs.** `scripts/resources.sh` runs the measure in a headless Weston of its own, its
output 3840 × 2160 at scale 3, the high tier's display: the game's window, 1280 × 720 logical
pixels, fills it, and the editor's, 1280 × 800, is as wide. With `--gpu`, on the machine's GPU
through Weston's GL renderer, on a machine at rest. Nothing reaches the desktop.

**The editor's memory** on the sample's zone, resident: at most 256 MiB on the high tier.

**The editor's video memory** on the sample's zone, asked of the driver, its window at 3840 pixels
wide: at most 1.5 GiB on the high tier.

**A game's power** at 60 frames a second: the sample's zone at 3840 × 2160 adds at most 50 W to the
GPU's power at the desktop's rest, on the RX 6900 XT. The other tiers' machines are not measured.

## Alternatives considered

- **The resident size alone**: no tool, but the allocator's pages kept after a Play read as
  growth, and a heap that grows within pages already resident does not show.
- **Valgrind's Massif**: the heap too, but the program runs on an emulated CPU, tens of times
  slower, too slow for a GPU's driver.
- **A benchmark of `slime-benches`**, as Play's time to control is measured: no MCP server between,
  but the measure is an agent's session, undo, redo and Play, which the server's tools already
  drive, and a run under `heaptrack` needs a command that a tool starts.
- **Memory checked in CI**: lavapipe opens no GPU of the kernel, so no video memory; the resident
  size varies with the runner; a check of the heap would need `heaptrack` in CI, which slows the
  editor. Kept by hand, as [ADR-0175](0175-the-quality-bar.md) decides.
- **The video memory resident** rather than asked: what the GPU holds now, but the driver may
  evict to the system's memory what a program asked for; what a program asks is what the card must
  find.
- **Budgets of 192 MiB and 512 MiB of memory, of 1.25 GiB and of the game's video memory plus
  256 MiB, of 40 W and 75 W**: the owner chose figures met today with room for the editor's next
  panels, an inspector, an asset browser and its thumbnails, and a second viewport, and that a
  doubling would pass.

## Consequences

- Memory, the editor's video memory and power are measured by hand, on the owner's machine, at
  each milestone that touches them.
- wgpu's pool of command buffers grows to its most as frames follow each other fast, and never
  shrinks: kept, not a leak.
- The editor's power at rest is judged against the sensor's resolution, a watt.
- The power read counts the compositor's work, here Weston's, as a desktop's would.
- A tier's machine other than the RX 6900 XT is not measured: debt
  ([`debt.md`](../debt.md)).
- The development shell gains `heaptrack`, which every Linux job of CI downloads.
