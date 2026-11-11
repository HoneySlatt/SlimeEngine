# 0238. The series' input, its table and its jobs

- Status: Accepted; amends [0195](0195-a-series-of-headless-compositors.md) (libei's client in
  Python on libei's C library, XTest through `xdotool`, one job for each compositor)
- Date: 2026-11-11

## Context

[ADR-0195](0195-a-series-of-headless-compositors.md) put the editor and *Hunna Wood* in five
headless compositors, gave each family its input, and left to M6d.6 the dependency of libei's
client for KWin and Mutter. Neither libei's tools nor `wlrctl` drive KWin and Mutter: libei's
`ei-demo-client` sends a fixed loop, and both compositors give libei's socket on the session bus,
not where libei's tools look. What M6d.6 found on 2026-11-11, at the revision of nixpkgs that
`flake.lock` pins:

- KWin 6.7.5 gives the socket through `org.kde.KWin.EIS.RemoteDesktop.connectToEIS`, Mutter 50.4
  through a session of `org.gnome.Mutter.RemoteDesktop`, which ends when its caller leaves the bus.
  Both run headless on a session bus of their own.
- The options for libei's client: its C library, MIT, by Red Hat, its ABI stable until 2.0; the
  Rust crate `reis` 0.7.1, MIT, one maintainer, "incomplete" by its own word; the Python binding
  `snegg`, unstable and not in nixpkgs; or no libei, through Mutter's older D-Bus methods and
  KWin's `org_kde_kwin_fake_input`, both legacy.
- A virtual device made for each command and gone after it loses its keys: the compositor tells
  its clients that the seat has a keyboard, then none, before the window binds it. `wlrctl`, which
  makes one for each command, typed nothing into the editor.
- The windowed checks take 9 to 11 minutes in one Weston in CI; five compositors in one job pass
  the owner's limit of 20 minutes.
- KWin's `--scale` is lost to the output's settings that it writes at its first start, and its
  virtual output refreshes at 60 Hz, which no option nor custom mode changes.
- Xvfb runs no window manager, and gives no window the keyboard's focus.

## Decision

The owner's decisions of 2026-11-11, one question at a time.

**libei's client** is a program of the project's own in Python, on libei's C library through
`ctypes`, and on the session bus through GLib's Gio, which the development shell already has for
the screen readers' check. libei enters the series' shell alone: nothing enters `Cargo.lock`, and
nothing is compiled in the other jobs.

**X11's input** goes through XTest, by `xdotool`.

**One input program** for the series, [`scripts/compositor-input.py`](../../scripts/compositor-input.py),
which holds a keyboard and a pointer for the whole session and takes commands on a Unix socket:
wlroots' virtual pointer and keyboard for sway, spoken on the Wayland socket itself, libei for KWin
and Mutter, `xdotool` for X11. On X11 it gives the focus to the window under the pointer, as a
window manager whose focus follows the pointer would.

**One job for each compositor**, side by side, in a workflow of its own, `compositors.yml`: each
within 20 minutes, a failure named by its compositor. It runs when the platform layer, the windowed
loop, the series or `flake.lock` change, every week and on demand.

**The table is the check.** [`docs/compositors.md`](../compositors.md) holds, for each compositor,
what it gives and what the engine does there; [`scripts/compositors.sh`](../../scripts/compositors.sh)
observes each row and fails where an observation differs from its cell, or has no row. A quirk found
becomes a row, as ADR-0195 asks.

## Alternatives considered

- **`reis`, in Rust:** typed and checked by the compiler, but a crate whose author calls it
  incomplete, with one maintainer, in `Cargo.lock` and built by every job.
- **No libei:** no library, but two legacy mechanisms that their compositors may close, and Mutter's
  needs a stream of its screen for absolute motion.
- **XTest through `ctypes` on libXtst:** one program for three families, but code of the project's
  own for what `xdotool` does.
- **One job, the five in a row:** one build, but about 25 to 35 minutes with the game everywhere.
- **One build, then five jobs on its programs as an artifact:** one build, but artifacts of debug
  programs of hundreds of megabytes, and the session tests run outside cargo.
- **`wlrctl` for sway, as `scripts/input-to-present.sh` uses it:** no code, but its keys are lost.
- **A window manager in Xvfb:** decorations, minimizing and the focus as a person's X11 gives them,
  but one more dependency; left for later.

## Consequences

- The series' shell, `nix develop .#compositors`, adds 712 MB to the store beyond the default shell:
  KWin 360 MB, Mutter 203 MB, Xvfb 3 MB; libei, `xdotool` and `wayland-info` share what the default
  shell holds (`docs/measurements.md`).
- `ctypes` signatures are the project's: a mistake shows at run time, in the weekly run. The
  signatures follow libei 1.6's `libei.h`; a pointer passed to its variadic function is typed at
  each call.
- Five jobs build the programs five times; GitHub's runners are free for a public repository.
- slime-mcp's `drive` test reports, rather than fails, a minimized game that draws its frames when
  `SLIME_SERIES` names a compositor of the series; in `scripts/check.sh`, in Weston, it still fails.
- The `window` example takes `--frame-rate`, so that its frames are timed at 240 a second.
