# Glossary

Terms that this project defines, or uses with a narrower meaning than usual. Standard Rust and
game-development vocabulary is not repeated here. Entries are in alphabetical order.

**ADR (Architecture Decision Record).** A short document in [`docs/adr/`](adr/README.md) that
records one decision: its context, the decision, the alternatives rejected and the consequences.

**Application time.** The sum of the deltas of all frames so far (`FrameTime::elapsed`). It
falls behind wall-clock time when a frame delta is limited, for example after a pause.

**Definition of done.** The conditions a milestone must meet before it is reported as finished,
listed in [`AGENTS.md`](../AGENTS.md#definition-of-done).

**Driver.** The loop that decides when frames run and what time each one represents. `run` has a
headless driver (frames back to back, simulated time) and a windowed driver, called back by the
windowing library's event loop, which measures real time and limits the frame rate. All drivers
run frames the same way ([ADR-0010](adr/0010-main-loop-ownership.md)).

**Example.** A runnable program in the `examples/` directory that demonstrates one feature.

**Fixed step.** A duration of time, constant for a run, in which code that must not depend on
the frame rate advances (`slime_app::time::FixedStep`). A frame runs zero or more fixed steps.

**Frame.** One iteration of the main loop. Its *delta* is the time it represents: the time since
the previous frame, limited to a maximum (`slime_app::time::FrameClock`).

**Frame limiter.** The part of the windowed driver that waits between frames so that at most
`AppConfig::max_frame_rate` frames run per second. Temporary: presenting frames on screen will
pace them once the GPU layer exists.

**Headless.** Running the engine without a window, renderer or audio output. It is a normal
configuration, used by automated tests and, later, by dedicated servers.

**Interpolation factor.** The fraction of a fixed step accumulated but not yet run
(`FixedStep::alpha`), between 0 and 1. Used to draw a state between the last two fixed steps.

**Logical key.** What a key means with the current keyboard layout and modifiers
(`slime_platform::Key`): on a French AZERTY keyboard, the first letter key of the top row gives
the logical key `"a"`. Suits shortcuts and text. See also *physical key*.

**Milestone.** A small, verifiable step of a stage, listed with its goal, its done criteria and
how to test it in [`docs/PLAN.md`](PLAN.md). Milestones are numbered `M<stage>.<n>`, for example
M1.3.

**Owner.** The person who validates the project's direction and its architecture decisions.

**Physical key.** The position of a key on the keyboard, whatever the layout
(`slime_platform::KeyCode`), named after the character it produces on a US keyboard: the first
letter key of the top row is `KeyQ` on every layout, whether it is labelled Q (QWERTY) or A
(AZERTY). Suits movement keys. See also *logical key*.

**Sample.** The example that demonstrates the exit criterion of a stage. Every stage ends with
one.

**Simulated time.** Frame deltas supplied as a constant duration instead of being measured, so
that a run gives the same results on every machine. Headless runs use it.

**Skeleton (crate).** A crate that exists with its documentation and inherits the workspace
policies, but holds no code yet.

**Spiral of death.** A fixed-step loop that falls further behind at each frame because running
the overdue steps takes longer than the time they cover. `FixedStep` avoids it by limiting the
steps per frame and dropping the rest.

**Stage.** A step of [`docs/ROADMAP.md`](ROADMAP.md), ordered by dependency, with a goal and an
exit criterion. Stages are numbered from 0; a stage may be split (Stage 3a and 3b).

**Termination signal.** A request from outside the process to stop it: SIGINT (Ctrl+C in a
terminal), SIGTERM (a service manager or `kill`) or SIGHUP (the terminal closing). The first one
stops a run cleanly; a second one ends the process at once
([ADR-0012](adr/0012-clean-shutdown-on-termination-signals.md)).
