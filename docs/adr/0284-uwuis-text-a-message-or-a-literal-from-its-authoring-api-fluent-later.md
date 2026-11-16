# 0284. uwui's text a message or a literal from its authoring API; Fluent later

- Status: Accepted
- Date: 2026-11-16

## Context

[ADR-0190](0190-uwuis-requirements.md) asks uwui for translation built in, through Fluent, with
plurals and the formats of dates and numbers; [ADR-0132](0132-the-engines-ui-framework.md) asks
that every text shown can become a key of a translation table; the editor's texts go through
translation tables from Stage 6c's resumption
([ADR-0207](0207-what-slimeengine-is-to-become.md)). uwui's roadmap places translation through
Fluent among the last of its work before its 1.0.

On 2026-11-16 a label is a `String` (`Kind::Label`), and a number field reads `.` alone
([`debt.md`](../debt.md)). uwui's first stage writes its authoring API, its library of components
and four programs ported. Each text that they write as a plain string is one to find and key
again when translation comes.

## Decision

The owner's decision of 2026-11-16.

**uwui's authoring API, in its first stage, carries the model of text:**

- **A text shown is a message or a literal,** never a bare string: a message by its identity and
  its arguments, which a plural or a name needs; a literal marked as one, for what is never
  translated, a file's name or a user's input.
- **An application has a locale,** which its numbers are read and written by: a field takes the
  locale's decimal separator.
- **A fake language of test runs in CI:** every message shown longer and changed, so that a text
  written in place of a message, and a text cut short, show.
- **Until Fluent comes, a message shows its English source.**

**Fluent, its catalogues and the tools that extract messages come before uwui's 1.0,** as uwui's
roadmap has them; Fluent's crates are put to the owner then, as a dependency.

M6d.10 places the model among the milestones of uwui's authoring API.

## Alternatives considered

- **Everything in the first stage, Fluent included:** translations ready early, but the most work
  now, and a dependency taken before its first translation.
- **Translation as planned, before the 1.0 alone:** nothing now, but every label of uwui, of its
  components, of four programs and of the editor found and keyed again then.

## Consequences

- Writing a text costs a message's identity, or a literal said to be one.
- The fake language finds what the review of a change would miss.
- No program is translated before Fluent comes.

## Validation and reconsideration

Read: ADR-0132, ADR-0190 and ADR-0207 on translation; uwui's roadmap; how a label and a number
field hold their text. Assumed: that a message's identity and arguments, without Fluent, fit the
shape that Fluent asks for later.

Proposing another decision is justified if Fluent's model of messages, when it comes, asks for an
identity or arguments that the model cannot carry.
