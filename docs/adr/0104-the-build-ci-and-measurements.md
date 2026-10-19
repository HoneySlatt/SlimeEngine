# 0104. The build, CI and measurements

- Status: Accepted; amends [0004](0004-workspace-layout-and-crate-naming.md) (a top-level
  `benches/` package) and [0077](0077-hot-reload-of-shaders.md) (what an edited shader costs)
- Date: 2026-10-19

## Context

The audit of 0.4 ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)) found
that the build and CI cost more than they need to, and that nothing tracks a measurement from one
commit to the next ([`docs/PLAN.md`](../PLAN.md), P5):

- **B1-F01.** Every crate of the workspace that depends on its procedural macro,
  `slime-reflect-derive`, was compiled twice. Cargo builds a procedural macro with the `dev`
  profile when it is built as a member of the workspace, and with `dev`'s `build-override`, which
  has no debug information by default, when it is built for the crates that use it: `cargo build
  --workspace` built the first, `cargo test --workspace` the second, and with it every crate that
  depends on it.
- **B1-F02.** Most of the `target` directory was the dependencies' debug information, which a
  developer of the engine rarely reads.
- **B1-F05.** CI's Windows job was 12 seconds under the 20 minutes that the owner allows, and each
  job cooked *Hunna Wood*'s zone again, about two minutes and a half on Windows.
  [ADR-0072](0072-stage-4b-live-assets.md) and [ADR-0081](0081-stage-5a-the-world.md) had decided
  to keep the cooked zone in CI's cache past 19 minutes, and ADR-0081 to split the Windows job in
  two jobs that run in parallel past 20. The job's duration depends on its runner more than on
  the code: from one run to the next of P5, with the same work, it took 13 minutes 38 seconds, 21
  minutes 54 seconds and 19 minutes 5 seconds, each of its steps about as much slower as the
  others.
- **C6-04, C7-08.** The audit's benchmarks stayed outside the repository, and documented figures
  were stale or wrong: the workspace's tests took twice as long on lavapipe as the known debt said;
  ADR-0077 gave 7 ms for the renderer's pipelines made again "as when one file changed", a figure
  measured with every shader already in the driver's cache, which an edited shader is not: the
  audit measured 46 to 50 ms after an edit.

Measured from an empty target directory, with the default debug settings, on a machine of 4
logical CPUs:

| Profiles | `cargo build --workspace` | then `cargo test --workspace --no-run` | libraries compiled again | `target` |
|---|---|---|---|---|
| Before | 144.6 s | 94.6 s | 12 | 12.47 GB |
| `build-override` with `dev`'s debug information | 132.5 s | 83.4 s | 0 | 11.95 GB |
| and dependencies without debug information | 112.3 s | 68.0 s | 0 | 6.76 GB |
| and dependencies with their line tables only | 118.7 s | 69.1 s | 0 | 7.99 GB |

## Decision

**Each crate compiled once.** `[profile.dev.build-override]` has `debug = true`, the debug
information of `dev`: the procedural macro built for the crates that use it is then the one built
as a member, and `cargo test` after `cargo build` compiles no library again.

**Dependencies with their line tables only.** `[profile.dev.package."*"]` has `debug =
"line-tables-only"`: a backtrace still places a dependency's frames in its source, its file and
its line, which a panic inside wgpu or winit needs to be understood, and the rest of the
dependencies' debug information, which a debugger reads to show their variables, is not built.
They cost 1.2 GB and 6 seconds more than no debug information at all. A developer who steps
through a dependency in a debugger builds it with its debug information for that session:

```sh
cargo build --config 'profile.dev.package."*".debug = true'
```

**The cooked zone in CI's cache.** Every job keeps `target/cooked/hunnawood` from one run to the
next with `actions/cache`, keyed on what makes its bytes: its sources' list and their metadata, its
scene files, the sources of the cooker and of the crates of the workspace that the cooker depends
on, the workspace's `Cargo.toml`, `Cargo.lock`, the toolchain, the runner's system and processor.
The cook still runs: it reuses each file of the cache, which it checks before reusing it
([ADR-0102](0102-a-damaged-file-gives-an-error.md)), and cooks again what the cache does not hold.

**The Windows job in two**, as ADR-0081 decided, since a run took 21 minutes 54 seconds. The job
`windows` runs the tests, the image tests and the samples of the debug build, then cooks the zone
for the tests that read it, which use that build; the job `windows-release` runs clippy, builds the
programs that CI publishes in release, cooks the zone for them, runs them and publishes them. The
two cook the zone each, from the same cache; its key changes with `Cargo.toml`, `Cargo.lock` and
the cooker's crates, and both cook it again then.

**The engine's benchmarks, a program of the workspace.** [`slime-benches`](../../benches/README.md),
a package in a top-level directory of its own, `benches/`, beside `examples/`, which ADR-0004's
list of top-level directories gains, runs scenarios of the size that the engine aims at, as
ADR-0097 measures it: 10 000 objects drawn, 100 000 entities placed, manifests of 100 000 assets,
and the meshes of *Hunna Wood*'s zone, each placed once, since the zone's scene holds components
that only the game registers. Each is timed a number of times, and the program prints rows of a
Markdown table, the median and the 90th percentile of each series or what its row says, after the
machine, its GPU and its build. It depends on the engine's crates as a game does, through their
public API, and measures with `std::time::Instant` and the renderer's own GPU timings
([`RenderStats`](../../crates/slime-render/src/renderer.rs)).

**A file of measurements, versioned.** [`docs/measurements.md`](../measurements.md) keeps, one
section per milestone or machine, the build's figures and the benchmarks', which
[`scripts/measure.sh`](../../scripts/measure.sh) writes. A milestone that claims a gain shows the
section before it and the section after it. The figures of the plan's known debt and of the ADRs
stay where they are, each with its machine, and are corrected there when a measurement
contradicts them.

**A trace of the spans, for profilers.** `slime-app` writes every span that the process enters and
leaves, the profiling spans of each frame of a run among them, to the file that the environment
variable `SLIME_TRACE` names, in the Chrome Trace Event format, which Perfetto opens in a browser
without sending the file anywhere. A layer of the engine's own, about a hundred lines on
`tracing-subscriber`, which the engine already uses, writes it.

**ADR-0077's figure corrected.** The figure of 7 ms holds when the driver's cache holds every
shader, as when shaders that the driver compiled before come back; an edited shader is new to the
driver. `Renderer::set_shaders`'s rustdoc gives both; `slime-benches`'s `render.shaders` measures
both on the machine that runs it.

## Alternatives considered

- **Dependencies without debug information**, as the plan proposed: 1.2 GB and 6 seconds less, but
  a panic inside a dependency, as those of wgpu on a lost device that
  [ADR-0103](0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md) works around, gives a
  backtrace without its files and lines.
- **A cache of the compiled dependencies in CI**, which ADR-0081 rejected: it stays rejected; the
  cooked zone was the lever that ADR-0072 kept, and a cooked zone does not depend on how the
  dependencies were compiled.
- **`criterion` or `divan`** for the benchmarks: a library of statistics for benchmarks of
  microseconds, its own reports and, for `criterion`, dozens of dependencies, where the engine's
  scenarios take milliseconds to seconds, need a GPU and a cooked sample opened once, and give a
  table that a milestone keeps.
- **Benchmarks of each crate under `cargo bench`**: each crate would open its own GPU and cook its
  own sample, and the figures would come out as many outputs, not one table.
- **The benchmarks in CI**: CI's machines are shared, and their figures vary from one run to the
  next more than the gains that a milestone measures; CI builds the program so that it keeps
  compiling.
- **The measurements in a file that a program reads**, TOML or JSON, compared by a tool: no
  consumer reads it yet but a person, who compares two sections; a tool comes with the first
  consumer that needs one.
- **The Windows jobs split otherwise.** The tests that read the zone in the release job, in a
  release build, would have saved the cooker's build and cook in the tests' job, but run without
  the debug build's assertions on Windows. In the release job in a debug build, they would have
  built every dependency a second time, in debug. The tests split in two jobs, by crate, would
  have built the debug dependencies twice.
- **`tracing-chrome`** for the trace: a dependency for what about a hundred lines do.
- **Tracy** (`tracing-tracy`): a profiler of more power, but a client in C++ built into the
  engine, which listens on a network port for the profiler, and a program of its own to install.
- **`puffin`**: a profiler drawn with egui, whose macros the engine's code would carry beside
  `tracing`'s spans, two ways to mark the same work.

## Consequences

- `cargo test --workspace` after `cargo build --workspace` compiles no library again, and `target`
  is smaller, in `scripts/check.sh` as on a developer's machine. CI's Windows jobs, which build the
  tests without building the workspace first, gain only the smaller debug information. A
  debugger shows a dependency's frames and lines, not its variables, unless the developer asks for
  them as above.
- A crate of the workspace that the cooker comes to depend on joins the cache's key in
  `.github/workflows/ci.yml`; without it, after a change to that crate, the cooker would reuse
  the cache's meshes and textures as they were, since they pass its checks.
- Each milestone of the audit pass from P4 on, and each later one that changes what the benchmarks
  measure, appends its section to `docs/measurements.md`. The benchmarks' figures compare only on
  the same machine, driver and build, which each section names.
- When `SLIME_TRACE` is unset, the subscriber has no layer of the trace. When it is set, each span
  costs a lock, and every log record of the dependencies reaches the subscriber, which drops those
  that the log leaves out; `TRACE_ENV_VAR`'s rustdoc says what is not measured.
- The two Windows jobs take the time of the longer, and a change to the cooker's key cooks the
  zone in each.
- ADR-0077's measurements stand, but its "as when one file changed" does not.
