# 0286. Every document of uwui versioned, as dock layouts are

- Status: Accepted; made precise on 2026-11-17, by V1: a node `version` anywhere but first is
  refused, so no rule of a style sheet is named `version`, the owner's choice; a whole number
  above the reader's version is a later version, whatever else its node holds.
- Date: 2026-11-16

## Context

uwui reads its documents in KDL, each by its own reader: themes, style sheets, tables of shortcuts
and dock layouts ([ADR-0241](0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)).

On 2026-11-16 only a dock layout carries a version: `version 1` its first node, a layout of a later
version neither read nor set aside nor written over, a version that breaks the format written under
another name and migrated once ([ADR-0241](0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md)).
Themes, style sheets and shortcuts carry none: a reader cannot tell a file of an older format from a
wrong one, and a change of what a format means would misread the files already written.

At M6d.11 the editor's look becomes the official theme Pro
([ADR-0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md)); uwui's first stage then writes the
themes and sheets of four programs, and users keep shortcuts of their own.

## Decision

The owner's decision of 2026-11-16.

**Every document of uwui carries its format's version, as a dock layout does:** `version N` its
first node, in themes, style sheets and tables of shortcuts.

- A document of an older version is migrated as it is read.
- A document of a later version than the reader knows is refused, with a message that names both
  versions; a file that a program writes, as a user's shortcuts, is never written over then.
- Each version has a fixture in the reader's tests, read and migrated.
- A document without the node reads as version 1, the format of before this ADR.

**Built by V1,** a milestone of its own on track A, after M6d.9a2b2 and before M6d.10.

## Alternatives considered

- **A version on the files that users write alone,** shortcuts: less work, but themes travel apart
  from the programs that use them, the theme Pro and those of others.
- **No version before uwui's 1.0:** no work, but every document written until then without one.

## Consequences

- A change of a format's meaning becomes a new version with its migration, tested.
- Existing documents keep reading, as version 1.
- The documents' versions join the formats that the quality bar's scenario of older files checks
  ([`ROADMAP.md`](../ROADMAP.md#architecture-and-stability)).

## Validation and reconsideration

Read: ADR-0241's rule for dock layouts, and the readers of themes, sheets and shortcuts, which read
no version. Assumed: that dock layouts' rule fits the other documents.

Proposing another decision is justified if a document needs to be read by readers of several
versions at once, which a single version number does not serve.
