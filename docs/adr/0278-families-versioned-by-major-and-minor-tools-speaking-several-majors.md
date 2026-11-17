# 0278. Families versioned by major and minor, tools speaking several majors

- Status: Accepted; amends
  [0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md) (the versions
  of the core and of each family) and
  [0275](0275-a-familys-refusal-gives-the-version-that-the-program-serves.md) (the version that a
  refusal gives); superseded in part by
  [0295](0295-each-familys-messages-described-and-held-to-its-version-from-m6d9a2c.md) (when each
  family's description and its test are built)
- Date: 2026-11-16

## Context

A tool, `slime-mcp` today and `uwui dev` later, drives a program, a game, the editor or an
application on uwui, through uwui's core for agents
([ADR-0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md)). The core
has a version, and each family of requests and events has its own.

- Versions must be equal today. The welcome refuses a tool of another core's version whole, and
  a family asked at another version alone (`crates/uwui-agent/src/session.rs`). The families'
  versions changed 4 times in 3 days before this decision.
- That holds while a tool and a program are built from one tree. After uwui's 1.0, programs on
  several releases of uwui are in use, and one `uwui dev` drives them all.
- The quality bar's scenario "uwui released apart" asks that the tools of either release drive the
  program ([`ROADMAP.md`](../ROADMAP.md#architecture-and-stability)); equal versions fail it.
- From uwui's 1.0, a change of how versions are agreed costs a major release.
- No message of the core or its families refuses an unknown field: `serde` ignores them unless
  told otherwise, and no type of `uwui-agent`, `uwui-mcp`, `slime-channel` or `slime-mcp` says so.

## Decision

The owner's decision of 2026-11-16.

**The core and each family have a major and a minor version.**

- **An addition raises the minor:** an optional field, a request or an event more. An older end
  ignores what it does not know; a newer end gives the default to what an older one leaves out,
  and asks a program no request beyond the minor that the program gave.
- **Any other change raises the major,** and sets the minor to 0.

**A program serves one major of each family; a tool may speak several.** The tool's first message
gives, for the core and each family, the majors that it speaks; the program serves each family at
its own major and minor when the tool speaks that major, and refuses it alone otherwise, giving the
version that it serves. The code that keeps an older major lives in tools, which are few and
updated often; programs keep their current version alone.

**A test holds each family to its rule.** Each family's messages are described in a file kept
with its code; a change that removes or changes what the file holds, without raising the major,
fails.

**Built before uwui's 1.0,** by the milestone of uwui's first stage that brings `uwui dev`'s
carrier, which M6d.10 places. Until then versions stay equal, and a family's version is raised
only when an older end would misread the change, as `Family::version`'s rustdoc says.

## Alternatives considered

- **Equal versions, as now:** nothing to build, but each tool drives the programs of its own
  release alone, and the quality bar's scenario never passes.
- **Programs that serve older versions too:** an old tool drives a new program, but every program
  keeps the translation of every version, for good.
- **One version for everything:** refused by ADR-0246; uwui and the engine would raise one number
  without knowing of each other.

## Consequences

- The first message changes once more: a tool of before cannot speak to a program of after, as
  with each version of the core before. That break comes before uwui's 1.0, then no other for the
  agreement itself.
- `uwui_agent::Family` and the welcome's refusal carry two numbers; `served` gives the program's
  major and minor.
- Each family keeps the file that describes its messages, updated with each change.
- A tool decides how long it keeps an older major; `slime-mcp`'s rule is set when the milestone is
  built.

## Validation and reconsideration

Read: the welcome's checks in `session.rs`, and the absence of any refusal of unknown fields in
the four crates. Assumed: that most changes of a family are additions.

Proposing another decision is justified if majors rise as often as versions do now, or if a tool's
code for older majors outgrows the families that it serves.
