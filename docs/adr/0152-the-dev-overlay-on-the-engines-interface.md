# 0152. The dev overlay on the engine's interface

- Status: Accepted; amends [0131](0131-stage-6b-and-stage-6c.md) (M6b.8 cut in two: the editors
  and the world panel, then the overlay over the games' views), [0137](0137-the-canvas.md) (the
  canvas fills a box with a gradient between its corners' colors) and
  [0142](0142-input-and-focus.md) (a focus that the pointer gave takes no keys but a field's and an
  open list's)
- Date: 2026-10-27

## Context

[ADR-0132](0132-the-engines-ui-framework.md) made the dev overlay the first client of the engine's
interface, `slime-ui`: its statistics, time controls, world panel and editors of reflected values
rewritten on it, the world panel laying out only the rows that show (G46), its fields changing a
value only when the user changed it (G33), and egui, egui-wgpu and the exception for their fonts
leaving the engine once something does at least as well, on a checklist written at that milestone
([ADR-0052](0052-amend-roadmap-before-0-1.md)). [ADR-0148](0148-widgets.md) gave the overlay the
editor's look, `slime-ui`'s module `look`. M6b.8 does it.

**What the overlay does today**, the checklist that the rewrite must hold, each item kept by a test
where a test is practical:

- *Hunna Wood* (`games/hunnawood/src/overlay.rs`, `screen.rs`): the overlay made hidden at the
  start; F1 shows the statistics, F2 the world panel, both while a field takes the keys; F3 pauses
  and F4 steps the game's time, only while no field takes them. The statistics, at the window's top
  start, not sized by the user: the adapter's name, the window's size, the frame's time smoothed
  (nine tenths of the last plus a tenth of the new), the CPU's time of the render, the GPU's time of
  its seven passes or "not measured", the draw calls, triangles and shadow draw calls, the size the
  view is drawn at, the view's settings edited as reflected values, the game's speed on a
  logarithmic slider from 0.1 to 4 whose value shows in its label, a hint for F3 and F4, a button
  that loses the GPU device on purpose, and "F1 hides this". What the panel asks goes back to the
  screen: the device lost after the frame is drawn, the speed set, the commands sent; the world's
  refusals come back to the world panel. The overlay is drawn after the view.
- The `world` example: F2 shows the world panel, F3 the views' window, the settings of each view
  edited as reflected values with the size it is drawn at; Tab and Escape do not reach the yard
  while a field takes the keys; a press picks an entity in the view only when the overlay does not
  take the pointer.
- The `scene` example: its statistics shown at the start, F1 hides them; the same figures as the
  game's, the sky's CPU time, and three sliders, the exposure (EV100) from 18 down to 0, bloom from 0
  to 0.2 and the ring's glow from 0 to 4 where the scene has the ring; presses and the wheel over a
  panel do not reach the orbit camera, releases and motion do; Escape always quits; nothing is built
  while the scene loads.
- The editors of reflected values (`edit_reflected`): a boolean as a check box; numbers as fields
  dragged, by a tenth for integers and a hundredth of the value for floats, with their unit, a
  value out of the field's range sent and refused by the world, NaN never sent; text on one line;
  an asset's ID, or its error; a reference to an entity as a link that selects it, "despawned" once
  gone; a struct as its fields, "holds no value" when it has none; an enum as a list of its variants,
  those that cannot start disabled, each variant's documentation as its tooltip, the new variant
  keeping the fields of the same name and type; an option as a check box then its value; a list or
  an array as its rows, "empty" when it has none, with no row added or removed; a vector of three as
  three fields on one line; a rotation as three angles in degrees, about Y, X then Z, the angles
  typed kept while the rotation they made is unchanged; a color as a swatch that opens a picker,
  alpha blended, not premultiplied; anything else as its kind. A part sits on its label's line when
  it fits, or under a section that folds, open at first. A field's label has a tooltip: its
  documentation in plain text, its range, and "Not saved." for a transient field. The start values
  of an option or a variant: false, empty text, an empty list, an option of nothing, the first of 0,
  1 and the range's bounds that the range holds, the first variant that can start.
- The world panel: at the window's top end, 380 logical pixels wide, 160 to 640 high; a filter by
  name, ignoring case; a row per entity, its name or its handle; the list made again only when the
  world's structure or its count of entities changes; the selection kept by handle and by
  persistent ID, found again by ID after a load or a reload; the entity selected: its name, its
  identity or "No identity yet", the prefab it places, the last refusal in the error's color, then a
  section per component, its documentation as its tooltip; each edit a `Command::SetProperty`, sent
  only for an entity with an identity; the refusal cleared by a new edit or selection.
- Input: the overlay takes a press over its panels and the keys while a field or an open list takes
  them; each program asks it before giving the event to the game; panels keep their place as the
  scale factor changes.
- Drawing and the device: drawn over the view's image, after tone mapping; a frame with no panel
  draws nothing; on a lost device, the overlay's GPU objects are released and its panels kept, then
  made again on the new device; two cases still panic inside egui-wgpu (Known debt).
- What it does not do today: screen readers, the MCP server (which only captures it), input
  methods, the clipboard, cursor shapes; the game's shipping build carries it.

**What `slime-ui` lacks for it**, found by reading both: one interface per presentation
(`Presentation::ui`), which the overlay and a game's interface would share; the canvas drawn only
through a whole `Renderer`, which *Hunna Wood*'s view and the scene example keep private; a focus
that takes the arrows, Tab, Enter, Space and Escape once a press gave it to any button, slider or
check box, where egui took the keys only for a field or an open popup, so that a click on the
overlay would take the game's arrows and Escape; a slider on a logarithmic scale, or from high to
low; options of a list to choose from that are disabled or carry help; a field of numbers that keeps
what was committed when a program holding an `f32` gives it back; a picker of colors.

The overlay is about 2 600 lines in `slime-devui`, half of them tests, and 500 in the programs that
build panels with it. Several parts of its design were the owner's to choose. This record was first
proposed, the parts that held no open choice built meanwhile; the owner took the choices on
2026-10-27, among the options that "Alternatives considered" lists.

## Decision

**M6b.8 in two.** M6b.8a, *the editors of reflected values and the world panel*: rewritten on
`slime-ui`, with the picker of colors, beside the overlay of egui, tested without a GPU. M6b.8b,
*the overlay over the games' views*: the overlay hosted in *Hunna Wood* and the `world` and `scene`
examples, their panels rewritten on it, the keys, the lost device, the game's shipping build, and
egui leaving. M6b.8 is twice the largest milestone of Stage 6b, and its two halves are reviewed and
checked apart: the first by tests alone, the second by hand in the programs.

**Built first, in M6b.8a, without an open choice:**

- *Sliders on a logarithmic scale, and from high to low*: a slider's track may map its values
  logarithmically, its least value above 0 (`Element::logarithmic`), and may run from its most
  value at its start to its least at its end (`Element::reversed`), as the exposure's does.
- *Options disabled, with help*: a list to choose from may disable some of its options
  (`ChoiceOption`), which the keys and the pointer pass over and neither the user nor an agent
  chooses, and give each a tooltip, as an enum's variants need.
- *Numbers that the program holds in an `f32`*: a field of numbers told that its number is an
  `f32` (`Element::single_precision`) shows the shortest text that reads back to that `f32`, `0.1`
  rather than the `0.10000000149011612` that its `f64` is, and so keeps the `0.1` that the user
  committed when the program gives it back. Telling the field, rather than guessing from the
  number, keeps a program that holds an `f64` from seeing its number shortened; the editors of
  reflected values know the type of what they edit.
- *The editors of reflected values and the world panel* on `slime-ui`, in `slime-devui`'s module
  `interface`, beside the overlay of egui until the programs move, tested without a GPU: the
  checklist's editors, each field changing the value only when the user changed it (G33); the world
  panel's rows in a list that builds only those that show, within its budget at 100 000 entities
  (G46); the edits as commands; every element named and given a role, which agents and screen
  readers read. Each builds its elements in a slot of the program's interface, by a revision of its
  own, again only once what it shows changed, so that it can also set the states that a builder
  cannot, an option's box disabled, the row of the entity selected. An angle that a field holds in
  radians is shown and typed in degrees; a unit outside the families that fields of numbers know,
  `cd`, follows its label. Text is set as it is committed, not at each key typed, as egui set it:
  one command for a name typed, not one for each of its letters. A number is dragged by steps that
  keep it round, 1 for an integer, which stays one, and for a float the power of ten at most a
  hundredth of it, where egui dragged an integer by a tenth and a float by a hundredth of itself. A
  list's elements are named by their places, `weights [1]`. A color is a swatch and its four
  channels until the picker replaces them.

**The picker of colors, in M6b.8a**, a widget of `slime-ui` that the editor's inspector reuses: a
*color well*, a swatch of the color that a press opens, in a layer beside it that the well owns, as
a list to choose from opens its list. The layer holds a square of saturation and value, a strip of
hue and one of alpha, each dragged by the pointer and moved by the arrows, the four channels as
fields, shown to three decimals, and the hexadecimal code, which gives the color whole. The
program's color is linear; hue, saturation and value are read on its sRGB encoding, as every picker
and the hexadecimal code read them, the well keeping the hue of a gray and the saturation of black,
which the color cannot give back. The program reads the color as actions, changed while the pointer
drags and committed once it lets go or a field commits, under a check box's rule.

**Gradients in the canvas.** The square and the strips are gradients, which the canvas did not
draw: a box of the display list, and the canvas's, may take a color at each corner in place of its
fill, mixed across it on sRGB's encoding, as CSS mixes a gradient and as the interface blends
([ADR-0137](0137-the-canvas.md)). The square is one box, white, the hue, black and black at its
corners; the strip of hue is six. Games' interfaces draw their fades and bars with it too.

**The overlay over the games' views, in M6b.8b:**

- *Roots of the presentation's one interface.* The overlay builds its panels as roots above the
  program's own, in the `Ui` that `Presentation::ui` gives: one tree, one queue of actions sorted
  by root, one focus, the overlay seen by agents and screen readers as the rest of the interface
  is, and a press over a panel never lost to an element beneath it.
- *A focus that the pointer gave takes no keys.* A button, a check box, a slider, a segment, a tab
  or a color well that a press gave the focus takes no keys: they reach the program, as egui left
  them. A field still takes them, so does an open menu or list, and a focus that Tab gave takes the
  keys until a press elsewhere, as keyboard users need. The rule holds in every program, the
  editor's too, whose viewport wants its keys back after a click on a button.
- *The overlay in development builds only.* It goes behind *Hunna Wood*'s `development` feature, as
  the development channel does ([ADR-0122](0122-the-development-channel.md)): the build for
  shipping carries neither the world panel that edits the world nor the editor's look and its
  fonts, about 1.2 MB. A statistics overlay for players is the game's own interface, Stage 7's.
- *Drawn with the view's renderer.* Each program draws its interface after its view, with the
  renderer of its view, which lends its canvas.

## Alternatives considered

- **M6b.8 whole**: one milestone, but twice the largest of Stage 6b, reviewed and checked at once.
- **A second interface for the overlay**, which `slime-app` gives the events to first: the overlay
  apart from the game's interface, but two trees for agents, screen readers and the focus to join,
  and `slime-app` holding two of each.
- **The rule of the keys for the overlay's roots alone**: the editor's interface left as it is, but
  two rules to know. **Any focus taking the keys**, as before: a click on the overlay takes the
  game's arrows and Escape until a press in its view.
- **The overlay in every build**, as egui's was: F1 and F2 in the shipped game, with the fonts.
- **The swatch and four fields, the picker left to Stage 6c**: less work now, but the overlay loses
  the picker that egui's has, and the editor needs one.
- **Gradients made of many small boxes**, 32 × 32 to 64 × 64 for the square: no change to the
  renderer, but steps that show, thousands of items while the picker is open, and a stopgap that
  the editor would replace. **An image of the square made on the CPU**: smooth, but the interface's
  images are textures that the program gives, which a widget has none of.
- **A crate of its own for the editors of reflected values**, beside `slime-devui`: the editor of
  Stage 6c would not depend on the overlay's crate, but a crate for one client until Stage 6c, and
  the overlay is the editors' first user; the split can come when the editor needs it.
- **Drawing the overlay with a second `Renderer`**: no change to the programs' views, but every
  pipeline of the scene compiled twice for a canvas; each program lends the renderer of its view
  instead.

## Consequences

- `slime-ui` gains the parts above, the color well among them; its display list's boxes and the
  canvas's take a gradient; the rule of the keys changes for every interface.
- `slime-devui` gains the editors and the world panel on `slime-ui`, then loses egui, with
  `deny.toml`'s exception for egui's fonts and its use of `slime-gfx`'s `wgpu` feature.
- *Hunna Wood*'s shipping build loses the overlay; its development build holds the editor's look.
