# 0206. Slatty ported again; the DAW later

- Status: Accepted; amends [0203](0203-pilots-replaced-pluto-and-a-daw.md) (Slatty ported again,
  the DAW no longer a program of uwui's first stage) and
  [0205](0205-signals-change-uwuis-interface.md) (signals checked against Slatty's list once
  written); the check done on 2026-11-07, no need left uncovered
- Date: 2026-11-07

## Context

[ADR-0203](0203-pilots-replaced-pluto-and-a-daw.md) took Slatty, the owner's launcher, out of the
programs of uwui's first stage, its screen of reference kept as a bench alone, and put Pluto's
native client and a DAW in place of Slatty's port and of a media application written anew. Slatty's
stop was never recorded in its own repository: it is still developed.

The DAW, Magnolia, is a project in its own repository. Its audio engine, its hosting of plugins
and its format of projects can advance without an interface.

[ADR-0205](0205-signals-change-uwuis-interface.md) chose signals from what Opium, Pluto's client and
the DAW ask of uwui. Slatty's list did not exist then: its interview on its long-term direction
starts now.

## Decision

The owner's decisions of 2026-11-07.

**Slatty comes back among the programs of uwui's first stage,** in the DAW's place, ported whole,
as [ADR-0198](0198-screens-of-reference-and-the-rivals-measured.md) had it before ADR-0203. Its
screen of reference stays as it is.

**The DAW, Magnolia, comes later.** It stays a project of the owner's, a client of uwui after its
first stage. Its engine, its plugins' hosting and its format of projects advance without an
interface, in its repository.

**uwui's first stage runs:** Opium on the desktop and its Android prototype, ported; Pluto's
client; Slatty, ported. Opium and Slatty run at least as well as on their present frameworks, on
the figures measured first; Pluto's client at the bar that its design sets.

**The state model is checked against four lists,** each of what a program asks of uwui: Opium's,
Pluto's and Magnolia's, already written, and Slatty's, once written and pushed. Magnolia's stays
an input: the model is chosen for the long term, and uwui must serve the DAW later. ADR-0205 stays
accepted. Once Slatty's list is pushed, each need that it marks as general is checked against
signals; a need that signals do not cover is put to the owner, with options, and the choice may
be reopened by a new ADR.

## Alternatives considered

- **The DAW kept in the first stage, Slatty's screen alone:** a heavy creative tool testing uwui
  before its 1.0, but the port of a program that the owner still develops left aside, and a DAW's
  interface built before its engine.
- **The four programs, Slatty and the DAW both:** every test kept, but a longer first stage.
- **ADR-0205 reopened until Slatty's list exists:** the rule of all the lists before the choice
  kept to the letter, but M6d.1c, which depends on the model, waiting.

## Consequences

- Before uwui's 1.0, no heavy creative tool tests uwui in use: the editor moves onto uwui after its
  1.0 ([ADR-0190](0190-uwuis-requirements.md)), and the DAW comes later. Its general needs stay
  requirements that the choices check, not a program that proves them.
- Slatty's screen of reference measures its port again, beside its figures on Iced.
- M6d.1b3b's choice stands, with a check left once Slatty's list is pushed.
