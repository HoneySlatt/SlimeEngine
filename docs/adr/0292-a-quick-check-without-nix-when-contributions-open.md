# 0292. A quick check without Nix, when contributions open

- Status: Accepted
- Date: 2026-11-16

## Context

`scripts/check.sh` stops without the lavapipe driver that the development shell pins
(`SLIME_LAVAPIPE_ICD`); without Nix, `CONTRIBUTING.md` gives cargo's commands, run by hand on Linux
and Windows. Reference images are compared strictly for any adapter named llvmpipe
(`examples/common/reference_image.rs`), so another Mesa's lavapipe, a distribution's, may fail
them: not tried. Contributions from outside open at the engine's 0.14
([ADR-0214](0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md)) and at uwui's 1.0
([ADR-0197](0197-uwuis-repository-public-from-now.md)); until then the owner and agents work in the
development shell.

## Decision

The owner's decision of 2026-11-16.

**When each repository opens to contributions,** it gains:

- `scripts/check.sh quick`, which runs with rustup alone: formatting, clippy, the tests that need no
  GPU and the documentation;
- reference images compared strictly for the version of lavapipe that the shell pins alone, other
  versions with the tolerance of other drivers
  ([ADR-0030](0030-image-tests-strict-on-the-reference-driver.md),
  [ADR-0054](0054-image-tests-allow-0-2-percent-on-other-drivers.md)).

The whole check stays the development shell's and CI's.

## Alternatives considered

- **A container built from the flake:** the whole check wherever a container runs, but an image to
  keep for contributors who are not there yet.
- **Nothing, Nix required and CI for the rest:** no work, but a first contributor without Nix who
  can check nothing alone.

## Consequences

- Until contributions open, nothing changes.
- The milestones that open contributions hold this work.

## Validation and reconsideration

Read: `scripts/check.sh`'s requirement of the driver, the images' tolerance by the adapter's name,
`CONTRIBUTING.md`'s paths without Nix. Not tried: another Mesa's lavapipe on the images.

Proposing another decision is justified if someone without Nix works on either repository before
contributions open.
