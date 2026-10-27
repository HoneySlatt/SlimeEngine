# slime-ui

SlimeEngine's UI framework, for the editor and for the interface of games
(`docs/adr/0132-the-engines-ui-framework.md`). It runs without a GPU or a window: what it draws
leaves it as data. Today it holds its text (`docs/adr/0136-the-interfaces-text.md`), its display
list (`docs/adr/0139-text-drawn-and-the-display-list.md`), the tree of elements, their typed styles
and their layout (`docs/adr/0140-the-tree-of-elements.md`), and the style sheets and themes that
style them (`docs/adr/0141-style-sheets-and-themes.md`), the input and the focus
(`docs/adr/0142-input-and-focus.md`), what the tree means to agents
(`docs/adr/0143-what-agents-see-of-the-interface.md`), fields of text and of numbers
(`docs/adr/0145-text-edited.md`), what changed in the tree's meaning, which screen readers read
(`docs/adr/0146-screen-readers.md`), the widgets of a panel and the editor's look
(`docs/adr/0148-widgets.md`), and themes that compute their colors, transitions and the look's
other foundations (`docs/adr/0149-the-looks-foundations.md`).

## Role

- `tree::Ui` is an interface: its roots, each a surface of a size, a scale and a direction, and
  their trees of elements, kept from one frame to the next in an arena. `Ui::build` runs a builder
  when the revision that it reads moved, and matches the elements it adds with those before by their
  keys, so that their state and scroll survive; `Ui::frame` styles again the elements that changed,
  lays out by flexbox and grid the roots where something changed, mirrored for a root laid out from
  right to left, draws again what changed or moved, and gives the display list. Its elements
  (`tree::Element`) are boxes, labels, parts of images, buttons, boxes that scroll, lists that build
  and lay out only the rows that show, and fields of text and of numbers, each with its classes.
- `theme::StyleSheet` holds rules that select elements by kind, classes, key, states and ancestor
  and set a part of their style, and `theme::Theme` the named values that the rules give, which
  may name each other and compute colors in OKLCH (`mix`, `lighten`, `darken`, `alpha`), read
  from their text or their cooked binary encoding; `Ui::set_style` styles an interface with them,
  its themes stacked, the last that gives a name giving its value, and the element's own style
  over the rules (`docs/adr/0149-the-looks-foundations.md`).
- A style's `transition` moves a change of it over time: its colors and its opacity, and a
  section's height as it opens or folds, over the time that the program gives each frame
  (`Ui::advance`), which `Ui::set_reduced_motion` drops; a frame says whether one goes on
  (`FrameStats::animating`). A style's paint holds an opacity, which its descendants share; the
  display list places boxes, images and clips on whole pixels of the screen; a text's style names
  OpenType features (`TextStyle::features`); an element focused by the keyboard, an agent or the
  program is in the state `FocusVisible`, one that the pointer focused is not.
- `Ui` takes a person's input, the pointer, the wheel, the modifiers and the keys in the types of
  `slime-input`, and turns it into `action::Action`s: a button pressed, an intent of navigation, the
  command of a shortcut. The program reads them from a queue (`Ui::take_actions`); an agent sends
  them (`Ui::act`). The interface moves the focus itself, by Tab in the order of the tree and by the
  arrows to the nearest element on screen, within its scope; a root may be a popup at a place of
  its own, or modal, keeping the input and the focus while it is open.
- `shortcuts::Shortcuts` is a table of shortcuts, read from a document: chords of the keys' meaning
  that name commands, in scopes, the classes of the elements around the focus; `Ui::set_shortcuts`
  takes them. Its errors and the style sheets' are a `DocumentError`.
- A field (`Element::text_field`, `Element::text_area`, `Element::number_field`) keeps what the
  user types, selects with the keys and the pointer, undoes, copies and pastes through the clipboard
  that the program gives (`tree::Clipboard`, `Ui::set_clipboard`), and composes through an input
  method (`Ui::ime`); its text is laid out as a label's, parley moving its selection. The program
  reads it as actions, changed and committed (`action::FieldValue`), and the window asks it for
  the pointer's shape and where text is typed (`Ui::cursor`, `Ui::text_input`). The input calls
  give whether the interface took the event, which a game then leaves.
- Widgets, each an element kind that keeps its state in the tree: check boxes
  (`Element::check_box`) and sliders (`Element::slider`), whose values the program reads as
  actions, changed and committed (`action::FieldValue`), the program's value replacing the user's
  only when it changes; sections that fold what they hold (`Element::section`, `Ui::set_open`); bars
  of tabs (`Element::tab_bar`, `Element::tab`), whose choice runs again the builder that shows its
  panel (`Children::chosen`); two panels and the handle between them (`Element::split`); and the
  scroll bars of a box that scrolls and of a list. Their parts apart from their box take the style's
  `control` part (`style::ControlStyle`), and rules select them by the states `Checked`, `Expanded`
  and `Selected`.
- `look`, built only with the feature `editor-look`, is the editor's look, made for the editor and
  the dev overlay, and drawn by the `widgets` example: its fonts, Geist and Geist Mono
  (`look::register_fonts`), its style sheet (`look::style_sheet`), and its themes, stacked: a base
  of scales, a theme of colors, dark, light or high contrast, and a compact one
  (`look::themes`, `look::Colors`, `look::Density`). Its files are in [`look/`](look/README.md),
  with the fonts' license, which a program that builds them in ships beside it.
- `Ui::meaning` says what the tree means, made when asked: each element drawn that means
  something, its `tree::Role`, named as AccessKit names roles, its name, its value, a field's, a
  slider's or two panels' share, its
  states, the `tree::ElementAction`s that an agent can do to it now, the part of it that shows and
  its path, `#0/bar/open`, which `Ui::element_by_path` reads back. A box that only lays others out
  means nothing; a builder gives an element its role and its name (`Element::role`,
  `Element::named`). `Ui::set_value` gives a field a value, as a person who replaced its text and
  committed would, and a slider or two panels theirs.
- `Ui::take_meaning_changes` gives what changed in that meaning since it was last taken, as a
  screen reader's adapter sends it (`tree::MeaningChanges`): each element marked as its meaning
  changes, named by a number that no other element is ever given, with the elements that mean
  something nearest below it, and a field's text by runs, which screen readers move through by
  characters, words and lines (`Ui::field_runs`, `Ui::select_text`). `slime-app` translates them
  into AccessKit's tree.
- `style::Style` is an element's style, typed, in logical pixels: its layout, CSS's flexbox and
  grid properties with start and end sides, its paint, a box's fill, border, corners and shadow,
  and its text, reflected, so that style sheets set its parts.
- `text::Fonts` holds the fonts that text is laid out with: those registered from their bytes,
  named by their families in a style's stack, and the system's where a program asks for them, as
  the editor will, the font found for a writing system named for it from then on. A game draws
  with the fonts it ships, never the system's.
- `Fonts::lay_out` lays a text out in a `text::TextStyle` and a box, `text::TextBounds`: shaped,
  put in the order of its directions, broken into lines, aligned, and cut with an ellipsis when
  the box holds fewer lines than the text. The `text::TextLayout` gives its lines and its glyph
  runs, the cluster under a point, the position nearest it, carets and the rectangles of a
  selection.
- `text::GlyphRasterizer` draws a glyph's image, coverage or color, from the `text::GlyphKey` that
  a placed glyph carries: its face, size and position between two pixels.
- `text::TextCache` keeps laid-out text from one frame to the next within a budget of memory, and
  gives it shared, `Arc<TextLayout>`, so that a display list holds it without copying it.
- `display::DisplayList` is what an interface draws in a frame, as owned data: boxes, the shadows
  that boxes cast, texts laid out and parts of images, which it names by keys
  (`display::ImageKey`) that the program drawing it resolves, each with its clip, its transform
  and its layer, the identity and the root's until Stage 7. `slime-ui-render` draws it through the
  renderer's canvas.
- `Rect` is the rectangle the framework speaks in, by its top left corner and its size, as the
  renderer's canvas gives its own.

## Depends on

parley, fontique and swash, the text; taffy, the tree's flexbox and grid, run through its traits on
the tree's arena; their types stay inside the crate (`docs/adr/0132-the-engines-ui-framework.md`).
icu_properties, for the writing system of a character; `slime-color`, the colors of the styles and
the display list; `slime-reflect` and `slime-serialize`, the styles described and the style sheets,
themes and shortcuts read; `slime-input`, the input it takes; thiserror and tracing. No other engine
crate: text is laid out and rasterized on the CPU, `slime-ui-render` draws the display list, a
program reads the cooked documents, and the window's input reaches the interface through the
program.

## Must not

- Use a GPU, a window or the platform's events: the framework runs headless, in tests and on
  servers, and what it draws leaves it as data.
- Find a font by fallback where a stack can name it: the system's fallback costs up to a hundred
  times more than a font named (`docs/adr/0133-font-files-under-the-open-font-license.md`).
- Show parley's, fontique's, swash's or taffy's types in its API, nor depend on AccessKit:
  `slime-app` translates the tree's meaning into AccessKit's (`docs/adr/0146-screen-readers.md`).
