# 0218. One core for agents, in uwui

- Status: Accepted; amends [0099](0099-ai-agents-drive-the-engine.md) (agents named and attached to
  a program already open), [0122](0122-the-development-channel.md) (the channel's protocol built on
  uwui's core), [0190](0190-uwuis-requirements.md) (the engine no longer keeps a channel of its own
  apart), [0209](0209-platforms-the-editor-and-agents.md) (the protections of agents built once) and
  [0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md) (the actions declared once serve the
  engine too); amended by [0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md) (M6d.9a cut in five)
- Date: 2026-11-07

## Context

[ADR-0190](0190-uwuis-requirements.md) gave uwui a channel and an MCP server of its own, and said
that the engine keeps its own for games; Stage 6d's M6d.9 builds uwui's with "the engine's own
channel unchanged". [ADR-0209](0209-platforms-the-editor-and-agents.md) gave the engine's MCP server
rights for each agent, a preview before applying and tools found by families;
[ADR-0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md) gave uwui's the same, and actions that
an application declares once for its users' agents. [ADR-0214](0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md)
adds a hosted assistant, and agents that people bring.

What the engine's channel is today ([`slime-channel`](../../crates/slime-channel/README.md),
[`slime-mcp`](../../crates/slime-mcp/README.md)):

- one request, then its reply, nothing else; a late reply breaks the connection;
- a first message with a version and a token, and nothing that names the agent;
- a program listens only when the tool that started it put a token in its environment, so that no
  agent can reach an editor that a person already has open;
- messages encoded through `slime-reflect` and `slime-serialize`, which uwui leaves for `serde`;
- interface requests of its own, beside uwui's, and the editor's operations with untyped parameters
  and no description, permission or family;
- none of ADR-0209's protections built yet.

## Decision

The owner's decision of 2026-11-07.

**One core, in uwui,** under MIT OR Apache-2.0: a protocol that no transport binds; a first message
that names the agent, with the person's consent agent by agent; numbered requests, and events that
the program sends; attaching to a program already running; actions declared once, with typed
parameters, a description, a permission and a family; rights, previews before applying, and tools
found by families. uwui's MCP server is built on it.

**The engine adds its families** on that core: the world, play and editing. `slime-mcp` becomes
uwui's MCP library with them; the engine's channel speaks the core's protocol. The editor's
operations are the declaration's first client: what they change becomes their permission and
family, their parameters typed, each runnable without a window.

**M6d.9 is cut in two:** M6d.9a, the core and the MCP server, with the agent's identity, rights and
families; M6d.9b, the actions declared once, checked on paper against Android's AppFunctions, Apple's
App Intents and Windows' registry of agents.

## Alternatives considered

- **Two systems, as planned:** nothing to rethink now, and two protocols, two systems of rights and
  two ways of declaring an action, to merge after uwui's 1.0 has frozen its own.
- **The core in the engine, uwui using it:** uwui never depends on the engine
  ([ADR-0181](0181-uwuis-scope.md)), and the core would be under the engine's licence.

## Consequences

- [`docs/uwui-needs.md`](../uwui-needs.md) no longer keeps the engine's channel, commands and
  previews for agents inside the engine: it asks them of uwui's core.
- The engine's channel is written again on the core; most of its requests carry over as they are.
- Who owns commands, undo and their journal, uwui or the engine, is decided at M6d.10, once the
  declaration exists.
- A person's own agent, and the hosted assistant, reach an editor already open.
