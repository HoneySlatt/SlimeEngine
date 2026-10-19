# 0099. AI agents drive the engine through its own MCP server

- Status: Accepted; amends [0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)
  (a criterion of the foundations pass)
- Date: 2026-10-19

## Context

The engine is developed mostly by AI agents ([`AGENTS.md`](../../AGENTS.md)), and its games will
be too. Today an agent reaches the engine through the command line: it builds, runs tests, runs a
game headless or in a compositor without a display, and reads logs. It cannot look into a running
game or editor, change a value in it, or see a frame without a capture tool of its own. The checks
on screen are what slows such work the most.

The Model Context Protocol (MCP) is an open protocol through which coding agents, such as Claude
Code and Codex, call the tools of other programs. One server that speaks it serves every such
agent, without an integration per vendor.

Much of what makes an engine drivable by an agent exists or is planned:

- reflection lists and edits any component's properties generically
  ([ADR-0080](0080-reflection-and-serialization.md)), so that an agent needs no code per type;
- scenes and saves are text files ([ADR-0089](0089-scenes-and-prefabs.md));
- the engine runs headless;
- Stage 6 makes every edit a command that undo reverses, and runs the game in a child process of
  the editor ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)), so that
  the editor and the game need a channel between them.

ADR-0097 left, for Stage 6 to weigh, commands that tests and agents can drive and a harness that
drives a game or the editor without a person. Nothing in the roadmap decides it.

AI inside the games themselves, such as characters that answer through a language model, is not
the subject: the owner decided on 2026-10-19 that the engine integrates AI to develop the engine
and its games, not as a feature of the games.

## Decision

**A principle of the roadmap: what a person does, an agent does.** Every operation of the editor
and of a running game that a person can perform is a command that an agent can call, the same
command that the person's action runs, not a path of its own.

**The engine has its own MCP server, from Stage 6.** It serves the editor and the games that run
from it. Through it an agent reads the world (entities, components, values, through reflection),
acts through the editor's commands, undoable as a person's are, captures the image of a view, reads
measurements and logs, cooks assets, runs tests and plays a game with scripted input, and reloads
what hot reload reloads.

**Each stage from Stage 6 on exposes its systems to it** (physics, audio, animation, networking and
the others), as it instruments them for the profiler: a continuous track of the roadmap.

**The foundations pass weighs one more criterion**: the channel between the editor and the game's
process, which it decides with how the editor runs a game's code, also serves tools outside the
editor, the MCP server among them.

**The server is reachable from the user's own machine only, and is absent from the builds a game
ships.** The threat model of [ADR-0096](0096-the-files-the-engine-reads.md) is extended to it when
it is built: an agent acts with the rights of the person who runs it, and no other machine or user
reaches the server.

Its dependency, a Rust implementation of MCP, is asked for at Stage 6, with the alternatives
considered.

## Alternatives considered

- **A small MCP server on the game sooner, during the audit pass**: the agents that build the
  engine and *Hunna Wood* would gain it at once, but it would be built before the foundations pass
  decides the channel and the commands, and likely be written again; the audit pass would grow.
- **Later, with plugins (Stage 8) or the final tooling (Stage 11)**: the engine would be built for
  years without agents in mind, and the systems exposed to them afterwards, at a higher cost.
- **One integration per agent or vendor**: each would be maintained apart; MCP is served once.
- **Tools of their own for agents, apart from the editor's commands**: two ways of doing the same
  edit, which drift, and edits by an agent that undo would not reverse.

## Consequences

- Stage 6 grows by the MCP server; each later stage, by exposing its systems.
- Every command of the editor is designed to be called without its UI.
- The agents that develop the engine and its games can check on screen what they change, without
  a person.
- An agent with the server can change a project as the user can: the server is local, and never in
  a shipped game.
