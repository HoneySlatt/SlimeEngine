# 0143. What agents see of the interface

- Status: Accepted; amends [0122](0122-the-development-channel.md) (version 8 of the protocol: an
  interface read, acted on and captured), [0132](0132-the-engines-ui-framework.md) (an element's
  value comes with the first element that holds one) and [0140](0140-the-tree-of-elements.md) (each
  element carries the mark of its meaning from the milestone that builds the screen readers'
  adapter, not from this one)
- Date: 2026-10-25

## Context

[ADR-0132](0132-the-engines-ui-framework.md) decided that the tree says what it means: each element
that a person reads or uses has a role, a name, a value, states, the actions it takes, its bounds
and its path; that agents read it and act on it through the development channel
([ADR-0122](0122-the-development-channel.md)) and the MCP server, with the captures that exist; and
that screen readers read it later, through AccessKit's adapters. [ADR-0142](0142-input-and-focus.md)
built the input and the focus, and the queue of actions that a program reads, which an agent's
actions were to reach by the same path as a person's. [ADR-0140](0140-the-tree-of-elements.md) said
that each element would carry the mark of its meaning from the milestone that gives the tree its
meaning, M6b.5b.

Building M6b.5b raised what those left open: how the tree reaches the development channel, which
program an agent's session drives in the checks, where the roles come from, and when the meaning is
made. The owner decided each on 2026-10-25, among the options that "Alternatives considered" lists.

## Decision

**`slime-app` reads the interface.** A presentation gives its interface, the `Ui` that its last
frame drew (`Presentation::ui`), and `slime-app` answers the tools itself: it lists the elements,
finds the one at a pixel, acts on one, and captures the part of the window's image where one shows.
`slime-app` depends on `slime-ui` for it, which needs no window nor GPU. A presentation without an
interface gives none, and the tools are told so. A run without a window never calls its
presentation, and has no interface to read.

**The meaning is made when asked.** `Ui::meaning` walks the tree from its last frame and gives, in
the order of the tree, each element drawn that means something: its role, its name, its states,
what can be done to it now, the part of it that shows, in pixels of the surface, and its path. A
box that only lays others out means nothing, and is left out. Nothing is kept between two asks, so
nothing can drift: the mark of the meaning that changed, which the screen readers' adapter needs to
send only what changed, comes with that adapter, at M6b.6.

**Roles of the engine's own, named as AccessKit names them.** `Role` is an enum of `slime-ui`, of
the roles that its elements play today: a generic container, a group, a label, an image, a button, a
box that scrolls, a list, a row of a list, a dialog and a toolbar, each under AccessKit's name, so
that the adapter maps them one to one. An element's kind gives its role, a container in a list being
a row; its builder may give another (`Element::role`), a dialog or a toolbar. AccessKit's types stay
out of `slime-ui`'s interface.

**A name, states, actions, bounds and a path.** An element's name is the one its builder gives
(`Element::named`), or else a button's label or a text's; a container named means something, a
group. Its states are those that rules select by. Its actions are what an agent can do to it now: a
button that a person could press takes a press and the focus; a box that scrolls and a list take a
scroll; an element below a modal root takes nothing. Its bounds are the part of it that its clips
and its root's edges leave, none where they hide it whole, in pixels of the surface, which are the
window image's. Its path is the number of its root, in the order the roots were added, then its
keys, `#0/bar/open`, `#0/content/list/#57`, which names it from one frame to the next as long as
its root's number holds.

**Elements have no value yet.** ADR-0132 lists an element's value; none of today's elements holds
one that changes, a field of text or of a number. The value comes with the first that does, at
M6b.6.

**The development channel carries it.** Its protocol, version 8, asks the elements of a role and a
name, the element at a pixel of the window's image, an action on an element, a press, the focus or a
scroll, or an action of none, a navigation intent or a command, and the capture of an element. An
action reaches the interface as a person's input does, `Ui::act`, `Ui::set_focus` or
`Ui::scroll_to`, and is refused, with what the element takes, when the element does not take it now.
The MCP server gives them as four tools, `find_elements`, `element_at`, `act_on_interface` and
`capture_element`.

**What changes the world leaves as a command.** The program turns an action into what it changes:
the `interface` example counts the presses of its bar's buttons in a component of its world, through
the commands that its presentation sends (`Context::send`), which the host applies at the start of
the next frame. An agent's session in the windowed checks drives that example: it finds a button by
its role and its name, presses it, reads the count in the world, and captures the button.

## Alternatives considered

- **Each program relays its interface**, `slime-app` left without `slime-ui`: the presentation
  gives the meaning as a value of reflection and takes the actions as text, as it gives its views'
  settings. `slime-app` stays smaller, but every program with an interface writes the same relay, as
  each writes the translation of the window's events today.
- **The `world` example driven**, a panel of `slime-ui` over the yard: closer to the editor, but two
  interfaces over one window, egui's overlay and `slime-ui`'s, until M6b.8 rebuilds the overlay.
- ***Hunna Wood* driven**, through a first menu of the game: the games' menus are Stage 7's.
- **AccessKit's `Role`**, some 180 roles, in `slime-ui`'s interface now: no mapping later, but the
  dependency a milestone early, and its types in the engine's interface, against ADR-0132's rule
  that they stay inside.
- **The mark of the meaning now**, as ADR-0140 planned: kept up to date at every change, but read by
  nothing until the adapter.

## Consequences

- `Presentation` gains `ui`, which a presentation with an interface implements in one line, and
  `Element` its role and its name, which only change what the element means: a name changed draws
  nothing again.
- Games link `slime-ui` through `slime-app`, whether they have an interface or not.
- A tool reads an interface only in a windowed run, through its last frame; an action that it sends
  reaches the program at its next frame.
- A path holds while the roots below it stay: a root removed below renumbers those above. A key's
  name that holds `/`, or that `#` and digits make, cannot be read back from a path's text.
- A row of a list is found by its number, its name being its builder's: the names of its texts are
  not gathered into it.
