# 0248. The host's API cut in five: built where it is read, three channels and a waker, AccessKit's translation in a crate of its own

- Status: Accepted; amends [0181](0181-uwuis-scope.md) (AccessKit's translation in a crate of its
  own), [0226](0226-an-application-started-in-one-call-each-root-given-its-look.md) and
  [0227](0227-the-time-that-motion-reads.md) (what M6d.5's API carries, built where it is read),
  and [0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md) (three
  seams that M6d.9a1b keeps)
- Date: 2026-11-12

## Context

M6d.5's card ([`stage-6d.md`](../stage-6d.md#m6d5--the-hosts-api)) holds the whole of the API by
which a host drives uwui ([ADR-0181](0181-uwuis-scope.md)): the surface and its scale, input,
input methods, the lifecycle, the clipboard, frames and times
([ADR-0227](0227-the-time-that-motion-reads.md)), AccessKit's translation, the system's
preferences, insets, an application's events, a model of menus, each root's target and look
([ADR-0226](0226-an-application-started-in-one-call-each-root-given-its-look.md)); `slime-app` a
host through it alone; a table mapping each item to winit, Android, the web, UIKit and AppKit.
Stage 6d's rule lets a milestone be cut at its start.

The facts, read in the repository and in the current documentation on 2026-11-12:

- uwui has no host's type. `uwui::tree::Ui` is owned by a *presentation* (what draws into a
  window: the editor's model, the development overlay); `slime-app` borrows it through
  `Presentation::ui` and calls its methods one by one: `pointer_moved`, `key`, `ime`,
  `advance(Duration)`, `next_change`, `cursor`, `text_input`, `take_meaning_changes`. The
  presentations read the window's size and scale themselves at each frame.
- About 700 lines of `slime-app` are uwui's host: input and the clipboard (`interface.rs`),
  AccessKit's translation (`accessibility.rs`, 453 lines of code and 822 of tests), the bridge to
  `uwui-render`, the clock and the frames in `windowed.rs`. `slime-platform` translates winit 0.30.
- A key's handler answers at once whether the interface took it (`Ui::key` returns it): the web's
  `preventDefault`, Android's `onKeyDown` and the engine's games need that answer before the
  platform's handler returns.
- Android's `InputConnection` and AppKit's `NSTextInputClient` ask for the text around the caret
  and a range's rectangle synchronously, in the middle of a keystroke. Reading the clipboard is
  asynchronous on the web, with the person's permission, on Wayland, and in winit 0.31's betas;
  a paste typed on the web must let the browser fire its `paste` event.
- winit 0.31 is in beta (0.31.0-beta.4, 2026-10-24): it reshapes the pointer, input methods both
  ways, an asynchronous clipboard, surfaces apart from `resumed`. No version of winit stamps input
  events. `accesskit_winit` 0.34.2 binds winit 0.30. AccessKit has adapters for Windows, macOS,
  Unix, Android and iOS, none for the web.
- `Ui::take_meaning_changes` empties the changes in one read: AccessKit's translation and the
  agents' events ([ADR-0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md))
  would take each other's.
- Touch is outside Stage 6d ([`stage-6d.md`](../stage-6d.md#stage-6d--uwui-leaves-the-engine));
  ADR-0226 gives uwui's first stage the reader of the system's preferences, the facade and a
  look replaced; ADR-0227 gives it what reads the times: the caps, `now`, `shown_at`,
  `next_frame` and the civil time.
- Track A runs M6d.9a beside M6d.5 ([ADR-0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md)).
  M6d.9a1b moves the interface's family of the agents into `uwui`, through the same methods of
  `Ui` that AccessKit's translation calls, and designs the threads and the wake.

The design was attacked by a fresh agent before it went to the owner. Its first shape, values in
and values out once per step, could neither answer a key at once nor an input method's synchronous
question without a mirror of uwui's fields in each host; it denied the waker that ADR-0211 already
asks; it placed AccessKit's version in uwui's public API; it built items that no code reads; and it
let the two tracks collide on shared types, not only on files.

## Decision

The owner's decisions of 2026-11-12.

**M6d.5 builds what has a reader.** The host's side of the times as ADR-0227's consequences list
them; the pointer, the keyboard, input methods both ways and the clipboard; the outputs for each
target; the lifecycle, and the surface lost and made again; AccessKit's translation in uwui; the
preferences that uwui reads today, the text's size and less motion. Touch, gamepads, native menus,
the civil time, a rate asked for a surface, insets, an application's events, a look replaced while
it runs and forced colours are described in the API's ADR and checked in its table on the five
platforms, so that the API can carry them; each is built by the milestone that first reads it.
"M6d.5's API carries" in ADR-0226 and ADR-0227 means this. The reader of the system's preferences
stays uwui's first stage's.

**M6d.5 is cut in five**, each part in a session of its own:

- M6d.5a, the API's ADR and its table: every item and how it maps on each platform; a prototype
  without a window, thrown away, tries the three hard cases before that ADR is accepted: an input
  method's synchronous question, a paste as the web gives it, two targets.
- M6d.5b, the times: `Instant` and `RootTime`, each window's frame time and its source, each root's
  clock, each event's time, the next frame due and the next wake, the wake.
- M6d.5c, input and output in uwui: events with their target, time and answer, the host's
  questions, input methods both ways, copy, cut and paste as events, the outputs for each target.
- M6d.5d, AccessKit's translation in uwui, each reader of meaning following its own changes.
- M6d.5e, `slime-app` a host through the API alone: the lifecycle, the surface made again, each
  root's size and scale given by the host; rest, input to present and the editor's start measured.

**Beside track A.** M6d.5a's ADR fixes three seams before M6d.9a1b starts: one waker, the host's,
which the agents' sessions use too; meaning and actions stay in uwui's core, each reader following
its own changes; an element's capture is an item of the host's API, since it needs the host's
image. M6d.5a and M6d.5b run beside M6d.9a1a; M6d.5c beside M6d.9a1b; M6d.5d and M6d.5e after
M6d.9a1b, beside M6d.9a2. M6d.5 never edits track A's files: `slime-app`'s `src/channel.rs` and
`tests/channel.rs`, `slime-channel`, `slime-mcp`, `uwui-agent`, `slime-editor`'s `src/play.rs`.
Shared files are edited in small steps, after a fetch.

**Three channels and one callback.**

1. *Events*, given one at a time, each with its target, its time and its source. Each answers at
   once: taken or not, and what must happen inside the platform's handler, such as the text of a
   copy, which the web writes during the gesture itself. Pointer moves are gathered for each frame
   with their history, as Android's and the web's coalesced events are.
2. *Questions* that the host asks, read-only, answered from uwui's live state: the text around the
   caret, the selection and the composition; a range's rectangle and the character at a point;
   AccessKit's first whole tree; whether a point is uwui's, for decorations and drops.
3. *Outputs*, which the host reads after each event and each frame, for each target, only what
   changed: the frame due and the next wake, the cursor, the input method's state, a write to the
   clipboard or a request to read it, the accessibility update. An answer to a request comes back
   as an event: a paste carries its text.

The one callback from uwui to its host is the waker of ADR-0211, which any thread may call and
which only asks for a wake. uwui's core runs on the platform's thread of the interface.

**AccessKit's translation in a crate of its own**, working name `uwui-accesskit`, over uwui's
public meaning, depending on `accesskit`; M6d.10 names it. The platform's adapter and the window's
node stay with each host. `accesskit` moves from `slime-app` to it, `accesskit_consumer` its tests'.

## Alternatives considered

- **Every item typed in code now**, as the ADRs' "carries" reads literally: types that no code
  reads guess their readers' needs, against the project's rules and
  [ADR-0237](0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md)'s refusal
  of fields that nothing reads.
- **Everything built and delivered by the engine**, touch, gamepads through `gilrs`, native menus
  through `muda`: outside Stage 6d, two dependencies, a milestone several times larger.
- **M6d.5 after the whole of M6d.9a**: no conflict, but track B idle for five parts and the wake
  still designed twice; **no order at all**: conflicts in `app.rs` and in the changes of meaning.
- **Values in and values out once per step**: no answer to a key, input methods answered with a
  text one step late, or each host repeating uwui's rules of fields, as Flutter's embedders do.
- **A trait that the host implements and uwui calls**, as today's `Clipboard`: it cannot serve an
  asynchronous clipboard, and it calls the host in the middle of an event.
- **AccessKit's translation as a feature of `uwui`**: each version of `accesskit` would break
  uwui's API once stable, and its features are edited by track A too; **left in `slime-app`**:
  against ADR-0181, and no accessibility for applications without the engine.

## Consequences

- ADR-0181's list of what leaves with uwui holds: AccessKit's translation leaves `slime-app` for a
  crate of uwui's rather than for `uwui` itself.
- `Ui`'s methods for input, time and the clipboard change; the editor's paste through uwui's own
  shortcuts becomes a request and its answer.
- The questions left to the parts, raised by the attack on the design:
  - M6d.5a: the prototype's three cases; the field's purpose and surrounding text bounded, and
    their units, UTF-16 on Android, Apple, the web and Windows, UTF-8 in uwui.
  - M6d.5b: a host's clock that counts a suspend (`CLOCK_BOOTTIME` on Linux), a dependency put to
    the owner at its start.
  - M6d.5c: a paste pending when the focus moves, a second paste, no answer, a huge text; a hit
    test and a display list for each target; the cost of each pointer move on the lowest machine.
  - M6d.5d: late answers to a screen reader numbered, as today's guard against a tree that changed.
  - M6d.5e: a lost surface made again rather than ending the run; Windows turning touch into
    mouse events, AppKit's title bar as an inset, for the table.
- `accesskit_winit` binds winit 0.30: moving to winit 0.31 waits for it, a risk for M6d.7 and M6d.8.
