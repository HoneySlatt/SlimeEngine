# 0203. uwui's pilots replaced: Pluto's client and a DAW

- Status: Accepted; amends [0178](0178-two-pilots-and-the-frameworks-name.md) (the launcher no
  longer ported), [0181](0181-uwuis-scope.md) (the four programs before the API is declared
  stable), [0190](0190-uwuis-requirements.md) (what uwui runs; the state model chosen once three
  programs have said what they ask of uwui), [0194](0194-stage-6d-uwui-leaving-the-engine.md) (the
  programs of uwui's first stage), [0198](0198-screens-of-reference-and-the-rivals-measured.md)
  (Slatty's screen kept as a bench alone; Magnolia abandoned; the media application written anew
  replaced) and [0199](0199-uwuis-targets-tools-and-references-narrowed.md) (the pilots that
  `uwui bundle` and updates wait for); amended by
  [0206](0206-slatty-ported-again-the-daw-later.md) (Slatty ported again, the DAW later)
- Date: 2026-11-06

## Context

[ADR-0181](0181-uwuis-scope.md) set four programs to run on uwui before its API is declared
stable: the music player Opium on the desktop, the launcher Slatty, Opium's Android prototype, and
a media application. [ADR-0198](0198-screens-of-reference-and-the-rivals-measured.md) wrote the
media application anew on uwui rather than port Magnolia, a quick prototype. The screens of
reference include Slatty's library of 10,000 games, measured on Iced, with its rivals
([ADR-0201](0201-the-rivals-narrowed.md)); M6d.1b's ideal code and bench were written on it.

The owner no longer needs Slatty: GOG Galaxy is coming to Linux, and the owner was accepted into
its beta. Two other programs are ones that the owner will really use: the native client of Pluto,
the owner's harness of agents, and a digital audio workstation.

M6d.1b3 was to wait for what Opium's and Slatty's long-term directions ask of uwui before the
state model is chosen. The needs that weigh most on a state model are those of the two new
programs: text that streams in without end for Pluto; data in real time at 240 Hz and timelines
of great length for a DAW.

## Decision

The owner's decisions of 2026-11-06.

**Slatty is no longer developed, nor ported to uwui.** Its interview on its long-term direction
is cancelled.

**Slatty's screen of reference stays as it is:** the library of 10,000 games, its figures on Iced,
`slatty-measure`'s fake profile, its rivals, and the ideal code and bench written on it. A
reference chosen before the work is not changed after it. On uwui, that screen alone is rebuilt,
as a bench, never the whole application.

**Magnolia is abandoned.** It was not to be ported already.

**Two programs replace Slatty's port and the media application written anew:**

- **Pluto's native client,** the owner's harness of agents, today in Rust on egui over a fork, in
  TypeScript, of T3 Code's server. The owner wants it to differ from T3 Code, and perhaps to give
  it a server of its own: its interview decides.
- **A DAW** of the class of FL Studio and Pro Tools, which hosts plugins in VST3 and CLAP, those
  for Windows too on Linux through Wine. A new project, in its own repository.

**uwui's first stage runs four programs and a screen:** Opium on the desktop and its Android
prototype, ported, their figures with their present frameworks measured first; Pluto's client and
the DAW, each with its bar set when it is designed, from its interview and the market; and
Slatty's library rebuilt alone, at least as well as Slatty on Iced.

**The state model is chosen once three lists exist,** each of what a program asks of uwui,
written and pushed in its own repository after its interview: Opium's, Pluto's and the DAW's.
Each model is checked against the needs that they mark as general; a need of one program alone
does not enter uwui. The screens of reference and [ADR-0202](0202-how-m6d1-compares-ways-of-writing-an-application.md)'s
criteria do not change.

## Alternatives considered

- **Slatty ported all the same:** the stage's plan unchanged, but the work spent on a program
  that the owner no longer uses.
- **Slatty's screen dropped, or replaced by one of Pluto's or the DAW's:** a bench that follows the
  pilots, but a reference changed after the work on it, its figures before and its rivals lost.
- **The media application kept:** a program written anew without a user.
- **The state model chosen on Opium's list alone:** sooner, but before the needs that weigh most
  on a state model, streams of text, data in real time and long timelines, are known.

## Consequences

- uwui's first stage changes its programs and its exit; uwui's roadmap says so.
- M6d.1b3b waits for three interviews, each in its own repository.
- What the DAW asks of uwui may be far from what the other programs ask; only what its list
  marks as general is weighed against uwui's bar.
- Slatty's measures and tools in uwui's repository stay as the records of its screen:
  `slatty-check.sh`, its probe's check, its rivals' figures.
- M6d.1c's prototype "of Slatty's size" keeps its size, Slatty's screen being kept.
