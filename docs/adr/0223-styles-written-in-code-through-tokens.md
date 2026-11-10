# 0223. Styles written in code, through tokens, under the user's layer

- Status: Accepted; amends [0141](0141-style-sheets-and-themes.md) (the element's own style takes
  states and names the themes' values by tokens; the user's layer comes over it),
  [0149](0149-the-looks-foundations.md) (the density and the contrast reach what code writes as
  literals too) and [0181](0181-uwuis-scope.md) (without `serde`, no theme written in code); amended by [0226](0226-an-application-started-in-one-call-each-root-given-its-look.md) (the bottom of the stack is any look, not the core's; under forced colours, the sheets' literal colours give way too), then by [0230](0230-the-cores-roles-and-each-kinds-defaults.md) (a token resolved when the interface takes its themes, read when an element is styled; the core's seven roles)
- Date: 2026-11-09

## Context

Today an element's code gives it classes, and the style sheets' rules style it. Its own style, set
in code, wins over the rules field by field, but holds no states: "States are therefore styled by
rules alone" ([ADR-0141](0141-style-sheets-and-themes.md)). It names no theme's value: a colour in
code is a literal. Themes stack, the engine's look first, "then a project's theme, then a user's"
([ADR-0149](0149-the-looks-foundations.md)). The density and the contrast are themes of values
stacked on the look: a compact theme, a theme of high contrast. Three densities apply over any
theme ([ADR-0190](0190-uwuis-requirements.md)). M6d.2 types styles per component.
[ADR-0181](0181-uwuis-scope.md) has a program without `serde` write its theme in code.

The references write styles in code: GPUI and gpui-kit through methods with states, `hover` and
`active`; Floem through `.style(|s| …)` with typed properties; SwiftUI through modifiers; Iced
through each widget's `.style(|theme, status| …)`
([uwui's survey](https://github.com/HoneySlatt/uwui/blob/main/docs/survey-writing-an-application.md#7-styles-written-in-code-beside-themes)).
[ADR-0222](0222-what-a-theme-is.md) put the themes' parts in data files.

A style in code reloads through `subsecond` and `dx`
([ADR-0212](0212-a-functions-body-reloaded-through-subsecond-and-dx.md)), 1.7 s at the size of
Slatty's interface
([measured](https://github.com/HoneySlatt/uwui/blob/main/docs/measurements.md#a-functions-body-reloaded-through-subsecond-2026-11-07)).
A file reloads through `uwui dev` in a tenth of that or less: its interval, then a few
milliseconds of the program's
([ADR-0224](0224-a-files-change-reaches-the-application-through-the-development-tool.md)).
The first proposal was attacked before it reached the owner. A literal size or colour in code
would beat the density, the contrast and the size of text that the user chose. A base set in code
would silence the states that the rules give. A theme's value named in code would be found missing
only when an element using it is styled. The owner decided on 2026-11-09.

## Decision

**An element's own style is written in code, where the element is made,** in M6d.2's types, with
its states:

```rust
label(title)
    .class("title")
    .style(|s| {
        s.padding(12)
            .background(SURFACE)
            .hover(|s| s.background(SURFACE_HOVER))
    })
```

**A theme's value is named in code by a typed token.** `pub const SURFACE: Token<Color>` names the
value `surface` and gives a fallback. The core declares the roles that every theme gives; a theme's
crate or the application may declare their own, as an application names a theme's classes today.
A token is resolved when the element is styled: a change of theme styles again and runs no view.
Each crate lists its tokens. A debug build checks them when the interface takes its themes, and
the themes' test kit checks them against each official theme. A token that the theme lacks takes
its fallback, with one warning; never a panic.

**Each field is styled in this order:**

1. the bases: the rules of the core's look, of the themes and of the application's sheets, then the
   element's own style;
2. the states: the rules', then the element's own (`hover`, `pressed`, `focus_visible`,
   `disabled`);
3. the *user's layer*, what the user chose over every look:
   - the size of text scales every text's size, wherever it was written;
   - the density, which its theme gives as values to the rules and the tokens, also scales the
     lengths that code writes as literals, by the factor that the density's theme gives;
   - under high contrast, which its theme gives as values too, the colours that code writes as
     literals give way to the rules', as CSS's forced colours replace an author's.

A literal colour in code stays possible for the other looks; the themes' test kit flags it.

**Each call of `style` keeps the place of the code that made it** (`std::panic::Location::caller`),
so that the live inspector and agents find the line to change.

**Rules stay in files.** No style sheet is built in Rust.

## Alternatives considered

- **Rules written in code as well**, a style sheet built in Rust with the files' selectors: an
  application written in Rust alone, and M6d.3's theme without `serde` already there. But two ways
  to write a rule, and rules that reload in 1.7 s rather than through a file's interval.
- **No style in code**, classes alone, as today: one place for every style, the fastest reload. But
  each panel's own margin needs a class and a rule, which ADR-0141 already weighed; every rival
  writes styles in code.
- **The code's style last**, over everything, as ADR-0141 has it: the simplest order. But a
  literal `padding(12)` would beat the user's density, contrast and size of text, and a base set in
  code would hide the rules' hover.

## Consequences

- M6d.2 types the element's own style with its states, tokens and their fallbacks, and the order
  above. It follows the debt that the code cannot set a field to its default against a rule
  ([`debt.md`](../debt.md)).
- M6d.3, whose themes without `serde` were to be written in code (ADR-0181), reads or turns the
  themes' files into values another way: M6d.3 decides how.
- A change of a style in code is a change of a function's body: within the budget of a change
  seen only through `dx`, at the sizes measured. A change in a file is seen after `uwui dev`'s
  interval and a few milliseconds of the program's.
- The themes' test kit grows checks: each token given, and literal colours flagged.
