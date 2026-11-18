# 0295. Each family's messages described and held to its version, from M6d.9a2c

- Status: Accepted; supersedes in part
  [0278](0278-families-versioned-by-major-and-minor-tools-speaking-several-majors.md) (when each
  family's description and its test are built); superseded in part by
  [0308](0308-m6d9a2c-cut-in-four-each-familys-messages-described-in-lines-read-from-its-types-a-kept-copy-for-each-version.md)
  (the rule on a family's description, made precise)
- Date: 2026-11-17

## Context

[ADR-0278](0278-families-versioned-by-major-and-minor-tools-speaking-several-majors.md) gives the
core and each family a major and a minor version, and holds each family to its rule by a test: its
messages described in a file kept with its code, a change that removes or changes what the file
holds without raising the major failing. It builds both with the milestone of uwui's first stage
that brings `uwui dev`'s carrier.

Read on 2026-11-17, at `5ac2532a`:

- No family has such a file, and nothing checks that a family's version moved when its messages
  changed: the version is raised by hand, as `Family::version`'s rustdoc says
  (`crates/uwui-agent/src/session.rs`).
- No message refuses a field that it does not know (`serde`'s default), so a change of a message's
  shape under the same version gives no error: the two ends read it differently.
- The families' versions changed four times in three days before ADR-0278; M6d.9a2c raises
  `interface` to 4 and `editing` to 3, and more milestones of track A change families before the
  first stage.

The review of uwui's architecture of 2026-11-17 proposed building the description and its test now,
the major and minor later.

## Decision

The owner's decision of 2026-11-17. The rule in force on a family's description:

**Each family's messages are described in a file kept with its code,** the requests, the events and
their fields, in a form that the test reads; **a test fails on a change of the messages that the
file does not show,** and on a change of the file that removes or changes what it held without
raising the family's version. Until majors and minors exist, raising the version is raising the
major.

**Built by M6d.9a2c,** for every family, the core's and the engine's, before it raises `interface`
to 4 and `editing` to 3. The major and the minor, and tools that speak several majors, stay with the
milestone of uwui's first stage that brings `uwui dev`'s carrier, as ADR-0278 has it.

The rest of ADR-0278 stands.

## Alternatives considered

- **Built with the major and the minor, as ADR-0278 had it:** one milestone for the whole, but each
  change of a family until then checked by review alone, at a time when families change most.
- **A field unknown refused by every message:** a change of shape shown at once, but every
  addition then a break, against ADR-0278's minor.

## Consequences

- M6d.9a2c grows by a file and a test for each family.
- A change of a family's messages then edits its file in the same change, and raises the version
  where the test asks.
- The milestone of `uwui dev`'s carrier starts from the files, adding the minor.

## Validation and reconsideration

Read: the welcome's checks and `Family::version` in `session.rs`; the absence of any description
of the families. Assumed: that a description in a file costs less to keep than the reviews that
would catch the same changes. Proposing another decision is justified if keeping the files costs
more than the changes they catch.
