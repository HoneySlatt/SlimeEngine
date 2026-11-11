# 0238. Tokens: the themes' values named in code, typed by kind, declared by each crate

- Status: Accepted; amends [0223](0223-styles-written-in-code-through-tokens.md) (a token has a
  kind; a crate declares its tokens to the interface, which reads them in every build, not a debug
  build alone; density and more contrast reach them by the themes' names alone),
  [0229](0229-styles-typed-as-declarations.md) (a declaration holds no payload: no token, no state,
  no origin), [0230](0230-the-cores-roles-and-each-kinds-defaults.md) (the core's roles are public
  tokens), [0231](0231-an-elements-own-style-written-in-code.md) and
  [0232](0232-transitions-read-the-derives-colours-and-share-what-they-show.md) (a field of text
  named by a theme and the reload through `subsecond` leave M6d.2b2b3). Two precisions decided by
  the owner at the milestone's closing review: a token declared again, of the same name, kind and
  fallback, is the same token, declared once, and only a name declared again with another kind or
  fallback is refused; a number that is not finite refuses the themes wherever it is read, a rule
  naming it too, and only a negative duration that a rule names stays debt until the look's tokens; amended by [0241](0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md) (a theme writes a value's kind by its group; a field of text names a theme's value)
- Date: 2026-11-11

## Context

[ADR-0223](0223-styles-written-in-code-through-tokens.md) has code name a theme's value by a typed
token, `pub const SURFACE: Token<Color>`, with a fallback; each crate lists its tokens; a token that
the themes lack takes its fallback with one warning.
[ADR-0230](0230-the-cores-roles-and-each-kinds-defaults.md) resolves names once, when the interface
takes its themes, and made the core's seven roles, kept private until tokens. ADR-0229 planned a
*payload* in each declaration: whether a value is a literal or a token, the state that set it, and
the origin of a rule's value. [ADR-0237](0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md)
left density and more contrast to reach what code writes through the themes' names, and left open
whether a token that a density's theme does not give grows by a factor.

Magnolia, the owner's audio workstation, asked on 2026-11-10: an application adds its own tokens,
colours, sizes, spacings and motion, for its own components, and replaces existing ones without a
fork; those components take every value from the theme, so that density, high contrast, less motion
and less transparency reach them; a duration never grows with density as a length does.

The facts, read in the repository and on 2026-11-10 and 11:

- Nothing reads a declaration's state or a rule's origin. ADR-0227's two clocks can be chosen for
  each field from the rule's selector and the own declaration's state.
- CSS's forced colours keep the system's colours: a colour is forced "if its computed value is a
  color other than a system color". A colour that names a role stays, under forced colours too.
- `uwui-macros`'s derive refuses generic types. `Ui::set_style` is called at 49 places. No code of
  the repository outside tests writes a colour in an own style.
- The look's themes accept a number that is not finite, and a negative duration, where a rule names
  it; the roles refuse a colour that is not finite.
- Declaring the editor's look's 29 scales as tokens raised the allocations of `set_style` by 197,
  and by 700 with its 39 colours, in a debug build; a counter that the parent milestone allows only
  to fall.
- The design-token format of the W3C's community group (DTCG, 2025.10) parts colour, dimension,
  duration and number; a tool "MUST NOT" guess a token's type from its value, and reports an alias
  that it cannot resolve. Slint parts `length`, `duration`, `color` and `float`. WinUI refuses a
  resource's key that no dictionary holds when the page loads. SwiftUI's environment keys,
  Compose's composition locals and CSS's registered properties each carry a default.
- `slime-serialize` lets a text stand for another value only where the type is not text; M6d.3
  replaces that reading with `serde`.

The lead's draft was attacked before it reached the owner. The attack found that forced colours
keep the system's colours, against the draft's reason for recording no origin; that tokens given
with a theme must be given again at each change of theme; that two crates' names may meet in one
space, a workstation's `track` becoming the look's; that themes accept values that are not
finite; that the look's tokens raise a counter; and that the milestone held twelve deliveries. A
prototype of a token beside its declaration allocated nothing more over 1 000 elements. The owner
decided on 2026-11-11, one question at a time.

## Decision

**M6d.2b2b3 is one milestone, without the editor's look's tokens.** The look declares its tokens
when it becomes a value, at uwui's first stage; until then its names are typed where a rule names
them, as today.

**A token rides beside its declaration.** A setter of an own style takes a literal or a token of
its kind; the own style keeps the declaration, holding the token's fallback, and the token's name.
The cascade applies the declaration with the token's value, read from a table that `Ui::set_style`
fills. The enums of declarations are unchanged, and hold no payload:

- no token in a rule's declarations: a rule's names are replaced by their values when the sheets
  are read;
- no state: a field's clock is chosen from the rule's selector and the own declaration's state;
- no origin: a colour of code says through its token whether it names a role; forced colours, at
  uwui's first stage, record while reading the sheets which leaves of a rule named a role.

Two own styles are the same only if they name the same tokens where they name any.

**A token has one of four kinds,** each its own type:

| Kind | Value | A theme writes it |
|---|---|---|
| `Color` | `LinearColor` | as a colour, `"#rrggbb"`, a name or a function |
| `Length` | logical pixels, `f32`; a size's `Dimension::Pixels` | a number |
| `Duration` | `core::time::Duration` ([ADR-0227](0227-the-time-that-motion-reads.md)) | a number of seconds |
| `Number` | `f32`: an opacity, a weight, a share | a number |

```rust
pub const MAGNOLIA_KNOB_FACE: Token<Color> =
    Token::new("magnolia_knob_face", Fallback::Srgb(0x2A2D33));
pub const MAGNOLIA_KNOB_RING: Token<Color> =
    Token::new("magnolia_knob_ring", Fallback::Token(&uwui::theme::ACCENT));
pub const MAGNOLIA_KNOB_SIZE: Token<Length> =
    Token::new("magnolia_knob_size", Fallback::Value(28.0));
pub const MAGNOLIA_TOKENS: Tokens = Tokens::new(&[&MAGNOLIA_KNOB_FACE, &MAGNOLIA_KNOB_RING, &MAGNOLIA_KNOB_SIZE]);

Element::button("Gain").style(|s| s.width(MAGNOLIA_KNOB_SIZE).background(MAGNOLIA_KNOB_FACE))
```

- A setter takes its kind alone: a duration in a padding does not compile.
- A fallback is a value, a colour in sRGB decoded when the themes are read, or another token.
- A value that is not finite, and a negative duration, refuse the themes.
- A theme's file writes no kind or unit until M6d.3 decides its notation.

**The core's seven roles are public tokens** of the kind `Color`, `uwui::theme::ACCENT` and the
others, with ADR-0230's names and fallbacks; the core declares them always.

**A crate lists its tokens, and a program declares the lists to its interface**,
`ui.declare_tokens(&MAGNOLIA_TOKENS)`, once; a list declared after `set_style` is read at the next
one.

- `set_style` reads each declared token once, in its kind. A value of another kind refuses the
  themes, with an error that names the theme and the token, and the interface keeps the style it
  had, as for a role.
- A token that the themes lack takes its fallback, with one warning for each token and interface,
  given only when themes are given.
- A token that code writes and no list declared takes its fallback, with one warning: its list
  was not declared.
- A name declared by two lists is refused, whatever their kinds. Replacing a value is a theme's
  work: a theme stacked over the look gives `accent`, and every token that names it follows. An
  application's own tokens take its name as a prefix, `magnolia_`, so that they never meet the
  look's undeclared names.
- A list holds the tokens' names, kinds and fallbacks: a token is known by its name, not by its
  address, which a `const` does not keep, and which a reload through `subsecond` may change.

**Density and more contrast reach tokens by the themes' names alone.** A token takes the value of
the last theme that gives it. An application gives its own themes for each density and for high
contrast, or names the look's values in its own theme, `magnolia_knob_gap = "$space_m"`, which then
follows every density. No factor reaches a token that a density's theme does not give. Durations
and numbers never change with density.

**Magnolia's other needs come with their first readers:**

- a token read outside a style, a knob or a meter drawn on the canvas, and less motion for an
  application's own motion, are read when they are drawn, as ADR-0227 has values that move: with
  M6d.4's canvas, where a change of theme or of the preferences draws again without running a view;
- less transparency, by names, as more contrast: a set of values of less transparency, with its
  reader, at uwui's first stage.

**A field of text named by a theme moves to M6d.3,** which reads the documents with `serde` and
decides their notation. Tokens have no kind of text until then.

**A style's change in code is not timed again through `subsecond`.** It is a function's body,
which M6d.1c3a timed through `subsecond` and `dx`; it is timed again when `uwui dev` is built, at
uwui's first stage. A token added during a reload shows its fallback until the next `set_style`.

## Alternatives considered

The cut:

- **Two milestones, the look's tokens second:** the look's debt closed within Stage 6d. But one
  more milestone, and a counter's rise to accept.
- **One milestone with the look's tokens:** one review. But the longest session before its first
  check, and the same rise.

The payload:

- **`Spec<T>` at every leaf, literal or token,** ADR-0229's letter: one shape for rules and code.
  But a `Spec` in every rule that is always a literal, the cascade given the table, and a mark of
  kind on each number.
- **Tokens resolved when the view builds:** the cascade unchanged. But a change of theme runs the
  views again, which ADR-0223 refuses.

The kinds:

- **The leaf's Rust type,** `Token<LinearColor>` and `Token<f32>`: fewer types. But a duration
  accepted as a padding, and Magnolia's rule kept by convention.
- **Every leaf's type,** shadows, lists of fonts, easings: the widest. But types that nothing reads.

How a program gives its lists:

- **With the theme that gives their values,** `Theme::declaring`: `set_style` unchanged. But given
  again at each change of theme, an empty theme for an application without one, and nothing
  declared by a look read from files.
- **A third argument of `set_style`:** no order between two calls. But 49 call sites, most giving
  nothing, and the lists given again at each change of theme.

Density:

- **A factor on every length token that the density's theme does not give:** an application's
  lengths follow the compact density without a theme of its own. But a second way for density,
  the factor that ADR-0237 refused, and lengths that move differently as a theme gives them or not.

Magnolia's other needs:

- **An accessor read in a view,** `ui.token(&T)`: available now. But a value that a change of theme
  leaves behind, and no reader in the repository.
- **A theme of less motion,** its durations at zero: by names, as contrast. But a second choice of
  themes by hand, outside `Preferences`, and a duration that means something, a meter's fall,
  zeroed.

A field of text: **named now, `"$$"` for a `$`:** the debt closed here. But `slime-serialize`
changed, outside the milestone, and replaced at M6d.3.

`subsecond`: **timed now, `dx` in the shell:** a figure for styles. But a tool added, and a
machine at rest that the parallel tracks do not give.

## Consequences

- `ColorRole` gives way to the roles' tokens; the kinds' defaults read them.
- 39 setters of an own style take a literal or a token of their kind: an API change, in the
  changelog.
- `Ui::declare_tokens` joins `Ui::set_style`; the `interface` example declares and writes its own
  tokens, the first reader outside tests.
- The themes' values that are not finite, and negative durations, are refused where a token reads
  them; where a rule alone names them they are still accepted, debt until the look's tokens.
- A counter of `ui.tree.look`'s family counts a tree whose own styles name tokens.
- The editor's and the dev overlay's lengths written as numbers stay: the editor is never compact.
- M6d.3 decides a field of text named by a theme, and the notation of kinds and units in a theme's
  file; M6d.4 reads tokens and the preferences when it draws; uwui's first stage declares the look's
  tokens, records which leaves of a rule named a role for forced colours, adds less transparency,
  and times a style's reload through `uwui dev`.
