# 0096. The files the engine reads: what they may hold, and what the engine promises

- Status: Accepted; amended by [0122](0122-the-development-channel.md) (the threat model extended to
  the development channel and the MCP server)
- Date: 2026-10-19

## Context

Before Stage 6, the editor, the engine was audited at 0.4, on 2026-10-18 and 2026-10-19, without
changing it: its correctness and robustness, its performance measured against budgets, and its
architecture against the games it aims at. Each defect that the first part confirmed came with a
test that fails. Many came from files that the engine reads:

- a save edited by hand, or made by another version of the game, made *Hunna Wood* panic at its
  first step, or refuse to start at every launch;
- one byte changed in a cooked texture made the `ktx2` crate panic while the texture loaded;
- a scene file of 1 MB, made for it, made a world allocate a gigabyte, and a save of 16 MiB made the
  reader of documents allocate 7 GB, where their rustdoc promised a bound;
- a light of extreme intensity in a cooked scene made `Renderer::render` panic at the first frame,
  under a rule that [ADR-0042](0042-lights-beyond-the-most-drawn-and-checked-scene-values.md) wrote
  for values from a game's code, before lights came from the world's data
  ([ADR-0084](0084-what-the-world-draws.md));
- the cooker reused a cooked file left empty, as a power cut can leave one, and two copies of the
  game writing one save left a file that neither could read.

Nothing said what the engine promises about such files. Each reader chose how far it trusted its
input; [ADR-0008](0008-errors-logging-and-assertions.md) only says that a broken invariant, which
can only come from a bug, panics. The audit needed a scale to rank its defects, and their fixes
need a rule to meet. The owner decided on 2026-10-19.

## Decision

**The files.** This decision covers what the engine reads from outside its program: cooked data
and the manifest that lists it, saves, scene files, and asset sources with their metadata files
(`.meta`). Any of them may be:

- damaged: an error of the disk, a power cut during a write, a copy cut short, a synchronization
  tool;
- edited by a player: mods, saves edited or shared;
- made by a tool with a bug, the engine's own cooker of another version included.

**The rule.** No such file may make the program panic, hang, exhaust its memory, or lose the
player's progress. Each gives a clear error, which names the file and, where the reader can tell,
what in it is wrong, and leaves the program in a safe state: an asset refused is not drawn, a save
refused is set aside, and neither is applied in part.

What the rule asks of the code:

- **A reader checks before it uses.** A value that came from a file is never an invariant that the
  program guarantees: it is checked where it enters, and a reader refuses it with an error, never
  with a panic. Panics stay for bugs of the program (ADR-0008).
- **A read is bounded by its file's size.** Before it allocates, a reader bounds the memory that a
  file can make it take, and the time, linear in the file's size, not only the file's size or the
  number of its entities.
- **What the player cannot recover is written whole, durably, and never over what was not read.**
  A save is written whole or not at all, is on the disk once reported written, and a file that
  could not be read is set aside under a name of its own, never replaced.
- **Each reader is tested against damaged input.** Its tests cut a valid file at every byte, change
  its bytes, and mutate valid files at random, with a few lines of the test's own (no dependency),
  under `cargo test`.

**Out of scope.**

- An attacker who tries to run code through a file. Mods are not isolated from the program: a file
  that meets the rule may still make a game do what its rules allow.
- The network, which the engine does not have yet.
- The program itself: a game's code, and the engine's API used against its rustdoc. A game's
  function that a read calls, an upgrade or a check, and that panics on a value it did not expect,
  is a bug of the game; the read leaves the world as it found it.

**How a defect against this rule is ranked**, in the plan and in reviews:

- **critical**: the player's progress lost or made wrong without a word, a game that cannot start,
  or a crash that no damaged file causes;
- **high**: a crash, a hang or memory exhausted by a file that this decision covers, a wrong result
  delivered without a word, or a defect in an API that the editor is about to build on;
- **medium**: rare conditions, or a workaround exists;
- **low**: an unlikely case.

## Alternatives considered

- **Trust cooked data, and check only what players edit.** Cooked data is what a mod replaces, and
  what a disk damages: one changed byte in a texture ended the game.
- **A checksum for every cooked file, a file whose checksum differs refused.** It finds damage, not
  a file that a mod or another version of a tool made with a matching checksum: a reader must be
  safe on such a file anyway. Checksums may come later beside the rule, with archives.
- **Isolate mods from the program.** Out of reach before scripting, and a larger project of its
  own; out of scope, as said above.

## Consequences

- The audit pass before Stage 6
  ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)) applies the rule: its
  milestone P1 to saves, P2 to every other reader. P2 amends ADR-0042, so that the renderer leaves
  out a light it cannot draw rather than panic, and corrects the bounds that ADR-0086 and ADR-0091
  promise.
- Every new reader comes with its damaged-input tests, and its rustdoc says what it bounds and what
  it refuses.
- Readers grow longer, and checks cost some time at load, measured where a load matters.
- A game that ships mods it does not trust, or reads files from the network, needs more than this
  decision: isolation and an attacker in the model, decided when it comes.