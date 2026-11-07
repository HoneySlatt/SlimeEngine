# 0207. What SlimeEngine is to become

- Status: Accepted; amends [0098](0098-the-games-the-engine-aims-at.md) (four profiles added;
  three profiles lead the stages; a small game of the owner's shipped around Stage 9b; mobile and
  web games in the horizon) and the roadmap's principle 8 (the public Rust API stable after 1.0, as
  the C ABI); with [ADR-0208](0208-the-engines-promises-and-systems.md) and
  [ADR-0209](0209-platforms-the-editor-and-agents.md), the record of one interview; amended by
  [0214](0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md) (a royalty above a threshold, a licence of the engine's own), then by [0215](0215-the-engine-measured-against-the-market.md) (games named as benches, reports sent by choice), [0219](0219-test-games-in-place-of-hunna-wood.md) (the first game sold is the 2D test game) and [0220](0220-the-roadmap-reshaped-by-the-market.md) (CI's runners before Stage 7)
- Date: 2026-11-07

## Context

Two hundred and six decisions answered the needs of each stage. What SlimeEngine must become in
the end, for whom, how it lives and how it is paid for, had never been asked whole. The owner had
just answered the same question for uwui ([ADR-0190](0190-uwuis-requirements.md)) and for three of
their programs.

The owner answered on 2026-11-07, theme by theme, in a design session that built nothing. Before
the questions, four surveys read the field: Unreal, Unity and Godot; Bevy, Fyrox, O3DE and the
other open engines, with the libraries of Rust's ecosystem; the engines that large studios keep for
themselves; and agents in editors, the versioning of assets, platforms, accessibility, the
governance of open projects, modding and stores. A reviewing agent then attacked the
recommendations; where it changed one, the owner decided again.

What the survey found missing in the field, and what these decisions answer:

- Frames that hitch: shaders compiled while a game is played, and loads met while the player
  crosses a world. Every large engine treats both late.
- A game's simulation on one thread, in the most used engine of large studios, which its next
  version means to change.
- Levels in binary files, which two people cannot merge, so that teams lock files.
- Agents that reach editors through plugins added afterwards, which run any code they are given,
  with lists of tools too large for their context.
- Engines in Rust that break their API at each release, every three or four months, and none that
  ships a finished editor.
- Mods that run native code: in 2026 several games had to turn off their mods after malware.
- Engines whose owner changed its price, and lost the trust of developers for years.

The decisions are recorded in three ADRs, by theme: this one, what the engine is and how it lives;
[ADR-0208](0208-the-engines-promises-and-systems.md), its measured promises and its systems;
[ADR-0209](0209-platforms-the-editor-and-agents.md), its platforms, its editor and its agents.

## Decision

The owner's decisions of 2026-11-07.

**Whom it is for.** A public engine, adopted widely in the end, judged as a whole against Unreal,
Unity and Godot: better as a whole, never on one feature alone. Until it opens, the owner's games
direct it, and an outside need enters only if it is general. It is made first for a small team
helped by AI agents, making games of the class that large studios make; the tools of large teams
come later, in the horizon. Uses other than games, film, architecture, simulation, are welcome but
not aimed at: no feature enters for them alone.

**When it opens.** The repository stays public and unannounced until 0.14, Stage 11's release,
when an outside project can be built without touching the engine: the engine is announced then,
and contributions of code open. Outside users come before 1.0, while the API can still change. No
game made by people outside the project is tried before Stage 11, as
[ADR-0098](0098-the-games-the-engine-aims-at.md) has it; no private alpha.

**How it is paid for.** The engine is free, under MIT OR Apache-2.0
([ADR-0182](0182-the-engine-under-mit-or-apache-2-0.md)), with no royalty, ever. Money comes from
around it:

- the layers for consoles, private, given to the studios that the makers of consoles approve
  ([ADR-0209](0209-platforms-the-editor-and-agents.md));
- online services, closed and paid: hosting, the platform of online games, its accounts and what
  it keeps, builds and a shared cache of cooked data in the cloud;
- a share of what sells in a store of plugins and assets, after 0.14;
- the owner's games.

The engine stays complete without those services. Its dedicated server, its relays for games that
a player hosts, and the protocols they speak are part of it, published, and a reference relay and
server run anywhere, at Stage 10b; the services sell their hosting and their operation. A policy for
the engine's name and marks, before 0.14, keeps others from selling a service under it.

**Who decides.** The owner, alone, until 1.0, with ADRs as the public process: others propose, the
owner decides. Every contribution, a person's or an agent's, passes the same checks: the ADRs, the
tests, the budgets, the review. A contribution written with an AI is accepted when it says so and a
named person answers for it; the project says openly that agents write most of it. A foundation,
a second maintainer and a written plan for the project's continuation are considered after 1.0.

**Releases and stability.** Before 1.0, any API may break; the user's data never: levels, saves,
layouts and keys are migrated, as the rules already ask. After 1.0, the public API, a façade of
Rust chosen for games and checked by a tool such as `cargo-semver-checks`, and the C ABI keep
semantic versioning strictly: nothing breaks within a major version; what must change is first
deprecated, announced; a 2.0 comes with a tool that migrates code, projects and assets. What lies
behind the façade stays free to change. A version comes out when it is ready: no calendar, no
version of long support. Critical and security fixes are carried to the previous minor version for
twelve months.

**Finished or absent.** A feature is in a release only once it meets its bar, its budgets, tests
and documentation, on the platforms it promises. Before that, it waits behind an option marked
experimental, its state written in its documentation, and is never presented as finished.

**The games.** Four profiles join the nine of section 1, so that no choice closes their door,
without building anything for them ahead of time:

- a fighting game: frames replayed when an opponent's input arrives late, precise to the frame;
- a strategy game: thousands of units, one simulation run the same on every machine, large maps;
- a game of survival and building: a world that the players change in bulk, kept, often played
  together;
- a racing game: vehicles at high speed, fine tyre physics, fast streaming.

Three profiles lead the stages: the realistic open world, the sample's; the competitive shooter;
and the 2D game. Each stage is planned and checked against them. For the ten others, each ADR
checks only that its design does not block them. The number of machines that check the work by
themselves counts as much as the work: before Stage 9a, runners of CI at the owner's, on the Steam
Deck and on a desktop GPU, measure what is today measured by hand.

The first game sold on the engine is a small, finished game of the owner's, made with agents,
around Stage 9b, once the engine packages and updates a game: it proves the whole chain, the store,
the Steam Deck, updates and saves, years before *Hunna Wood*. Which game is decided when Stage 9b
is planned.

**What newcomers get, at 0.14.** Templates of games, each drawn from a real one, the sample's for
the third person, a 2D game's, a shooter's later, checked in CI and made by one command; a book
that leads from nothing to a game shipped, from those templates; the API's reference; examples that
run; a site. In English, checked in CI: the book's examples build and run. The editor speaks
English alone; its texts go through translation tables from Stage 6c's resumption, so that a
translation stays possible.

**Nothing leaves the machine.** The engine and the editor send nothing over the network of their
own accord, ever. A game may add statistics or send its crash reports through an optional crate,
only with the player's consent, which the player can withdraw.

## Alternatives considered

- **The engine of the owner's games alone**, open but not a product: no cost of a public product,
  but nothing that forces it to be easy, complete and documented for others.
- **The best engine of one genre**: the sharpest focus, but against "general" and the profiles of
  ADR-0098.
- **All team sizes alike**, as the largest engine does: the whole market, but the tools of large
  teams early. **Solo developers and small studios only**: quick adoption, against games of large
  studios' class.
- **Opening at 1.0**, as uwui does: a cared-for first impression, but an API frozen without outside
  users. **Opening with the editor, at 0.7**: a community sooner, but reviews and an unstable API
  under strangers, and the owner's pace set by others.
- **A royalty** above a threshold, as the largest engine takes: income that follows the games'
  success, but a brake on adopting a new engine, a reversal of ADR-0182, and the code published
  until then still free to fork. **An open core**, some modules paid: income tied to the engine,
  but a split that communities distrust and that a free rewrite undoes.
- **Services open and self-hosted, their hosting paid**: no lock-in, but others may host the
  owner's code; the owner chose closed services, the engine complete without them.
- **Contributions by people only**, as one large open engine decided in 2026: reassuring to some,
  but at odds with an engine that agents write.
- **A foundation at the opening**: studios reassured about the long term, but heavy, and the
  direction shared early. **A second maintainer and a plan of continuation before 1.0**, which the
  reviewing agent asked for: studios reassured sooner; the owner chose to consider them after 1.0.
- **Stability of the C ABI alone**, principle 8 as it stood: freedom to change the Rust API, but
  games in Rust broken at every release, the complaint most heard of engines in Rust.
- **A calendar and a version of long support each year**: what studios plan around, but two
  branches to keep; the owner chose a version when it is ready, with security fixes for a year.
- **Experimental features early**, as the largest engine ships them: feedback sooner, but users who
  never know what to rely on.
- **All thirteen profiles weighed alike**: no genre left behind, but most of the work unchecked or
  absent for years.
- **A private alpha with two or three outside teams around Stage 9b**: outside feedback before the
  API settles; the owner kept ADR-0098's rule.
- **The editor in English and French**: closer to the owner and to more users, but every text
  translated twice; English alone, translatable.
- **Anonymous statistics of use**, as one large engine sends: data to choose priorities, at the cost
  of the trust it lost.

## Consequences

- Section 1 of [`ROADMAP.md`](../ROADMAP.md) says whom the engine is for, how it is paid for and
  when it opens; principle 8 says what is stable after 1.0; principle 11 says *finished or absent*;
  the horizon holds the services, the store, the foundation, mobile and web games.
- Stage 11 delivers the templates, the book and the site, and the policy of the engine's marks
  before 0.14.
- The release checklist of [ADR-0177](0177-audits-and-releases.md) gains, after 1.0, the façade
  checked by a tool, and the fixes carried to the previous minor version.
- Every ADR from now on weighs its design against the three leading profiles, and states that it
  does not block the others.
- The editor's texts go through translation tables from Stage 6c's resumption.
- The services and the store are products of their own, closed, outside this repository; the
  engine's ADRs never depend on them.
- Reversing the licence later binds only versions to come; the code published stays free.
