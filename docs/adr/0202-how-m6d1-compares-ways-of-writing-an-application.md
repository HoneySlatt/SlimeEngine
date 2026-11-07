# 0202. How M6d.1 compares ways of writing an application

- Status: Accepted; amends [0190](0190-uwuis-requirements.md) (four state models, today's as the
  control), [0194](0194-stage-6d-uwui-leaving-the-engine.md) (M6d.1 cut in four) and
  [0199](0199-uwuis-targets-tools-and-references-narrowed.md) (the references read, not written
  again); amended by [0204](0204-signals-written-by-hand-for-the-prototypes.md) (the signals of
  the prototypes written by hand) and [0210](0210-signals-named-for-a-reload.md) (M6d.1c cut in
  three; `serde` for a type's state)
- Date: 2026-11-06

## Context

[ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md) makes M6d.1 the milestone that decides how an
application on uwui is written, from prototypes that are thrown away and figures that are kept.
[ADR-0190](0190-uwuis-requirements.md) names three state models to measure on the screens of
reference: signals, views that say they changed as in GPUI, and messages as in Elm.
[ADR-0199](0199-uwuis-targets-tools-and-references-narrowed.md) adds references to read, Floem,
Rune and Makepad among them, and three reloads to answer apart: a style or a layout, a function's
body, a type whose state migrates.

M6d.1 holds some seven decisions, three families of prototypes and one review. Its questions
depend on each other. The state model decides whether Rust's code can be reloaded and who owns
the interface's tasks. The format of layouts and styles written in code come last, with what
M6d.5's host API takes from them.

The survey of M6d.1a, in uwui's repository
([`docs/survey-writing-an-application.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md)),
found that:

- the models differ in what the processor does before the display list: which code of the
  application runs again, and what is styled, laid out and drawn again. The drawing on the GPU is
  uwui's canvas in every model;
- in a whole framework, the cost of the state model cannot be told apart from the rest of the
  framework: each frame of a GPUI window that changed walks its whole tree, and Iced lays out from
  the root after each message;
- uwui's tree already counts each frame's work (`FrameStats`), and its benchmarks have a global
  allocator that counts each thread's allocations, though not their bytes
  ([ADR-0186](0186-counters-and-ratios-in-ci.md)).

The owner decided the protocol on 2026-11-06, one question at a time. The method that carries it
out, at the end of the decision, is the milestone's own choice.

## Decision

**M6d.1 is cut in four,** in the order of its questions' dependencies, each part with its review:

- M6d.1a, the survey and this protocol;
- M6d.1b, the state model;
- M6d.1c, Rust's code reloaded and the interface's executor;
- M6d.1d, layouts, styles and the host.

**Four models.** Today's rebuild by keys (`Ui::build` run again when a revision moves) is the
control. Beside it: signals, views that say they changed, and messages. Each is prototyped over
uwui's tree, on a branch, then thrown away.

**Measured in the program, without a window.** Each phase of a frame is timed and counted apart,
on uwui's tree, on a clock that the test moves:

- the application's code and the views run again;
- the styles;
- the layout;
- the display list.

Only the rivals are measured from outside, by
[uwui's `scripts/measure.py`](https://github.com/HoneySlatt/uwui/blob/main/scripts/measure.py).
On the owner's very modest laptop, only the owner measures, by hand, if they want to, with a
command that M6d.1b documents. The figures go to uwui's
[`docs/measurements.md`](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md).

**The criteria.** For the author of an application:

- the lines of each screen's code;
- the concepts to learn: each type, trait, macro and rule that the screen's code needs, listed;
- the errors, as below;
- the state kept after each of the three reloads.

For the machine:

- the processor's time and the allocations of each phase of a frame, counted by a global
  allocator that counts each thread's calls and bytes;
- a frame where nothing changed costs nothing: no code of a view runs, nothing is allocated;
- what each change dirties in the display list: its items changed, out of all of them;
- the time from an input to the display list that shows it, in frames and in processor time, on
  the clock that the test moves;
- the memory held while a screen is open, and the memory still held after it closes, against
  what was held before it opened.

For the move: what moving the editor and *Hunna Wood* to each model would cost, as the places in
their code that would change, counted: builders, actions read, revisions kept by hand. They are
counted, not estimated in hours.

**Three kinds of errors,** each counted by a tool, not by an opinion:

- *Wrong renders.* After each scripted interaction, the display list is compared with that of a
  new interface built from the same state; any difference is a wrong render. Each mistake that the
  compiler lets through, a `notify` forgotten, a key forgotten, a revision not moved, is shown by
  a small case that compiles and renders wrong.
- *Panics.* Code that looks right and panics, a value borrowed twice, a scope already disposed,
  each shown by a small case.
- *Compile errors* met while writing each screen's ideal code, then its prototype, recorded by
  their code. They are an indication, not a score: they depend on who writes.

Leaks are not counted here: the memory held after a close measures them.

**The screens and their scripted interactions.** Their data are made in the program, at the sizes
of the screens of reference; images are keys, decoded nowhere.

- "Hello, World!": its button pressed.
- Slatty's grid of 10,000 games: scrolled as its
  [screen of reference](https://github.com/HoneySlatt/uwui/blob/main/docs/ROADMAP.md#screens-of-reference)
  scrolls; a filter typed a character at a time; the selection moved by the arrows; a game's page opened, its key art
  fading in, then closed; the theme changed.
- Opium's table of 8,500 songs: scrolled from end to end; a song playing, its wave of progress
  moving; a column sorted; the selection moved; the theme changed.
- A synthetic case: a document of 100,000 lines, one line of which changes. Zed's file is not a
  screen of M6d.1: uwui has no editor of code before its 1.0.

**The ideal code first.** At M6d.1b's start, before any prototype, each screen's code is written
for each model as an author would want to write it. It is compared with the references' own
examples of the same kind of screen, put to the owner, and kept in uwui's repository. Each
prototype then takes that code; the lines that it forces to change are counted.

**The references are read, not written again.** Their lines and concepts are read in their own
examples, beside the ideal code. Their errors are read in their documentation and their sources.
Their reloads are timed at M6d.1c, on their own examples. The cost of each phase is measured on
uwui's prototypes alone.

**Dependencies to come, not added now.** Each is put to the owner when a milestone first needs it:

- a library of signals, or one written by hand for the prototypes (M6d.1b);
- `subsecond`, for a function's body reloaded (M6d.1c);
- `serde`, for a type's state migrated (M6d.1c), which [M6d.3](../PLAN.md#m6d3--documents-with-serde)
  brings to uwui's documents.

**The method,** chosen within the milestone, not by the owner; M6d.1b may change it at its start,
and says why:

- release builds, on the high tier's machine; each figure the median of its runs, with their range;
- lines counted after rustfmt, without blank lines and comments;
- the state checked after a reload: the scroll, the selection, a field's text, the filter, and a
  page open.

## Alternatives considered

- **M6d.1 whole:** fewer milestones, but three families of prototypes and some seven ADRs under
  one review, and the reload decided before the model it depends on.
- **Three models, without the control:** less work, but a choice without a point of comparison,
  and the cost of leaving today's model unseen. **Five, with the immediate mode of egui:** the whole
  field, but each frame that it draws, on a hover too, runs the whole interface's code and lays it
  out again: nothing is kept between frames for the counters to tell apart, and it overlaps the
  control.
- **The prototypes measured in a window too,** by `measure.py`: figures beside the rivals', but a
  window for each prototype, the desktop host that M6d.7 builds, and noise larger than the gaps
  between the models. **In a window alone:** the gaps unseen.
- **The criteria without the two additions,** a still frame's cost and the memory held while
  open: the cost of signals' nodes and work at rest hidden. **A shorter list,** lines,
  concepts, errors and each phase's cost: nothing on delay, memory or the move.
- **Errors met while writing the prototypes:** natural, but it measures the author, here an
  agent, not the model. **Wrong renders alone:** panics and the friction of compiling lost.
- **The screens without the scroll and the page opened and closed:** less work, but the case that
  tells the models apart most left out. **Fewer screens:** a grid and a table do not cost the same.
- **One ideal code for each screen, neutral to the models:** less text, but a neutral code always
  leans towards one model unseen. **No ideal code:** the prototype dictates the API.
- **Measures on the owner's laptop required before the choice:** the lowest machine weighed, but
  the decision waiting on the owner's time, for gaps that allocations and work counted already show.
- **The references written again on the screens,** Slatty's grid in Floem, GPUI with gpui-kit and
  Iced, or every screen in every reference: an order of size for each model, but figures that mix
  the model with each framework's renderer, and some ten programs thrown away.

## Consequences

- M6d.1b grows: the benchmarks' allocator also counts bytes; each prototype carries the same
  counters, the comparison with a new interface and the scripted interactions. It may be cut at
  its start.
- A prototype written by hand may serve its model worse than the framework that inspired it. The
  ideal code compared with the references' examples, and the review, guard against it.
- The prototypes' figures do not compare with the rivals' measured from outside; the cost of each
  model on the start, the memory that the system sees and power waits for M6d.4 and M6d.7.
- Without the owner's measures, the order of the models on the lowest machine is inferred from the
  high tier.
- Whether each reload works on the web and on Android is weighed at M6d.1c, from the survey.
