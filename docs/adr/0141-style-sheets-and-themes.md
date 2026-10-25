# 0141. Style sheets and themes

- Status: Accepted; amends [0140](0140-the-tree-of-elements.md) (a change of an element's state
  styles it again, which draws it again only where its style changed; a button's looks in its states
  come from rules, not from its code)
- Date: 2026-10-25

## Context

[ADR-0132](0132-the-engines-ui-framework.md) decided that an interface's style sheets and themes
are documents of reflected types, read and checked by `slime-serialize`
([ADR-0080](0080-reflection-and-serialization.md)), with named values that rules share and rules
selected by an element's kind, classes, key and states, cooked and loaded again while a program
runs ([ADR-0076](0076-hot-reload-of-cooked-data.md)), and matched again only when an element's
classes, state or place change. [ADR-0140](0140-the-tree-of-elements.md) built the tree of
elements, whose code gave each element its whole style, and left the style sheets to M6b.4b.

Building them raised what ADR-0132 left open: what a rule holds, where the named values live, which
of the code and a rule wins, which of two rules, what a rule may select by, where a document's types
are checked, and how a color is written. The owner decided each on 2026-10-25, among the options
that "Alternatives considered" lists.

## Decision

**A style sheet is rules, in the order of the file.** A style sheet, `slime_ui::theme::StyleSheet`,
is a document whose each table is a rule, named as its author likes: `select`, what elements it
selects, and `style`, the part of their style that it sets. The rules apply in the order of the
file, the sheets in the order given; of two rules that set one field of an element, the later wins.

**A rule sets a part of a `Style`.** `style` is a part of a value of `slime_ui::style::Style`, as
`slime-serialize` reads a save's differences: what it leaves out, it does not set; an option is a
list of none or one value, so that a rule can empty one. The style's types are reflected, under
names that start with `Ui`, so that no other type of the engine shares them; reflection lays a part
over a value (`Registry::overlay`).

**A rule selects by kind, classes, key, states, and an ancestor.** A rule selects an element of a
kind (`Container`, `Label`, `Image`, `Button`, `Scroll`, `List`), that holds classes, all of them,
of a key, in states (`Hovered`, `Pressed`, `Disabled`), all of them, and inside an ancestor of a
kind or a class, at any depth: every condition it gives holds. An element's classes are given by
the code that builds it (`Element::class`).

**The element's own style wins over the rules.** An element's style is the default style, then the
parts of the rules that select it in their order, then the fields that the code's style sets apart
from their defaults: the code keeps what it sets, as CSS's inline style does, and a field that it
leaves at its default takes the rules'. A color, an array, a list and an option are each one field:
the code's replaces the rules' whole. States are therefore styled by rules alone: a button's looks
in each state, which the code gave before, are rules now.

**A theme holds the named values; a sheet names them.** A theme, `slime_ui::theme::Theme`, is a
document of named values, colors, numbers, lists; a rule's field gives `"$name"` in place of its
value. A program styles an interface with sheets and a theme (`Ui::set_style`), and changes the
theme while it runs, the rules unchanged, as a dark theme and a light one share one sheet. A
theme's value names no other.

**A color is written in sRGB.** In a style sheet and a theme, a color may be given as
`"#rrggbb"` or `"#rrggbbaa"`, the notation of design tools, its alpha straight; the linear form
that every other document of the engine writes, `{ r, g, b, a }`, stays good there too.

**Names and colors through the reader's references.** `slime-serialize`'s reader offers each text
that stands where a value of a type other than text goes to its `References`
(`References::value`), which may give the value instead; `slime-ui` gives the theme's values for
`$name` and the colors for `#…`. No other document changes: their readers give nothing.

**Types are checked when the interface reads them.** The cooker cooks a style sheet, `.style`, and a
theme, `.theme`, into their documents' binary encoding, of the kinds `AssetKind::StyleSheet` and
`AssetKind::Theme`, checking their syntax alone, as it cooks a scene file. `Ui::set_style` reads and
checks their types, names and colors: a document at fault is refused, by its name and the place of
the fault, and the interface keeps the style it had, as a program that loads them again when they
change keeps going.

**Styled again where it changed.** Each element carries a third mark, its style, beside its layout
and its paint ([ADR-0140](0140-the-tree-of-elements.md)): a new element, a change of its style or
its state marks it, a change of its classes marks it and what lies below it, and new sheets or a new
theme mark every element. A frame makes the marked elements' styles first, then lays out what a
change of layout asks for and draws again what changed. A style whose layout, text or border's width
changed lays the element out again, the border cutting what scrolls inside it; one whose paint alone
changed draws it again. The styles made are kept by the rules that selected them and the element's
own style, and shared: elements of one style that the same rules select are styled once.

## Alternatives considered

- **A rule as paths of properties**, `"paint.background" = …`: freer names, checked only when the
  rule is applied, and a second way of writing a part of a value in the engine.
- **A type that mirrors `Style`, each field optional**: fast and typed in Rust, but two types to
  keep alike by hand at each property added.
- **The named values beside the rules, in one file**, replaced when the file is cooked: simpler when
  the interface runs, but another palette would need a copy of every rule.
- **No named values**: each rule gives its values, copied where they repeat; against ADR-0132.
- **The theme over the code's style**: a theme could restyle anything, its layout too, but the code
  could no longer keep the layout it needs.
- **Classes alone, no style in the code**: the clearest split, but every size of a panel's own
  would need a class in a file.
- **CSS's specificity**, the most precise rule winning: familiar to web authors, and a known source
  of surprises, a rule written later losing to one written earlier.
- **The element alone, or CSS's combinators**, for what a rule selects by: an element alone would
  need the code to give a class to each button of a toolbar; combinators, children, siblings, the
  n-th child, are more code and more cases than anything asks for yet.
- **Types checked by the cooker**, which would place an error at its line during a watch: the
  cooker would build `slime-ui` and its text, parley, swash and taffy, slower to build here and in
  CI.
- **Colors in linear values alone**: one notation, but each color of a design tool converted by
  hand.

## Consequences

- An editor's tree of 12 500 elements is styled again whole, after its theme changed, in 16 ms, a
  frame's hitch; its first frame, which styles every element, takes as long as before styles came
  ([`docs/measurements.md`](../measurements.md), M6b.4b).
- A theme's value names no other value, and a field of the type `String` takes no named value,
  since it takes a text as it is: a style holds none.
- An error of type in a cooked document is placed by its byte and the path of its part, not by its
  line: the cooker places by their lines only the errors of syntax that it checks.
- The code cannot set a field to its default against a rule: a field at its default takes the
  rules'.
- `slime-ui` depends on `slime-reflect` and `slime-serialize`.
