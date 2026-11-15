# 0275. A family's refusal gives the version that the program serves

- Status: Accepted; amends
  [0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md) (the welcome's
  refusals)
- Date: 2026-11-15

## Context

[ADR-0274](0274-the-engines-families-typed-on-the-cores-way-a-family-not-served-refused-as-it-is-sent-m6d9a2b-cut-in-two.md)
has `slime-mcp` refuse a tool of a family that the program serves at another version, in words
that give both versions and name the side to build again, the older one.

The closing review of M6d.9a2b1 found, on 2026-11-15, that the server could not decide which side
is older. The core's welcome gave each family refused as the tool asked it, and a reason in free
text, `the program serves version 2 of "editing"`: the program's version was in that sentence
alone, and absent for a family that the program does not serve at all.

## Decision

The owner's decision of 2026-11-15.

**A refusal in the welcome gives the version of the family that the program serves**, `served`,
beside the family as the tool asked it and the reason; none when the program serves none. It is
optional on the wire: a welcome of a program of before, without it, reads, and gives none. A tool
compares it with its own version: `slime-mcp` names itself to build again when the program's is
newer, the program when its own is.

## Alternatives considered

- **The version read out of the reason's sentence**: no change of the core's messages, but a fact
  with two homes, and a sentence that may change.
- **ADR-0274 amended to let the agent compare**: both numbers given, "build the older of the two
  again"; the agent does the server's work, and a family that the program lacks has one number.

## Consequences

- `uwui_agent::Refusal` gains a public field, `served`: code that builds a refusal names it.
- A tool of before reads a welcome with the field, which it ignores.
- A program of before gives none: `slime-mcp` then says that the program does not serve the
  family at its version, and gives the program's reason.
