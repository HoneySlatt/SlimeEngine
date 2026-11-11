# 0195. A series of headless compositors

- Status: Accepted; amends [0185](0185-rest-checked-in-ci.md) (rest also checked on each
  compositor of the series, in a job of its own); amended by
  [0240](0240-the-series-input-its-table-and-its-jobs.md) (libei's client, XTest's tool, one job
  for each compositor)
- Date: 2026-11-06

## Context

An application on uwui must work on any compositor, not on the owner's alone. CI runs windowed
checks in a headless Weston ([ADR-0185](0185-rest-checked-in-ci.md)), and input to present in a
headless sway, through `wlrctl` ([ADR-0191](0191-input-in-sway-start-budgets-and-the-editors-rate.md)).
Compositors differ in the protocols that they give, and a program must adapt to those present or
absent without breaking or degrading.

What their current documentation and sources said on 2026-11-06:

| | Headless | Input given | Decorations drawn by the compositor | Fractional scale | Presentation times | Clipboard managers' protocol | Cursor shapes |
|---|---|---|---|---|---|---|---|
| Weston 16 | yes | none, but through a test plugin that nixpkgs does not build | no | no | yes | no | no |
| sway 1.12 (wlroots) | yes | virtual pointers and keyboards | yes | yes | yes | yes | yes |
| KWin 6.8 (`--virtual`) | yes, with a session bus | libei, through D-Bus | yes | yes | yes | `ext` alone | yes |
| Mutter 51 (`--headless --virtual-monitor`) | yes, with a session bus | libei or its remote desktop, through D-Bus | no | yes, unconfirmed on a virtual monitor | yes | no | yes |
| Xvfb (X11) | yes | XTest | — | — | — | — | — |

- No headless compositor emulates a variable refresh rate. Xvfb reports no refresh rate.
- Neither libei's tools nor `wlrctl` drive KWin and Mutter: they need a small client.
- The engine's clipboard on Wayland uses the protocol of the clipboard's managers, and falls back
  on X11 through XWayland elsewhere ([`debt.md`](../debt.md)): on Mutter or Weston without
  XWayland, it has none.
- Beyond the development shell's Weston, sway, XWayland and Mesa, nixpkgs' KWin 6.7.5 adds 1 029 MB,
  310 MB downloaded, and Mutter 50.4 adds 320 MB, 90 MB downloaded; Xvfb, 2 MB. CI's jobs already
  come close to their 20 minutes.

## Decision

The owner's decision of 2026-11-06.

**The series.** The same tests run on headless Weston, sway, KWin and Mutter, and on X11 through
Xvfb, each the reference of a family; Windows stays checked by hand in its virtual machine.
Nothing in them is specific to the owner's compositor.

**What they check.** Rest, an agent's session, and how a program adapts: decorations drawn by the
compositor or by the program, the fractional scale, frames on the compositor's signal and
presentation times, the clipboard, the cursor's shapes. `wayland-info` lists each compositor's
protocols with the results. Variable refresh rates stay checked by hand on the owner's displays.

**Input for each family:** wlroots' virtual pointers and keyboards; libei for KWin and Mutter, by
a small tool of the project's own, whose dependency is put to the owner at M6d.6; XTest for X11.
Headless Weston takes no input: the tests that need it do not run there, and its report says so.

**A job of its own** in CI, run when the desktop host or the platform layer changes, every week,
and on demand, in a development shell of its own, so that the default shell does not grow. The
existing jobs are unchanged.

**When.** Milestone M6d.6, before the desktop host: the series runs first on the editor and *Hunna
Wood*, so that the desktop host's ADR, at M6d.7, starts from what differs. The desktop host then
runs in it. After the move, uwui keeps the series for its host; how the engine keeps one for its
platform layer is decided at M6d.10.

## Alternatives considered

- **Every compositor in the existing jobs:** each regression seen at once, but jobs past their
  20 minutes.
- **KWin and Mutter checked by hand:** no cost in CI, but KDE and GNOME installed on the owner's
  machine, and regressions seen late.
- **Weston and sway alone:** no new compositor, but neither GNOME's, which draws no decorations
  and gives no clipboard managers' protocol, nor KDE's.

## Consequences

- A regression specific to KWin or Mutter in a change that the job's paths do not watch waits at
  most a week.
- The clipboard's design is decided again with the desktop host, across the series.
- Each compositor's quirks found become tests, or debt.
