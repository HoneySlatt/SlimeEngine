# 0293. Actions declared once in uwui's core; undo and the journal the program's

- Status: Accepted; supersedes in part [0218](0218-one-core-for-agents-in-uwui.md) (the actions
  declared once, and who owns commands, undo and their journal)
- Date: 2026-11-17

## Context

[ADR-0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md) has an application declare its
actions once, so that its user's agents use it through the interface's own commands, each undone and
logged. [ADR-0218](0218-one-core-for-agents-in-uwui.md) put the declaration in uwui's core for agents,
made the editor's operations its first client, and left to M6d.10 who owns commands, undo and their
journal. M6d.9b, which builds the declaration, comes before M6d.10 on track A.

Read on 2026-11-17, at `9dce4803`:

- uwui has two models of commands already, both names without types: `ActionKind::Command(String)`
  (`crates/uwui/src/action.rs`), which a shortcut's table names, and `PaletteCommand`
  (`crates/uwui/src/tree/palette.rs`). A command that an agent sends is any string, passed on
  unchecked (`crates/uwui/src/agents.rs`).
- uwui's answers to agents sit behind its feature `agents`, which a build for shipping leaves out
  (`crates/uwui/Cargo.toml`). A declaration placed there would be missing where a shipped program's
  menus, shortcuts and palette, and its user's agents, need it.
- The editor's 60 operations are names with untyped parameters (`crates/slime-editor/src/operation.rs`);
  its journal of steps, each with its author and time, is the editor's, made of the world's
  commands.
- uwui keeps no history but a text field's own undo of its text.
- The engine asks that actions declared once feed the menus, the shortcuts, the palette of commands
  and agents, and that an agent's action be undone as a person's ([`uwui-needs.md`](../uwui-needs.md)).

The architecture review of 2026-11-16 found that M6d.9b, by where it puts the declaration, would
settle M6d.10's question without the owner deciding it.

## Decision

The owner's decision of 2026-11-17. With it, the rule in force on actions declared once:

**An action is declared once, in uwui's core,** the `uwui` crate, in every build and behind no
feature. Its declaration gives its name, its typed parameters, a description, the permission that
it asks, its family, and whether it can be undone. Its description becomes a message once uwui's
model of text exists ([ADR-0284](0284-uwuis-text-a-message-or-a-literal-from-its-authoring-api-fluent-later.md)).

**Every way of running an action reads that one declaration:** a shortcut's table, the palette of
commands, a menu, an element that names a command
([ADR-0225](0225-layouts-written-in-kdl-reached-from-code-by-their-names.md)), and agents, through
uwui's core for agents. `ActionKind::Command(String)` and `PaletteCommand` give way to it. A command
that names no declared action is refused, whoever sends it: an agent gets a refusal that says so;
a shortcut's table that names one is an error of its document.

**The program runs its actions.** uwui gives the program the action, its typed arguments and who
ran it, a person or a named agent; the program's handler does the work.

**Undo and the journal are the program's.** uwui keeps no history of the program's state and
models none of its changes. It asks the program, through a small interface of its own, to undo and
redo, and whether it can; an action declared as one that can be undone is undone through it, as a
person's would be. A text field's own undo of its text stays the field's. The editor keeps its
journal of the world's commands, behind that interface.

**The editor's operations are the declaration's first client,** as ADR-0218 has it: what each
changes becomes its permission and family, its parameters typed, each runnable without a window.
The declaration is checked on paper against Android's AppFunctions, Apple's App Intents and
Windows' registry of agents, so that an adapter never changes it.

**Built by M6d.9b,** which now changes `uwui` too. Which builds serve the declared actions to agents,
a shipped program's to its user's agents among them, with the person's consent, stays with M6d.9a3a,
M6d.9a3b and M6d.9b ([ADR-0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md)). M6d.10 no
longer decides who owns commands, undo and their journal.

The rest of ADR-0218 stands: one core for agents in uwui, the engine's families on it, M6d.9 cut in
two.

## Alternatives considered

- **uwui owns commands, undo and the journal:** one model everywhere, but uwui would model the
  engine's changes to its world, and the dependency between them would turn around.
- **The engine owns all three, uwui's core for agents carrying the declaration's format:** the
  least work now, but the declaration missing from shipped builds, a second model of commands kept
  in uwui, and ADR-0213's user's agents with nothing to read.
- **Left to M6d.10, as planned:** no decision now, but M6d.9b, built first, would have chosen by
  where it puts the declaration.

## Consequences

- uwui's core gains a concept, the declared action, and an interface of undo that the program fills.
- uwui's two models of commands become one; shortcuts' tables, the palette and menus name declared
  actions.
- An agent's command is checked against the declaration, which closes a command passed on unchecked.
- M6d.9b changes `uwui`, uwui's core for agents and `slime-editor`; the editor's operations stop
  being names matched by hand.
- Reversing it later, once programs declare their actions, would move every declaration.

## Validation and reconsideration

Read: ADR-0213 and ADR-0218 on actions; the two models of commands and the agents' path in uwui;
the feature `agents`; the editor's operations. Assumed: that a small interface of undo serves the
editor's journal and an application's own history; that one declaration fits the three systems'
models. Proposing another decision is justified if the editor's journal cannot be driven through
that interface, or if a system's adapter needs what the declaration cannot hold.
