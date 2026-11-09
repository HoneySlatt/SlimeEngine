# 0222. What a theme is: values, rules and parts written as data

- Status: Accepted; amends [0141](0141-style-sheets-and-themes.md) (a theme holds parts too; rules
  select a component's parts and its states), [0190](0190-uwuis-requirements.md) (a theme builds a
  component differently through its parts, written as data),
  [0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md) (the official themes are crates of
  data, which may add components of their own) and
  [0220](0220-the-roadmap-reshaped-by-the-market.md) (M6d.1d cut in two); amended by
  [0225](0225-layouts-written-in-kdl-reached-from-code-by-their-names.md) (the parts written in
  KDL, in a file of their own) and [0226](0226-an-application-started-in-one-call-each-root-given-its-look.md) (a look given to each root and replaced while it runs; how parts and rules stack over it)
- Date: 2026-11-09

## Context

[ADR-0190](0190-uwuis-requirements.md) says that a theme "may build a component differently, its
shape and structure, not only recolour it".
[ADR-0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md) keeps one look in uwui's core and
makes Glass, Material 3 Expressive and Pro official themes, each a crate of its own.
[ADR-0220](0220-the-roadmap-reshaped-by-the-market.md) found that Stage 6d's next milestones would
lock in "a theme that cannot change a component's structure", and gave M6d.1d the question.

Today a theme cannot. A theme holds named values and a style sheet holds rules
([ADR-0141](0141-style-sheets-and-themes.md), [ADR-0149](0149-the-looks-foundations.md)). Each
component's *parts*, the pieces that draw it, are fixed in Rust: a check box's box and its mark, a
slider's track and its thumb (`uwui::style::ControlStyle`). A theme changes their colours and sizes.
It cannot add a part, remove one or order them otherwise.

[uwui's survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#9-themes-that-change-a-components-structure)
read ten references on 2026-11-09. Two let a theme change the parts' tree while the control keeps
its behaviour: Qt Quick Controls, through a QML file per control, and WPF and WinUI, through a
`ControlTemplate`. Both keep the control's behaviour and accessibility apart from its parts, under
a contract of named parts. Slint changes the tree too, but each of its styles owns its widgets
whole, behaviour and accessibility repeated in each. Flutter,
Compose and gpui-kit give another look as another library of components. Iced, Floem and
gpui-kit's JSON themes change values alone.

At its start, M6d.1d held what a theme is, styles written in code, the format of layouts and their
reload, an application started in one call that names its look, and the time of a frame that
motion reads. A first proposal, parts written in Rust as a view built from the component's state,
was attacked before it reached the owner. Such a view runs code for each instance, a list's rows
among them, and at each frame of a value that moves. It reloads through `dx` in one to two
seconds, may panic, and must be compiled. The owner decided on 2026-11-09, one question at a time.

## Decision

**M6d.1d is cut in two,** in the order of the questions' dependencies:

- M6d.1d1: what a theme is, styles written in code beside the themes' files, and a style's change
  reloaded and timed;
- M6d.1d2: the format of layouts and their reload, an application started in one call that names
  its look, and the time of each frame that motion reads; M6d.5 updated.

The structure that a theme may change decides what a layout describes, and the look that an
application names at its start is a theme.

**A theme holds values, rules and parts.**

- *Values*: named values, as today, typed by M6d.2.
- *Rules*: style sheets, as today. A rule may also select a component's part by its name, and the
  component's states: `Checked`, `Pressed`, `Hovered`, `FocusVisible`, `Disabled`, `Selected`,
  `Expanded`.
- *Parts*: for any component it chooses, the tree of named parts that draws it. It is written as
  data in the theme's files, in the format that M6d.1d2 chooses for layouts.

**The component keeps what it does.** Its behaviour, its state, its focus, its keys, its meaning
to screen readers and the area that the pointer hits stay on its root element. The parts are drawn
alone: they take no input and carry no meaning. One exception: a part may be a component that the
component names, as a list to choose from names its field, its list and its options, or a box that
scrolls names its scroll bars. Such a part keeps its own input and meaning, and its own parts come
from the theme too.

- *States reach the parts through rules.* A rule selects a part in the component's states. No code
  of the theme runs when a state changes.
- *Values that move reach the parts as shares read when drawn*, as live values are
  ([ADR-0205](0205-signals-change-uwuis-interface.md)): a slider's value, a switch's travel, a
  bar's progress. No view runs again for them, at any frame.
- *The application's content enters through slots*, which the parts place where they like: a
  button's label, a switch's icon.
- *A part that the component needs* for its geometry, as a slider's track to read the pointer's
  value, is named in the component's contract. When the parts lack it, the component uses its own
  box.
- *Parts that do not hold the contract fall back to the core's.* That is, a slot left out, or parts
  that do not read. The development tools note it. Nothing panics.
- *The component's box does not depend on `Hovered` or `Pressed`*, so that a part that shrinks
  under the pointer cannot make it leave and come back without end.
- *The core draws the ring of the focus,* around the part named `focus`, or else around the root.
  No theme can lose it.
- *A component for which the theme gives no parts takes the core's,* styled by the theme's values
  and rules. A community's theme may be values alone.
- *The core's own look is written the same way*: its parts are data too.

**The official themes are crates of data.** Each crate holds its values', rules' and parts' files,
its fonts and its icons, built into a release. It may also add components of its own, written in
Rust, which an application names knowingly; never one chosen in place of a core's.

**The example.** The same switch, under the core's look and under Material 3 Expressive, written
in the notation of today's style sheets and themes. Three things are new: a table of parts,
`[switch.parts]`; a part named in a selector, `part`; and a place along a share, `along`. The kind
`Switch` is the component's. M6d.1d2's format may change how it is written, not what it says. In
the core's look, the switch is a box holding a tick, filled once checked:

```toml
[switch.parts]
box = { mark = {} }

[switch-box]
select = { kind = "Switch", part = "box" }
style.paint = { background = "$track", radii = ["$radius_s", "$radius_s", "$radius_s", "$radius_s"] }
style.layout = { size = [{ Pixels = 18 }, { Pixels = 18 }], align_items = "Center" }

[switch-box-checked]
select = { kind = "Switch", part = "box", states = ["Checked"] }
style.paint.background = "$accent"

[switch-mark]
select = { kind = "Switch", part = "mark" }
style.layout.display = "None"

[switch-mark-checked]
select = { kind = "Switch", part = "mark", states = ["Checked"] }
style.layout.display = "Flex"
style.paint.background = "$mark"
```

In Material 3 Expressive, it is a pill whose thumb runs along it, grows once checked and shows the
application's icon; the track takes the primary colour once checked. Its values are the theme's
own, which its file gives:

```toml
[switch.parts]
track = { thumb = { icon = { slot = "icon" } } }

[switch-track]
select = { kind = "Switch", part = "track" }
style.paint = { background = "$surface_highest", radii = [16, 16, 16, 16] }
style.layout = { size = [{ Pixels = 52 }, { Pixels = 32 }], align_items = "Center" }

[switch-track-checked]
select = { kind = "Switch", part = "track", states = ["Checked"] }
style.paint.background = "$primary"

[switch-thumb]
select = { kind = "Switch", part = "thumb" }
style.paint = { background = "$outline", radii = [12, 12, 12, 12] }
style.layout = { size = [{ Pixels = 16 }, { Pixels = 16 }], along = "travel" }
style.transition = { duration = 0.2, easing = "EaseOut" }

[switch-thumb-checked]
select = { kind = "Switch", part = "thumb", states = ["Checked"] }
style.paint.background = "$on_primary"
style.layout.size = [{ Pixels = 24 }, { Pixels = 24 }]

[switch-icon]
select = { kind = "Switch", part = "icon" }
style.layout.display = "None"

[switch-icon-checked]
select = { kind = "Switch", part = "icon", states = ["Checked"] }
style.layout.display = "Flex"
```

The application's code is the same under both, `switch(enabled).icon(Icon::CHECK)`, and so is
what a screen reader hears: a switch, on. `along = "travel"` places the thumb at the switch's
travel, a share from 0 to 1 that the switch moves when it is pressed and that is read when the
part is drawn. It is a primitive of layout that uwui's first stage builds, with transforms and
transitions of layout.

## Alternatives considered

- **Parts written in Rust**, a view built from the component's state given as signals: anything
  can be written, typed and checked by the compiler. But the view runs for each instance, in a
  list of 100,000 rows as on a page, and at each frame of a value that moves, where live values
  exist so that no code runs. It reloads through `dx` in one to two seconds, may panic, and a
  theme must be compiled. Qt and WPF, the two models that change a tree under a control, write it
  in declarative files, QML with its bindings, and XAML.
- **Values and rules alone**, as Iced, Floem, Flutter's `ThemeData` and gpui-kit's JSON themes:
  nothing to build. But Glass, Material 3 Expressive and Pro could not differ in shape, against
  ADR-0190, and the core would hold every variant of every design system as options.
- **A library of components for each design system**, as Compose's Material, Flutter's
  `material_ui` and gpui-kit's components over `gpui-base`: any freedom. But an application's code
  names the design system's components, and changing its look changes its code, against
  [ADR-0215](0215-the-engine-measured-against-the-market.md)'s look named in one call.
- **A theme that owns the whole widget, its behaviour included**, as Slint's styles: no contract
  to design. But behaviour and accessibility are copied into each theme and drift.

## Consequences

- M6d.1d2's format describes a theme's parts as well as an application's layouts: one format for
  both.
- uwui's first stage builds the parts. That means named parts and slots, rules on parts and on a
  component's states, and shares read when drawn. It also needs a part placed along a share,
  transforms and transitions of layout, and the core's components written as parts. It also builds
  the *themes' test kit*, which renders each theme's components in each state, density and
  direction and checks their contracts.
- Each part is an element of uwui's tree. Their cost in lists is measured when they are built,
  against the budget of a list's frame: `ui.tree.list.100k`, and `editor.hierarchy.100k`, whose
  rows hold buttons.
- Each component's parts, states and slots become uwui's API, under its versioning: a theme
  written for one version names them.
- M6d.2 types rules for each component and each of its parts.
- M6d.11 moves the editor's look into Pro as data.
- A theme can be read while a program runs, without compiling: the community's themes, a game's
  mods within the limits of content from outside.
