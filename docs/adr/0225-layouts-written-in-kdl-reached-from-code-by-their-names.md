# 0225. Layouts written in KDL, reached from code by their names

- Status: Accepted; amends [0080](0080-reflection-and-serialization.md) (layouts, the engine's
  UI documents among them, are KDL; other text stays TOML), [0132](0132-the-engines-ui-framework.md)
  (the notation of UI documents decided: uwui's layouts), [0141](0141-style-sheets-and-themes.md)
  (a layout gives an element its classes too), [0181](0181-uwuis-scope.md) (layouts and a theme's
  parts are KDL; the other documents stay TOML), [0190](0190-uwuis-requirements.md) (the format of
  layouts decided), [0210](0210-signals-named-for-a-reload.md) (an element that a layout names is
  keyed by its name, not its place), [0220](0220-the-roadmap-reshaped-by-the-market.md) (M6d.1d2
  cut in two) and [0222](0222-what-a-theme-is.md) (a theme's parts written in KDL, in a file of
  their own beside its values and rules); amended by [0226](0226-an-application-started-in-one-call-each-root-given-its-look.md) (M6d.1d2b cut in two; `layouts!` becomes `files!`; blank renamed Plain)
- Date: 2026-11-09

## Context

[ADR-0190](0190-uwuis-requirements.md) has an application written "in Rust for its logic, and in
a format of files for its layout, both reloaded at once". It left that format to Stage 6d, and with
it the notation of the engine's UI documents, the games' screens as data, which
[ADR-0132](0132-the-engines-ui-framework.md) had left to Stage 7. Agents write screens in that
format and see them at once; a visual editor of games' interfaces works over it at Stage 11
([ADR-0209](0209-platforms-the-editor-and-agents.md)). A theme's parts are written in the same
format ([ADR-0222](0222-what-a-theme-is.md)). `uwui dev` sends a changed file's text to the
application ([ADR-0224](0224-a-files-change-reaches-the-application-through-the-development-tool.md)).
The engine's text is TOML ([ADR-0080](0080-reflection-and-serialization.md)), its scene files
among it ([ADR-0089](0089-scenes-and-prefabs.md)). [ADR-0181](0181-uwuis-scope.md) has uwui's
documents read as TOML: its themes, style sheets, shortcuts and docks' layouts. The format of
screens did not exist then.

Of the references, only Makepad and Slint write layouts outside Rust, each in a language of its
own with expressions
([uwui's survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#2-the-format-of-layouts)).
Frameworks that write a tree in a file and the logic in code reach the file's elements by their
names: some through accessors generated and checked when the program is built, as Android's View
Binding and XAML's `x:Name`; others by looking names up while the program runs, as Qt's
`findChild`
([read on 2026-11-09](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#notations-for-trees-and-code-that-reaches-a-files-elements)).

The draft of this decision was attacked before it reached the owner. The attack found that the
draft gave the file no way to name what a button does, so that every press would wait for a patch
of Rust. It found that kinds of components from other crates were needed at once: a theme's own
components (ADR-0222) and the engine's layer for games (ADR-0209). It found that "no code of the
application runs" on a reload was not true: the bindings' closures run again. And it counted too
many choices for one milestone.

The same screens were written in each notation, KDL 2.0, TOML in four forms and a subset of XML,
and counted: KDL took the fewest tokens by every count, well ahead of TOML and XML
([counted](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#the-notations-of-layouts-counted-2026-11-09)).
TOML 1.0 lets an array span lines, not an inline table; TOML 1.1 lets an inline table span lines
too, but taplo, a tool that editors use for TOML and whose author has left it, refused TOML
1.1. TOML's specification promises no order of a table's pairs. KDL 2.0 is final, and
its tools are young
([read on 2026-11-09](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#notations-for-trees-and-code-that-reaches-a-files-elements)).

The owner decided on 2026-11-09, one question at a time.

## Decision

**M6d.1d2 is cut in two,** in the order of the questions' dependencies:

- M6d.1d2a: the format of layouts, its notation and its reload, timed against the budget of 2 s;
- M6d.1d2b: an application started in one call that names its look, blank defined, the time of
  each frame that motion reads, and M6d.5 updated.

A look holds parts, written in M6d.1d2a's notation.

**A layout is a screen's structure, as data.** A file of layouts holds named layouts. Each is a
tree of elements:

- the core's elements and components: `column`, `row`, `stack`, `label`, `button`, `image`,
  `pressable`, `search-field`, `grid`, `list`;
- each with an optional name, unique within its layout, its classes, literal properties (a label's
  text, a field's placeholder) and children;
- *slots*, named places that code fills with a view;
- an element may name a *command*, `action=page.close`, as a table of shortcuts names one
  ([ADR-0142](0142-input-and-focus.md)): a press runs the handler that the application gives for
  it, the one that a shortcut naming it runs. A button for an existing command then works after
  `uwui dev`'s interval, without a change of code. Code may not bind a press of its own on an
  element that names a command: the macro below refuses it, and a reload notes it;
- a kind from another crate is named with that crate's prefix, `m3e.fab`: a theme's own
  components, the engine's layer for games, an application's own components. The crate registers
  the kind with uwui, as it registers the kind's builder.

The file holds no expression, no condition and no loop. A list or a grid gets its items from code.
What shows or not is a slot that code fills, or a named element whose display code binds. The
file holds no style: an element takes classes, which the style sheets style
([ADR-0141](0141-style-sheets-and-themes.md)), and its own style is written in code
([ADR-0223](0223-styles-written-in-code-through-tokens.md)).

**Code reaches a named element by its name,** with the methods that an element made in code has.
A *binding* is what code gives a named element so: a value, a handler, a slot's view.

```rust
uwui::layouts!("ui/library.kdl");

// `shown`, `cover` and `game_page` are the application's own functions.
fn library() -> impl View {
    let filter = RwSignal::kept("filter", String::new);
    let page = RwSignal::kept("page", || None);
    layouts::Library::new()
        .search(|field| field.value(filter))
        .covers(|grid| grid.items(move || shown(filter), |&game| game, cover))
        .page(move || page.get().map(game_page))
        .on_command("page.close", move || page.set(None))
}
```

- A macro generates these methods from the file when the program is built: a type for each
  layout, in `UpperCamelCase` (`game-page` is `GamePage`), and a method for each name, in
  `snake_case` (`key-art` is `key_art`). A name or a kind that code binds and the file lacks is an
  error of the compiler, and so is a name that is a keyword of Rust or that two names become once
  converted.
- Underneath, bindings are made by name while the program runs. A layout from outside, a mod's, is
  bound that way, its names checked as it is read.
- The macro's methods are generic, compiled in the application's crate. They reach uwui's runtime
  through functions neither generic nor inlined, as
  [ADR-0212](0212-a-functions-body-reloaded-through-subsecond-and-dx.md) asks of what a patch links
  again.

**A name keys its element.** A named element is placed under its name among its siblings; an
unnamed one under its place among the unnamed ones. The scopes below an element take its key into
their paths. An element added before a named sibling leaves the named one's state in place: its
scroll, its selection, its focus, and its kept signals across a patch of Rust too. This answers,
among siblings, the limit that [ADR-0210](0210-signals-named-for-a-reload.md) records: an element
added before a dynamic view. Two limits stay. A name keys its element among its siblings alone:
an element added before an unnamed ancestor, or a named element moved under another parent,
changes its path, and its state starts again. And what a layout's change makes again starts again:
the state that the views of its slots and rows own, their kept signals among it.

**A layout's change is applied between two frames.** `uwui dev` sends the file's text
(ADR-0224). The interface reads it, then makes its instances again:

- the view's function does not run again;
- the bindings that it gave are applied again to the new elements by name, and their closures run
  once each, to give the values that they show;
- what the layout holds below a slot or a list is made again: the views of the rows that show and
  of the slots;
- elements matched by their keys keep their own state; signals owned by the view's scope keep
  theirs.

A file that does not read keeps the last good layouts, with its error, its file and its line. A
binding whose name the new file lacks, or whose element is now of another kind, waits unused, with
a note; it applies again once its element comes back. A new name that code must bind needs a
change of Rust, a patch through `dx`.

**The notation is KDL 2.0.** In a layout, a node is an element: its node's name is its kind, its
first argument its name, its properties its classes, texts and command:

```kdl
layout game-page {
    column class=page {
        button back text=Library action=page.close
        image key-art class=key-art
        label title class=title
    }
}
```

**A theme's parts are written in KDL too,** in a file of their own beside the theme's values and
rules, which stay TOML. In a component's parts every node is a part, whose node's name is the
part's name, and `slot=` names the slot of the application's content that the part shows.
ADR-0222's switch in Material 3 Expressive, its parts:

```kdl
parts switch {
    track { thumb { icon slot=icon } }
}
```

Its rules select the parts by their names, as ADR-0222 writes them.

**The other documents stay TOML:** themes' values and rules, style sheets, shortcuts, settings and
docks' layouts, which hold tables, and the engine's scenes and saves.

**uwui reads KDL with a reader of its own,** written in its first stage: its depth bounded, its
input's size and time within limits, fuzzed in CI, never crashing, as all content from outside is
read ([ADR-0190](0190-uwuis-requirements.md)). It writes a document back with its order, its
formatting and its comments kept, as agents and the visual editor need. The prototypes read KDL
through the crate `kdl` 6.7.1, which M6d.1d2a found to overflow its stack on some inputs: its
reading recurses without a bound. A look at the text before it reads guards the prototype from the
cases found, not from every input.

**Measured on M6d.1's bench.** Slatty's library written as layouts, played to a game's page, its
file saved with a label added before the grid and taken away again: each frame showed the change
with the scroll, the selection, the filter, the field's text and the page kept, and the view's
function never ran again. The program's part, from the save to the frame's display list, is far
within the budget of 2 s, in a release build and in an author's debug build
([uwui's figures](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#a-layouts-change-seen-2026-11-09)).
The whole, through `uwui dev`'s watcher, its interval and its connection, is measured when the
tool is built. The prototype keyed an unnamed element by its place among all its siblings; the
change measured moved only named ones. That focus is kept, and that kept signals stay across a
patch after an element is added, were not tried.

## Alternatives considered

- **Names bound by strings alone,** without a macro, as Qt's `findChild` and a game engine's
  interface toolkit look them up: nothing generated, and every change reloads. But a name mistyped
  shows only when the program runs.
- **A language with bindings, conditions and loops in the file,** as Slint's and QML: more changes
  reload without compiling. But a second language holds logic, against ADR-0190; it needs an
  interpreter; its errors of type show while the program runs.
- **No layout in files,** the screens in Rust reloaded by `dx`, the themes' parts alone as data:
  one way to write. But ADR-0190 rejected Rust alone; ADR-0209's editor writes files; `dx`'s time
  grows with the crate
  ([measured](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#a-functions-body-reloaded-through-subsecond-2026-11-07)).
- **TOML for every document,** the trees in positional arrays: one notation, no dependency. But
  many more tokens than KDL, and screens less readable. Inline tables nested over several lines
  need TOML 1.1, which taplo refuses; flat tables lose the tree
  and take their order from a table, which TOML does not promise.
- **KDL for the screens alone,** the parts kept as TOML inline tables in the themes' files: a theme
  in one notation. But two notations for trees, against ADR-0222's one format for both, and
  sibling parts whose order TOML does not promise.
- **KDL for every document from M6d.3,** themes, sheets, shortcuts and docks: one notation in the
  end. But every theme and sheet of the engine rewritten. M6d.3 weighs it for themes.
- **A subset of XML:** the notation that agents and tools know best. But some twice KDL's tokens.
- **A markup of uwui's own,** mirroring Rust's builders: any syntax, no dependency. But a parser, a
  grammar, a formatter and a language server to write, and a notation no agent knows.
- **The crate `kdl` in uwui,** the reference reader of KDL 2.0: the whole specification, and a
  document written back with its formatting kept, nothing to write. The owner chose it first.
  But its reading recurses without a bound and overflows its stack on inputs that an author
  rarely writes and a mod may: bad input would crash the program. It also brings `miette`,
  `unicode-width` 0.1 and a second `winnow`.
- **The crate `kdl` on a thread whose stack grows with its input,** its input's size capped:
  simple. But a guard, not a fix, and memory reserved as the input grows.

## Consequences

- uwui's first stage builds the reader and its writer, the macro, the bindings, the reload and
  the registry of kinds that other crates add. `uwui dev` sends layouts as it sends themes and
  sheets (ADR-0224). The official tests of KDL's specification, which the reader is checked
  against, are downloaded then, put to the owner first.
- uwui takes no dependency for KDL. The prototypes' `kdl` stays on their branch, thrown away with
  it.
- A program that reads layouts from outside ships the reader. Its weight in a package for the web,
  against [the budget of 1 MB](0181-uwuis-scope.md), is measured when an application ships it.
- Editors' support for KDL 2 is young: some editors' grammars lag, and its language server gives
  diagnostics alone. Agents have read KDL 1 the most, which differs from KDL 2 on keywords and raw
  strings. The agent bench measures how agents write it (ADR-0213).
- A tool can write a layout back with its order and comments kept, as agents and the visual editor
  need.
- What `dx` does when a file that the macro reads changes is checked when the macro is built: a
  rebuild that restarts the program would lose the state at each save.
- Games' screens as data, the engine's UI documents, take this format, read by uwui's reader: the
  cooker reads KDL beside the scenes' TOML.
- M6d.3 decides the cooked form of layouts and parts with the other documents', and weighs
  writing a theme's values and rules in KDL too, so that a theme reads in one notation.
