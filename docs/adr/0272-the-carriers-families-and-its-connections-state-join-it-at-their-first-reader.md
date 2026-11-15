# 0272. A carrier's families served and its connection's state join it at their first reader

- Status: Accepted; amends
  [0269](0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md)
  (what the library's carrier gives)
- Date: 2026-11-15

## Context

[ADR-0269](0269-uwuis-mcp-library-reaches-a-program-through-its-host-faults-as-tool-errors-changes-followed-by-requests-that-wait-m6d9a2-cut-in-three.md)
gave uwui's MCP library a small interface to reach a program: send a method and its parameters
and await the answer, cancel a request, the families served, and whether the connection is lost.

Building M6d.9a2a showed, on 2026-11-15:

- The interface's tools need only to send and to cancel. A request of a family that the program
  does not serve is answered with the core's fault, `NOT_SERVED`, or refused by the carrier, and
  either reaches the agent as a tool's error.
- No tool of M6d.9a2a reads the families served, nor whether the connection is lost: the engine's
  carrier, `slime-mcp`'s server, connects again on its own when its connection is lost.
- `AGENTS.md` asks for no abstraction without a concrete use. The closing review of M6d.9a2a found
  the two missing parts as drift from ADR-0269.

## Decision

The owner's decision of 2026-11-15.

**`uwui_mcp::Carrier` sends a request and cancels it, and no more, until a tool reads the rest.**
The families served and the connection's state join it at their first reader, at M6d.9a2b, whose
server refuses a tool of a family that the program does not serve with a message of its own.

## Alternatives considered

- **Both built now, as ADR-0269 wrote:** the ADR followed to the letter, but code without a
  reader, whose shape its first reader may change.

## Consequences

- M6d.9a2b gives the carrier the families served, and the connection's state if one of its tools
  reads it.
- Until then, a tool of a family not served is refused by the program or by the carrier, in their
  own words.
