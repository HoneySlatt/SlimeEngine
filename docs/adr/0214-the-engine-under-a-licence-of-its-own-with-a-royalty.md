# 0214. The engine under a licence of its own, with a royalty

- Status: Accepted; supersedes [0182](0182-the-engine-under-mit-or-apache-2-0.md) for the engine
  (uwui stays under MIT OR Apache-2.0); amends [0006](0006-dependency-policy.md) (the licence that
  dependencies must fit), [0207](0207-what-slimeengine-is-to-become.md) (how the engine is paid
  for), [0209](0209-platforms-the-editor-and-agents.md) (a paid assistant beside the agents one
  brings) and [0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md) (uwui paid for by its
  patrons); amended by [0219](0219-test-games-in-place-of-hunna-wood.md) (examples and templates under MIT-0)
- Date: 2026-11-07

## Context

[ADR-0207](0207-what-slimeengine-is-to-become.md) made the engine free, with no royalty, ever,
paid for by what surrounds it: private layers for consoles, closed online services, a share of a
store of plugins and assets, and the owner's games. [ADR-0182](0182-the-engine-under-mit-or-apache-2-0.md)
put it under MIT OR Apache-2.0. On 2026-11-07 the owner asked how the engine earns the most, and
chose a royalty.

What constrains the choice:

- **What is already given.** Every commit pushed to the public repository from `2a133a8`
  (2026-11-05) is under MIT OR Apache-2.0, and that grant cannot be taken back. Releases up to
  `v0.6.0` are under MPL-2.0. No release was tagged under MIT OR Apache-2.0.
- **Who holds the rights.** Every commit is under the owner's identity: the owner alone may license
  what comes next differently. Outside contributions of code open at 0.14 (ADR-0207).
- **A royalty needs a licence of the engine's own.** MIT and Apache-2.0 let anyone use the engine
  without paying.
- **The copyright of code written by agents.** In several countries, the United States among them,
  a work made without enough human authorship is not protected. Agents write most of the engine; a
  licence that asks for money rests on the owner's direction and review, on a contract that the
  user accepts, and on the engine's marks. A lawyer checks this before the engine relies on it.
- **The market, as of 2026-11-07.** Unreal takes 5 % of a game's gross revenue above 1 M$ over
  its life; 3.5 % when the game launches on the Epic Games Store at the same time; nothing on sales
  made in that store. Unity takes no royalty. Fab, Epic's store of assets, leaves 88 % to the
  creator; Unity's Asset Store, 70 %. Epic's online services are free; Epic does not host dedicated
  servers.
- **Dependencies.** The lock file holds no crate under GPL, LGPL, AGPL or MPL-2.0 that the engine
  must take under it; the rest ask for attribution at most. Fonts are under OFL-1.1 and ISC or MIT;
  the sample's assets under CC0-1.0.

## Decision

The owner's decisions of 2026-11-07.

**A royalty above a high threshold.** The engine is free to use until a game has earned 1 M$ of
gross revenue over its life; above it, the engine takes 3 % of the revenue beyond, whatever the
store, without exclusivity; 2 % for a game that uses the engine's online services. What surrounds
the engine stays: the layers for consoles, the services, the store and the owner's games.

**A licence of the engine's own,** as Unreal's: the source visible, free under the threshold, the
royalty above it, for good. Its text is written with a lawyer. It grants in writing:

- a studio may give its game's tools of mods to its players, free;
- anyone may run the server and the relay of a game that they own, free, also after its studio
  stops its own;
- the network's protocols and the file formats are documented publicly.

**What stays free.** uwui, under MIT OR Apache-2.0: `crates/uwui` and `crates/uwui-render` hold
their own licence files and say so in their manifests, as do `slime-color` and `slime-input`, which
leave with uwui ([ADR-0181](0181-uwuis-scope.md)). Everything else in this repository, the
runtime, the editor, the tools, the SDK, the server and the relay, the tools of mods, comes under
the engine's licence. Code already published keeps the licence it was published under.

**An interim notice, now.** Until the licence's text is ready, `LICENSE.md` at the repository's
root says: all rights reserved, the source visible, the licence to come, what earlier commits and
uwui keep. The repository stays public. Nothing new leaves under MIT OR Apache-2.0.

**Contributions, from 0.14:** a contributor licence agreement. The contributor keeps the copyright
and grants the owner the right to use, sell and license the contribution again, and warrants the
right to give it, help from an AI included. It is written with the same lawyer. uwui takes
contributions under MIT OR Apache-2.0, without an agreement.

**Support for studios, after 0.14.** Priority support with response times guaranteed, realistic
at first and widened with the capacity to keep them; a version's maintenance extended for games
that have shipped, its security and critical fixes up to three or five years; services of experts:
porting, optimisation to the budgets, audits of performance; licences made to measure, a fee paid
ahead in place of the royalty, on templates of contracts that the lawyer writes.

**Partners and patrons.** For the engine, a paid programme of partners, by subscription, from 0.14:
versions before their release, a voice on the roadmap's priorities, which gives an opinion and
never a decision, as the offer says, their logo on the site, a direct contact. For uwui, open
patronage, through GitHub Sponsors or Open Collective, from its 1.0. uwui has no other paid offer
for now.

**An assistant in the editor.** Bringing one's own agent through the engine's MCP server stays free
and open, for good. Beside it, a paid assistant, hosted, by subscription: ready in the editor,
tuned on the engine's skills and documentation, improved through the agent bench, on the best models
of the time. It sends the project only when the user asks it to; its price covers the models' cost
with a margin.

**The store takes 10 %,** the creator keeps 90 %, and 0 % in the first year of each creator on the
store.

**The services, paid by use at first.** Later, the platform, its accounts, matchmaking, lobbies,
voice and saves, free up to a generous volume, once the revenue covers its cost, which the owner
decides on its figures. Hosting is priced below the market, as the cost of a server per player
allows. Builds and the shared cache, by use. A game that uses the services pays the 2 % royalty
from the start.

## Alternatives considered

- **No royalty, ever,** as ADR-0207 had it: the widest adoption, and no income from the success of
  others' games.
- **A licence that turns into MIT after some years,** each version under the royalty for about four
  years: the community reassured, a studio free to stay on an old version without paying.
- **A double licence,** MIT for small revenues and a commercial one above: the MIT version would
  serve a large studio too, unless a clause on commerce made it the first choice again.
- **Unreal's rate, 5 % above 1 M$:** the most per game, and no reason in money to leave Unreal,
  cheaper with its 3.5 %.
- **5 % above 3 M$:** small and middle studios never pay, the large successes pay more than with
  Unreal.
- **The servers, the relay, the tools of mods and the formats' specifications left free:** the same
  promises, at the cost of more code outside the licence; the licence's grants keep them.
- **No written grants,** each studio deciding: a simpler licence, ADR-0207's and ADR-0208's promises
  dropped.
- **A transfer of copyright** from contributors: the strongest control, refused by many, and not
  wholly possible in France, where moral rights cannot be given. **No outside code:** no agreement
  to keep, ADR-0207's opening at 0.14 undone.
- **No paid support,** or support **without licences made to measure:** less to run, the largest
  contracts of the market left aside.
- **A fund of patrons for the engine:** few give to an engine that charges a royalty.
- **Only the paid assistant:** against principle 10 and the open MCP server.
- **The store at 12 %,** as Fab, or **30 %,** as Unity's: no reason for a creator to come, or none
  coming.
- **The platform free at once,** as Epic's: adoption, its cost carried before any revenue.
- **Waiting for the lawyer's text** before the notice, or **the repository made private:** every
  push meanwhile given under MIT for good, or the project hidden, against ADR-0207, and CI's free
  minutes lost.
- **uwui paid** through a double licence: against ADR-0181 and ADR-0213, and a framework that few
  would adopt.

## Consequences

- `LICENSE.md` replaces `LICENSE-MIT` and `LICENSE-APACHE` at the root; uwui's two crates,
  `slime-color` and `slime-input` hold their own. The workspace's other crates take `license-file`;
  those four keep `license = "MIT OR Apache-2.0"`.
- uwui still depends on `slime-reflect`, `slime-serialize`, `slime-gfx` and `slime-render`, under
  the engine's licence: it is usable alone under MIT OR Apache-2.0 once Stage 6d has removed those
  dependencies, as its plan already does (M6d.1 to M6d.11).
- The engine is not open source in the sense of the Open Source Initiative. The documents that said
  so, or "free, with no royalty", say what this ADR decides.
- Until its licence is published, nobody outside may use the engine but to read it, and fork it on
  GitHub as GitHub's terms allow. No one outside uses it yet.
- Waiting for the lawyer: the licence's text, its grants, the agreement for contributors, the
  copyright of code written by agents, the notice's wording, whether rivals' terms allow their
  figures and captures to be published, and the contracts of the store, the partners and support.
- Open, for the owner: whether the sample game, the examples and the templates that users will copy
  stay under a permissive licence; whether MPL-2.0 stays among the licences allowed to dependencies,
  which would oblige every game shipped to offer that crate's files; a tool that writes the notices
  of third parties into each package.
- ADR-0006's rule reads: a dependency's licence must allow the engine to ship under its own licence
  and games under any licence; uwui's, under MIT OR Apache-2.0.
- The services, the store, the support, the partners and the assistant are products of their own,
  closed, outside this repository, as ADR-0207 has it.
