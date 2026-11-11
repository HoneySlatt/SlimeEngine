# 0226. An application started in one call, each root given its look

- Status: Accepted; amends [0149](0149-the-looks-foundations.md) (less motion keeps brief fades;
  the system's preferences read on Linux too), [0181](0181-uwuis-scope.md) (each root given a look
  and a target; the system's preferences read by a crate of uwui's that the engine's platform layer
  calls too), [0190](0190-uwuis-requirements.md) (uwui's own design system no longer the default;
  the system's preferences that a look follows), [0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md)
  (the core's look gains a variant, Plain), [0215](0215-the-engine-measured-against-the-market.md)
  (the look named blank is Plain, a variant of the core's look),
  [0220](0220-the-roadmap-reshaped-by-the-market.md) (blank renamed Plain; M6d.5 checks a target
  for each root, not a surface),
  [0222](0222-what-a-theme-is.md) (a look is a theme with its fonts and icons, given to each root and
  replaced while it runs; how an application's parts and rules stack over a look's),
  [0223](0223-styles-written-in-code-through-tokens.md) (the bottom of the stack is any look, not
  the core's; under forced colours, the sheets' literal colours give way too) and [0225](0225-layouts-written-in-kdl-reached-from-code-by-their-names.md)
  (M6d.1d2b cut in two; `layouts!` becomes `files!`; blank renamed Plain); amended by
  [0231](0231-an-elements-own-style-written-in-code.md) (the order of styling read field by field) and [0237](0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md) (the curve of the size of text given by stops; the user's preferences taken by the core before their reader)
- Date: 2026-11-09

## Context

[ADR-0215](0215-the-engine-measured-against-the-market.md) has `uwui new` ask which look an
application takes: uwui's own, Glass, Material 3 Expressive, Pro or blank, none chosen in silence.
[ADR-0190](0190-uwuis-requirements.md) calls uwui's own design system "the default".
[ADR-0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md) ships one look in uwui's core and
rejected "no look at all": components unusable without work, accessibility not given. It also
names the *blank window*, the program that only clears its window and presents it, against which a
program's own cost is measured. [ADR-0222](0222-what-a-theme-is.md) makes a theme values, rules and
parts. [ADR-0225](0225-layouts-written-in-kdl-reached-from-code-by-their-names.md) left to
M6d.1d2b an application started in one call that names its look, blank defined, and the time of
each frame that motion reads.

uwui has no start of its own today. A program makes a `Ui`, adds its roots and calls it at each
frame from its own loop, as the engine's `slime-app` does. Of ten references, nine start an
application in one call: `iced::run`, `floem::launch`, `dioxus::launch` and the others
([uwui's survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#6-an-application-started-in-one-call)).
A call that starts an application needs the core, the renderer, the components, a look and a
host. It cannot live in `uwui`, which `uwui-render` depends on, nor in `uwui-render`, which the
engine uses without uwui's host. The engine has its own platform layer on winit, `slime-platform`
([ADR-0181](0181-uwuis-scope.md)); it must not compile uwui's desktop host beside it.

Of the system's preferences, winit 0.30 reports only light or dark, and only on Windows, macOS and
the web. On Linux the desktop's settings portal holds light or dark, contrast and reduced motion,
not the size of text nor reduced transparency; KDE's gives no contrast
([read on 2026-11-09](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#10-the-systems-preferences)).

The draft of this decision was attacked before it reached the owner. The attack found that the one
call could not be spelled the same on Android, where a Kotlin program has no `main`. It found that
a look named only in Rust shut out games' and mods' looks, which are data. It found that the
draft's stack broke ADR-0223's order of styling, and that a macro that read the author's files at
run time would start empty on the web and Android. It found that a look of two colours had fewer
roles than the forced colours that it should share a path with. It also found eleven questions
for one milestone, in two groups that share no decision.

The owner decided on 2026-11-09, one question at a time. A closing review then found three points
that the decisions had left unsaid, which the owner decided the same day: what forced colours
override, how many crates read the system's preferences, and whether Plain follows light and dark.

## Decision

**M6d.1d2b is cut in two,** in two groups that share no decision:

- M6d.1d2b1: an application's start and its look, here;
- M6d.1d2b2: the time of each frame that motion reads, what timers and input's events read, and
  M6d.5 updated for them.

**An application in the core's look names one crate, a *facade*,** which holds the call. It stands
above uwui's core, its renderer, its components, the core's look and the host for the platform. It
chooses the host by the target: the desktop on Linux, Windows and macOS, the web on wasm32, uwui's
own activity on Android, a UIKit view on iOS.

```rust
#[uwui::main]
fn main() {
    uwui::run(uwui::looks::CORE, hello)
}
```

The paths are illustrative: the crates' names are M6d.10's. `hello` is the application's view.

- *An attribute on the entry function* writes each platform's entry: `main` on the desktop and the
  web, the native entry of uwui's activity on Android, and iOS's. The same code builds on each, as
  the budget of porting asks (ADR-0213). An attribute on a function cannot write a crate's
  `windows_subsystem`: how a program on Windows starts without a console beside its window, and
  where a failed start's error then goes, is left to the milestone that builds the facade.
- *`run` never returns,* on any target, so that no code after it differs by platform.
- *A start that finds no usable GPU* falls back on the renderer on the CPU where it is built in
  ([ADR-0190](0190-uwuis-requirements.md)). Any other failure, a missing display among them, is
  reported by the host as its platform does.
- *A Kotlin program attaches uwui to a surface that it gives*, through a call of the Rust side
  that names the look ([ADR-0181](0181-uwuis-scope.md)).
- *Applications name the facade; libraries of components and hosts name the core.* The
  documentation shows the facade's paths.
- *The engine never depends on the facade,* nor on uwui's hosts. A check in CI proves that its
  programs compile none of them: the other side of M6d.7's check that a program on uwui compiles
  neither the engine's renderer, its world nor its editor.
- *The facade is one dependency for the core's look and Plain.* Another look is a crate of its own,
  named beside it, with its own version (ADR-0213).
- The crates' names are left to M6d.10.

**Every root is given its look when it is made, and there is no default.**

- A *look* is a theme in ADR-0222's sense, values, rules and parts, with its fonts and its icons.
  It is the bottom of a root's stack. uwui's own, Plain, Glass, Material 3 Expressive and Pro are
  looks. A look holds sets of values for light, dark and high contrast. The densities apply over any
  look.
- A look is a value, built in code as an official crate gives it, or read from texts that the host
  gives: a game's cooked files, a mod's
  ([ADR-0224](0224-a-files-change-reaches-the-application-through-the-development-tool.md)).
- The one call takes it first. A host that embeds uwui gives it with each root that it adds: the
  engine's editor names Pro, a game names its own, a Kotlin program names it as it attaches, a web
  page as it mounts uwui in a canvas. `uwui new` asks which look and writes it into the call.
- A root is given a *target*, where it draws, not a surface: a window, a region of the host's
  image, a texture. A game's HUD and its menus share its window; a root placed in the world has
  none.
- *A root's look may be replaced while it runs,* its state kept: the components keep their
  behaviour and state on their root elements (ADR-0222); only their parts are made again.
- A layer that uwui opens over a root, a menu, a tooltip, a list's options or a dialog, takes that
  root's look, and follows it when it is replaced.
- A look's texts that do not read are an error given to the host, which names another look. A
  replacement that does not read leaves the root's look in place, with its error, as a style that
  does not read keeps the last good one (ADR-0224).
- uwui's own design system is no longer "the default" of ADR-0190. It is one look among five. The
  core's *parts* stay the fallback of every look, for a component whose parts the look lacks
  (ADR-0222).

**An application's files stack over its look.**

- From the bottom: the look; the application's themes and sheets; the element's own style in code;
  the user's layer.
- Values of a higher layer replace a lower one's by their names. Rules add, in order.
- ADR-0223's order of styling holds: first the bases (the look's rules, the themes' and the
  sheets', then the element's own style), then the states (the rules', then the element's own),
  then the user's layer.
- *A component's parts given by a higher layer replace the lower one's whole,* never merged part by
  part, as a `ControlTemplate` replaces a control's tree. Rules still select parts by their names
  across layers: a look's rule styles a part of the same name that the application's parts hold,
  and the higher layer wins field by field. The themes' test kit checks it.
- High contrast: the look gives its values; the user's layer applies them (ADR-0223).

**One macro names an application's files.** `uwui::files!("ui")` reads a folder: layouts, themes,
sheets and parts. It generates the layouts' types and methods, which ADR-0225's `layouts!` did. It
embeds every file in every build, development builds included, since the web and Android cannot see
the author's files (ADR-0224). Under `uwui dev`, the text sent replaces the embedded one. When the
macro is built, two things are shown. A plain `cargo build` ships each file of the folder as saved,
a file added or removed included, which stable Rust's macros do not track by themselves. And a save
under `uwui dev` does not make `dx` build again. Each file is read when the program is built, as
the layouts' names are checked (ADR-0225): one that does not read fails the build.

**A look follows the system's preferences,** each with its changes:

- light or dark, not the accent (ADR-0190); with no preference, the one that the look's author
  chose;
- more contrast: the look's high-contrast values;
- *forced colours*, Windows' contrast themes and the web's `forced-colors`: the system gives a few
  colours by their *roles*, as CSS's system colours: text and its background, selected text and its
  background, disabled text, links, a button's face and its text. Any look keeps its shapes and
  takes these colours. Every other colour gives way, the sheets' literals and code's alike, as CSS's
  forced colours replace an author's. An element whose colour is its content may opt out: a swatch
  of colour, an image, a game's view;
- less motion: travel, scale, shape and blur change at once; opacity and colour keep a brief fade.
  A motion that lasts, a wheel of waiting or a wave of progress, stops and shows a still state that
  its part gives;
- less transparency: glass and backdrops drawn opaque;
- the size of text: a curve from the system's setting to each size of text, since Android 14's
  scale is not linear;
- saving power, on battery or in the system's saving mode: effects lighten, as ADR-0190 asks.

A *reader of the system's preferences*, one small crate of uwui's with a part for each platform
chosen when it is compiled, reads them. uwui's hosts call it, and so does the engine's platform
layer, so that the editor follows them on Linux too. Where a platform says nothing, uwui takes a
default: no preference of light or dark, normal contrast, no forced colours, full motion and
transparency, text at the look's sizes, not saving power. The user changes each in the application's
settings, over the system's. On Linux it reads the portal; on a desktop whose portal is older than
its reduced motion (1.21.0), GNOME's own settings. Whether the first frame waits for it, which a
portal that does not answer can hold 100 ms (see [`debt.md`](../debt.md)), against the budget of the
start, is the reader's milestone's. Its dependencies are put to the owner when it is built: `zbus`
on Linux, already in the engine's tree through AccessKit's adapter, and `windows`, already in its
lockfile, for Windows' `UISettings`.

**Plain is a variant of the core's look,** the start of a designer's own look:

- the core's parts, with values alone: no radius, shadow, tint, blur, motion or accent;
- the colours of forced colours' roles: black and white by default, black on white in light and
  white on black in dark; the system's palette under forced colours;
- lines one logical pixel wide, placed on whole pixels, one pixel of the screen at least;
- states shown by their shape: pressed inverts, hovered underlines, disabled dashes, selected marks;
- the ring of the focus twice the lines' width, with a gap, so that it never reads as an element's
  own line;
- Geist and Geist Mono, and the three densities.

It is usable and accessible from its first frame. By default, its contrasts exceed WCAG's 4.5:1 for
text and 3:1 for a component's edges and states, since its colours are the roles' extremes. Its name
is *Plain*: "blank" stays the blank window's, the program measured.

## Alternatives considered

On the start:

- **`run` on the desktop and the web, without an attribute;** Android and iOS with calls of their
  own: no macro. But an application writes lines for each phone, against the budget of porting.
- **No facade,** the application naming its host's crate: nothing to build above. But several
  dependencies, and a call for each platform.
- **`run` in the desktop host's crate:** no new crate. But the application names two crates, and
  the call differs on the web.
- **`run` in the core, behind a feature:** one crate. But the core would depend on the renderer,
  which depends on the core.

On the look:

- **A look fixed for a root's life:** simpler to build. But changing it makes the root again and
  loses its state; Opium could not change its theme in place.
- **The core's look when none is named:** one line less in "Hello, World!". But a choice made in
  silence, against ADR-0215.
- **One look for each application:** one place to name it. But the engine's editor and a game's
  interface differ in one program.

On the stack:

- **Parts replaced part by part,** the thumb alone: finer. But a contract to merge trees that often
  hold on one line, and two themes mixed in one component.
- **Parts replaced whole, the look's rules for that component dropped too,** as WPF's templates
  do: no style leaks from one look to another. But each replacement is styled again from nothing.

On the files:

- **A macro for each file,** `layouts!` and another for themes and sheets: each file named. But two
  ways to name a file, and a line for each file added.
- **Files read at run time,** no macro for themes: nothing to compile when a file changes. But it
  fails on the web and Android, and a package that forgets them starts without its look.

On the preferences:

- **Five preferences, each host reading its own:** less to build. But no saving of power, against
  ADR-0190; and the engine writes the reader again, or its editor follows nothing on Linux.
- **What winit gives, the rest later:** nothing to build now. But Linux follows nothing, and M6d.5's
  API is fixed with a gap.

On less motion:

- **Every transition ended at once,** as today: nothing to change. But a colour that jumps is harder
  to follow, and a motion that lasts has no rule.
- **Each look deciding:** free. But a look may keep travel, and the test kit checks each look.

On blank:

- **Two colours, a look of its own:** more radical. But fewer roles than forced colours, disabled
  text, a selection and links alike, and a second path to test beside forced colours.
- **No fifth look,** the core's sober look as the start: nothing to build. But a designer first
  undoes the core's radii, accent and motion, and `uwui new` loses a choice.
- **"Blank" kept, the program renamed** "the bare window": the owner's word kept. But more places
  change: ADR-0213, both repositories' budgets, M6d.7, the glossary.
- **Both names kept,** each defined: nothing renamed. But a lasting confusion in texts and figures.

## Consequences

- uwui's first stage builds the facade and its attribute, the look as a value, a root's look
  replaced, `files!`, the reader of the system's preferences and Plain. The themes' test kit renders
  Plain and forced colours too.
- Less motion changes today's behaviour: `uwui`'s transitions end at once under it
  (`Ui::set_reduced_motion`); uwui's first stage keeps their brief fades.
- M6d.5's API carries a target and a look for each root, a look replaced, and the system's
  preferences with their changes; its table checks them against the five platforms.
- M6d.7 adds a check that the engine's programs compile none of uwui's hosts.
- M6d.10 names the facade and the reader of preferences.
- The engine's editor follows the system's preferences on Linux once its platform layer calls the
  reader, when it moves onto uwui's 1.0 at Stage 6c's resumption.
- The sources of saving power on each platform were not read: the reader's milestone reads them.
- A look from another crate costs an application a second dependency, which keeps its own
  version.
