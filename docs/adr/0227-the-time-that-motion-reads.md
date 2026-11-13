# 0227. The time that motion reads

- Status: Accepted; amends [0009](0009-frame-timing.md) and [0107](0107-the-execution-model.md)
  (uwui's interface reads the host's clock, uncapped), [0149](0149-the-looks-foundations.md) (the
  interface's time is when its frame shows, and each root's clock),
  [0171](0171-effects-of-the-interface.md) (how each animation's rate is capped),
  [0181](0181-uwuis-scope.md) (what the host gives of frames, roots' clocks and input's times),
  [0187](0187-a-hidden-window-at-rest.md) (a hidden window's loop wakes for its tasks, and draws
  nothing), [0190](0190-uwuis-requirements.md) (saving power halves lasting motion's rate; motion
  that follows input keeps the display's), [0205](0205-signals-change-uwuis-interface.md) (a timer
  at a granularity ends at civil boundaries too) and
  [0211](0211-the-executor-and-other-threads-through-channels.md) (tasks' timers on their clock's
  present time, and tasks woken outside a frame, run at a wake that draws nothing); amended by
  [0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md) (what M6d.5's API carries is built where it is read)
  and [0251](0251-the-engines-host-clock-and-its-times.md) (the next wake and a wake that draws
  nothing come with uwui's executor)
- Date: 2026-11-10

## Context

[ADR-0226](0226-an-application-started-in-one-call-each-root-given-its-look.md) cut M6d.1d2b in two
and left this half the time of each frame that motion reads, what timers and input's events read,
and M6d.5 updated for them.

Today `Ui::advance(elapsed)` adds the time that the program gives to the interface's one clock, a
`Duration` from zero. Transitions, tooltips' rests, notifications and the *waiting bar*, a bar of
progress without a share, read it. Every root shares it. `slime-app`'s windowed loop gives it the
time since its last call at each event and frame, read from `Instant::now()` and uncapped.
[ADR-0107](0107-the-execution-model.md) says the interface reads real time limited after a pause,
which [ADR-0009](0009-frame-timing.md) caps: the code and the ADR disagree, a known debt. Tasks'
timers follow the frame's clock, and the executor runs within a frame
([ADR-0211](0211-the-executor-and-other-threads-through-channels.md)). A hidden window draws no
frame ([ADR-0187](0187-a-hidden-window-at-rest.md)), so a task's `sleep` never ends while it is
hidden. While anything moves, `Ui::needs_frame` asks for every frame of the display. M6d.1b's
prototypes read `frame_time`, a signal that runs a tracked closure at each frame.

The references read the frame's start: GPUI, Iced, Floem and Slint, and Android before API 33, whose
documentation warns that animations then "skip forward after late frames". One reads when the frame
will show, on macOS alone. What each platform gives of a frame's time, of input's, of saving power
and of its clocks across a suspend was read on 2026-11-09 ([uwui's
survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#what-platforms-give-of-a-frames-time),
[and
more](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#more-of-a-frames-time-of-power-and-of-clocks)).
In short:

- a future time of presentation on Apple and on Android from API 33;
- a past one on Wayland, Windows, X11's Present, and Android from API 35;
- none on the web, nor under Xwayland, whose Present times are its frame callbacks' arrivals;
- input's times in each platform's own clock, some with an undefined base, some wrapping;
- neither winit 0.30 nor wgpu passes any time on;
- the clocks of frames and input stop during a suspend on Linux, Apple and Android, not on Windows;
- `std::time::Instant::now()` and `SystemTime::now()` panic on `wasm32-unknown-unknown`.

The draft of this decision was attacked before it reached the owner. The attack found that the
draft's rule for frames in flight was FIFO's, while the engine presents in Mailbox. It found that
hover and press are styles' transitions too, so a list by kind leaked, and that game time read
"as the frame shows" led the drawn world by one step. It showed that drawing on each visible change
does not give Opium's four frames a second by itself. It also found a question missing: whether
the host's clock counts a suspend. The owner decided on 2026-11-10, one question at a time. A
closing review then found two points left unsaid, which the owner decided the same day: a pause
menu in its game's root, and a task woken outside a frame.

## Decision

### A frame's time is when it shows

**Motion reads the time at which its frame is expected to show,** one time for each window that
presents. Every root drawn into that window's frame reads it. A root drawn into a texture reads
the time that its host gives with the draw. The host gives the time with its *source*, so that
traces and tests know how good it is:

1. *Given*: Apple's `targetTimestamp`; Android's expected presentation time, from API 33.
2. *Predicted*, at a fixed rate, from past presentations: Wayland's `wp_presentation` with its
   `refresh`, Windows' statistics, X11's Present, Android's present fences from API 35. Under FIFO,
   the refresh after the frames still in flight; under Mailbox, which replaces an image that waits,
   the first refresh after the frame is ready.
3. *Asked*: where the host may ask when a frame shows (Wayland's `wp_commit_timing_v1`, Windows'
   presentation manager's `SetTargetTime`, Android's `setDesiredPresentTimeNanos`), it asks for the
   refresh that prediction gives. A platform takes the time asked as a floor, not a promise.
4. *Ready*, under a variable refresh: the frame's start plus the time that recent frames took to
   present, never sooner than the last presentation plus its `refresh`.
5. *On the grid*, where no presentation time exists (the web, Xwayland, a compositor without
   `wp_presentation`): the last observed frame's time plus whole periods of the display, anchored
   on each observed time, never accumulated. The period is the display's rate, else measured from
   the frames' gaps.
6. *Simulated*: in tests and headless runs, the time that the test gives
   ([ADR-0009](0009-frame-timing.md)).

A frame's time never goes back. Under FIFO it is at least one refresh after the last frame's.

Why: motion placed where it is when the light leaves the screen moves in the display's even steps,
not in the program's wake-ups, and stays in step with audio's clock. The cost is code for each
platform: uwui's desktop host reads it at M6d.7, the trials on Android and the web theirs. Until
M6d.7's ADR of the frames' rate, the engine's windowed loop gives the time when it handles each
event and starts each frame, as it does today.

### The time of input

**Each input event carries a time in the host's clock,** with its source:

- *a clock that the host knows*: Windows' `PerformanceCount`, Android's uptime, the web's
  `timeStamp`, converted as the host's clock below says;
- *another clock, or an undefined base*: Wayland's and X11's milliseconds, Windows' tick count,
  Apple's `systemUptime` until it is shown to share the frames' clock. The gaps between events are
  kept from their own times; the whole is anchored on their reception by the least gap between
  reception and the event's time over a sliding window, never after its reception, and the anchor
  is reset when the system resumes. Counters of 32 bits are unwrapped by their differences;
- *reception*, where nothing gives a time: winit 0.30 gives none on any platform, so uwui's
  desktop host stamps each event as winit hands it over. Stamps taken in a batch, or behind a busy
  frame, cluster, and a fling's speed suffers exactly when frames are slow. This is debt, measured
  at M6d.7, which also proposes winit a pull request, after a search for an existing one; not a
  fork.

**Motion that an input starts begins by its kind:**

- motion that continues the input's own, a fling or a drag released, starts at the input's time
  and place, with its speed, so that it never jumps back by the latency;
- a transition that an input triggers, a fade or a menu that opens, starts at its first frame's
  time, so that its first step shows. With two or three frames in flight, a fade of 150 ms
  started at the input's time would lose 22 to 33 % of its start at 60 Hz, and about a third on a
  phone at 90 Hz, its touch's latency counted.

The same holds for the first motion after a rest.

### The host's clock counts a suspend

**One clock for every time of the host,** one that counts the time the machine sleeps:
`CLOCK_BOOTTIME` on Linux, Android's `elapsedRealtime`, `mach_continuous_time` on Apple, the
performance counter on Windows. Times that a platform gives in a clock that stops during a suspend
(`CLOCK_MONOTONIC`, `mach_absolute_time`, Android's uptime) convert by their unit plus the time
spent suspended, which the host reads again each time it leaves a wait. Timers, transitions and
input's anchors then behave alike on every platform: a reminder in ten minutes ends ten minutes
later, sleep or not. The web's clock counts a suspend or not by browser; uwui takes what it gives.

A wait armed before a suspend must still end on time. On the desktop, winit waits on
`std::time::Instant`, which stops during a suspend on Linux and macOS: such a wait ends late by the
sleep's length, unless the host learns that the system resumed and arms it again. M6d.7 finds that
notice on each platform.

### No cap after a late frame

**Motion is where the host's clock puts it.** Every motion that uwui gives is placed by its formula
at the frame's time, never integrated step by step: curves, springs solved again at each change of
their target with their speed kept, a fling. A late frame shows the motion further on; a window
shown again shows its transitions ended. Motion in step with audio reads the audio's clock at the
frame's time.

A rest is not a late frame: while the host waits, for nothing or for a tooltip or a timer, the time
passes whole and what waits ends on time.

The engine keeps the cap of ADR-0009 for what lives on its *real time*, the camera and the dev
overlay, and for game time; the editor's navigation bounds its own steps. uwui's interface reads
the host's clock uncapped: ADR-0107's real time, limited after a pause, is no longer the
interface's. `slime-app` already gives uwui uncapped time; the debt closes with this decision.

### What a root's clock drives

**A root's clock is two values that the host gives at each frame,** its time and its *speed*: 1 for
the host's pace, 0 for a pause, less to slow it, more to hasten it. It is not an object: ADR-0009
rejected a clock behind a trait. By default, the host's clock at speed 1. For a game, the host gives
the game time that the frame shows, interpolated between its last two steps as the world is drawn:
`(steps - 1 + fraction) × step`, zero before the first step. A deadline in a root's time converts to
the host's through the speed, and never comes while the root is paused.

**What follows it, by state.** On the host's clock, always: the transitions of the states `Hovered`,
`Pressed` and `FocusVisible` ([ADR-0222](0222-what-a-theme-is.md)), and uwui's own timings: the
rests of tooltips and submenus, the caret, scrolling and its physics, double clicks and long
presses, notifications' durations, the waiting bar. They answer a person, whose input carries the
host's times. Everything else follows the root's clock: the other transitions of its styles, the
animations of its parts and code, its tasks' timers. A HUD slowed with its game slows its own
motion, and still answers hover.

**What runs during a pause is a root of its own,** on the host's clock, sharing its game's window
(ADR-0226): a pause menu, a dialog over the paused game. In the game's root, a switch's travel, a
panel's entrance or a task's timeout stop with the game.

### What timers and tasks read

**Two kinds of deadline:**

- *what changes the drawing*: a tooltip that shows, a notification that ends, a clock shown, a
  timer at a granularity ([ADR-0205](0205-signals-change-uwuis-interface.md)). It wakes nothing
  while its root does not show, and applies at its next frame;
- *a task's `sleep`*: it ends when its clock reaches its deadline, the host's, or its root's
  converted through the speed. The host wakes for it without a frame. Such a *wake* runs the
  executor, its tasks and the bindings that they change; styling, layout and drawing wait for a
  frame. A hidden window runs its tasks' timers, and draws nothing.

**A task woken outside a frame asks for a wake, not a frame,** a channel's message from another
thread among them: one rule for every task. When what the wake's bindings change shows in a root
on screen, the wake asks for a frame; a hidden window never draws for it. The executor runs at a
wake as at a frame (ADR-0211).

`Ui::next_change` gives way to two instants in the host's clock: the next frame due and the next
wake. A host promises only the first wake that its platform allows, which may slow or stop a
program in the background.

### How often motion asks for frames

**A motion asks for a frame when what it draws changes visibly:**

- an item placed on whole pixels, when its place on them changes;
- an antialiased edge, at a quarter of a device pixel;
- a colour, at one step of the target's encoding;
- glass, blur and the axes of a variable font, which magnify motion: every frame of their cap.

The time of the next visible change is bounded from the motion's speed, never searched for. The
thresholds are uwui's first stage's, checked by its measure of smoothness.

**Each animation has a cap,** one frame in every n refreshes, so that a cap never judders: a cap
that divides no refresh rounds to the nearest whole n, at least one. Under a variable refresh, the
cap is taken as it is. By default it is the display's rate; the author may lower it, as Opium's wave
steps four times a second.

**Saving power halves the caps of motion that lasts,** a wheel of waiting, a wave, a glow. Motion
that follows input, scrolling and dragging, keeps the display's rate unless the system caps it:
ADR-0190's scrolling "at the display's rate" wins over its "rate capped" for that motion. A motion
capped samples on its own steps, so that its group, kept as an image
([ADR-0171](0171-effects-of-the-interface.md)), is drawn again only at them. Where a platform takes
a rate for a surface (Apple's `preferredFrameRateRange`, Android's `setFrameRate` and
`setRequestedFrameRate`), the host gives it the highest rate that its motions ask, so that the
display may refresh slower.

### `frame_time` and what replaces it

**`frame_time` is no longer a signal:** a tracked closure that reads it runs at each frame, against
the rule that no code runs for what moves (ADR-0205, ADR-0222). In its place:

- `now()`: the host's present time, read untracked, where code records when something happened;
- `shown_at(root)`: when that root's frame shows; outside a frame, the next frame's predicted time;
- motion that runs no code: values read when they are drawn, a phase moving at a speed from a
  start in whole nanoseconds, curves and springs. Opium's wave is a phase from the song's start;
- for code that must truly run at each frame, a task that awaits `next_frame()`, which resolves
  with that frame's time and asks for frames while it waits.

The name avoids "target": a root's target is where it draws (ADR-0226).

### The time's types

**Two types of uwui's own, in whole nanoseconds (`u64`), and `core::time::Duration` for gaps:**

- `Instant`, a time of the host's clock, its origin the clock's own zero, so that a platform's time
  converts by its unit and the time suspended alone;
- `RootTime`, a time of a root's clock.

They cannot be mixed: subtracting one from the other does not compile. A float comes only from a
`Duration`, never from an origin, whose distance would cost it its precision: `f32` seconds step by
7.8 ms after a day. Not `std::time::Instant`: it panics on the web, cannot be made from a
platform's time nor by a test, and does not count a suspend on Linux and macOS. Each host reads its
platform's clock itself, through `libc` or `rustix`, `windows-sys`, `objc2` and `js-sys`, each put
to the owner when its host is built.

### The time of day

**The host gives the civil time,** the page's on the web, where `std`'s panics; the zone's name; the
offset from UTC and its next change, so that a clock shown needs no database of zones; and their
changes. The timer at a granularity of ADR-0205 is the one mechanism: it ends at the next civil
second or minute, and is armed again at each tick from the civil time read again, when the system
resumes, and when the system tells that its clock or zone changed. It corrects itself where the
system tells nothing, Windows' zone and the web. It rests while its root does not show. Formats and
the rules of zones are an application's crate's, or uwui's internationalisation later.

## Alternatives considered

On a frame's time:

- **The frame's start,** as most references: no code for each platform. But steps follow the
  wakes' jitter, motion lags by the pipeline's depth, and Android advises against it.
- **The start now, the presentation later:** nothing to build yet. But the API changes its meaning
  later, and tests are written against the start.

On input's motion:

- **Always at the input's time:** one rule. But a fade loses a part of its start that grows as the
  display slows, as above.
- **Always at the frame that handles it:** one rule. But a fling jumps back by the latency.

On the clock and a suspend:

- **Each platform's own clock:** no conversion. But Windows differs from the rest, a timer ignores
  a suspend elsewhere, and a transition resumes midway after one.
- **Two clocks,** one for motion and input, one for timers: each use its natural clock. But two
  clocks that must never mix.

On a late frame:

- **Even steps that catch up past a bound:** no jump. But motion lags the host's clock and audio for
  a while, and every motion carries a second time.
- **A cap,** as ADR-0009 and Rune: ADR-0107 kept as written. But motion behind for good
  after each long pause, and transitions resumed midway after a suspend.

On a root's clock:

- **Everything in the root on its clock:** one rule. But a paused or slowed game also freezes or
  slows its HUD's hover and caret, and input's times mix with the root's.
- **No clock for each root,** the engine's layer slowing what it animates: simpler for uwui. But
  against [`uwui-needs.md`](../uwui-needs.md), and a HUD's timers run during the pause.

On a pause menu in its game's root:

- **A branch of the tree on the host's clock,** declared by an element: more freedom. But two
  clocks in one tree, to test everywhere.
- **uwui's layers on the host's clock,** menus, dialogs and tooltips that uwui opens: automatic for
  them. But a pause menu written as a plain box still freezes, and the rule grows.

On timers:

- **Timers at frames:** one time. But a hidden window wakes and draws for each, against ADR-0187,
  and on Wayland it gets no frame at all.
- **Every deadline wakes the host:** one rule. But a hidden window wakes for its tooltips and
  clocks, for nothing.
- **A task woken outside a frame asks for a frame,** as ADR-0211 wrote: nothing changes for
  channels. But two rules, and a hidden window draws at each message.

On the rate of frames:

- **The display's rate while anything moves,** as today: the simplest. But a slow wave costs every
  frame of the display, against the budget of power.
- **A cap for each animation alone:** simple. But the tuning falls on each author, and an animation
  left alone runs at the display's rate.

On `frame_time`:

- **The name kept for an untracked read:** what the prototypes wrote. But the name hides that the
  time is in the future, and the time that GPUI, Iced, Floem and Slint read is the start.
- **`frame_time` a signal:** the most direct. But code runs at each frame of a motion.

On the types:

- **One type:** simpler. But a paused game's time and the host's mix without a word.
- **`std::time::Instant`, `web-time` on the web:** familiar. But no platform's times, no tests, no
  suspend counted.

On the time of day:

- **Nothing in uwui:** each application reads `SystemTime`. But it panics on the web, and a clock
  shown misses a change of the system's time.
- **Zones and formats in uwui:** complete. But a database of zones in every application.

## Consequences

- M6d.5 builds the host's side, in `uwui` and `slime-app`: the two types; each window's frame
  given its own time and source, and each root its own clock, where one clock serves every root
  today; each event's time; the two instants and a wake. `slime-app` gives what it can: the
  frame's start, events stamped at reception. uwui's first stage builds what reads them: the caps
  and the visible changes, `now`, `shown_at` and `next_frame`, and the civil time.
- Today the waiting bar turns by `Ui::now` read as `f32` seconds from zero (`control.rs`); the
  phases in whole nanoseconds come with the first stage's motion.
- M6d.5's API carries, for each window that presents, its frame's time, its source, its deadline
  where given, the display's rate and whether it varies; the two deadlines, and a wake that runs
  tasks without drawing; each root's time and speed; each event's time and source; the time spent
  suspended; the civil time, its zone, its offset and their changes; a rate asked for a surface.
  Its table holds the engine as a column, and winit split into Wayland, X11, Windows and macOS
  where it gives nothing. A pause menu as a root of its own is written in the documentation of the
  engine's layer for games' interfaces.
- M6d.6 records which compositors give presentation times, and how much frames' starts vary at
  240 Hz. M6d.7 reads the platforms' times in uwui's desktop host, measures the stamps taken at
  reception and proposes winit their times, and finds the notice that the system resumed. It
  checks whether a host can reach the presentation's times under wgpu: on Wayland, its own
  presentation feedback and commit timer on a surface whose commits Vulkan's driver makes; on
  Windows, DXGI's statistics, the presentation manager, and DWM's timing with several displays. Its
  ADR of the frames' rate decides the engine's.
- The engine's interface now reads uncapped time by decision, not by drift; ADR-0107's cap stays for
  the camera, the dev overlay and game time, and for the network later.
- Each host's clock adds a direct dependency of its platform, put to the owner when the host is
  built.
- Not verified, read only in code: the clock of X11's `ust`; WebKit's halved rate under Low Power
  Mode. Not documented: how Windows' interrupt time converts to its performance counter, and a
  change of zone on Windows. Not read: how each platform limits a program's timers in the
  background, where each tells that the system resumed, and how each gives the next change of the
  offset from UTC.
