# 0231. An element's own style written in code, by declarations with states

- Status: Accepted; amends [0141](0141-style-sheets-and-themes.md) (an element's own style is the
  declarations that its code wrote, with states; it no longer wins over every rule),
  [0223](0223-styles-written-in-code-through-tokens.md) (the order of styling read field by field:
  the own base gives way only to a rule that names a state; eight states, not four; M6d.2b2 cut in
  two, tokens in its second half), [0226](0226-an-application-started-in-one-call-each-root-given-its-look.md)
  (the same order, in its stack) and [0229](0229-styles-typed-as-declarations.md) (an array is one
  leaf for each element; the own style is declarations, not a `Style`); amended by
  [0232](0232-transitions-read-the-derives-colours-and-share-what-they-show.md) (M6d.2b2b cut in
  three) and [0237](0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md) (the fourth step of the mask order is the size of text), then by [0238](0238-tokens-typed-named-in-code-declared-by-each-crate.md) (a field of text named by a theme and `subsecond` leave M6d.2b2b)
- Date: 2026-11-10

## Context

An element's own style is a whole `Style` that every constructor takes
(`Element::label(text, Style::default())`). The cascade applies the kind's base, the selected
rules' declarations in their order, then the fields of the own style whose bits differ from
`Style::default()` ([ADR-0229](0229-styles-typed-as-declarations.md)). So the own style wins over
every rule, its states included; it holds no state of its own; a field set to its default is not
seen; an array set by code, a size, replaces the rules' whole.

[ADR-0223](0223-styles-written-in-code-through-tokens.md) decided styles written in code with
states, the themes' values named by tokens, the order "the bases (the rules, then the element's own
style), then the states (the rules', then the element's own), then the user's layer", and the
place of each call kept. M6d.2b2's card held ten deliveries and the migration of the call sites:
about 660 calls of constructors, 111 of `Ui::add_root` and some 500 assignments of a style's field,
235 calls and 222 assignments outside uwui.

The lead's draft was attacked twice before it reached the owner. The first attack found that
ADR-0223's order read to the letter, every rule with a state after every rule without, changes
what the editor draws: in its look, `[button-hovered]`, `[button-pressed]` and `[button-focused]`
come before the rules of a list's rows, a row's marks, links and a notification's close button,
which clear their background and border; 13 pairs on real buttons would draw otherwise, in states
that no image test captures. It found that the styles made, shared by rules and own style, would
serve an own hover to an element not hovered; that tokens have no reader in the repository until
the user's layer, which tells a literal from a token; and that the call sites are many. The second
attack found that keeping the constructors' `Style` would leave both debts of an own style on all
the code of the repository, and that an own hover or press changing the element's box contradicts
[ADR-0222](0222-what-a-theme-is.md)'s rule that a component's box depends on neither. The owner
decided on 2026-11-10, one question at a time.

## Decision

**M6d.2b2 is cut in two,** behaviour first, each abstraction with its first reader:

- M6d.2b2a, an element's own style written in code: its declarations with states, the order, the
  call's place, the two debts of an own style, the call sites;
- M6d.2b2b, tokens and the user's layer: a declaration's payload (`Spec<T>`, and whether it records
  the state that set it), the token as a public type and an application's own tokens and their
  lists (colours, lengths and durations), the derive's list of colours and lengths with
  transitions reading it, the size of text, the density's factor on the lengths that code writes,
  the colours that code writes giving way under high contrast, a field of text named by a theme,
  and `subsecond`. Its questions are put at its start.

**An own style is the declarations that code wrote,** with their states, built by
`Element::style`:

```rust
Element::button("Save")
    .class("primary")
    .style(|s| {
        s.padding(12.0)
            .background(surface)
            .hover(|s| s.background(surface_hover))
    })
```

- What code sets is kept, whatever its value: a field set to its default wins over a rule.
- The constructors take no style; `Ui::add_root` and `Ui::open_layer`, whose root has no
  constructor of its own, take the function that `Element::style` takes. Every call site of the
  repository moves now, uwui's and the engine's: one way to write a style.
- `Style` stays the type that layout, paint and transitions read: `Ui::style` gives it, computed.

**Each field is styled in this order,** one pass, the *mask order*:

1. the kind's base, then the selected rules in their order, stateful or not: of two rules that set
   one field, the later wins (ADR-0141);
2. the own base, for each field, unless the last rule that set it names a state;
3. the own states that hold, in a fixed order, the last winning: `hovered`, `focused`,
   `focus_visible`, `pressed`, `checked`, `expanded`, `selected`, `disabled`;
4. the user's layer, at M6d.2b2b.

A rule's state beats a base set in code, as ADR-0223 wants, and no rule changes its place against
another: nothing that the look draws changes. A hover set in code alone wins over the look's press
too; an element given one should give a press. States reach only the kinds that take them: the
pointer hovers and presses what takes it, buttons and fields, not a label or a box; a debug build
warns once for each kind and state that an own style names in vain.

**A hover or a press set in code cannot change the element's box,** as ADR-0222 has it: their
builder offers no field that lays out (the layout, the border's width, the text's style, a
control's size). The compiler refuses the call. Rules in files are not checked so until rules are
typed for each state, at uwui's first stage.

**An array is one field for each of its elements:** `gap`, `size`, `min_size`, `max_size` and
`radii`. Code sets a width without a height, a gap between columns without one between rows. A
style sheet still writes them whole, `size = [a, b]`, read as one declaration for each element.
Lists, a grid's tracks and a text's fonts and features, and options, a shadow, an alignment of its
own, a locale, are each set whole.

**Each call of `Element::style` keeps its place** (`std::panic::Location::caller`, through
`#[track_caller]`), with the declarations that it wrote, for the live inspector and agents. The
place is the compiler's hint, the path as Cargo gave it, relative to the workspace for its members,
with `\` or `/` on Windows; it is not normalized.

**The look shows the keyboard's focus on every button.** Its rules for a list's rows, a row's
marks, links and a notification's close button cleared the border that shows it; four rules give
it back, in a commit of their own.

## Alternatives considered

The cut:

- **The builder with its tokens first, the user's layer second:** ADR-0223's example works in the
  first half. But tokens with no reader in the repository until the second, and a first half that
  holds both changes of behaviour, the tokens and the migration.
- **One milestone:** one review, but a long session before its first check.
- **Three, the migration apart:** smaller milestones, but two ways to write a style between the
  first and the second.

The order:

- **ADR-0223's letter,** every stateful rule after every stateless one: no amendment, and the focus
  of rows shown again. But some eight rules added to the look to keep the rest of its drawing, and a
  rule written later losing to one written earlier, which ADR-0141 rejected.
- **The own style last, as before:** the simplest. But a base set in code silences the rules'
  hover, which ADR-0223 rejected.

A hover or a press that changes the box:

- **Allowed, documented:** one builder, the freedom of the files. But ADR-0222 amended, and the
  risk of a box that leaves the pointer and comes back without end left to the author.
- **Refused in files too:** one rule everywhere. But a change of the sheets' format, beyond this
  milestone.

The call sites:

- **The constructors' `Style` kept, the builder beside it:** a small change, nothing moved before
  Stage 6c. But two ways to write a style, both debts kept on all the code of the repository, and a
  builder that nothing uses.
- **uwui and the examples moved, the engine's crates bridged until Stage 6c:** uwui leaves clean.
  But a third state, and the debts kept in the engine.

Arrays:

- **Set whole:** fewer fields. But setting a width still replaces the rules' height.

The focus:

- **Written as debt:** the milestone keeps its scope. But a fault of accessibility left in the
  editor and the dev overlay.

## Consequences

- An own style that set a field to its default now wins over a rule. Every call site moved writes
  only what it wrote apart from the default before, so that nothing drawn changes; the two fixtures
  of styles and the images check it.
- The styles made are shared by the rules selected, the kind's base, the own style's declarations
  compared by bits, and the own states that hold.
- The property test of M6d.2a keeps its oracle, reflection, for the rules; an own style's part is
  checked by a model of the mask order and by a test of every field; the test of a gap set whole
  over a rule is written again for arrays set by element, as the owner agreed.
- The derive numbers the leaves of a style, 66 of them, and says which lay out.
- uwui's authoring API, at its first stage, names the constructors anew; the bodies of the closures
  given to `style` stay.
- M6d.2b2b decides what a declaration holds at each colour and length, and the tokens.
